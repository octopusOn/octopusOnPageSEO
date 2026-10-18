//! Background work: analyses, searches, rank checks and the OctopusOn SEO sign-in, reported to
//! the window as they go.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, PoisonError};
use std::time::{SystemTime, UNIX_EPOCH};

use seonexus_serp::Engine;
use seonexus_serp::provider::{Attempt, SearchEvent};
use seonexus_serp_app::analysis::{AnalysisEvent, AnalyzeRequest};
use seonexus_serp_app::rank::TrackEvent;
use seonexus_serp_app::{CrawlControl, SearchRequest, SerpAppError, SerpWorkspace};
use serde::Serialize;
use tauri::{Emitter, Manager};

use crate::AppState;

/// Event that carries job updates to the window.
pub(crate) const PROGRESS_EVENT: &str = "serp-progress";

static NEXT: AtomicU64 = AtomicU64::new(1);

/// Hands a change of the job to the window.
type Report<'a> = &'a dyn Fn(&dyn Fn(&mut Job));

/// What a job does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum JobKind {
    Analysis,
    Search,
    Track,
    /// Signing in to the OctopusOn SEO account.
    Account,
}

/// Where a job stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum JobStatus {
    Running,
    /// The browser shows a check page that the person has to clear.
    Challenge,
    Done,
    Failed,
    Cancelled,
}

/// A check page waiting for the person.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct JobChallenge {
    provider: String,
    kind: seonexus_serp::challenge::ChallengeKind,
    url: String,
}

/// A job as the window sees it.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct Job {
    pub(crate) id: String,
    pub(crate) kind: JobKind,
    /// The keyword, or the project name.
    pub(crate) label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) engine: Option<Engine>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) project_id: Option<i64>,
    pub(crate) status: JobStatus,
    /// `search`, `waiting`, `page`, `crawl`, `domains`, `volume`, `scoring` or `keyword`.
    pub(crate) step: &'static str,
    pub(crate) done: u64,
    pub(crate) total: u64,
    /// Detail of the step: the provider, the keyword, seconds to wait.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) detail: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) challenge: Option<JobChallenge>,
    /// Analysis, snapshot or run id, once done.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) result: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) error: Option<String>,
    /// Provider attempts of a failed search.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(crate) attempts: Vec<Attempt>,
    /// Page saved for fixing the parser selectors.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) diagnostic: Option<String>,
    /// Milliseconds since the Unix epoch.
    pub(crate) started_at: u64,
    pub(crate) revision: u64,
}

impl Job {
    fn new(kind: JobKind, label: String, engine: Option<Engine>) -> Self {
        let order = NEXT.fetch_add(1, Ordering::Relaxed);
        Self {
            id: format!("job-{order}"),
            kind,
            label,
            engine,
            project_id: None,
            status: JobStatus::Running,
            step: "search",
            done: 0,
            total: 0,
            detail: None,
            challenge: None,
            result: None,
            error: None,
            attempts: Vec::new(),
            diagnostic: None,
            started_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |elapsed| {
                    u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX)
                }),
            revision: 0,
        }
    }

    pub(crate) const fn is_active(&self) -> bool {
        matches!(self.status, JobStatus::Running | JobStatus::Challenge)
    }

    fn apply_search(&mut self, event: &SearchEvent) {
        match event {
            SearchEvent::Waiting { provider, seconds } => {
                self.status = JobStatus::Running;
                self.step = "waiting";
                self.detail = Some(format!("{provider}:{seconds}"));
            }
            SearchEvent::Page {
                provider,
                page,
                pages,
            } => {
                self.status = JobStatus::Running;
                self.step = "page";
                self.detail = Some(format!("{provider}:{page}/{pages}"));
            }
            SearchEvent::Challenge {
                provider,
                kind,
                url,
            } => {
                self.status = JobStatus::Challenge;
                self.challenge = Some(JobChallenge {
                    provider: provider.clone(),
                    kind: *kind,
                    url: url.clone(),
                });
            }
            SearchEvent::Resumed { .. } => {
                self.status = JobStatus::Running;
                self.challenge = None;
            }
        }
    }

    fn apply_analysis(&mut self, event: &AnalysisEvent) {
        match event {
            AnalysisEvent::Search { event } => self.apply_search(event),
            AnalysisEvent::Crawl { done, total } => {
                self.step = "crawl";
                self.done = *done;
                self.total = *total;
                self.detail = None;
            }
            AnalysisEvent::Domains { count } => {
                self.step = "domains";
                self.done = 0;
                self.total = u64::try_from(*count).unwrap_or(u64::MAX);
            }
            AnalysisEvent::Volume => self.step = "volume",
            AnalysisEvent::Scoring => self.step = "scoring",
        }
    }

    fn apply_track(&mut self, event: &TrackEvent) {
        match event {
            TrackEvent::Keyword {
                done,
                total,
                keyword,
                engine,
            } => {
                self.status = JobStatus::Running;
                self.challenge = None;
                self.step = "keyword";
                self.done = u64::from(*done);
                self.total = u64::from(*total);
                self.detail = Some(keyword.clone());
                self.engine = Some(*engine);
            }
            TrackEvent::Search { event } => {
                // Keep the keyword in view; only waits and check pages matter here.
                if matches!(
                    event,
                    SearchEvent::Challenge { .. } | SearchEvent::Resumed { .. }
                ) {
                    self.apply_search(event);
                }
            }
        }
    }

    fn fail(&mut self, error: SerpAppError) {
        self.challenge = None;
        if let SerpAppError::Search {
            error: seonexus_serp::provider::ProviderError::Cancelled,
            ..
        } = error
        {
            self.status = JobStatus::Cancelled;
            return;
        }
        self.status = JobStatus::Failed;
        self.error = Some(error.to_string());
        if let SerpAppError::Search {
            attempts,
            diagnostic,
            ..
        } = error
        {
            self.attempts = attempts;
            self.diagnostic = diagnostic.map(|path| path.display().to_string());
        }
    }
}

/// A job and how to stop it.
pub(crate) struct RunningJob {
    pub(crate) job: Job,
    pub(crate) control: Arc<CrawlControl>,
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
fn spawn<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    job: Job,
    work: impl FnOnce(&SerpWorkspace, &CrawlControl, Report<'_>) -> Result<i64, SerpAppError>
    + Send
    + 'static,
) -> Result<String, String> {
    let state = app.state::<AppState>();
    let id = job.id.clone();
    let control = Arc::new(CrawlControl::new());
    let _ = app.emit(PROGRESS_EVENT, &job);
    state
        .jobs
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .insert(
            id.clone(),
            RunningJob {
                job,
                control: Arc::clone(&control),
            },
        );
    let workspace = Arc::clone(&state.workspace);
    let handle = app.clone();
    let job_id = id.clone();
    std::thread::Builder::new()
        .name(format!("serp-{id}"))
        .spawn(move || {
            let report = |change: &dyn Fn(&mut Job)| update(&handle, &job_id, change);
            let outcome = work(&workspace, &control, &report);
            let stopped = control.is_stopped();
            update(&handle, &job_id, move |job| {
                job.challenge = None;
                match outcome {
                    Ok(result) => {
                        job.status = JobStatus::Done;
                        job.result = Some(result);
                        job.done = job.total;
                    }
                    Err(_) if stopped => job.status = JobStatus::Cancelled,
                    Err(error) => job.fail(error),
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

/// Starts an analysis.
pub(crate) fn start_analysis<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    request: AnalyzeRequest,
) -> Result<String, String> {
    let job = Job::new(
        JobKind::Analysis,
        request.search.query.keyword.clone(),
        Some(request.search.query.engine),
    );
    spawn(app, job, move |workspace, control, report| {
        let outcome = workspace.analyze(&request, control, &|event| {
            report(&|job: &mut Job| job.apply_analysis(event));
        })?;
        Ok(outcome.analysis_id)
    })
}

/// Starts a search.
pub(crate) fn start_search<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    request: SearchRequest,
) -> Result<String, String> {
    let job = Job::new(
        JobKind::Search,
        request.query.keyword.clone(),
        Some(request.query.engine),
    );
    spawn(app, job, move |workspace, control, report| {
        let cancelled = || control.is_stopped();
        let outcome = workspace.search(&request, &cancelled, &|event| {
            report(&|job: &mut Job| job.apply_search(event));
        })?;
        Ok(outcome.snapshot_id)
    })
}

/// Signs in to the OctopusOn SEO account with one click: the account site opens in the browser
/// and comes straight back for a person who is signed in there.
pub(crate) fn start_account_login<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    open: fn(&str) -> bool,
) -> Result<String, String> {
    let state = app.state::<AppState>();
    if let Some(running) = state
        .jobs
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .values()
        .find(|running| running.job.is_active() && running.job.kind == JobKind::Account)
    {
        return Ok(running.job.id.clone());
    }
    let mut job = Job::new(JobKind::Account, "OctopusOn SEO".to_owned(), None);
    job.step = "account";
    spawn(app, job, move |workspace, control, _report| {
        let cancelled = || control.is_stopped();
        workspace.connect(&open, &cancelled)?;
        Ok(0)
    })
}

/// Starts checking the ranks of a project, unless a check of it is running.
pub(crate) fn start_track<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    project_id: i64,
    name: String,
) -> Result<String, String> {
    let state = app.state::<AppState>();
    if let Some(running) = state
        .jobs
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .values()
        .find(|running| running.job.is_active() && running.job.project_id == Some(project_id))
    {
        return Ok(running.job.id.clone());
    }
    let mut job = Job::new(JobKind::Track, name, None);
    job.project_id = Some(project_id);
    job.step = "keyword";
    spawn(app, job, move |workspace, control, report| {
        let cancelled = || control.is_stopped();
        let summary = workspace.track(project_id, &cancelled, &|event| {
            report(&|job: &mut Job| job.apply_track(event));
        })?;
        Ok(summary.run_id)
    })
}

/// Asks a job to stop.
pub(crate) fn cancel(state: &AppState, id: &str) -> Result<(), String> {
    let jobs = state.jobs.lock().unwrap_or_else(PoisonError::into_inner);
    let running = jobs.get(id).ok_or_else(|| format!("no job `{id}`"))?;
    running.control.stop();
    Ok(())
}

/// Where a job stands, if it is known.
pub(crate) fn status(state: &AppState, id: &str) -> Option<JobStatus> {
    state
        .jobs
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .get(id)
        .map(|running| running.job.status)
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
