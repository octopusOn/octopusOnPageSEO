//! Rank Tracker: where the project's site ranks for its keywords on Google, Bing or Yandex, by
//! country, city and device: positions and their history, visibility, the site's pages that
//! rank (and compete with each other), the elements of the result page, whether an AI
//! Overview is shown and cites the site, and the competitors' shares over time.
//!
//! A check is always asked fresh (DataForSEO's live result page, on the user's key or on
//! credits) and only its reading is kept, in the rank history. A run is a job; the schedule
//! starts runs when due ([`run_due`]): the desktop program asks every minute while it is open,
//! the web server for every account.

use std::collections::{BTreeMap, HashMap};
use std::str::FromStr;
use std::sync::Arc;

use octopuson_core::Timestamp;
use octopuson_dataforseo::keywords as labs;
use octopuson_dataforseo::rank::{self as check, RankQuery};
use octopuson_dataforseo::{Client, Market};
use octopuson_platform::{
    CacheClass, CheckResult, ProjectRow, RankCheckRow, RankPlan, RankRunRow, RankSettings, RunEnd,
    TrackedKeyword, TrackerRow, TrackerSpec, unix_now,
};
use octopuson_serp::{Device, Engine, registrable_domain};
use octopuson_serp_score::rank::{
    self as measure, Check, CompetitorShare, ResultPage, Schedule, Trend, VisibilityPoint,
};
use serde::{Deserialize, Serialize};

use crate::fetch::{Estimate, Paid};
use crate::jobs::{JobControl, JobDone, JobManager, JobStatus};
use crate::router::{Context, ErrorCode, Router};
use crate::{AppError, Workspace, now};

/// Module id, in spending records.
const MODULE: &str = "rank";

/// Kind of the job of a run.
pub const JOB_KIND: &str = "rank-check";

/// Days of history a report shows unless asked otherwise.
const DEFAULT_DAYS: u32 = 90;

/// Most days of history a report shows.
const MAX_DAYS: u32 = 365;

/// Runs listed.
const RUNS_SHOWN: u32 = 20;

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

/// The checks of one run and their list price.
fn plan_cost(trackers: &[TrackerRow], keywords: usize, settings: &RankSettings) -> (u32, f64) {
    let mut checks = 0_u32;
    let mut usd = 0.0;
    for tracker in trackers {
        let engine = engine_of(tracker).unwrap_or(Engine::Google);
        let each = check::estimate(engine, settings.depth, settings.ai_overview);
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

/// What a run and a month of runs cost.
#[derive(Debug, Serialize)]
struct RankEstimate {
    /// Checks of one run (keywords × places).
    checks: u32,
    /// One run, now.
    run: Estimate,
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
    let (checks, usd) = plan_cost(&trackers, keywords, &settings);
    let run = context
        .workspace
        .estimate_spend(octopuson_dataforseo::ID, usd)?;
    let runs_per_month = Schedule::parse(&settings.schedule).per_month();
    Ok(RankEstimate {
        checks,
        month_usd: usd * f64::from(runs_per_month),
        month_credits: run
            .credits
            .map(|credits| credits * f64::from(runs_per_month)),
        runs_per_month,
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
        run_checks(&workspace, project_id, run_id, control)
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
const fn stops(error: &AppError) -> bool {
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
    cost_usd: f64,
}

#[allow(clippy::too_many_lines)] // one loop with its bookkeeping
fn run_checks(
    workspace: &Workspace,
    project_id: i64,
    run_id: i64,
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
                    let mut row = RankCheckRow {
                        run_id,
                        project_id,
                        keyword_id: keyword.id,
                        tracker_id: tracker.id,
                        checked_at: now(),
                        position: None,
                        url: None,
                        pages: Vec::new(),
                        features: Vec::new(),
                        ai_overview: false,
                        ai_cited: false,
                        snippet: false,
                        results: Vec::new(),
                        error: None,
                    };
                    let stop = match outcome {
                        Ok(answered) => {
                            let found = check::rank_from_result(&answered.result, &project.domain);
                            row.checked_at = answered.fetched_at;
                            row.position = found.position;
                            row.url = found.url;
                            row.pages = found.pages;
                            row.features = found.features;
                            row.ai_overview = found.ai_overview;
                            row.ai_cited = found.ai_cited;
                            row.snippet = found.snippet;
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
    let status = if cancelled {
        "cancelled"
    } else if summary.checked == 0 && (stopped.is_some() || summary.failed > 0) {
        "failed"
    } else if summary.failed > 0 || stopped.is_some() {
        "partial"
    } else {
        "done"
    };
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
    share_history: Vec<SharePoint>,
    last_run: Option<RankRunRow>,
    /// Start of the history shown, RFC 3339.
    since: String,
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
    }
}

#[allow(clippy::cast_precision_loss)] // volumes
fn rank_report(context: &Context<'_>, ask: ReportAsk) -> Result<RankReport, AppError> {
    let days = ask.days.unwrap_or(DEFAULT_DAYS).clamp(7, MAX_DAYS);
    let since = rfc3339(unix_now() - i64::from(days) * 86_400);
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
    for keyword in &keywords {
        let own: &[&RankCheckRow] = by_keyword.get(&keyword.id).map_or(&[], Vec::as_slice);
        let views: Vec<Check<'_>> = own.iter().map(|check| view(check)).collect();
        let latest = own.iter().rev().find(|check| check.error.is_none());
        if let Some(latest) = latest {
            pages.push(page_of(latest, weight(&keyword.id), &label));
        }
        rows.push(KeywordRank {
            keyword_id: keyword.id,
            keyword: keyword.keyword.clone(),
            tags: keyword.tags.clone(),
            volume: keyword.volume,
            trend: measure::trend(&views),
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
    Ok(RankReport {
        summary: summarize(&rows, &visibility),
        tracker,
        rows,
        visibility,
        competitors,
        share_history,
        last_run,
        since,
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
    router.add("rank_keyword", rank_keyword);
    router.add("rank_volumes_estimate", rank_volumes_estimate);
    router.add("fetch_rank_volumes", fetch_rank_volumes);
}
