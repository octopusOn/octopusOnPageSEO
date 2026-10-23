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
        let workspace = Arc::new(workspace);
        let jobs = JobManager::new(sink);
        // Work that may raise alerts is looked at when it ends.
        let watched = Arc::clone(&workspace);
        jobs.on_finish(move |job| modules::alerts::after_job(&watched, job));
        Self {
            workspace,
            jobs,
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
        let answer = self.router.call(&context, command, arguments);
        if !modules::reads_only(command) {
            self.workspace.touch();
        }
        answer
    }

    /// Tells open pages that data changed by a sync, as a finished job of kind `sync` (pages
    /// read everything again; it is not listed among the jobs).
    pub fn announce_sync(&self) {
        self.jobs.announce("sync");
        // Alerts a sync brought are shown here (the desktop) once.
        if let Err(error) = modules::alerts::announce(&self.workspace) {
            tracing::warn!(%error, "alerts were not shown");
        }
    }

    /// Looks for alerts in data a sync brought (the web, after a desktop's changes), in the
    /// background.
    pub fn look_for_alerts(&self) {
        modules::alerts::after_sync(&self.workspace);
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

    /// Reads the answers of rank checks waiting in the provider's queue and finishes their
    /// runs. Hosts call it every minute: the desktop program while it is open (whether or not
    /// it runs the schedule), the web server for each account with checks waiting.
    ///
    /// # Errors
    /// [`CallError`] when the workspace cannot be read.
    pub fn collect_queued(&self) -> Result<modules::rank::Collected, CallError> {
        let mut total = modules::rank::Collected::default();
        for (kind, collected) in
            modules::collect_queued(&self.workspace).map_err(CallError::from)?
        {
            // Pages showing what was read read it again.
            if collected.read + collected.failed > 0 {
                self.jobs.announce(kind);
            }
            total.read += collected.read;
            total.waiting += collected.waiting;
            total.failed += collected.failed;
            total.finished += collected.finished;
        }
        // Runs finished from the queue may raise alerts.
        if total.finished > 0 {
            modules::alerts::after_sync(&self.workspace);
        }
        Ok(total)
    }

    /// When the provider's queue should be read next, seconds since the Unix epoch.
    ///
    /// # Errors
    /// [`CallError`] when the workspace cannot be read.
    pub fn next_collect(&self) -> Result<Option<i64>, CallError> {
        modules::next_collect(&self.workspace, &self.jobs).map_err(CallError::from)
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
