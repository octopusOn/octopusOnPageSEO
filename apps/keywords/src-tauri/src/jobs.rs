//! Background work: discovery, enrichment, result pages, clustering, Search Console imports
//! and the Google and OctopusOn SEO sign-ins, reported to the window as they go.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, PoisonError};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use seonexus_kw_app::runner::{CheckPage, Failure, Progress, RunControl, RunSummary};
use seonexus_kw_app::store::RunStatus;
use seonexus_kw_app::{KeywordWorkspace, KwAppError};
use serde::Serialize;
use tauri::{Emitter, Manager};

use crate::AppState;

/// Event that carries job updates to the window.
pub(crate) const PROGRESS_EVENT: &str = "keywords-progress";

static NEXT: AtomicU64 = AtomicU64::new(1);

/// What a job does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum JobKind {
    Discover,
    Enrich,
    Serp,
    Cluster,
    SearchConsole,
    GoogleLogin,
    AccountLogin,
}

/// Where a job stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum JobStatus {
    Running,
    Done,
    /// Finished with some requests failing.
    Partial,
    Failed,
    Cancelled,
}

/// A job as the window sees it.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct Job {
    pub(crate) id: String,
    pub(crate) kind: JobKind,
    /// What it works on (the project name).
    pub(crate) label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) project_id: Option<i64>,
    pub(crate) status: JobStatus,
    /// The current request (a source id) or phase.
    pub(crate) step: String,
    pub(crate) done: u64,
    pub(crate) total: u64,
    /// Keywords or pages found so far.
    pub(crate) found: u64,
    /// Waiting for the Wordstat quota until about this time.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) waiting_until: Option<String>,
    /// A check page in the browser window waiting for the person.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) check: Option<CheckPage>,
    /// Run or clustering id, once done.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) result: Option<i64>,
    /// US dollars spent.
    pub(crate) cost_usd: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) error: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(crate) failures: Vec<Failure>,
    /// Milliseconds since the Unix epoch.
    pub(crate) started_at: u64,
    pub(crate) revision: u64,
}

impl Job {
    pub(crate) fn new(kind: JobKind, label: String, project_id: Option<i64>) -> Self {
        let order = NEXT.fetch_add(1, Ordering::Relaxed);
        Self {
            id: format!("job-{order}"),
            kind,
            label,
            project_id,
            status: JobStatus::Running,
            step: String::new(),
            done: 0,
            total: 0,
            found: 0,
            waiting_until: None,
            check: None,
            result: None,
            cost_usd: 0.0,
            error: None,
            failures: Vec::new(),
            started_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |elapsed| {
                    u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX)
                }),
            revision: 0,
        }
    }

    pub(crate) const fn is_active(&self) -> bool {
        matches!(self.status, JobStatus::Running)
    }

    fn apply(&mut self, progress: &Progress) {
        self.step.clone_from(&progress.step);
        self.done = u64::from(progress.done);
        self.total = u64::from(progress.planned);
        self.found = progress.found;
        self.waiting_until.clone_from(&progress.waiting_until);
        self.check.clone_from(&progress.check);
        self.result = Some(progress.run_id);
    }
}

/// What a job ended with.
pub(crate) enum Finish {
    /// A paid run: its status, cost and problems.
    Run(RunSummary),
    /// A result id (a clustering, a Search Console run).
    Done(i64),
}

/// A job and how to stop it.
pub(crate) struct RunningJob {
    pub(crate) job: Job,
    pub(crate) stop: Arc<AtomicBool>,
}

/// Applies a change to a job, then tells the window.
fn update<R: tauri::Runtime>(app: &tauri::AppHandle<R>, id: &str, change: impl FnOnce(&mut Job)) {
    let state = app.state::<AppState>();
    let snapshot = {
        let mut jobs = state.jobs.lock().unwrap_or_else(PoisonError::into_inner);
        let Some(running) = jobs.get_mut(id) else {
            return;
        };
        change(&mut running.job);
        running.job.revision += 1;
        running.job.clone()
    };
    let _ = app.emit(PROGRESS_EVENT, &snapshot);
}

/// Registers a job and runs `work` on a thread of its own.
pub(crate) fn spawn<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    job: Job,
    work: impl FnOnce(&KeywordWorkspace, &RunControl<'_>) -> Result<Finish, KwAppError> + Send + 'static,
) -> Result<String, String> {
    let state = app.state::<AppState>();
    let id = job.id.clone();
    let stop = Arc::new(AtomicBool::new(false));
    let _ = app.emit(PROGRESS_EVENT, &job);
    state
        .jobs
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .insert(
            id.clone(),
            RunningJob {
                job,
                stop: Arc::clone(&stop),
            },
        );
    let workspace = Arc::clone(&state.workspace);
    let handle = app.clone();
    let job_id = id.clone();
    std::thread::Builder::new()
        .name(format!("keywords-{id}"))
        .spawn(move || {
            let cancelled = || stop.load(Ordering::SeqCst);
            let progress =
                |progress: &Progress| update(&handle, &job_id, |job| job.apply(progress));
            let sleep = |duration: Duration| {
                // Sleep in short steps so a stop is noticed quickly.
                let mut left = duration;
                while !left.is_zero() && !stop.load(Ordering::SeqCst) {
                    let step = left.min(Duration::from_millis(200));
                    std::thread::sleep(step);
                    left = left.saturating_sub(step);
                }
            };
            let control = RunControl {
                cancelled: &cancelled,
                progress: &progress,
                sleep: &sleep,
            };
            let outcome = work(&workspace, &control);
            let stopped = stop.load(Ordering::SeqCst);
            update(&handle, &job_id, move |job| {
                // A wait for a check page ends with the job.
                job.check = None;
                match outcome {
                    Ok(Finish::Run(summary)) => {
                        job.status = match summary.status {
                            RunStatus::Done => JobStatus::Done,
                            RunStatus::Partial => JobStatus::Partial,
                            RunStatus::Cancelled => JobStatus::Cancelled,
                            RunStatus::Failed | RunStatus::Running => JobStatus::Failed,
                        };
                        job.result = Some(summary.run_id);
                        job.found = summary.found;
                        job.cost_usd = summary.cost_usd;
                        job.error = summary
                            .failures
                            .first()
                            .map(|failure| failure.error.clone());
                        job.failures = summary.failures;
                        job.waiting_until = None;
                    }
                    Ok(Finish::Done(result)) => {
                        job.status = JobStatus::Done;
                        job.result = Some(result);
                        job.done = job.total;
                    }
                    Err(_) if stopped => job.status = JobStatus::Cancelled,
                    Err(error) => {
                        job.status = JobStatus::Failed;
                        job.error = Some(error.to_string());
                    }
                }
            });
        })
        .map_err(|error| {
            state
                .jobs
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .remove(&id);
            error.to_string()
        })?;
    Ok(id)
}

/// Asks a job to stop.
pub(crate) fn cancel(state: &AppState, id: &str) -> Result<(), String> {
    let jobs = state.jobs.lock().unwrap_or_else(PoisonError::into_inner);
    let running = jobs.get(id).ok_or_else(|| format!("no job `{id}`"))?;
    running.stop.store(true, Ordering::SeqCst);
    Ok(())
}

/// Jobs, newest first.
pub(crate) fn list(state: &AppState) -> Vec<Job> {
    let jobs = state.jobs.lock().unwrap_or_else(PoisonError::into_inner);
    let mut list: Vec<Job> = jobs.values().map(|running| running.job.clone()).collect();
    list.sort_unstable_by(|a, b| {
        b.started_at
            .cmp(&a.started_at)
            .then_with(|| b.id.len().cmp(&a.id.len()).then_with(|| b.id.cmp(&a.id)))
    });
    list
}

/// Forgets finished jobs.
pub(crate) fn clear_finished(state: &AppState) {
    state
        .jobs
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .retain(|_, running| running.job.is_active());
}

/// Whether a job of a kind runs for a project.
pub(crate) fn running_for(
    state: &AppState,
    kind: JobKind,
    project_id: Option<i64>,
) -> Option<String> {
    state
        .jobs
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .values()
        .find(|running| {
            running.job.is_active()
                && running.job.kind == kind
                && running.job.project_id == project_id
        })
        .map(|running| running.job.id.clone())
}
