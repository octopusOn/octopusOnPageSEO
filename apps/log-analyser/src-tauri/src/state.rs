use std::collections::HashMap;
use std::sync::{Mutex, PoisonError};

use octopuson_log_app::{CancelToken, StoreError, Workspace};

/// Shared application state.
#[derive(Debug)]
pub(crate) struct AppState {
    /// Data directory shared with the `octopuson-log` CLI.
    pub(crate) workspace: Workspace,
    /// Cancellation tokens of running imports by client-chosen id.
    imports: Mutex<HashMap<String, CancelToken>>,
}

impl AppState {
    pub(crate) fn new() -> Result<Self, StoreError> {
        let memory_limit = std::env::var("OCTOPUSSEO_MEMORY_LIMIT_MB")
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(1024);
        let data_dir = std::env::var_os("OCTOPUSSEO_DATA_DIR").map(Into::into);
        Ok(Self {
            workspace: Workspace::resolve(data_dir)?.with_memory_limit_mb(memory_limit),
            imports: Mutex::new(HashMap::new()),
        })
    }

    /// State over an explicit workspace (tests).
    #[cfg(test)]
    pub(crate) fn with_workspace(workspace: Workspace) -> Self {
        Self {
            workspace,
            imports: Mutex::new(HashMap::new()),
        }
    }

    /// Registers a running import and returns its token.
    pub(crate) fn start_import(&self, id: &str) -> CancelToken {
        let token = CancelToken::new();
        self.imports
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(id.to_owned(), token.clone());
        token
    }

    /// Forgets a finished import.
    pub(crate) fn finish_import(&self, id: &str) {
        self.imports
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(id);
    }

    /// Requests cancellation; `false` when no such import runs.
    pub(crate) fn cancel_import(&self, id: &str) -> bool {
        let imports = self.imports.lock().unwrap_or_else(PoisonError::into_inner);
        imports.get(id).inspect(|token| token.cancel()).is_some()
    }
}
