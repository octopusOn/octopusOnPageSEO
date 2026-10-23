//! Rank Tracker: where the project's site ranks for its keywords on Google, Bing or Yandex, by
//! country, city and device: positions and their history, visibility, the site's pages that
//! rank (and compete with each other), the elements of the result page, whether an AI
//! Overview is shown and cites the site, and the competitors' shares over time.
//!
//! A check is always asked fresh and only its reading is kept, in the rank history. A run is a
//! job; the schedule starts runs when due ([`run_due`]): the desktop program asks every minute
//! while it is open, the web server for every account.
//!
//! Runs started by hand read DataForSEO's live result pages. Scheduled runs post their checks
//! to the standard queue instead, for less than a third of the price, wherever it is open (on
//! the user's own key; on credits once the gateway forwards the queue): the run's job ends
//! once they are posted, and [`collect_queued`] reads their answers every minute until none is
//! left, then finishes the run. The posted checks are kept here, so the answers are read after
//! a restart too; only the copy that posted them can read them.

use std::collections::hash_map::Entry;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::str::FromStr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use octopuson_cloud::Via;
use octopuson_core::Timestamp;
use octopuson_dataforseo::keywords as labs;
use octopuson_dataforseo::rank::{self as check, RankCheck, RankQuery};
use octopuson_dataforseo::{Client, DfsError, Market};
use octopuson_platform::{
    CacheClass, CheckResult, ProjectRow, RankCheckRow, RankPlan, RankRunRow, RankSettings,
    RankTaskRow, RunEnd, TrackedKeyword, TrackerRow, TrackerSpec, unix_now,
};
use octopuson_serp::{Device, Engine, registrable_domain};
use octopuson_serp_score::rank::{
    self as measure, Check, CompetitorShare, ResultPage, Schedule, Trend, VisibilityPoint,
};
use serde::{Deserialize, Serialize};

use crate::fetch::{Estimate, Paid, PaidRun, via_name};
use crate::jobs::{JobControl, JobDone, JobManager, JobStatus};
use crate::router::{Context, ErrorCode, Router};
use crate::{AppError, Workspace, now};

/// Module id, in spending records.
const MODULE: &str = "rank";

/// Kind of the job of a run.
pub const JOB_KIND: &str = "rank-check";

/// Kind of the note pages get when answers were read from the queue (not a job of its own).
pub const QUEUE_KIND: &str = "rank-queue";

/// Days of history a report shows unless asked otherwise.
const DEFAULT_DAYS: u32 = 90;

/// Most days of history a report shows.
const MAX_DAYS: u32 = 365;

/// Runs listed.
const RUNS_SHOWN: u32 = 20;

/// Seconds before a queued check's answer is first asked for; the wait doubles after each ask
/// that finds it unfinished, up to [`LONGEST_WAIT`].
pub(crate) const FIRST_WAIT: i64 = 60;

/// Longest wait between two asks for a queued check's answer, seconds.
pub(crate) const LONGEST_WAIT: i64 = 900;

/// Seconds after which a queued check that never answered is given up on.
pub(crate) const GIVE_UP: i64 = 86_400;

/// Answers read from the queue in one round at most, and for how long.
pub(crate) const COLLECT_LIMIT: u32 = 500;
pub(crate) const COLLECT_TIME: Duration = Duration::from_secs(40);

/// Seconds between two rounds of reading the queue while checks wait in it.
pub const COLLECT_EVERY: i64 = 60;

/// Commands after which the schedule may be due at another time (the web server notes it).
const SCHEDULE_COMMANDS: &[&str] = &[
    "save_rank_settings",
    "add_rank_tracker",
    "remove_rank_tracker",
    "track_keywords",
    "untrack_keywords",
    "start_rank_check",
    "delete_project",
];

/// Whether a command may move when the schedule is due next.
pub fn changes_schedule(command: &str) -> bool {
    SCHEDULE_COMMANDS.contains(&command)
}

fn rfc3339(seconds: i64) -> String {
    Timestamp::from_unix_micros(seconds.saturating_mul(1_000_000)).to_string()
}

fn engine_of(tracker: &TrackerRow) -> Result<Engine, AppError> {
    Engine::from_str(&tracker.engine).map_err(|error| AppError::Internal(error.to_string()))
}

fn device_of(tracker: &TrackerRow) -> Device {
    Device::from_str(&tracker.device).unwrap_or_default()
}

/// When a project's schedule is due, seconds since the Unix epoch; `None` when it checks
/// only when asked or has nothing to check.
fn due_at(plan: &RankPlan) -> Option<i64> {
    let interval = Schedule::parse(&plan.settings.schedule).interval_secs()?;
    if plan.keywords == 0 || plan.trackers == 0 {
        return None;
    }
    Some(
        plan.settings
            .last_run_at
            .as_deref()
            .and_then(measure::seconds_of)
            .map_or(0, |last| last + interval),
    )
}

/// The checks of one run and their list price, live or through the standard queue.
fn plan_cost(
    trackers: &[TrackerRow],
    keywords: usize,
    settings: &RankSettings,
    queued: bool,
) -> (u32, f64) {
    let mut checks = 0_u32;
    let mut usd = 0.0;
    for tracker in trackers {
        let engine = engine_of(tracker).unwrap_or(Engine::Google);
        let each = if queued {
            check::queue_estimate(engine, settings.depth, settings.ai_overview)
        } else {
            check::estimate(engine, settings.depth, settings.ai_overview)
        };
        let count = u32::try_from(keywords).unwrap_or(u32::MAX);
        checks = checks.saturating_add(count);
        usd += each * f64::from(count);
    }
    (checks, usd)
}

// ----- Setup -------------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct ProjectAsk {
    project_id: i64,
}

/// How a project is tracked.
#[derive(Debug, Serialize)]
struct Setup {
    settings: RankSettings,
    trackers: Vec<TrackerRow>,
    /// Keywords tracked.
    keywords: usize,
    /// Keywords without a known volume.
    missing_volumes: usize,
    /// When the schedule checks next, RFC 3339; `None` when checked only by hand.
    next_check_at: Option<String>,
}

fn setup(workspace: &Workspace, project_id: i64) -> Result<Setup, AppError> {
    let store = workspace.store()?;
    store.project(project_id)?;
    let settings = store.rank_settings(project_id)?;
    let trackers = store.rank_trackers(project_id)?;
    let keywords = store.tracked_keywords(project_id)?;
    let plan = RankPlan {
        project_id,
        settings: settings.clone(),
        keywords: keywords.len() as u64,
        trackers: trackers.len() as u64,
    };
    Ok(Setup {
        next_check_at: due_at(&plan).map(|at| rfc3339(at.max(unix_now()))),
        missing_volumes: keywords.iter().filter(|row| row.volume.is_none()).count(),
        keywords: keywords.len(),
        settings,
        trackers,
    })
}

fn rank_setup(context: &Context<'_>, ask: ProjectAsk) -> Result<Setup, AppError> {
    setup(context.workspace, ask.project_id)
}

#[derive(Debug, Deserialize)]
struct SettingsAsk {
    project_id: i64,
    schedule: Schedule,
    depth: u32,
    #[serde(default)]
    ai_overview: bool,
}

fn save_rank_settings(context: &Context<'_>, ask: SettingsAsk) -> Result<Setup, AppError> {
    context.workspace.store()?.save_rank_settings(
        ask.project_id,
        &RankSettings {
            schedule: ask.schedule.as_str().to_owned(),
            depth: check::depth(ask.depth),
            ai_overview: ask.ai_overview,
            last_run_at: None,
        },
    )?;
    setup(context.workspace, ask.project_id)
}

#[derive(Debug, Deserialize)]
struct TrackerAsk {
    project_id: i64,
    engine: Engine,
    #[serde(default)]
    device: Device,
    /// The project's country when empty.
    #[serde(default)]
    country: Option<String>,
    /// The project's language when empty.
    #[serde(default)]
    language: Option<String>,
    /// A city or region (`Istanbul,Istanbul,Turkey`).
    #[serde(default)]
    location: Option<String>,
}

fn tracker_spec(project: &ProjectRow, ask: &TrackerAsk) -> Result<TrackerSpec, AppError> {
    let pick = |given: &Option<String>, fallback: &str| {
        given
            .as_deref()
            .map(str::trim)
            .filter(|text| !text.is_empty())
            .unwrap_or(fallback)
            .to_ascii_lowercase()
    };
    let country = pick(&ask.country, &project.country);
    let language = pick(&ask.language, &project.language);
    let market = Market::new(&country, &language);
    market.location_name()?;
    let location = ask
        .location
        .as_deref()
        .filter(|place| !place.trim().is_empty())
        .map(|place| check::location(place, &market))
        .transpose()?;
    Ok(TrackerSpec {
        engine: ask.engine.as_str().to_owned(),
        country,
        language,
        device: ask.device.as_str().to_owned(),
        location,
    })
}

fn add_rank_tracker(context: &Context<'_>, ask: TrackerAsk) -> Result<Setup, AppError> {
    let store = context.workspace.store()?;
    let project = store.project(ask.project_id)?;
    store.add_rank_tracker(project.id, &tracker_spec(&project, &ask)?, &now())?;
    drop(store);
    setup(context.workspace, ask.project_id)
}

#[derive(Debug, Deserialize)]
struct RemoveTrackerAsk {
    project_id: i64,
    tracker_id: i64,
}

fn remove_rank_tracker(context: &Context<'_>, ask: RemoveTrackerAsk) -> Result<Setup, AppError> {
    let store = context.workspace.store()?;
    let trackers = store.rank_trackers(ask.project_id)?;
    if trackers.len() == 1 && !store.tracked_keywords(ask.project_id)?.is_empty() {
        return Err(AppError::Invalid(
            "keep at least one place while keywords are tracked".to_owned(),
        ));
    }
    store.remove_rank_tracker(ask.project_id, ask.tracker_id)?;
    drop(store);
    setup(context.workspace, ask.project_id)
}

#[derive(Debug, Deserialize)]
struct NewKeyword {
    keyword: String,
    /// Monthly searches, when the page knows them.
    #[serde(default)]
    volume: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct TrackAsk {
    project_id: i64,
    keywords: Vec<NewKeyword>,
    #[serde(default)]
    tags: Vec<String>,
}

#[derive(Debug, Serialize)]
struct Tracked {
    /// Keywords new to the project.
    added: usize,
    /// Keywords tracked now.
    total: usize,
}

/// Tracks keywords; the first ones get a place too: Google in the project's market, desktop.
fn track_keywords(context: &Context<'_>, ask: TrackAsk) -> Result<Tracked, AppError> {
    let store = context.workspace.store()?;
    let project = store.project(ask.project_id)?;
    if store.rank_trackers(project.id)?.is_empty() {
        let first = TrackerAsk {
            project_id: project.id,
            engine: Engine::Google,
            device: Device::Desktop,
            country: None,
            language: None,
            location: None,
        };
        store.add_rank_tracker(project.id, &tracker_spec(&project, &first)?, &now())?;
    }
    let keywords: Vec<(String, Option<u64>)> = ask
        .keywords
        .into_iter()
        .map(|entry| (entry.keyword, entry.volume))
        .collect();
    let added = store.track_keywords(project.id, &keywords, &ask.tags, &now())?;
    Ok(Tracked {
        added,
        total: store.tracked_keywords(project.id)?.len(),
    })
}

#[derive(Debug, Deserialize)]
struct KeywordsAsk {
    project_id: i64,
    keyword_ids: Vec<i64>,
}

fn untrack_keywords(context: &Context<'_>, ask: KeywordsAsk) -> Result<usize, AppError> {
    Ok(context
        .workspace
        .store()?
        .untrack_keywords(ask.project_id, &ask.keyword_ids)?)
}

#[derive(Debug, Deserialize)]
struct TagsAsk {
    project_id: i64,
    keyword_ids: Vec<i64>,
    tags: Vec<String>,
}

fn set_keyword_tags(context: &Context<'_>, ask: TagsAsk) -> Result<(), AppError> {
    Ok(context
        .workspace
        .store()?
        .set_keyword_tags(ask.project_id, &ask.keyword_ids, &ask.tags)?)
}

// ----- Runs --------------------------------------------------------------------------------

/// Whether a run's checks can go through the standard queue: on the user's own key always, on
/// credits once the gateway forwards the queue's requests for every engine tracked.
fn queue_open(workspace: &Workspace, via: Via, trackers: &[TrackerRow]) -> bool {
    match via {
        Via::Own => true,
        Via::Credit => trackers.iter().all(|tracker| {
            let engine = engine_of(tracker).unwrap_or(Engine::Google);
            workspace.credit_allows(
                octopuson_dataforseo::ID,
                "POST",
                &format!("/v3/{}", check::queue_post(engine)),
            ) && workspace.credit_allows(
                octopuson_dataforseo::ID,
                "GET",
                &format!("/v3/{}", check::queue_get(engine, "id")),
            )
        }),
    }
}

/// What a run and a month of runs cost.
#[derive(Debug, Serialize)]
struct RankEstimate {
    /// Checks of one run (keywords × places).
    checks: u32,
    /// One run started by hand, now (live result pages).
    run: Estimate,
    /// Scheduled runs go through the standard queue.
    queued: bool,
    /// List price of one scheduled run, US dollars.
    scheduled_usd: f64,
    /// Runs in thirty days on the schedule.
    runs_per_month: u32,
    /// List price of thirty days, US dollars.
    month_usd: f64,
    /// Credits of thirty days, when on credits.
    month_credits: Option<f64>,
}

fn rank_estimate(context: &Context<'_>, ask: ProjectAsk) -> Result<RankEstimate, AppError> {
    let store = context.workspace.store()?;
    store.project(ask.project_id)?;
    let settings = store.rank_settings(ask.project_id)?;
    let trackers = store.rank_trackers(ask.project_id)?;
    let keywords = store.tracked_keywords(ask.project_id)?.len();
    drop(store);
    let (checks, usd) = plan_cost(&trackers, keywords, &settings, false);
    let run = context
        .workspace
        .estimate_spend(octopuson_dataforseo::ID, usd)?;
    let queued = run
        .via
        .is_some_and(|via| queue_open(context.workspace, via, &trackers));
    let (_, scheduled_usd) = plan_cost(&trackers, keywords, &settings, queued);
    let runs_per_month = Schedule::parse(&settings.schedule).per_month();
    let month_usd = scheduled_usd * f64::from(runs_per_month);
    Ok(RankEstimate {
        checks,
        month_credits: run
            .credits
            .filter(|_| usd > 0.0)
            .map(|credits| credits / usd * month_usd),
        month_usd,
        runs_per_month,
        queued,
        scheduled_usd,
        run,
    })
}

/// Starts a run of a project as a job, or returns the one running.
fn start(
    workspace: &Arc<Workspace>,
    jobs: &JobManager,
    project: &ProjectRow,
    scheduled: bool,
) -> Result<String, AppError> {
    if let Some(id) = jobs.running_for(JOB_KIND, Some(project.id)) {
        return Ok(id);
    }
    let store = workspace.store()?;
    // The schedule waits while the last run's checks are still in the queue.
    if scheduled && store.waiting_rank_tasks(project.id)? > 0 {
        return Err(AppError::Busy(
            "the last scheduled run is still waiting for its answers".to_owned(),
        ));
    }
    let keywords = store.tracked_keywords(project.id)?.len();
    let trackers = store.rank_trackers(project.id)?.len();
    if keywords == 0 || trackers == 0 {
        return Err(AppError::Invalid(
            "add keywords to track before checking".to_owned(),
        ));
    }
    let total = u32::try_from(keywords * trackers).unwrap_or(u32::MAX);
    let started = now();
    store.mark_rank_run(project.id, &started)?;
    let run_id = store.begin_rank_run(project.id, scheduled, total, &started)?;
    drop(store);
    let workspace = Arc::clone(workspace);
    let project_id = project.id;
    jobs.start(JOB_KIND, &project.name, Some(project.id), move |control| {
        run_checks(&workspace, project_id, run_id, scheduled, control)
    })
}

fn start_rank_check(context: &Context<'_>, ask: ProjectAsk) -> Result<String, AppError> {
    let estimate = rank_estimate(
        context,
        ProjectAsk {
            project_id: ask.project_id,
        },
    )?;
    if estimate.checks == 0 {
        return Err(AppError::Invalid(
            "add keywords to track before checking".to_owned(),
        ));
    }
    if let Some(blocked) = estimate.run.blocked {
        return Err(match blocked.code {
            ErrorCode::Budget => AppError::Budget(blocked.message),
            _ => AppError::NoSource(blocked.message),
        });
    }
    let project = context.workspace.store()?.project(ask.project_id)?;
    start(context.workspace, context.jobs, &project, false)
}

/// Whether a failed check stops the run: everything but a provider's answer about this one
/// keyword or a passing outage.
pub(crate) const fn stops(error: &AppError) -> bool {
    !matches!(
        error,
        AppError::Provider(_) | AppError::Unavailable(_) | AppError::Busy(_)
    )
}

/// How a run went, as its job's result.
#[derive(Debug, Default, Serialize)]
struct RunSummary {
    run_id: i64,
    checked: u32,
    failed: u32,
    /// Checks posted to the standard queue, read later.
    queued: u32,
    cost_usd: f64,
}

/// A check's row before it is read.
fn blank_row(run_id: i64, project_id: i64, keyword_id: i64, tracker_id: i64) -> RankCheckRow {
    RankCheckRow {
        run_id,
        project_id,
        keyword_id,
        tracker_id,
        checked_at: now(),
        position: None,
        url: None,
        pages: Vec::new(),
        features: Vec::new(),
        ai_overview: false,
        ai_cited: false,
        snippet: false,
        owned: Vec::new(),
        results: Vec::new(),
        error: None,
    }
}

/// Fills a check's row with what its result page showed.
fn read_into(row: &mut RankCheckRow, found: RankCheck) {
    row.position = found.position;
    row.url = found.url;
    row.pages = found.pages;
    row.features = found.features;
    row.ai_overview = found.ai_overview;
    row.ai_cited = found.ai_cited;
    row.snippet = found.snippet;
    row.owned = found.owned;
    row.results = found
        .results
        .into_iter()
        .map(|result| CheckResult {
            position: result.position,
            domain: result.domain,
            url: result.url,
            title: result.title,
        })
        .collect();
}

/// How a run ended, from its counts.
pub(crate) const fn end_status(
    cancelled: bool,
    checked: u32,
    failed: u32,
    stopped: bool,
) -> &'static str {
    if cancelled {
        "cancelled"
    } else if checked == 0 && (stopped || failed > 0) {
        "failed"
    } else if failed > 0 || stopped {
        "partial"
    } else {
        "done"
    }
}

#[allow(clippy::too_many_lines)] // one loop with its bookkeeping
fn run_checks(
    workspace: &Workspace,
    project_id: i64,
    run_id: i64,
    scheduled: bool,
    control: &JobControl,
) -> Result<JobDone, AppError> {
    let (project, settings, trackers, keywords) = {
        let store = workspace.store()?;
        (
            store.project(project_id)?,
            store.rank_settings(project_id)?,
            store.rank_trackers(project_id)?,
            store.tracked_keywords(project_id)?,
        )
    };
    let total = (trackers.len() * keywords.len()) as u64;
    let mut summary = RunSummary {
        run_id,
        ..RunSummary::default()
    };
    let mut stopped: Option<AppError> = None;
    let mut done = 0_u64;
    match workspace.paid_run(octopuson_dataforseo::ID) {
        Err(error) => stopped = Some(error),
        Ok(session) if scheduled && queue_open(workspace, session.via(), &trackers) => {
            let queued = Queued {
                project: &project,
                settings: &settings,
                trackers: &trackers,
                keywords: &keywords,
                run_id,
            };
            return post_queued(workspace, &session, &queued, control);
        }
        Ok(session) => {
            'places: for tracker in &trackers {
                let engine = engine_of(tracker)?;
                let market = Market::new(&tracker.country, &tracker.language);
                for keyword in &keywords {
                    if control.cancelled() {
                        break 'places;
                    }
                    control.progress(&keyword.keyword, done, total);
                    let query = RankQuery {
                        keyword: &keyword.keyword,
                        engine,
                        market: &market,
                        location: tracker.location.as_deref(),
                        device: device_of(tracker),
                        depth: settings.depth,
                        ai_overview: settings.ai_overview,
                    };
                    let outcome = check::task(&query)
                        .map_err(AppError::from)
                        .and_then(|task| {
                            let paid = Paid {
                                travels: true,
                                module: MODULE,
                                provider: octopuson_dataforseo::ID,
                                endpoint: check::endpoint(engine),
                                class: CacheClass::Serp,
                                project_id: Some(project_id),
                                task,
                                estimate_usd: check::estimate(
                                    engine,
                                    settings.depth,
                                    settings.ai_overview,
                                ),
                            };
                            session.ask(&paid, |route| {
                                let credentials = route.key(octopuson_dataforseo::SECRET)?;
                                let client = Client::new(route.transport, &credentials)?;
                                Ok(client.post(paid.endpoint, &paid.task)?.into())
                            })
                        });
                    done += 1;
                    let mut row = blank_row(run_id, project_id, keyword.id, tracker.id);
                    let stop = match outcome {
                        Ok(answered) => {
                            read_into(
                                &mut row,
                                check::rank_from_result(&answered.result, &project.domain),
                            );
                            row.checked_at = answered.fetched_at;
                            summary.checked += 1;
                            summary.cost_usd += answered.spent_usd;
                            control.spent(answered.spent_usd);
                            None
                        }
                        Err(error) => {
                            row.error = Some(error.to_string());
                            summary.failed += 1;
                            stops(&error).then_some(error)
                        }
                    };
                    workspace.store()?.add_rank_check(&row)?;
                    if let Some(error) = stop {
                        stopped = Some(error);
                        break 'places;
                    }
                }
            }
        }
    }
    control.progress("", done, total);
    let cancelled = control.cancelled();
    let status = end_status(
        cancelled,
        summary.checked,
        summary.failed,
        stopped.is_some(),
    );
    let message = stopped.as_ref().map(ToString::to_string);
    workspace.store()?.finish_rank_run(
        run_id,
        &RunEnd {
            status,
            checked: summary.checked,
            failed: summary.failed,
            cost_usd: summary.cost_usd,
            error: message.as_deref(),
        },
        &now(),
    )?;
    if status == "failed"
        && let Some(error) = stopped
    {
        return Err(error);
    }
    Ok(JobDone {
        status: if status == "done" || cancelled {
            JobStatus::Done
        } else {
            JobStatus::Partial
        },
        result: serde_json::to_value(&summary).ok(),
    })
}

/// A run whose checks go through the standard queue.
struct Queued<'a> {
    project: &'a ProjectRow,
    settings: &'a RankSettings,
    trackers: &'a [TrackerRow],
    keywords: &'a [TrackedKeyword],
    run_id: i64,
}

/// The tag a queued check is posted with, naming its keyword and place.
fn tag_of(keyword_id: i64, tracker_id: i64) -> String {
    format!("{keyword_id}|{tracker_id}")
}

/// Posts a run's checks to the standard queue, a hundred a request, each request paid within
/// the month's budget. The run keeps running until [`collect_queued`] has read every answer;
/// checks the queue refused are noted as failed at once.
#[allow(clippy::too_many_lines)] // one loop with its bookkeeping
fn post_queued(
    workspace: &Workspace,
    session: &PaidRun<'_>,
    run: &Queued<'_>,
    control: &JobControl,
) -> Result<JobDone, AppError> {
    let project_id = run.project.id;
    let total = (run.trackers.len() * run.keywords.len()) as u64;
    let via = via_name(session.via());
    let mut summary = RunSummary {
        run_id: run.run_id,
        ..RunSummary::default()
    };
    let mut stopped: Option<AppError> = None;
    let mut done = 0_u64;
    let fail = |keyword_id: i64, tracker_id: i64, error: &str| -> Result<(), AppError> {
        let mut row = blank_row(run.run_id, project_id, keyword_id, tracker_id);
        row.error = Some(error.to_owned());
        Ok(workspace.store()?.add_rank_check(&row)?)
    };
    'places: for tracker in run.trackers {
        let engine = engine_of(tracker)?;
        let market = Market::new(&tracker.country, &tracker.language);
        for batch in run.keywords.chunks(check::QUEUE_BATCH) {
            if control.cancelled() {
                break 'places;
            }
            control.progress(&batch[0].keyword, done, total);
            let mut tasks = Vec::with_capacity(batch.len());
            let mut sent: Vec<&TrackedKeyword> = Vec::with_capacity(batch.len());
            for keyword in batch {
                let query = RankQuery {
                    keyword: &keyword.keyword,
                    engine,
                    market: &market,
                    location: tracker.location.as_deref(),
                    device: device_of(tracker),
                    depth: run.settings.depth,
                    ai_overview: run.settings.ai_overview,
                };
                match check::queued_task(&query, &tag_of(keyword.id, tracker.id)) {
                    Ok(task) => {
                        tasks.push(task);
                        sent.push(keyword);
                    }
                    Err(error) => {
                        fail(keyword.id, tracker.id, &AppError::from(error).to_string())?;
                        summary.failed += 1;
                    }
                }
            }
            done += batch.len() as u64;
            if tasks.is_empty() {
                continue;
            }
            #[allow(clippy::cast_precision_loss)] // at most a hundred tasks
            let paid = Paid {
                travels: true,
                module: MODULE,
                provider: octopuson_dataforseo::ID,
                endpoint: check::queue_post(engine),
                class: CacheClass::Serp,
                project_id: Some(project_id),
                task: serde_json::Value::Null,
                estimate_usd: check::queue_estimate(
                    engine,
                    run.settings.depth,
                    run.settings.ai_overview,
                ) * tasks.len() as f64,
            };
            let outcome = session.ask(&paid, |route| {
                let credentials = route.key(octopuson_dataforseo::SECRET)?;
                let client = Client::new(route.transport, &credentials)?;
                Ok(client.post_tasks(paid.endpoint, &tasks)?.into())
            });
            match outcome {
                Ok(answered) => {
                    summary.cost_usd += answered.spent_usd;
                    control.spent(answered.spent_usd);
                    let posted: HashMap<String, check::Posted> =
                        check::posted_from_tasks(&answered.result)
                            .into_iter()
                            .map(|posted| (posted.tag.clone(), posted))
                            .collect();
                    let first_try = unix_now() + FIRST_WAIT;
                    let mut waiting = Vec::with_capacity(sent.len());
                    for keyword in sent {
                        match posted.get(&tag_of(keyword.id, tracker.id)) {
                            Some(check::Posted { id: Some(id), .. }) => {
                                waiting.push(RankTaskRow {
                                    id: 0,
                                    run_id: run.run_id,
                                    project_id,
                                    keyword_id: keyword.id,
                                    tracker_id: tracker.id,
                                    engine: engine.as_str().to_owned(),
                                    task_id: id.clone(),
                                    via: via.to_owned(),
                                    posted_at: answered.fetched_at.clone(),
                                    tries: 0,
                                    next_try_at: first_try,
                                });
                            }
                            other => {
                                let error = other
                                    .and_then(|posted| posted.error.clone())
                                    .unwrap_or_else(|| "the queue did not take it".to_owned());
                                fail(keyword.id, tracker.id, &format!("DataForSEO: {error}"))?;
                                summary.failed += 1;
                            }
                        }
                    }
                    summary.queued += u32::try_from(waiting.len()).unwrap_or(u32::MAX);
                    workspace.store()?.queue_rank_tasks(&waiting)?;
                }
                Err(error) => {
                    for keyword in &sent {
                        fail(keyword.id, tracker.id, &error.to_string())?;
                    }
                    summary.failed += u32::try_from(sent.len()).unwrap_or(u32::MAX);
                    if stops(&error) {
                        stopped = Some(error);
                        break 'places;
                    }
                }
            }
        }
    }
    control.progress("", done, total);
    let message = stopped.as_ref().map(ToString::to_string);
    if summary.queued == 0 {
        // Nothing waits: the run ends now.
        let status = end_status(control.cancelled(), 0, summary.failed, stopped.is_some());
        workspace.store()?.finish_rank_run(
            run.run_id,
            &RunEnd {
                status,
                checked: 0,
                failed: summary.failed,
                cost_usd: summary.cost_usd,
                error: message.as_deref(),
            },
            &now(),
        )?;
        if status == "failed"
            && let Some(error) = stopped
        {
            return Err(error);
        }
    } else {
        workspace
            .store()?
            .queue_rank_run(run.run_id, summary.failed, summary.cost_usd)?;
    }
    Ok(JobDone {
        status: if summary.failed == 0 && stopped.is_none() {
            JobStatus::Done
        } else {
            JobStatus::Partial
        },
        result: serde_json::to_value(&summary).ok(),
    })
}

/// Seconds before asking again for a queued answer asked for `tries` times: the wait doubles
/// from [`FIRST_WAIT`] up to [`LONGEST_WAIT`].
pub(crate) fn wait_after(tries: u32) -> i64 {
    FIRST_WAIT
        .saturating_mul(1_i64 << tries.min(10))
        .min(LONGEST_WAIT)
}

/// What a round of reading the queue did.
#[derive(Debug, Default, Clone, Copy, Serialize)]
pub struct Collected {
    /// Answers read and kept.
    pub read: u32,
    /// Checks still unfinished, asked again later.
    pub waiting: u32,
    /// Checks given up on or refused.
    pub failed: u32,
    /// Runs finished by this round.
    pub finished: u32,
}

/// Whether a failure to read a queued answer is final: the provider refused it (an unknown or
/// expired task). Anything else is asked again until [`GIVE_UP`].
pub(crate) const fn is_final(error: &AppError) -> bool {
    matches!(error, AppError::Provider(_) | AppError::Invalid(_))
}

/// A project's site and the keywords and places it still tracks, for reading its answers.
struct Watched {
    project: ProjectRow,
    keywords: HashSet<i64>,
    trackers: HashSet<i64>,
}

/// Reads the answers of queued checks that are due, keeps them as checks of their run, and
/// finishes the runs with nothing left in the queue. Hosts call it every minute (the desktop
/// program while it is open, even while the web runs the schedule: only this copy can read
/// what it posted; the web server for each account with checks waiting). A round reads at most
/// [`COLLECT_LIMIT`] answers for [`COLLECT_TIME`]; a round already reading makes this one
/// return at once.
///
/// # Errors
/// Storage errors.
#[allow(clippy::too_many_lines)] // one loop with its bookkeeping
pub fn collect_queued(workspace: &Workspace) -> Result<Collected, AppError> {
    let Ok(_reading) = workspace.collecting.try_lock() else {
        return Ok(Collected::default());
    };
    let started = Instant::now();
    let now_secs = unix_now();
    let due = workspace.store()?.due_rank_tasks(now_secs, COLLECT_LIMIT)?;
    let mut collected = Collected::default();
    if due.is_empty() {
        return Ok(collected);
    }
    let session = workspace.paid_run(octopuson_dataforseo::ID);
    let mut projects: HashMap<i64, Option<Watched>> = HashMap::new();
    let mut runs: BTreeSet<i64> = BTreeSet::new();
    for task in due {
        if started.elapsed() > COLLECT_TIME {
            break;
        }
        runs.insert(task.run_id);
        if let Entry::Vacant(slot) = projects.entry(task.project_id) {
            let store = workspace.store()?;
            let tracked = match store.project(task.project_id) {
                Ok(project) => Some(Watched {
                    keywords: store
                        .tracked_keywords(project.id)?
                        .iter()
                        .map(|keyword| keyword.id)
                        .collect(),
                    trackers: store
                        .rank_trackers(project.id)?
                        .iter()
                        .map(|tracker| tracker.id)
                        .collect(),
                    project,
                }),
                Err(_) => None,
            };
            slot.insert(tracked);
        }
        // Its keyword, place or project went meanwhile: nothing to keep.
        let Some(tracked) = projects
            .get(&task.project_id)
            .and_then(Option::as_ref)
            .filter(|tracked| {
                tracked.keywords.contains(&task.keyword_id)
                    && tracked.trackers.contains(&task.tracker_id)
            })
        else {
            workspace.store()?.drop_rank_task(task.id)?;
            continue;
        };
        let engine = Engine::from_str(&task.engine)
            .map_err(|error| AppError::Internal(error.to_string()))?;
        let answer: Result<Option<Vec<serde_json::Value>>, AppError> = match &session {
            Ok(session) if via_name(session.via()) == task.via => {
                session.free(octopuson_dataforseo::ID, |route| {
                    let credentials = route.key(octopuson_dataforseo::SECRET)?;
                    let client = Client::new(route.transport, &credentials)?;
                    match client.get(&check::queue_get(engine, &task.task_id)) {
                        Ok(answer) => Ok(Some(answer.result)),
                        Err(DfsError::NotReady) => Ok(None),
                        Err(error) => Err(error.into()),
                    }
                })
            }
            Ok(_) => Err(AppError::NoSource(
                "DataForSEO is paid another way than when the check was posted".to_owned(),
            )),
            Err(error) => Err(AppError::NoSource(error.to_string())),
        };
        let expired =
            measure::seconds_of(&task.posted_at).is_some_and(|posted| now_secs - posted > GIVE_UP);
        let mut row = blank_row(
            task.run_id,
            task.project_id,
            task.keyword_id,
            task.tracker_id,
        );
        match answer {
            Ok(Some(result)) => {
                read_into(
                    &mut row,
                    check::rank_from_result(&result, &tracked.project.domain),
                );
                collected.read += 1;
            }
            Err(error) if is_final(&error) || expired => {
                row.error = Some(error.to_string());
                collected.failed += 1;
            }
            Ok(None) if expired => {
                row.error = Some("DataForSEO: the queue did not answer within a day".to_owned());
                collected.failed += 1;
            }
            Ok(None) | Err(_) => {
                workspace
                    .store()?
                    .delay_rank_task(task.id, now_secs + wait_after(task.tries))?;
                collected.waiting += 1;
                continue;
            }
        }
        let store = workspace.store()?;
        store.add_rank_check(&row)?;
        store.drop_rank_task(task.id)?;
    }
    for run_id in runs {
        let store = workspace.store()?;
        let Some(run) = store.rank_run(run_id)? else {
            continue;
        };
        if run.status != "running" || run.waiting > 0 {
            continue;
        }
        let (checked, failed) = store.rank_run_tally(run_id)?;
        store.finish_rank_run(
            run_id,
            &RunEnd {
                status: end_status(false, checked, failed, false),
                checked,
                failed,
                cost_usd: run.cost_usd,
                error: None,
            },
            &now(),
        )?;
        collected.finished += 1;
    }
    Ok(collected)
}

/// When the queue should be read next, seconds since the Unix epoch: when a waiting check is
/// due, or in a minute while a run is posting checks; `None` with nothing in the queue.
///
/// # Errors
/// Storage errors.
pub fn next_collect(workspace: &Workspace, jobs: &JobManager) -> Result<Option<i64>, AppError> {
    let waiting = workspace.store()?.next_rank_task_at()?;
    let posting = jobs
        .list()
        .iter()
        .any(|job| job.kind == JOB_KIND && job.status == JobStatus::Running);
    Ok(if posting {
        Some(waiting.map_or(unix_now() + COLLECT_EVERY, |at| {
            at.min(unix_now() + COLLECT_EVERY)
        }))
    } else {
        waiting
    })
}

fn rank_runs(context: &Context<'_>, ask: ProjectAsk) -> Result<Vec<RankRunRow>, AppError> {
    Ok(context
        .workspace
        .store()?
        .rank_runs(ask.project_id, RUNS_SHOWN)?)
}

/// Starts the runs whose schedule is due; returns the jobs started (or already running).
/// A project that cannot start is skipped and logged, so the others still run.
///
/// # Errors
/// Storage errors.
pub fn run_due(workspace: &Arc<Workspace>, jobs: &JobManager) -> Result<Vec<String>, AppError> {
    let now_secs = unix_now();
    let store = workspace.store()?;
    let mut projects = Vec::new();
    for plan in store.rank_plans()? {
        if due_at(&plan).is_some_and(|at| at <= now_secs) {
            projects.push(store.project(plan.project_id)?);
        }
    }
    drop(store);
    let mut started = Vec::new();
    for project in &projects {
        match start(workspace, jobs, project, true) {
            Ok(id) => started.push(id),
            Err(error) => {
                tracing::warn!(project = project.id, %error, "a scheduled rank check did not start");
            }
        }
    }
    Ok(started)
}

/// When the next scheduled run is due, seconds since the Unix epoch (in the past when one is
/// due now); `None` when no project has a schedule.
///
/// # Errors
/// Storage errors.
pub fn next_due(workspace: &Workspace) -> Result<Option<i64>, AppError> {
    Ok(workspace
        .store()?
        .rank_plans()?
        .iter()
        .filter_map(due_at)
        .min())
}

// ----- Report ------------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct ReportAsk {
    project_id: i64,
    /// The first place when empty.
    #[serde(default)]
    tracker_id: Option<i64>,
    /// Days of history (7–365).
    #[serde(default)]
    days: Option<u32>,
    /// A day (`yyyy-mm-dd`) to compare the latest positions with.
    #[serde(default)]
    compare: Option<String>,
}

/// A keyword's position on the compared day.
#[derive(Debug, Serialize)]
struct Compared {
    /// The site's position at the last check on or before the day.
    position: Option<u16>,
    /// When that check was made, RFC 3339.
    checked_at: String,
}

/// A keyword's ranks on one place.
#[derive(Debug, Serialize)]
struct KeywordRank {
    keyword_id: i64,
    keyword: String,
    tags: Vec<String>,
    volume: Option<u64>,
    #[serde(flatten)]
    trend: Trend,
    /// The site's pages on the latest result page, by position.
    pages: Vec<(u16, String)>,
    /// Elements of the latest result page.
    features: Vec<String>,
    ai_overview: bool,
    ai_cited: bool,
    snippet: bool,
    /// Elements of the latest result page that show the site.
    owned: Vec<String>,
    /// Estimated monthly visits: its searches × the click share of its position.
    traffic: Option<f64>,
    /// The listed competitors' positions on the latest result page (first 20 results).
    rivals: BTreeMap<String, Option<u16>>,
    /// Its position on the compared day, when one was asked for and checked by then.
    #[serde(skip_serializing_if = "Option::is_none")]
    compared: Option<Compared>,
}

/// How often an element shows on the tracked keywords' pages, and shows the site.
#[derive(Debug, Serialize)]
struct FeatureCount {
    feature: String,
    /// Keywords whose latest page shows it.
    shown: u32,
    /// Of those, keywords where it shows the site.
    owned: u32,
}

/// A page of the site and the tracked keywords it ranks for.
#[derive(Debug, Serialize)]
struct LandingPage {
    url: String,
    /// Keywords it is the site's best page for.
    keywords: u32,
    best_position: u16,
    /// Estimated monthly visits from them.
    traffic: f64,
}

/// The report's figures.
#[derive(Debug, Default, Serialize)]
struct RankSummary {
    /// Latest visibility (0–100).
    visibility: Option<f64>,
    /// Change since the day before it.
    visibility_change: Option<f64>,
    /// Average position where the site ranks.
    average_position: Option<f64>,
    top3: u32,
    top10: u32,
    ranked: u32,
    /// Keywords with a check.
    checked: u32,
    improved: u32,
    declined: u32,
    /// Ranked before, not now.
    lost: u32,
    ai_overviews: u32,
    ai_cited: u32,
    snippets: u32,
    cannibalised: u32,
    /// Estimated monthly visits from the tracked keywords.
    traffic: f64,
    /// Elements of the latest pages, most shown first.
    features: Vec<FeatureCount>,
}

/// Shares of the site and its competitors on one day.
#[derive(Debug, Serialize)]
struct SharePoint {
    date: String,
    shares: BTreeMap<String, f64>,
}

#[derive(Debug, Serialize)]
struct RankReport {
    tracker: Option<TrackerRow>,
    rows: Vec<KeywordRank>,
    summary: RankSummary,
    visibility: Vec<VisibilityPoint>,
    competitors: Vec<CompetitorShare>,
    /// Estimated monthly visits of the site and each competitor from the tracked keywords.
    traffic: BTreeMap<String, f64>,
    /// The site's pages that rank, most visits first.
    landing_pages: Vec<LandingPage>,
    share_history: Vec<SharePoint>,
    last_run: Option<RankRunRow>,
    /// Start of the history shown, RFC 3339.
    since: String,
    /// The day compared with, when asked.
    compare: Option<String>,
}

/// Estimated monthly visits from a keyword at a position.
#[allow(clippy::cast_precision_loss)] // volumes
fn visits(volume: Option<u64>, position: Option<u16>) -> Option<f64> {
    volume
        .zip(position)
        .map(|(volume, position)| (volume as f64 * measure::ctr(position)).round())
}

fn view(check: &RankCheckRow) -> Check<'_> {
    Check {
        checked_at: &check.checked_at,
        position: check.position,
        url: check.url.as_deref(),
        pages: check.pages.len(),
        error: check.error.as_deref(),
    }
}

/// Names result hosts for the shares: the project's site and its listed competitors by their
/// domain, every other site by its registrable domain.
fn labeler(project: &ProjectRow) -> impl Fn(&str) -> String + '_ {
    move |host| {
        if check::belongs(host, &project.domain) {
            return project.domain.clone();
        }
        project
            .competitors
            .iter()
            .find(|competitor| check::belongs(host, competitor))
            .cloned()
            .unwrap_or_else(|| registrable_domain(host))
    }
}

fn page_of(check: &RankCheckRow, weight: f64, label: &dyn Fn(&str) -> String) -> ResultPage {
    ResultPage {
        weight,
        domains: check
            .results
            .iter()
            .map(|result| (result.position, label(&result.domain)))
            .collect(),
    }
}

/// The elements of the latest pages: how many keywords show each, and show the site in it.
fn feature_counts(rows: &[KeywordRank]) -> Vec<FeatureCount> {
    let mut counts: BTreeMap<&str, (u32, u32)> = BTreeMap::new();
    for row in rows {
        for feature in &row.features {
            counts.entry(feature).or_default().0 += 1;
        }
        for feature in &row.owned {
            let entry = counts.entry(feature).or_default();
            entry.1 += 1;
            entry.0 = entry.0.max(entry.1);
        }
    }
    let mut features: Vec<FeatureCount> = counts
        .into_iter()
        .filter(|(feature, _)| *feature != "organic")
        .map(|(feature, (shown, owned))| FeatureCount {
            feature: feature.to_owned(),
            shown,
            owned,
        })
        .collect();
    features.sort_by(|a, b| {
        b.shown
            .cmp(&a.shown)
            .then_with(|| a.feature.cmp(&b.feature))
    });
    features
}

/// The site's best pages for the keywords, with how many keywords and visits each brings.
fn landing_pages(rows: &[KeywordRank]) -> Vec<LandingPage> {
    let mut pages: HashMap<&str, LandingPage> = HashMap::new();
    for row in rows {
        let (Some(url), Some(position)) = (row.trend.url.as_deref(), row.trend.position) else {
            continue;
        };
        let page = pages.entry(url).or_insert_with(|| LandingPage {
            url: url.to_owned(),
            keywords: 0,
            best_position: position,
            traffic: 0.0,
        });
        page.keywords += 1;
        page.best_position = page.best_position.min(position);
        page.traffic += row.traffic.unwrap_or_default();
    }
    let mut pages: Vec<LandingPage> = pages.into_values().collect();
    pages.sort_by(|a, b| {
        b.traffic
            .total_cmp(&a.traffic)
            .then_with(|| b.keywords.cmp(&a.keywords))
            .then_with(|| a.url.cmp(&b.url))
    });
    pages
}

#[allow(clippy::cast_precision_loss)] // keyword counts
fn summarize(rows: &[KeywordRank], visibility: &[VisibilityPoint]) -> RankSummary {
    let count = |test: &dyn Fn(&KeywordRank) -> bool| {
        u32::try_from(rows.iter().filter(|row| test(row)).count()).unwrap_or(u32::MAX)
    };
    let positions: Vec<f64> = rows
        .iter()
        .filter_map(|row| row.trend.position)
        .map(f64::from)
        .collect();
    let last = visibility.last().map(|point| point.visibility);
    let before = visibility
        .len()
        .checked_sub(2)
        .and_then(|index| visibility.get(index))
        .map(|point| point.visibility);
    RankSummary {
        visibility: last,
        visibility_change: last
            .zip(before)
            .map(|(now, before)| ((now - before) * 10.0).round() / 10.0),
        average_position: (!positions.is_empty()).then(|| {
            (positions.iter().sum::<f64>() / positions.len() as f64 * 10.0).round() / 10.0
        }),
        top3: count(&|row| row.trend.position.is_some_and(|p| p <= 3)),
        top10: count(&|row| row.trend.position.is_some_and(|p| p <= 10)),
        ranked: count(&|row| row.trend.position.is_some()),
        checked: count(&|row| row.trend.checked_at.is_some()),
        improved: count(&|row| row.trend.change.is_some_and(|change| change > 0)),
        declined: count(&|row| row.trend.change.is_some_and(|change| change < 0)),
        lost: count(&|row| row.trend.position.is_none() && row.trend.previous.is_some()),
        ai_overviews: count(&|row| row.ai_overview),
        ai_cited: count(&|row| row.ai_cited),
        snippets: count(&|row| row.snippet),
        cannibalised: count(&|row| row.trend.cannibalised),
        traffic: rows.iter().filter_map(|row| row.traffic).sum(),
        features: feature_counts(rows),
    }
}

#[allow(clippy::cast_precision_loss, clippy::too_many_lines)] // volumes; one report
fn rank_report(context: &Context<'_>, ask: ReportAsk) -> Result<RankReport, AppError> {
    let days = ask.days.unwrap_or(DEFAULT_DAYS).clamp(7, MAX_DAYS);
    let since = rfc3339(unix_now() - i64::from(days) * 86_400);
    let compare = ask
        .compare
        .as_deref()
        .map(str::trim)
        .filter(|day| {
            day.len() == 10
                && day.chars().enumerate().all(|(at, c)| {
                    if at == 4 || at == 7 {
                        c == '-'
                    } else {
                        c.is_ascii_digit()
                    }
                })
        })
        .map(str::to_owned);
    let store = context.workspace.store()?;
    let project = store.project(ask.project_id)?;
    let trackers = store.rank_trackers(project.id)?;
    let tracker = match ask.tracker_id {
        Some(id) => Some(
            trackers
                .iter()
                .find(|tracker| tracker.id == id)
                .cloned()
                .ok_or_else(|| AppError::NotFound(format!("tracker {id}")))?,
        ),
        None => trackers.first().cloned(),
    };
    let keywords = store.tracked_keywords(project.id)?;
    let checks = match &tracker {
        Some(tracker) => store.rank_checks(project.id, tracker.id, None, Some(&since))?,
        None => Vec::new(),
    };
    // The last check of each keyword on or before the compared day, within the four months
    // before it.
    let mut then: HashMap<i64, Compared> = HashMap::new();
    if let (Some(tracker), Some(day)) = (&tracker, &compare) {
        let from = measure::seconds_of(&format!("{day}T00:00:00Z"))
            .map(|start| rfc3339(start - 120 * 86_400));
        for check in store
            .rank_checks(project.id, tracker.id, None, from.as_deref())?
            .into_iter()
            .filter(|check| check.error.is_none())
            .filter(|check| {
                check
                    .checked_at
                    .get(..10)
                    .is_some_and(|date| date <= day.as_str())
            })
        {
            then.insert(
                check.keyword_id,
                Compared {
                    position: check.position,
                    checked_at: check.checked_at,
                },
            );
        }
    }
    let last_run = store.rank_runs(project.id, 1)?.into_iter().next();
    drop(store);

    let weights: HashMap<i64, f64> = keywords
        .iter()
        .map(|keyword| (keyword.id, keyword.volume.unwrap_or(1).max(1) as f64))
        .collect();
    let weight = |id: &i64| weights.get(id).copied().unwrap_or(1.0);
    let mut by_keyword: HashMap<i64, Vec<&RankCheckRow>> = HashMap::new();
    for check in &checks {
        by_keyword.entry(check.keyword_id).or_default().push(check);
    }
    let label = labeler(&project);
    let mut rows = Vec::with_capacity(keywords.len());
    let mut pages = Vec::new();
    // Visits of each domain: only keywords with a known volume bring any.
    let mut visits_of: HashMap<String, f64> = HashMap::new();
    for keyword in &keywords {
        let own: &[&RankCheckRow] = by_keyword.get(&keyword.id).map_or(&[], Vec::as_slice);
        let views: Vec<Check<'_>> = own.iter().map(|check| view(check)).collect();
        let latest = own.iter().rev().find(|check| check.error.is_none());
        let mut rivals: BTreeMap<String, Option<u16>> = project
            .competitors
            .iter()
            .map(|competitor| (competitor.clone(), None))
            .collect();
        if let Some(latest) = latest {
            let page = page_of(latest, weight(&keyword.id), &label);
            let mut seen: HashSet<&str> = HashSet::new();
            for (position, domain) in &page.domains {
                if !seen.insert(domain) {
                    continue;
                }
                if let Some(slot) = rivals.get_mut(domain) {
                    *slot = Some(*position);
                }
                if let Some(visits) = visits(keyword.volume, Some(*position)) {
                    *visits_of.entry(domain.clone()).or_default() += visits;
                }
            }
            pages.push(page);
        }
        let trend = measure::trend(&views);
        rows.push(KeywordRank {
            keyword_id: keyword.id,
            keyword: keyword.keyword.clone(),
            tags: keyword.tags.clone(),
            volume: keyword.volume,
            traffic: visits(keyword.volume, trend.position),
            owned: latest.map(|check| check.owned.clone()).unwrap_or_default(),
            rivals,
            compared: then.remove(&keyword.id),
            trend,
            pages: latest.map(|check| check.pages.clone()).unwrap_or_default(),
            features: latest
                .map(|check| check.features.clone())
                .unwrap_or_default(),
            ai_overview: latest.is_some_and(|check| check.ai_overview),
            ai_cited: latest.is_some_and(|check| check.ai_cited),
            snippet: latest.is_some_and(|check| check.snippet),
        });
    }
    let visibility = measure::visibility(
        checks.iter().map(|check| (check.keyword_id, view(check))),
        &weight,
    );
    let competitors = measure::competitor_shares(&pages, &project.domain, &project.competitors);
    let domains: Vec<String> = competitors
        .iter()
        .map(|share| share.domain.clone())
        .collect();
    let mut days_seen: BTreeMap<&str, HashMap<i64, &RankCheckRow>> = BTreeMap::new();
    for check in checks.iter().filter(|check| check.error.is_none()) {
        days_seen
            .entry(check.checked_at.get(..10).unwrap_or_default())
            .or_default()
            .insert(check.keyword_id, check);
    }
    let share_history = days_seen
        .into_iter()
        .map(|(date, latest)| {
            let pages: Vec<ResultPage> = latest
                .values()
                .map(|check| page_of(check, weight(&check.keyword_id), &label))
                .collect();
            SharePoint {
                date: date.to_owned(),
                shares: measure::shares_of(&pages, &project.domain, &domains)
                    .into_iter()
                    .map(|share| (share.domain, share.visibility))
                    .collect(),
            }
        })
        .collect();
    let traffic = competitors
        .iter()
        .map(|share| {
            (
                share.domain.clone(),
                visits_of.get(&share.domain).copied().unwrap_or_default(),
            )
        })
        .collect();
    Ok(RankReport {
        summary: summarize(&rows, &visibility),
        landing_pages: landing_pages(&rows),
        tracker,
        rows,
        visibility,
        competitors,
        traffic,
        share_history,
        last_run,
        since,
        compare,
    })
}

#[derive(Debug, Deserialize)]
struct PlacesAsk {
    project_id: i64,
    /// Days of history the changes are read over (7–365).
    #[serde(default)]
    days: Option<u32>,
}

/// A keyword's latest rank on one place.
#[derive(Debug, Serialize)]
struct PlaceRank {
    tracker_id: i64,
    position: Option<u16>,
    /// Places gained since the check before.
    change: Option<i32>,
    url: Option<String>,
    checked_at: Option<String>,
    error: Option<String>,
}

/// A keyword on every place.
#[derive(Debug, Serialize)]
struct KeywordPlaces {
    keyword_id: i64,
    keyword: String,
    tags: Vec<String>,
    volume: Option<u64>,
    /// In the order of the places.
    places: Vec<PlaceRank>,
}

/// A place's headline figures.
#[derive(Debug, Serialize)]
struct PlaceSummary {
    tracker_id: i64,
    /// Latest visibility (0–100).
    visibility: Option<f64>,
    average_position: Option<f64>,
    top10: u32,
    ranked: u32,
}

#[derive(Debug, Serialize)]
struct PlacesReport {
    trackers: Vec<TrackerRow>,
    rows: Vec<KeywordPlaces>,
    summaries: Vec<PlaceSummary>,
}

/// Every tracked keyword on every place side by side: where the site ranks in each country,
/// city and device at once.
#[allow(clippy::cast_precision_loss)] // volumes and counts
fn rank_places(context: &Context<'_>, ask: PlacesAsk) -> Result<PlacesReport, AppError> {
    let days = ask.days.unwrap_or(DEFAULT_DAYS).clamp(7, MAX_DAYS);
    let since = rfc3339(unix_now() - i64::from(days) * 86_400);
    let store = context.workspace.store()?;
    let project = store.project(ask.project_id)?;
    let trackers = store.rank_trackers(project.id)?;
    let keywords = store.tracked_keywords(project.id)?;
    let mut checks = Vec::with_capacity(trackers.len());
    for tracker in &trackers {
        checks.push(store.rank_checks(project.id, tracker.id, None, Some(&since))?);
    }
    drop(store);
    let weights: HashMap<i64, f64> = keywords
        .iter()
        .map(|keyword| (keyword.id, keyword.volume.unwrap_or(1).max(1) as f64))
        .collect();
    let weight = |id: &i64| weights.get(id).copied().unwrap_or(1.0);
    let mut rows: Vec<KeywordPlaces> = keywords
        .iter()
        .map(|keyword| KeywordPlaces {
            keyword_id: keyword.id,
            keyword: keyword.keyword.clone(),
            tags: keyword.tags.clone(),
            volume: keyword.volume,
            places: Vec::with_capacity(trackers.len()),
        })
        .collect();
    let mut summaries = Vec::with_capacity(trackers.len());
    for (tracker, checks) in trackers.iter().zip(&checks) {
        let mut by_keyword: HashMap<i64, Vec<Check<'_>>> = HashMap::new();
        for check in checks {
            by_keyword
                .entry(check.keyword_id)
                .or_default()
                .push(view(check));
        }
        let mut positions: Vec<f64> = Vec::new();
        let mut top10 = 0_u32;
        for row in &mut rows {
            let trend = measure::trend(by_keyword.get(&row.keyword_id).map_or(&[], Vec::as_slice));
            if let Some(position) = trend.position {
                positions.push(f64::from(position));
                if position <= 10 {
                    top10 += 1;
                }
            }
            row.places.push(PlaceRank {
                tracker_id: tracker.id,
                position: trend.position,
                change: trend.change,
                url: trend.url,
                checked_at: trend.checked_at,
                error: trend.error,
            });
        }
        let visibility = measure::visibility(
            checks.iter().map(|check| (check.keyword_id, view(check))),
            &weight,
        );
        summaries.push(PlaceSummary {
            tracker_id: tracker.id,
            visibility: visibility.last().map(|point| point.visibility),
            average_position: (!positions.is_empty()).then(|| {
                (positions.iter().sum::<f64>() / positions.len() as f64 * 10.0).round() / 10.0
            }),
            top10,
            ranked: u32::try_from(positions.len()).unwrap_or(u32::MAX),
        });
    }
    Ok(PlacesReport {
        trackers,
        rows,
        summaries,
    })
}

#[derive(Debug, Deserialize)]
struct KeywordAsk {
    project_id: i64,
    keyword_id: i64,
    tracker_id: i64,
    #[serde(default)]
    days: Option<u32>,
}

/// One check as the keyword's page shows it.
#[derive(Debug, Serialize)]
struct CheckPoint {
    checked_at: String,
    position: Option<u16>,
    url: Option<String>,
    pages: Vec<(u16, String)>,
    features: Vec<String>,
    ai_overview: bool,
    ai_cited: bool,
    snippet: bool,
    error: Option<String>,
}

#[derive(Debug, Serialize)]
struct KeywordDetail {
    keyword: TrackedKeyword,
    tracker: TrackerRow,
    /// Every check in the period, oldest first.
    checks: Vec<CheckPoint>,
    /// The first results of the latest result page read.
    results: Vec<CheckResult>,
}

fn rank_keyword(context: &Context<'_>, ask: KeywordAsk) -> Result<KeywordDetail, AppError> {
    let days = ask.days.unwrap_or(MAX_DAYS).clamp(7, MAX_DAYS);
    let since = rfc3339(unix_now() - i64::from(days) * 86_400);
    let store = context.workspace.store()?;
    let keyword = store
        .tracked_keywords(ask.project_id)?
        .into_iter()
        .find(|keyword| keyword.id == ask.keyword_id)
        .ok_or_else(|| AppError::NotFound(format!("keyword {}", ask.keyword_id)))?;
    let tracker = store
        .rank_trackers(ask.project_id)?
        .into_iter()
        .find(|tracker| tracker.id == ask.tracker_id)
        .ok_or_else(|| AppError::NotFound(format!("tracker {}", ask.tracker_id)))?;
    let checks = store.rank_checks(
        ask.project_id,
        ask.tracker_id,
        Some(ask.keyword_id),
        Some(&since),
    )?;
    let results = checks
        .iter()
        .rev()
        .find(|check| check.error.is_none())
        .map(|check| check.results.clone())
        .unwrap_or_default();
    Ok(KeywordDetail {
        keyword,
        tracker,
        checks: checks
            .into_iter()
            .map(|check| CheckPoint {
                checked_at: check.checked_at,
                position: check.position,
                url: check.url,
                pages: check.pages,
                features: check.features,
                ai_overview: check.ai_overview,
                ai_cited: check.ai_cited,
                snippet: check.snippet,
                error: check.error,
            })
            .collect(),
        results,
    })
}

// ----- Volumes -----------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct VolumesAsk {
    project_id: i64,
    /// Measure every keyword again, not only those without a volume.
    #[serde(default)]
    all: bool,
}

/// The keywords to measure and the paid parts measuring them (Labs keyword numbers, at most
/// 700 keywords a part, kept 30 days like the Keyword Explorer's).
fn volume_parts(
    workspace: &Workspace,
    ask: &VolumesAsk,
) -> Result<(Vec<String>, Vec<Paid<'static>>), AppError> {
    let store = workspace.store()?;
    let project = store.project(ask.project_id)?;
    let keywords: Vec<String> = store
        .tracked_keywords(project.id)?
        .into_iter()
        .filter(|keyword| ask.all || keyword.volume.is_none())
        .map(|keyword| keyword.keyword)
        .filter(|keyword| keyword.chars().count() <= labs::MAX_KEYWORD)
        .collect();
    drop(store);
    if keywords.is_empty() {
        return Err(AppError::Invalid(
            "every tracked keyword has its volume".to_owned(),
        ));
    }
    let market = Market::new(&project.country, &project.language);
    let parts = keywords
        .chunks(labs::MAX_OVERVIEW)
        .map(|chunk| {
            let task = labs::overview_task(chunk, &market)?;
            let count = task["keywords"].as_array().map_or(0, Vec::len);
            Ok(Paid {
                travels: true,
                module: MODULE,
                provider: octopuson_dataforseo::ID,
                endpoint: labs::OVERVIEW,
                class: CacheClass::Keywords,
                project_id: Some(project.id),
                task,
                estimate_usd: labs::overview_estimate(count),
            })
        })
        .collect::<Result<Vec<_>, AppError>>()?;
    Ok((keywords, parts))
}

fn rank_volumes_estimate(context: &Context<'_>, ask: VolumesAsk) -> Result<Estimate, AppError> {
    let (_, parts) = volume_parts(context.workspace, &ask)?;
    context.workspace.estimate_many(&parts)
}

#[derive(Debug, Serialize)]
struct VolumesFetched {
    /// Keywords whose volume is known now.
    updated: usize,
    spent_usd: f64,
}

fn fetch_rank_volumes(context: &Context<'_>, ask: VolumesAsk) -> Result<VolumesFetched, AppError> {
    let (keywords, parts) = volume_parts(context.workspace, &ask)?;
    let fetched = context.workspace.fetch_many(&parts, false, |route, part| {
        let credentials = route.key(octopuson_dataforseo::SECRET)?;
        let client = Client::new(route.transport, &credentials)?;
        Ok(client.post(part.endpoint, &part.task)?.into())
    })?;
    let mut found: HashMap<String, Option<u64>> = HashMap::new();
    for result in &fetched.data {
        for data in labs::list_from_result(result).keywords {
            found.insert(data.keyword.to_lowercase(), data.volume);
        }
    }
    let volumes: Vec<(String, Option<u64>)> = keywords
        .into_iter()
        .filter_map(|keyword| {
            let volume = *found.get(&keyword.to_lowercase())?;
            Some((keyword, Some(volume.unwrap_or(0))))
        })
        .collect();
    context
        .workspace
        .store()?
        .set_keyword_volumes(ask.project_id, &volumes)?;
    Ok(VolumesFetched {
        updated: volumes.len(),
        spent_usd: fetched.spent_usd,
    })
}

/// Adds the commands.
pub fn register(router: &mut Router) {
    router.add("rank_setup", rank_setup);
    router.add("save_rank_settings", save_rank_settings);
    router.add("add_rank_tracker", add_rank_tracker);
    router.add("remove_rank_tracker", remove_rank_tracker);
    router.add("track_keywords", track_keywords);
    router.add("untrack_keywords", untrack_keywords);
    router.add("set_keyword_tags", set_keyword_tags);
    router.add("rank_estimate", rank_estimate);
    router.add("start_rank_check", start_rank_check);
    router.add("rank_runs", rank_runs);
    router.add("rank_report", rank_report);
    router.add("rank_places", rank_places);
    router.add("rank_keyword", rank_keyword);
    router.add("rank_volumes_estimate", rank_volumes_estimate);
    router.add("fetch_rank_volumes", fetch_rank_volumes);
}
