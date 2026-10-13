//! IPC commands. Each one runs a use-case on a blocking worker thread so the UI stays
//! responsive during imports and large queries.

pub(crate) mod analysis;
pub(crate) mod app;
pub(crate) mod archive;
pub(crate) mod bots;
pub(crate) mod catalog;
pub(crate) mod geo;
pub(crate) mod import;
pub(crate) mod query;
pub(crate) mod saved;

use crate::error::{CommandError, CommandResult};

/// Runs `work` on the blocking thread pool.
async fn blocking<T, F>(work: F) -> CommandResult<T>
where
    F: FnOnce() -> CommandResult<T> + Send + 'static,
    T: Send + 'static,
{
    tauri::async_runtime::spawn_blocking(work)
        .await
        .map_err(|error| CommandError::internal(error.to_string()))?
}
