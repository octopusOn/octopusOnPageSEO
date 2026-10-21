//! A workspace with its jobs and the commands: what a host (desktop, web, command line) holds.

use std::sync::Arc;

use serde_json::Value;

use crate::Workspace;
use crate::jobs::{EventSink, JobManager};
use crate::modules;
use crate::router::{CallError, Context, Opener, Router};

/// The platform of one workspace.
pub struct Platform {
    workspace: Arc<Workspace>,
    jobs: JobManager,
    router: Arc<Router>,
    open: Opener,
}

impl std::fmt::Debug for Platform {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Platform")
            .field("workspace", &self.workspace)
            .finish_non_exhaustive()
    }
}

impl Platform {
    /// The platform of `workspace` with every command, reporting jobs to `sink` and opening
    /// addresses with `open`.
    pub fn new(workspace: Workspace, sink: Arc<dyn EventSink>, open: Opener) -> Self {
        Self::with_router(workspace, sink, open, Arc::new(modules::router()))
    }

    /// The same with a router built once and shared (the web server keeps one for every
    /// account).
    pub fn with_router(
        workspace: Workspace,
        sink: Arc<dyn EventSink>,
        open: Opener,
        router: Arc<Router>,
    ) -> Self {
        Self {
            workspace: Arc::new(workspace),
            jobs: JobManager::new(sink),
            router,
            open,
        }
    }

    /// Calls a command with JSON arguments. Commands block (database, network); hosts call
    /// them off their event loop.
    ///
    /// # Errors
    /// [`CallError`] for an unknown command or the command's failure.
    pub fn call(&self, command: &str, arguments: Value) -> Result<Value, CallError> {
        let context = Context {
            workspace: &self.workspace,
            jobs: &self.jobs,
            open: &self.open,
        };
        self.router.call(&context, command, arguments)
    }

    /// Starts the scheduled work that is due (rank checks, audits); returns the jobs started. Hosts call
    /// it every minute: the desktop program while it is open, the web server for each account
    /// whose schedule is due.
    ///
    /// # Errors
    /// [`CallError`] when the workspace cannot be read.
    pub fn run_due(&self) -> Result<Vec<String>, CallError> {
        modules::run_due(&self.workspace, &self.jobs).map_err(CallError::from)
    }

    /// When scheduled work is due next, seconds since the Unix epoch; `None` without a schedule.
    ///
    /// # Errors
    /// [`CallError`] when the workspace cannot be read.
    pub fn next_due(&self) -> Result<Option<i64>, CallError> {
        modules::next_due(&self.workspace).map_err(CallError::from)
    }

    /// The workspace.
    pub fn workspace(&self) -> &Arc<Workspace> {
        &self.workspace
    }

    /// Its jobs.
    pub fn jobs(&self) -> &JobManager {
        &self.jobs
    }
}
