//! Background work reported as it goes: crawls, rank checks, AI answers, sign-ins.
//!
//! A job runs on its own thread; every change of it goes to the host's [`EventSink`] (window
//! events on the desktop, the account's event stream on the web), and the list is also read on
//! demand. Finished jobs are kept for a while so a page opened later still shows the outcome.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::AppError;
use crate::router::CallError;

/// Finished jobs kept in the list.
const KEPT_FINISHED: usize = 50;

/// Where a job stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum JobStatus {
    /// Still going.
    Running,
    /// Finished.
    Done,
    /// Finished with some steps failing.
    Partial,
    /// Stopped with an error.
    Failed,
    /// Stopped by the user.
    Cancelled,
}

/// A job as pages see it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Job {
    /// Job id.
    pub id: String,
    /// What it does (`account-login`, `crawl` …).
    pub kind: String,
    /// What it works on, for people.
    pub label: String,
    /// The project it belongs to.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_id: Option<i64>,
    /// Where it stands.
    pub status: JobStatus,
    /// The current step.
    pub step: String,
    /// Steps done.
    pub done: u64,
    /// Steps planned; 0 when unknown.
    pub total: u64,
    /// US dollars spent so far.
    pub cost_usd: f64,
    /// Why it failed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<CallError>,
    /// What it produced, once done.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    /// Milliseconds since the Unix epoch.
    pub started_at: u64,
    /// Milliseconds since the Unix epoch, once it ended.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finished_at: Option<u64>,
    /// Grows with every change, so a page keeps the newest copy.
    pub revision: u64,
}

/// How a finished job ended.
#[derive(Debug, Clone, PartialEq)]
pub struct JobDone {
    /// [`JobStatus::Done`] or [`JobStatus::Partial`].
    pub status: JobStatus,
    /// What it produced.
    pub result: Option<Value>,
}

impl JobDone {
    /// Finished, producing `result`.
    pub fn with(result: impl Serialize) -> Self {
        Self {
            status: JobStatus::Done,
            result: serde_json::to_value(result).ok(),
        }
    }
}

/// Receives every change of a job.
pub trait EventSink: Send + Sync {
    /// A job changed.
    fn job(&self, job: &Job);
}

/// A sink that drops events (command line, tests).
#[derive(Debug, Clone, Copy, Default)]
pub struct NoEvents;

impl EventSink for NoEvents {
    fn job(&self, _: &Job) {}
}

struct Entry {
    job: Job,
    cancel: Arc<AtomicBool>,
}

struct Inner {
    jobs: Mutex<VecDeque<Entry>>,
    sink: Arc<dyn EventSink>,
    next: AtomicU64,
}

/// The jobs of one workspace.
#[derive(Clone)]
pub struct JobManager {
    inner: Arc<Inner>,
}

impl std::fmt::Debug for JobManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JobManager")
            .field("jobs", &self.list().len())
            .finish_non_exhaustive()
    }
}

fn millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| {
            u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX)
        })
}

impl JobManager {
    /// Jobs reporting to `sink`.
    pub fn new(sink: Arc<dyn EventSink>) -> Self {
        Self {
            inner: Arc::new(Inner {
                jobs: Mutex::new(VecDeque::new()),
                sink,
                next: AtomicU64::new(1),
            }),
        }
    }

    fn jobs(&self) -> std::sync::MutexGuard<'_, VecDeque<Entry>> {
        self.inner
            .jobs
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    /// Every job, newest first.
    pub fn list(&self) -> Vec<Job> {
        self.jobs()
            .iter()
            .rev()
            .map(|entry| entry.job.clone())
            .collect()
    }

    /// One job.
    pub fn get(&self, id: &str) -> Option<Job> {
        self.jobs()
            .iter()
            .find(|entry| entry.job.id == id)
            .map(|entry| entry.job.clone())
    }

    /// Jobs still running.
    pub fn running(&self) -> usize {
        self.jobs()
            .iter()
            .filter(|entry| entry.job.status == JobStatus::Running)
            .count()
    }

    /// The running job of a kind for a project, so it is not started twice.
    pub fn running_for(&self, kind: &str, project_id: Option<i64>) -> Option<String> {
        self.jobs()
            .iter()
            .find(|entry| {
                entry.job.status == JobStatus::Running
                    && entry.job.kind == kind
                    && entry.job.project_id == project_id
            })
            .map(|entry| entry.job.id.clone())
    }

    /// Asks a running job to stop; `false` when there is no such running job.
    pub fn cancel(&self, id: &str) -> bool {
        self.jobs()
            .iter()
            .find(|entry| entry.job.id == id && entry.job.status == JobStatus::Running)
            .is_some_and(|entry| {
                entry.cancel.store(true, Ordering::SeqCst);
                true
            })
    }

    /// Asks every running job to stop (the program is closing).
    pub fn cancel_all(&self) {
        for entry in self.jobs().iter() {
            entry.cancel.store(true, Ordering::SeqCst);
        }
    }

    /// Forgets finished jobs.
    pub fn clear(&self) {
        self.jobs()
            .retain(|entry| entry.job.status == JobStatus::Running);
    }

    fn change(&self, id: &str, change: impl FnOnce(&mut Job)) {
        let job = {
            let mut jobs = self.jobs();
            let Some(entry) = jobs.iter_mut().find(|entry| entry.job.id == id) else {
                return;
            };
            change(&mut entry.job);
            entry.job.revision += 1;
            entry.job.clone()
        };
        self.inner.sink.job(&job);
    }

    /// Starts `work` on its own thread as a job of `kind` and returns its id. A running job of
    /// the same kind for the same project is returned instead of starting another.
    ///
    /// # Errors
    /// [`AppError::Internal`] when no thread can be started.
    pub fn start(
        &self,
        kind: &str,
        label: &str,
        project_id: Option<i64>,
        work: impl FnOnce(&JobControl) -> Result<JobDone, AppError> + Send + 'static,
    ) -> Result<String, AppError> {
        if let Some(id) = self.running_for(kind, project_id) {
            return Ok(id);
        }
        let id = format!("job-{}", self.inner.next.fetch_add(1, Ordering::SeqCst));
        let cancel = Arc::new(AtomicBool::new(false));
        let job = Job {
            id: id.clone(),
            kind: kind.to_owned(),
            label: label.to_owned(),
            project_id,
            status: JobStatus::Running,
            step: String::new(),
            done: 0,
            total: 0,
            cost_usd: 0.0,
            error: None,
            result: None,
            started_at: millis(),
            finished_at: None,
            revision: 0,
        };
        {
            let mut jobs = self.jobs();
            jobs.push_back(Entry {
                job: job.clone(),
                cancel: Arc::clone(&cancel),
            });
            let finished = jobs
                .iter()
                .filter(|entry| entry.job.status != JobStatus::Running)
                .count();
            if finished > KEPT_FINISHED
                && let Some(oldest) = jobs
                    .iter()
                    .position(|entry| entry.job.status != JobStatus::Running)
            {
                jobs.remove(oldest);
            }
        }
        self.inner.sink.job(&job);
        let control = JobControl {
            manager: self.clone(),
            id: id.clone(),
            cancel,
        };
        std::thread::Builder::new()
            .name(format!("{kind}-{id}"))
            .spawn(move || {
                let outcome = work(&control);
                let cancelled = control.cancelled();
                control.manager.change(&control.id, |job| {
                    job.finished_at = Some(millis());
                    match outcome {
                        _ if cancelled => job.status = JobStatus::Cancelled,
                        Ok(done) => {
                            job.status = done.status;
                            job.result = done.result;
                        }
                        Err(error) => {
                            job.status = JobStatus::Failed;
                            job.error = Some(CallError::from(error));
                        }
                    }
                });
            })
            .map_err(|error| AppError::Internal(format!("cannot start a job: {error}")))?;
        Ok(id)
    }
}

/// What running work sees of its job.
pub struct JobControl {
    manager: JobManager,
    id: String,
    cancel: Arc<AtomicBool>,
}

impl JobControl {
    /// The job's id.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Whether the user asked it to stop.
    pub fn cancelled(&self) -> bool {
        self.cancel.load(Ordering::SeqCst)
    }

    /// Reports the current step and how far it is.
    pub fn progress(&self, step: &str, done: u64, total: u64) {
        self.manager.change(&self.id, |job| {
            step.clone_into(&mut job.step);
            job.done = done;
            job.total = total;
        });
    }

    /// Adds spending.
    pub fn spent(&self, usd: f64) {
        self.manager.change(&self.id, |job| job.cost_usd += usd);
    }

    /// Waits up to `duration`, waking early when the job is cancelled; `false` when it was.
    pub fn sleep(&self, duration: Duration) -> bool {
        let step = Duration::from_millis(100);
        let mut left = duration;
        while !left.is_zero() {
            if self.cancelled() {
                return false;
            }
            let now = left.min(step);
            std::thread::sleep(now);
            left -= now;
        }
        !self.cancelled()
    }
}
