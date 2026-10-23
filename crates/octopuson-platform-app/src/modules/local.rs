//! Local SEO: the project's business on Google — its profile, the businesses of its kind
//! around it, where it shows on Google Maps from every point of a grid around it, and its
//! reviews and rating over time.
//!
//! A business found by name, its profile and the businesses around it are paid questions kept
//! like the other reports (`local_estimate`, `get_local`, `fetch_local`). A grid run is a job:
//! started by hand it reads each point's Maps page live, several at once; started by the
//! schedule it posts them to the standard queue wherever it is open, and [`collect_queued`]
//! reads the answers like the rank checks'. Reviews are read through the queue only; scheduled
//! runs read the newest ones too, so the rating's trend fills in.

use std::collections::hash_map::Entry;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, mpsc};
use std::time::Instant;

use octopuson_cloud::Via;
use octopuson_dataforseo::local::{self as places, Business, GridPoint, GridSummary};
use octopuson_dataforseo::{Client, DfsError, Market, queue};
use octopuson_platform::{
    CacheClass, LocalPointRow, LocalReviewRow, LocalRunRow, LocalSettings, LocalTaskRow,
    ProjectRow, RunEnd, unix_now,
};
use octopuson_serp_score::rank::seconds_of;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::rank::{
    COLLECT_LIMIT, COLLECT_TIME, Collected, FIRST_WAIT, GIVE_UP, end_status, is_final, stops,
    wait_after,
};
use crate::fetch::{Blocked, Estimate, Fetched, Paid, PaidRun, via_name};
use crate::jobs::{JobControl, JobDone, JobManager, JobStatus};
use crate::router::{Context, ErrorCode, Router};
use crate::{AppError, Workspace, now};

/// Module id, in spending records.
const MODULE: &str = "local";

/// Kind of the job of a grid run.
pub const JOB_KIND: &str = "local-grid";

/// Kind of the note pages get when answers were read from the queue (not a job of its own).
pub const QUEUE_KIND: &str = "local-queue";

/// Grid runs listed and drawn in the trend.
const RUNS_SHOWN: u32 = 12;

/// Reviews listed.
const REVIEWS_SHOWN: u32 = 500;

/// Newest reviews a scheduled run reads.
const SCHEDULED_REVIEWS: u32 = 20;

/// Reviews read when asked by hand unless asked otherwise.
const DEFAULT_REVIEWS: u32 = 100;

/// Live Maps pages read at once during a grid run (DataForSEO takes 30).
const PARALLEL: usize = 6;

/// Competitors listed under a grid.
const RIVALS_SHOWN: usize = 10;

/// Commands after which the schedule may be due at another time (the web server notes it).
const SCHEDULE_COMMANDS: &[&str] = &[
    "save_local_settings",
    "choose_business",
    "start_local_grid",
    "fetch_local_reviews",
    "delete_project",
];

/// Whether a command may move when the schedule is due next.
pub fn changes_schedule(command: &str) -> bool {
    SCHEDULE_COMMANDS.contains(&command)
}

/// Seconds between scheduled runs.
fn interval(schedule: &str) -> Option<i64> {
    match schedule {
        "weekly" => Some(7 * 86_400),
        "monthly" => Some(30 * 86_400),
        _ => None,
    }
}

/// Scheduled runs in thirty days.
fn per_month(schedule: &str) -> u32 {
    match schedule {
        "weekly" => 4,
        "monthly" => 1,
        _ => 0,
    }
}

fn rfc3339(seconds: i64) -> String {
    octopuson_core::Timestamp::from_unix_micros(seconds.saturating_mul(1_000_000)).to_string()
}

/// The business chosen for a project, when one is.
fn business_of(settings: &LocalSettings) -> Option<Business> {
    settings
        .business
        .clone()
        .and_then(|value| serde_json::from_value(value).ok())
}

/// When a project's grid is due, seconds since the Unix epoch; `None` when it is checked only
/// by hand or has nothing to check.
fn due_at(settings: &LocalSettings) -> Option<i64> {
    let interval = interval(&settings.schedule)?;
    if settings.keywords.is_empty() || business_of(settings).is_none() {
        return None;
    }
    Some(
        settings
            .last_run_at
            .as_deref()
            .and_then(seconds_of)
            .map_or(0, |last| last + interval),
    )
}

/// The points of a project's grid.
fn points_of(settings: &LocalSettings, business: &Business) -> Result<Vec<GridPoint>, AppError> {
    Ok(places::grid(
        business.lat,
        business.lng,
        settings.grid,
        settings.radius_km,
    )?)
}

/// Whether a request can go the way the source is paid now: on the user's own key always, on
/// credits once the gateway forwards it.
fn route_open(workspace: &Workspace, via: Via, method: &str, endpoint: &str) -> bool {
    via == Via::Own
        || workspace.credit_allows(
            octopuson_dataforseo::ID,
            method,
            &format!("/v3/{}", endpoint.trim_start_matches('/')),
        )
}

/// Why a request cannot go on credits yet.
fn not_on_credits() -> String {
    "DataForSEO: this request is not offered on OctopusOn SEO credits yet; it works with your own DataForSEO key".to_owned()
}

/// Whether grid points and reviews can go through the standard queue.
fn queue_open(workspace: &Workspace, via: Via) -> bool {
    route_open(workspace, via, "POST", places::MAPS_POST)
        && route_open(workspace, via, "GET", &places::maps_get("id"))
}

/// Whether reviews can be read (they are read through the queue only).
fn reviews_open(workspace: &Workspace, via: Via) -> bool {
    route_open(workspace, via, "POST", places::REVIEWS_POST)
        && route_open(workspace, via, "GET", &places::reviews_get("id"))
}

// ----- Setup -------------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct ProjectAsk {
    project_id: i64,
}

/// How a project's business is checked.
#[derive(Debug, Serialize)]
struct LocalSetup {
    business: Option<Business>,
    keywords: Vec<String>,
    grid: u32,
    radius_km: f64,
    schedule: String,
    last_run_at: Option<String>,
    /// The grid's points, to draw before the first run.
    points: Vec<GridPoint>,
    /// The Maps zoom the points are looked at with.
    zoom: u8,
    /// When the schedule runs next, RFC 3339; `None` when run only by hand.
    next_run_at: Option<String>,
    /// Reviews waiting in the queue.
    waiting_reviews: u32,
}

fn setup(workspace: &Workspace, project_id: i64) -> Result<LocalSetup, AppError> {
    let store = workspace.store()?;
    store.project(project_id)?;
    let settings = store.local_settings(project_id)?;
    let waiting_reviews = store.waiting_local_tasks(project_id, "reviews")?;
    drop(store);
    let business = business_of(&settings);
    let points = match &business {
        Some(business) => points_of(&settings, business).unwrap_or_default(),
        None => Vec::new(),
    };
    Ok(LocalSetup {
        next_run_at: due_at(&settings).map(|at| rfc3339(at.max(unix_now()))),
        zoom: places::zoom_for(settings.radius_km),
        points,
        business,
        keywords: settings.keywords,
        grid: settings.grid,
        radius_km: settings.radius_km,
        schedule: settings.schedule,
        last_run_at: settings.last_run_at,
        waiting_reviews,
    })
}

fn local_setup(context: &Context<'_>, ask: ProjectAsk) -> Result<LocalSetup, AppError> {
    setup(context.workspace, ask.project_id)
}

#[derive(Debug, Deserialize)]
struct SettingsAsk {
    project_id: i64,
    keywords: Vec<String>,
    grid: u32,
    radius_km: f64,
    schedule: String,
}

fn save_local_settings(context: &Context<'_>, ask: SettingsAsk) -> Result<LocalSetup, AppError> {
    let store = context.workspace.store()?;
    let known = store.local_settings(ask.project_id)?;
    store.save_local_settings(
        ask.project_id,
        &LocalSettings {
            keywords: ask.keywords,
            grid: ask.grid,
            radius_km: ask.radius_km,
            schedule: ask.schedule,
            ..known
        },
    )?;
    drop(store);
    setup(context.workspace, ask.project_id)
}

#[derive(Debug, Deserialize)]
struct ChooseAsk {
    project_id: i64,
    business: Business,
}

/// Makes a business the project's; its rating is noted for today.
fn choose_business(context: &Context<'_>, ask: ChooseAsk) -> Result<LocalSetup, AppError> {
    if ask.business.title.trim().is_empty() {
        return Err(AppError::Invalid("business: a name".to_owned()));
    }
    places::grid(ask.business.lat, ask.business.lng, 3, 1.0)?;
    let store = context.workspace.store()?;
    let known = store.local_settings(ask.project_id)?;
    let value = serde_json::to_value(&ask.business)
        .map_err(|error| AppError::Internal(error.to_string()))?;
    store.save_local_settings(
        ask.project_id,
        &LocalSettings {
            business: Some(value),
            ..known
        },
    )?;
    let at = now();
    store.note_local_profile(
        ask.project_id,
        &at[..10],
        ask.business.rating,
        ask.business.votes,
        None,
        &at,
    )?;
    drop(store);
    setup(context.workspace, ask.project_id)
}

// ----- Profile, businesses around, finding the business -----------------------------------

/// A paid question of the module.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Report {
    /// The chosen business's Google profile.
    #[default]
    Profile,
    /// The businesses of its category around it.
    Competitors,
    /// A business found by its name.
    Find,
}

#[derive(Debug, Deserialize)]
struct Ask {
    project_id: i64,
    #[serde(default)]
    report: Report,
    /// The business's name with its district or city (find).
    #[serde(default)]
    query: Option<String>,
    #[serde(default)]
    force: bool,
}

/// A business around the project's, marked when it is the project's.
#[derive(Debug, Serialize)]
struct Around {
    #[serde(flatten)]
    business: Business,
    own: bool,
    /// Kilometres from the project's business.
    distance_km: f64,
}

#[derive(Debug, Serialize)]
struct Competitors {
    businesses: Vec<Around>,
    /// Businesses of the category within the radius.
    total: Option<u64>,
}

/// Kilometres between two points (haversine).
fn distance_km(from: (f64, f64), to: (f64, f64)) -> f64 {
    let (lat1, lng1) = (from.0.to_radians(), from.1.to_radians());
    let (lat2, lng2) = (to.0.to_radians(), to.1.to_radians());
    let a = ((lat2 - lat1) / 2.0).sin().powi(2)
        + lat1.cos() * lat2.cos() * ((lng2 - lng1) / 2.0).sin().powi(2);
    let km = 2.0 * 6_371.0 * a.sqrt().asin();
    (km * 100.0).round() / 100.0
}

struct Question {
    report: Report,
    business: Option<Business>,
    paid: Paid<'static>,
}

fn question(workspace: &Workspace, ask: &Ask) -> Result<Question, AppError> {
    let store = workspace.store()?;
    let project = store.project(ask.project_id)?;
    let settings = store.local_settings(project.id)?;
    drop(store);
    let market = Market::new(&project.country, &project.language);
    let business = business_of(&settings);
    let paid = |endpoint: &'static str, task: Value, estimate_usd: f64| Paid {
        travels: true,
        module: MODULE,
        provider: octopuson_dataforseo::ID,
        endpoint,
        class: CacheClass::Domain,
        project_id: Some(project.id),
        task,
        estimate_usd,
    };
    let chosen = || {
        business
            .clone()
            .ok_or_else(|| AppError::Invalid("choose the business first".to_owned()))
    };
    let paid = match ask.report {
        Report::Find => {
            let query = ask
                .query
                .as_deref()
                .map(str::trim)
                .filter(|query| !query.is_empty())
                .ok_or_else(|| {
                    AppError::Invalid("query: the business's name and district".to_owned())
                })?;
            paid(
                places::BUSINESS_INFO,
                places::business_task(query, &market)?,
                places::business_estimate(),
            )
        }
        Report::Profile => {
            let business = chosen()?;
            let query = business
                .cid
                .as_ref()
                .map_or_else(|| business.title.clone(), |cid| format!("cid:{cid}"));
            paid(
                places::BUSINESS_INFO,
                places::business_task(&query, &market)?,
                places::business_estimate(),
            )
        }
        Report::Competitors => {
            let business = chosen()?;
            let categories: Vec<String> = business.category.clone().into_iter().collect();
            if categories.is_empty() {
                return Err(AppError::Invalid(
                    "the business has no category on Google".to_owned(),
                ));
            }
            paid(
                places::LISTINGS,
                places::listings_task(
                    &categories,
                    business.lat,
                    business.lng,
                    settings.radius_km,
                    places::LISTING_ROWS,
                ),
                places::listings_estimate(places::LISTING_ROWS),
            )
        }
    };
    Ok(Question {
        report: ask.report,
        business,
        paid,
    })
}

/// A kept or fetched answer as the page reads it.
fn shaped(question: &Question, result: &[Value]) -> Value {
    match question.report {
        Report::Find | Report::Profile => {
            serde_json::to_value(places::profile_from_result(result)).unwrap_or(Value::Null)
        }
        Report::Competitors => {
            let (businesses, total) = places::listings_from_result(result);
            let centre = question
                .business
                .as_ref()
                .map_or((0.0, 0.0), |business| (business.lat, business.lng));
            serde_json::to_value(Competitors {
                businesses: businesses
                    .into_iter()
                    .map(|business| Around {
                        own: question
                            .business
                            .as_ref()
                            .is_some_and(|own| own.is(&business)),
                        distance_km: distance_km(centre, (business.lat, business.lng)),
                        business,
                    })
                    .collect(),
                total,
            })
            .unwrap_or(Value::Null)
        }
    }
}

fn local_estimate(context: &Context<'_>, ask: Ask) -> Result<Estimate, AppError> {
    let question = question(context.workspace, &ask)?;
    let mut estimate = context.workspace.estimate(&question.paid)?;
    if estimate.via == Some(Via::Credit)
        && !route_open(
            context.workspace,
            Via::Credit,
            "POST",
            question.paid.endpoint,
        )
    {
        estimate.blocked = Some(Blocked {
            code: ErrorCode::NoSource,
            message: not_on_credits(),
        });
    }
    Ok(estimate)
}

fn get_local(context: &Context<'_>, ask: Ask) -> Result<Option<Fetched<Value>>, AppError> {
    let question = question(context.workspace, &ask)?;
    context
        .workspace
        .kept(&question.paid, |result| shaped(&question, result))
}

fn fetch_local(context: &Context<'_>, ask: Ask) -> Result<Fetched<Value>, AppError> {
    let question = question(context.workspace, &ask)?;
    let estimate = context.workspace.estimate(&question.paid)?;
    let kept_fresh = estimate.kept.as_ref().is_some_and(|kept| kept.fresh);
    if (ask.force || !kept_fresh)
        && estimate.via == Some(Via::Credit)
        && !route_open(
            context.workspace,
            Via::Credit,
            "POST",
            question.paid.endpoint,
        )
    {
        return Err(AppError::NoSource(not_on_credits()));
    }
    let fetched = context.workspace.fetch(
        &question.paid,
        ask.force,
        |route| {
            let credentials = route.key(octopuson_dataforseo::SECRET)?;
            let client = Client::new(route.transport, &credentials)?;
            Ok(client
                .post(question.paid.endpoint, &question.paid.task)?
                .into())
        },
        |result| {
            (
                shaped(&question, result),
                places::profile_from_result(result),
            )
        },
    )?;
    // A profile read now notes the day's rating and keeps the business's figures current.
    if question.report == Report::Profile
        && !fetched.cached
        && let Some(profile) = &fetched.data.1
    {
        let store = context.workspace.store()?;
        let settings = store.local_settings(ask.project_id)?;
        if let Some(own) = business_of(&settings)
            && own.is(&profile.business)
        {
            let value = serde_json::to_value(&profile.business)
                .map_err(|error| AppError::Internal(error.to_string()))?;
            store.save_local_settings(
                ask.project_id,
                &LocalSettings {
                    business: Some(value),
                    ..settings
                },
            )?;
        }
        let profile_value =
            serde_json::to_value(profile).map_err(|error| AppError::Internal(error.to_string()))?;
        store.note_local_profile(
            ask.project_id,
            &fetched.fetched_at[..10.min(fetched.fetched_at.len())],
            profile.business.rating,
            profile.business.votes,
            Some(&profile_value),
            &fetched.fetched_at,
        )?;
    }
    Ok(Fetched {
        data: fetched.data.0,
        fetched_at: fetched.fetched_at,
        fresh: fetched.fresh,
        cached: fetched.cached,
        spent_usd: fetched.spent_usd,
        via: fetched.via,
        saved_usd: fetched.saved_usd,
        total: fetched.total,
    })
}

// ----- Grid runs ---------------------------------------------------------------------------

/// What a grid run and a month of runs cost.
#[derive(Debug, Serialize)]
struct GridEstimate {
    /// Points of the grid.
    points: u32,
    /// Maps pages of one run (keywords × points).
    checks: u32,
    /// One run started by hand, now (live pages).
    run: Estimate,
    /// Scheduled runs go through the standard queue.
    queued: bool,
    /// List price of one scheduled run with its newest reviews, US dollars.
    scheduled_usd: f64,
    runs_per_month: u32,
    month_usd: f64,
    month_credits: Option<f64>,
}

fn local_grid_estimate(context: &Context<'_>, ask: ProjectAsk) -> Result<GridEstimate, AppError> {
    let store = context.workspace.store()?;
    store.project(ask.project_id)?;
    let settings = store.local_settings(ask.project_id)?;
    drop(store);
    let points = business_of(&settings)
        .map(|business| points_of(&settings, &business))
        .transpose()?
        .map_or(0, |points| points.len());
    let checks = points * settings.keywords.len();
    let mut run = context.workspace.estimate_spend(
        octopuson_dataforseo::ID,
        places::maps_estimate(checks, false),
    )?;
    if run.via == Some(Via::Credit)
        && !route_open(context.workspace, Via::Credit, "POST", places::MAPS_LIVE)
    {
        run.blocked = Some(Blocked {
            code: ErrorCode::NoSource,
            message: not_on_credits(),
        });
    }
    let queued = run
        .via
        .is_some_and(|via| queue_open(context.workspace, via));
    let reviews = if run
        .via
        .is_some_and(|via| reviews_open(context.workspace, via))
    {
        places::reviews_estimate(SCHEDULED_REVIEWS)
    } else {
        0.0
    };
    let scheduled_usd = places::maps_estimate(checks, queued) + reviews;
    let runs_per_month = per_month(&settings.schedule);
    let month_usd = scheduled_usd * f64::from(runs_per_month);
    Ok(GridEstimate {
        points: u32::try_from(points).unwrap_or(u32::MAX),
        checks: u32::try_from(checks).unwrap_or(u32::MAX),
        month_credits: run
            .credits
            .filter(|_| run.usd > 0.0)
            .map(|credits| credits / run.usd * month_usd),
        month_usd,
        runs_per_month,
        queued,
        scheduled_usd,
        run,
    })
}

/// Starts a grid run of a project as a job, or returns the one running.
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
    let settings = store.local_settings(project.id)?;
    let business = business_of(&settings)
        .ok_or_else(|| AppError::Invalid("choose the business first".to_owned()))?;
    if settings.keywords.is_empty() {
        return Err(AppError::Invalid(
            "add keywords to check the grid with".to_owned(),
        ));
    }
    // The schedule waits while the last run's points are still in the queue.
    if scheduled && store.waiting_local_tasks(project.id, "grid")? > 0 {
        return Err(AppError::Busy(
            "the last scheduled grid is still waiting for its answers".to_owned(),
        ));
    }
    let points = points_of(&settings, &business)?;
    let total = u32::try_from(points.len() * settings.keywords.len()).unwrap_or(u32::MAX);
    let started = now();
    store.mark_local_run(project.id, &started)?;
    let run_id = store.begin_local_run(
        project.id,
        scheduled,
        total,
        settings.grid,
        settings.radius_km,
        (business.lat, business.lng),
        &started,
    )?;
    drop(store);
    let workspace = Arc::clone(workspace);
    let project_id = project.id;
    jobs.start(JOB_KIND, &project.name, Some(project.id), move |control| {
        run_grid(&workspace, project_id, run_id, scheduled, control)
    })
}

fn start_local_grid(context: &Context<'_>, ask: ProjectAsk) -> Result<String, AppError> {
    let estimate = local_grid_estimate(
        context,
        ProjectAsk {
            project_id: ask.project_id,
        },
    )?;
    if estimate.checks == 0 {
        return Err(AppError::Invalid(
            "choose the business and add keywords first".to_owned(),
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

/// How a grid run went, as its job's result.
#[derive(Debug, Default, Serialize)]
struct RunSummary {
    run_id: i64,
    checked: u32,
    failed: u32,
    /// Points posted to the standard queue, read later.
    queued: u32,
    /// Reviews tasks posted with the run.
    reviews_queued: u32,
    cost_usd: f64,
}

/// One point of one keyword of a run.
#[derive(Debug, Clone)]
struct Job {
    keyword: String,
    point: GridPoint,
}

/// A point's row before it is read.
fn point_row(run_id: i64, project_id: i64, job: &Job) -> LocalPointRow {
    LocalPointRow {
        run_id,
        project_id,
        keyword: job.keyword.clone(),
        row: job.point.row,
        col: job.point.col,
        lat: job.point.lat,
        lng: job.point.lng,
        position: None,
        top: json!([]),
        error: None,
        checked_at: now(),
    }
}

/// Fills a point's row with what its Maps page showed.
fn read_into(row: &mut LocalPointRow, result: &[Value], business: &Business) {
    let reading = places::maps_from_result(result, business);
    row.position = reading.position;
    row.top = serde_json::to_value(reading.top).unwrap_or_else(|_| json!([]));
}

#[allow(clippy::too_many_lines)] // one run with its bookkeeping
fn run_grid(
    workspace: &Workspace,
    project_id: i64,
    run_id: i64,
    scheduled: bool,
    control: &JobControl,
) -> Result<JobDone, AppError> {
    let (project, settings) = {
        let store = workspace.store()?;
        (
            store.project(project_id)?,
            store.local_settings(project_id)?,
        )
    };
    let business = business_of(&settings)
        .ok_or_else(|| AppError::Invalid("choose the business first".to_owned()))?;
    let points = points_of(&settings, &business)?;
    let jobs: Vec<Job> = settings
        .keywords
        .iter()
        .flat_map(|keyword| {
            points.iter().map(|point| Job {
                keyword: keyword.clone(),
                point: *point,
            })
        })
        .collect();
    let zoom = places::zoom_for(settings.radius_km);
    let mut summary = RunSummary {
        run_id,
        ..RunSummary::default()
    };
    let mut stopped: Option<AppError> = None;
    match workspace.paid_run(octopuson_dataforseo::ID) {
        Err(error) => stopped = Some(error),
        Ok(session) if scheduled && queue_open(workspace, session.via()) => {
            let posted = Posting {
                project: &project,
                business: &business,
                jobs: &jobs,
                zoom,
                run_id,
            };
            return post_grid(workspace, &session, &posted, control);
        }
        Ok(session) if !route_open(workspace, session.via(), "POST", places::MAPS_LIVE) => {
            stopped = Some(AppError::NoSource(not_on_credits()));
        }
        Ok(session) => {
            let total = jobs.len() as u64;
            let next = AtomicUsize::new(0);
            let halt = AtomicBool::new(false);
            let (send, answers) =
                mpsc::channel::<(usize, Result<(Vec<Value>, f64, String), AppError>)>();
            std::thread::scope(|scope| -> Result<(), AppError> {
                for _ in 0..PARALLEL.min(jobs.len()) {
                    let send = send.clone();
                    let (session, jobs, next, halt) = (&session, &jobs, &next, &halt);
                    let language = project.language.as_str();
                    scope.spawn(move || {
                        loop {
                            let index = next.fetch_add(1, Ordering::SeqCst);
                            if index >= jobs.len() || halt.load(Ordering::SeqCst) {
                                break;
                            }
                            let job = &jobs[index];
                            let outcome =
                                places::maps_task(&job.keyword, &job.point, zoom, language)
                                    .map_err(AppError::from)
                                    .and_then(|task| {
                                        let paid = Paid {
                                            travels: true,
                                            module: MODULE,
                                            provider: octopuson_dataforseo::ID,
                                            endpoint: places::MAPS_LIVE,
                                            class: CacheClass::Serp,
                                            project_id: Some(project_id),
                                            task,
                                            estimate_usd: places::maps_estimate(1, false),
                                        };
                                        session.ask(&paid, |route| {
                                            let credentials =
                                                route.key(octopuson_dataforseo::SECRET)?;
                                            let client =
                                                Client::new(route.transport, &credentials)?;
                                            Ok(client.post(paid.endpoint, &paid.task)?.into())
                                        })
                                    })
                                    .map(|answered| {
                                        (answered.result, answered.spent_usd, answered.fetched_at)
                                    });
                            if send.send((index, outcome)).is_err() {
                                break;
                            }
                        }
                    });
                }
                drop(send);
                let mut done = 0_u64;
                for (index, outcome) in answers {
                    let job = &jobs[index];
                    let mut row = point_row(run_id, project_id, job);
                    match outcome {
                        Ok((result, spent, at)) => {
                            read_into(&mut row, &result, &business);
                            row.checked_at = at;
                            summary.checked += 1;
                            summary.cost_usd += spent;
                            control.spent(spent);
                        }
                        Err(error) => {
                            row.error = Some(error.to_string());
                            summary.failed += 1;
                            if stops(&error) && stopped.is_none() {
                                stopped = Some(error);
                                halt.store(true, Ordering::SeqCst);
                            }
                        }
                    }
                    workspace.store()?.add_local_points(&[row])?;
                    done += 1;
                    control.progress(&job.keyword, done, total);
                    if control.cancelled() {
                        halt.store(true, Ordering::SeqCst);
                    }
                }
                Ok(())
            })?;
        }
    }
    let cancelled = control.cancelled();
    let status = end_status(
        cancelled,
        summary.checked,
        summary.failed,
        stopped.is_some(),
    );
    let message = stopped.as_ref().map(ToString::to_string);
    workspace.store()?.finish_local_run(
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

/// A grid run whose points go through the standard queue.
struct Posting<'a> {
    project: &'a ProjectRow,
    business: &'a Business,
    jobs: &'a [Job],
    zoom: u8,
    run_id: i64,
}

/// The tag a queued point is posted with: its place in the run's list of points.
fn grid_tag(index: usize) -> String {
    format!("grid|{index}")
}

/// Posts a run's points to the standard queue, a hundred a request, and the business's newest
/// reviews with them; the run keeps running until [`collect_queued`] has read every point.
#[allow(clippy::too_many_lines)] // one loop with its bookkeeping
fn post_grid(
    workspace: &Workspace,
    session: &PaidRun<'_>,
    run: &Posting<'_>,
    control: &JobControl,
) -> Result<JobDone, AppError> {
    let project_id = run.project.id;
    let via = via_name(session.via());
    let total = run.jobs.len() as u64;
    let mut summary = RunSummary {
        run_id: run.run_id,
        ..RunSummary::default()
    };
    let mut stopped: Option<AppError> = None;
    let fail = |job: &Job, error: &str| -> Result<(), AppError> {
        let mut row = point_row(run.run_id, project_id, job);
        row.error = Some(error.to_owned());
        Ok(workspace.store()?.add_local_points(&[row])?)
    };
    let indexed: Vec<(usize, &Job)> = run.jobs.iter().enumerate().collect();
    let mut done = 0_u64;
    for batch in indexed.chunks(queue::QUEUE_BATCH) {
        if control.cancelled() {
            break;
        }
        control.progress(&batch[0].1.keyword, done, total);
        let mut tasks = Vec::with_capacity(batch.len());
        let mut sent: Vec<(usize, &Job)> = Vec::with_capacity(batch.len());
        for (index, job) in batch {
            match places::queued_maps_task(
                &job.keyword,
                &job.point,
                run.zoom,
                &run.project.language,
                &grid_tag(*index),
            ) {
                Ok(task) => {
                    tasks.push(task);
                    sent.push((*index, job));
                }
                Err(error) => {
                    fail(job, &AppError::from(error).to_string())?;
                    summary.failed += 1;
                }
            }
        }
        done += batch.len() as u64;
        if tasks.is_empty() {
            continue;
        }
        let paid = Paid {
            travels: true,
            module: MODULE,
            provider: octopuson_dataforseo::ID,
            endpoint: places::MAPS_POST,
            class: CacheClass::Serp,
            project_id: Some(project_id),
            task: Value::Null,
            estimate_usd: places::maps_estimate(tasks.len(), true),
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
                let posted: HashMap<String, queue::Posted> =
                    queue::posted_from_tasks(&answered.result)
                        .into_iter()
                        .map(|posted| (posted.tag.clone(), posted))
                        .collect();
                let first_try = unix_now() + FIRST_WAIT;
                let mut waiting = Vec::with_capacity(sent.len());
                for (index, job) in sent {
                    match posted.get(&grid_tag(index)) {
                        Some(queue::Posted { id: Some(id), .. }) => waiting.push(LocalTaskRow {
                            id: 0,
                            project_id,
                            kind: "grid".to_owned(),
                            run_id: Some(run.run_id),
                            keyword: Some(job.keyword.clone()),
                            row: Some(job.point.row),
                            col: Some(job.point.col),
                            lat: Some(job.point.lat),
                            lng: Some(job.point.lng),
                            task_id: id.clone(),
                            via: via.to_owned(),
                            posted_at: answered.fetched_at.clone(),
                            tries: 0,
                            next_try_at: first_try,
                        }),
                        other => {
                            let error = other
                                .and_then(|posted| posted.error.clone())
                                .unwrap_or_else(|| "the queue did not take it".to_owned());
                            fail(job, &format!("DataForSEO: {error}"))?;
                            summary.failed += 1;
                        }
                    }
                }
                summary.queued += u32::try_from(waiting.len()).unwrap_or(u32::MAX);
                workspace.store()?.queue_local_tasks(&waiting)?;
            }
            Err(error) => {
                for (_, job) in &sent {
                    fail(job, &error.to_string())?;
                }
                summary.failed += u32::try_from(sent.len()).unwrap_or(u32::MAX);
                if stops(&error) {
                    stopped = Some(error);
                    break;
                }
            }
        }
    }
    // The newest reviews with the run, so the rating's trend fills in.
    if stopped.is_none() && reviews_open(workspace, session.via()) {
        match post_reviews(
            workspace,
            session,
            run.project,
            run.business,
            SCHEDULED_REVIEWS,
        ) {
            Ok(spent) => {
                summary.reviews_queued += 1;
                summary.cost_usd += spent;
                control.spent(spent);
            }
            Err(error) => {
                tracing::warn!(project = project_id, %error, "reviews were not asked for");
            }
        }
    }
    control.progress("", total, total);
    let message = stopped.as_ref().map(ToString::to_string);
    if summary.queued == 0 {
        let status = end_status(control.cancelled(), 0, summary.failed, stopped.is_some());
        workspace.store()?.finish_local_run(
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
            .queue_local_run(run.run_id, summary.failed, summary.cost_usd)?;
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

/// Posts a task for the business's newest `depth` reviews; returns what it cost.
fn post_reviews(
    workspace: &Workspace,
    session: &PaidRun<'_>,
    project: &ProjectRow,
    business: &Business,
    depth: u32,
) -> Result<f64, AppError> {
    let market = Market::new(&project.country, &project.language);
    let task = places::reviews_task(business, &market, depth, "reviews")?;
    let paid = Paid {
        travels: true,
        module: MODULE,
        provider: octopuson_dataforseo::ID,
        endpoint: places::REVIEWS_POST,
        class: CacheClass::Domain,
        project_id: Some(project.id),
        task: Value::Null,
        estimate_usd: places::reviews_estimate(depth),
    };
    let answered = session.ask(&paid, |route| {
        let credentials = route.key(octopuson_dataforseo::SECRET)?;
        let client = Client::new(route.transport, &credentials)?;
        Ok(client
            .post_tasks(paid.endpoint, std::slice::from_ref(&task))?
            .into())
    })?;
    let posted = queue::posted_from_tasks(&answered.result);
    let Some(id) = posted.first().and_then(|posted| posted.id.clone()) else {
        let error = posted
            .first()
            .and_then(|posted| posted.error.clone())
            .unwrap_or_else(|| "the queue did not take it".to_owned());
        return Err(AppError::Provider(format!("DataForSEO: {error}")));
    };
    workspace.store()?.queue_local_tasks(&[LocalTaskRow {
        id: 0,
        project_id: project.id,
        kind: "reviews".to_owned(),
        run_id: None,
        keyword: None,
        row: None,
        col: None,
        lat: None,
        lng: None,
        task_id: id,
        via: via_name(session.via()).to_owned(),
        posted_at: answered.fetched_at,
        tries: 0,
        next_try_at: unix_now() + FIRST_WAIT,
    }])?;
    Ok(answered.spent_usd)
}

fn local_runs(context: &Context<'_>, ask: ProjectAsk) -> Result<Vec<LocalRunRow>, AppError> {
    Ok(context
        .workspace
        .store()?
        .local_runs(ask.project_id, RUNS_SHOWN)?)
}

/// Starts the grid runs whose schedule is due; returns the jobs started (or already running).
/// A project that cannot start is skipped and logged, so the others still run.
///
/// # Errors
/// Storage errors.
pub fn run_due(workspace: &Arc<Workspace>, jobs: &JobManager) -> Result<Vec<String>, AppError> {
    let now_secs = unix_now();
    let store = workspace.store()?;
    let mut projects = Vec::new();
    for (project_id, settings) in store.local_plans()? {
        if due_at(&settings).is_some_and(|at| at <= now_secs) {
            projects.push(store.project(project_id)?);
        }
    }
    drop(store);
    let mut started = Vec::new();
    for project in &projects {
        match start(workspace, jobs, project, true) {
            Ok(id) => started.push(id),
            Err(error) => {
                tracing::warn!(project = project.id, %error, "a scheduled grid did not start");
            }
        }
    }
    Ok(started)
}

/// When the next scheduled grid run is due, seconds since the Unix epoch.
///
/// # Errors
/// Storage errors.
pub fn next_due(workspace: &Workspace) -> Result<Option<i64>, AppError> {
    Ok(workspace
        .store()?
        .local_plans()?
        .iter()
        .filter_map(|(_, settings)| due_at(settings))
        .min())
}

// ----- Reviews -----------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct ReviewsAsk {
    project_id: i64,
    /// Newest reviews to read (10–500).
    #[serde(default)]
    depth: Option<u32>,
}

/// What reading reviews would cost, and whether they can be read.
fn local_reviews_estimate(context: &Context<'_>, ask: ReviewsAsk) -> Result<Estimate, AppError> {
    let depth = ask.depth.unwrap_or(DEFAULT_REVIEWS);
    let mut estimate = context
        .workspace
        .estimate_spend(octopuson_dataforseo::ID, places::reviews_estimate(depth))?;
    if let Some(via) = estimate.via
        && !reviews_open(context.workspace, via)
    {
        estimate.blocked = Some(Blocked {
            code: ErrorCode::NoSource,
            message: not_on_credits(),
        });
    }
    Ok(estimate)
}

#[derive(Debug, Serialize)]
struct ReviewsAsked {
    /// Reviews tasks waiting now.
    waiting: u32,
    spent_usd: f64,
}

/// Asks for the business's newest reviews through the queue; their answer arrives within
/// minutes. A request still waiting is not asked again.
fn fetch_local_reviews(context: &Context<'_>, ask: ReviewsAsk) -> Result<ReviewsAsked, AppError> {
    let store = context.workspace.store()?;
    let project = store.project(ask.project_id)?;
    let settings = store.local_settings(project.id)?;
    let waiting = store.waiting_local_tasks(project.id, "reviews")?;
    drop(store);
    let business = business_of(&settings)
        .ok_or_else(|| AppError::Invalid("choose the business first".to_owned()))?;
    if waiting > 0 {
        return Ok(ReviewsAsked {
            waiting,
            spent_usd: 0.0,
        });
    }
    let session = context.workspace.paid_run(octopuson_dataforseo::ID)?;
    if !reviews_open(context.workspace, session.via()) {
        return Err(AppError::NoSource(not_on_credits()));
    }
    let spent = post_reviews(
        context.workspace,
        &session,
        &project,
        &business,
        ask.depth.unwrap_or(DEFAULT_REVIEWS),
    )?;
    Ok(ReviewsAsked {
        waiting: 1,
        spent_usd: spent,
    })
}

/// The reviews' figures.
#[derive(Debug, Default, Serialize)]
struct ReviewSummary {
    count: u32,
    average: Option<f64>,
    /// Without the business's answer.
    unanswered: u32,
    /// Of those, rated 1–3 stars.
    unanswered_negative: u32,
    /// Published in the last thirty days.
    recent: u32,
    /// Reviews by stars, 1 to 5.
    stars: [u32; 5],
}

/// The business's rating on a day.
#[derive(Debug, Serialize)]
struct RatingDay {
    day: String,
    rating: Option<f64>,
    votes: Option<u64>,
}

#[derive(Debug, Serialize)]
struct ReviewsView {
    reviews: Vec<LocalReviewRow>,
    summary: ReviewSummary,
    ratings: Vec<RatingDay>,
    /// Reviews tasks waiting in the queue.
    waiting: u32,
}

#[allow(clippy::cast_precision_loss)] // review counts
fn local_reviews(context: &Context<'_>, ask: ProjectAsk) -> Result<ReviewsView, AppError> {
    let store = context.workspace.store()?;
    store.project(ask.project_id)?;
    let reviews = store.local_reviews(ask.project_id, REVIEWS_SHOWN)?;
    let ratings = store
        .local_profiles(ask.project_id)?
        .into_iter()
        .map(|day| RatingDay {
            day: day.day,
            rating: day.rating,
            votes: day.votes,
        })
        .collect();
    let waiting = store.waiting_local_tasks(ask.project_id, "reviews")?;
    drop(store);
    let month_ago = rfc3339(unix_now() - 30 * 86_400);
    let mut summary = ReviewSummary::default();
    let mut stars_total = 0_u32;
    let mut rated = 0_u32;
    for review in &reviews {
        summary.count += 1;
        if let Some(stars) = review.rating {
            stars_total += u32::from(stars);
            rated += 1;
            summary.stars[usize::from(stars.clamp(1, 5) - 1)] += 1;
        }
        if review.owner_answer.is_none() {
            summary.unanswered += 1;
            if review.rating.is_some_and(|stars| stars <= 3) {
                summary.unanswered_negative += 1;
            }
        }
        if review
            .published_at
            .as_deref()
            .is_some_and(|at| at >= month_ago.as_str())
        {
            summary.recent += 1;
        }
    }
    summary.average =
        (rated > 0).then(|| (f64::from(stars_total) / f64::from(rated) * 10.0).round() / 10.0);
    Ok(ReviewsView {
        reviews,
        summary,
        ratings,
        waiting,
    })
}

// ----- The grid report ---------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct GridAsk {
    project_id: i64,
    /// The latest run with points when empty.
    #[serde(default)]
    run_id: Option<i64>,
}

/// A point as the map draws it.
#[derive(Debug, Serialize)]
struct GridCell {
    row: u32,
    col: u32,
    lat: f64,
    lng: f64,
    position: Option<u16>,
    top: Value,
    error: Option<String>,
}

/// One keyword's grid.
#[derive(Debug, Serialize)]
struct KeywordGrid {
    keyword: String,
    summary: GridSummary,
    cells: Vec<GridCell>,
}

/// A business showing among the first three on the grid's points.
#[derive(Debug, Serialize)]
struct Rival {
    title: String,
    cid: Option<String>,
    /// Points (over every keyword) where it is in the first three.
    top3: u32,
    /// Its average place there.
    average: f64,
    rating: Option<f64>,
    votes: Option<u64>,
}

/// A run's figures for one keyword, for the trend.
#[derive(Debug, Serialize)]
struct TrendPoint {
    run_id: i64,
    started_at: String,
    keyword: String,
    arp: Option<f64>,
    atrp: Option<f64>,
    solv: f64,
}

#[derive(Debug, Serialize)]
struct LocalGrid {
    run: Option<LocalRunRow>,
    keywords: Vec<KeywordGrid>,
    /// Every keyword's points together.
    summary: GridSummary,
    rivals: Vec<Rival>,
    trend: Vec<TrendPoint>,
    runs: Vec<LocalRunRow>,
}

/// Groups a run's points by keyword, in the order of the settings' keywords then any others.
fn by_keyword(points: Vec<LocalPointRow>, order: &[String]) -> Vec<(String, Vec<LocalPointRow>)> {
    let mut groups: BTreeMap<String, Vec<LocalPointRow>> = BTreeMap::new();
    for point in points {
        groups.entry(point.keyword.clone()).or_default().push(point);
    }
    let mut ordered: Vec<(String, Vec<LocalPointRow>)> = Vec::new();
    for keyword in order {
        if let Some(points) = groups.remove(keyword) {
            ordered.push((keyword.clone(), points));
        }
    }
    ordered.extend(groups);
    ordered
}

fn positions(points: &[LocalPointRow]) -> Vec<Option<u16>> {
    points
        .iter()
        .filter(|point| point.error.is_none())
        .map(|point| point.position)
        .collect()
}

#[allow(clippy::cast_precision_loss, clippy::too_many_lines)] // point counts; one report
fn local_grid(context: &Context<'_>, ask: GridAsk) -> Result<LocalGrid, AppError> {
    let store = context.workspace.store()?;
    store.project(ask.project_id)?;
    let settings = store.local_settings(ask.project_id)?;
    let runs = store.local_runs(ask.project_id, RUNS_SHOWN)?;
    let run = match ask.run_id {
        Some(id) => Some(
            runs.iter()
                .find(|run| run.id == id)
                .cloned()
                .ok_or_else(|| AppError::NotFound(format!("grid run {id}")))?,
        ),
        None => runs
            .iter()
            .find(|run| run.status != "running" && run.checked > 0)
            .or_else(|| runs.first())
            .cloned(),
    };
    let points = match &run {
        Some(run) => store.local_points(run.id)?,
        None => Vec::new(),
    };
    let mut trend = Vec::new();
    for past in runs.iter().filter(|run| run.status != "running").rev() {
        for (keyword, points) in by_keyword(store.local_points(past.id)?, &settings.keywords) {
            let figures = places::summarize(&positions(&points));
            trend.push(TrendPoint {
                run_id: past.id,
                started_at: past.started_at.clone(),
                keyword,
                arp: figures.arp,
                atrp: figures.atrp,
                solv: figures.solv,
            });
        }
    }
    drop(store);
    let own = business_of(&settings);
    let mut every: Vec<Option<u16>> = Vec::new();
    let mut rivals: HashMap<String, (Rival, f64)> = HashMap::new();
    let keywords: Vec<KeywordGrid> = by_keyword(points, &settings.keywords)
        .into_iter()
        .map(|(keyword, points)| {
            let read = positions(&points);
            every.extend(&read);
            for point in &points {
                let top: Vec<places::MapPlace> =
                    serde_json::from_value(point.top.clone()).unwrap_or_default();
                for place in top {
                    let business = Business {
                        title: place.title.clone(),
                        cid: place.cid.clone(),
                        ..Business::default()
                    };
                    if own.as_ref().is_some_and(|own| own.is(&business)) {
                        continue;
                    }
                    let key = place.cid.clone().unwrap_or_else(|| place.title.clone());
                    let entry = rivals.entry(key).or_insert_with(|| {
                        (
                            Rival {
                                title: place.title.clone(),
                                cid: place.cid.clone(),
                                top3: 0,
                                average: 0.0,
                                rating: place.rating,
                                votes: place.votes,
                            },
                            0.0,
                        )
                    });
                    entry.0.top3 += 1;
                    entry.1 += f64::from(place.position);
                }
            }
            KeywordGrid {
                summary: places::summarize(&read),
                cells: points
                    .into_iter()
                    .map(|point| GridCell {
                        row: point.row,
                        col: point.col,
                        lat: point.lat,
                        lng: point.lng,
                        position: point.position,
                        top: point.top,
                        error: point.error,
                    })
                    .collect(),
                keyword,
            }
        })
        .collect();
    let mut rivals: Vec<Rival> = rivals
        .into_values()
        .map(|(mut rival, sum)| {
            rival.average = (sum / f64::from(rival.top3.max(1)) * 10.0).round() / 10.0;
            rival
        })
        .collect();
    rivals.sort_by(|a, b| {
        b.top3
            .cmp(&a.top3)
            .then_with(|| a.average.total_cmp(&b.average))
            .then_with(|| a.title.cmp(&b.title))
    });
    rivals.truncate(RIVALS_SHOWN);
    Ok(LocalGrid {
        summary: places::summarize(&every),
        run,
        keywords,
        rivals,
        trend,
        runs,
    })
}

// ----- Reading the queue -------------------------------------------------------------------

/// Reads the answers of queued grid points and reviews that are due, keeps them, and finishes
/// the runs with nothing left in the queue. Hosts call it every minute, as for the rank checks.
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
    let due = workspace
        .store()?
        .due_local_tasks(now_secs, COLLECT_LIMIT)?;
    let mut collected = Collected::default();
    if due.is_empty() {
        return Ok(collected);
    }
    let session = workspace.paid_run(octopuson_dataforseo::ID);
    let mut businesses: HashMap<i64, Option<Business>> = HashMap::new();
    let mut runs: BTreeSet<i64> = BTreeSet::new();
    for task in due {
        if started.elapsed() > COLLECT_TIME {
            break;
        }
        if let Some(run_id) = task.run_id {
            runs.insert(run_id);
        }
        if let Entry::Vacant(slot) = businesses.entry(task.project_id) {
            slot.insert(
                workspace
                    .store()?
                    .local_settings(task.project_id)
                    .ok()
                    .as_ref()
                    .and_then(business_of),
            );
        }
        // The project or its business went meanwhile: nothing to keep.
        let Some(business) = businesses.get(&task.project_id).cloned().flatten() else {
            workspace.store()?.drop_local_task(task.id)?;
            continue;
        };
        let path = if task.kind == "reviews" {
            places::reviews_get(&task.task_id)
        } else {
            places::maps_get(&task.task_id)
        };
        let answer: Result<Option<Vec<Value>>, AppError> = match &session {
            Ok(session) if via_name(session.via()) == task.via => {
                session.free(octopuson_dataforseo::ID, |route| {
                    let credentials = route.key(octopuson_dataforseo::SECRET)?;
                    let client = Client::new(route.transport, &credentials)?;
                    match client.get(&path) {
                        Ok(answer) => Ok(Some(answer.result)),
                        Err(DfsError::NotReady) => Ok(None),
                        Err(error) => Err(error.into()),
                    }
                })
            }
            Ok(_) => Err(AppError::NoSource(
                "DataForSEO is paid another way than when the task was posted".to_owned(),
            )),
            Err(error) => Err(AppError::NoSource(error.to_string())),
        };
        let expired = seconds_of(&task.posted_at).is_some_and(|posted| now_secs - posted > GIVE_UP);
        let failure = match answer {
            Ok(Some(result)) => {
                let store = workspace.store()?;
                if task.kind == "reviews" {
                    let page = places::reviews_from_result(&result);
                    let at = now();
                    let rows: Vec<LocalReviewRow> = page
                        .reviews
                        .into_iter()
                        .map(|review| LocalReviewRow {
                            review_id: review.review_id,
                            rating: review.rating,
                            text: review.text,
                            author: review.author,
                            author_url: review.author_url,
                            url: review.url,
                            published_at: review.published_at,
                            owner_answer: review.owner_answer,
                            answered_at: review.answered_at,
                            photos: review.photos,
                            local_guide: review.local_guide,
                            first_seen: at.clone(),
                        })
                        .collect();
                    store.save_local_reviews(task.project_id, &rows, &at)?;
                    store.note_local_profile(
                        task.project_id,
                        &at[..10],
                        page.rating,
                        page.votes,
                        None,
                        &at,
                    )?;
                } else {
                    let mut row = grid_row_of(&task);
                    read_into(&mut row, &result, &business);
                    store.add_local_points(&[row])?;
                }
                store.drop_local_task(task.id)?;
                collected.read += 1;
                continue;
            }
            Err(error) if is_final(&error) || expired => error.to_string(),
            Ok(None) if expired => "DataForSEO: the queue did not answer within a day".to_owned(),
            Ok(None) | Err(_) => {
                workspace
                    .store()?
                    .delay_local_task(task.id, now_secs + wait_after(task.tries))?;
                collected.waiting += 1;
                continue;
            }
        };
        let store = workspace.store()?;
        if task.kind == "reviews" {
            tracing::warn!(project = task.project_id, error = %failure, "reviews were not read");
        } else {
            let mut row = grid_row_of(&task);
            row.error = Some(failure);
            store.add_local_points(&[row])?;
        }
        store.drop_local_task(task.id)?;
        collected.failed += 1;
    }
    for run_id in runs {
        let store = workspace.store()?;
        let Some(run) = store.local_run(run_id)? else {
            continue;
        };
        if run.status != "running" || run.waiting > 0 {
            continue;
        }
        let (checked, failed) = store.local_run_tally(run_id)?;
        store.finish_local_run(
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

/// A queued point's row before it is read.
fn grid_row_of(task: &LocalTaskRow) -> LocalPointRow {
    LocalPointRow {
        run_id: task.run_id.unwrap_or_default(),
        project_id: task.project_id,
        keyword: task.keyword.clone().unwrap_or_default(),
        row: task.row.unwrap_or_default(),
        col: task.col.unwrap_or_default(),
        lat: task.lat.unwrap_or_default(),
        lng: task.lng.unwrap_or_default(),
        position: None,
        top: json!([]),
        error: None,
        checked_at: now(),
    }
}

/// When the queue should be read next for local tasks, seconds since the Unix epoch.
///
/// # Errors
/// Storage errors.
pub fn next_collect(workspace: &Workspace, jobs: &JobManager) -> Result<Option<i64>, AppError> {
    let waiting = workspace.store()?.next_local_task_at()?;
    let posting = jobs
        .list()
        .iter()
        .any(|job| job.kind == JOB_KIND && job.status == JobStatus::Running);
    let soon = unix_now() + super::rank::COLLECT_EVERY;
    Ok(if posting {
        Some(waiting.map_or(soon, |at| at.min(soon)))
    } else {
        waiting
    })
}

/// Adds the commands.
pub fn register(router: &mut Router) {
    router.add("local_setup", local_setup);
    router.add("save_local_settings", save_local_settings);
    router.add("choose_business", choose_business);
    router.add("local_estimate", local_estimate);
    router.add("get_local", get_local);
    router.add("fetch_local", fetch_local);
    router.add("local_grid_estimate", local_grid_estimate);
    router.add("start_local_grid", start_local_grid);
    router.add("local_runs", local_runs);
    router.add("local_grid", local_grid);
    router.add("local_reviews_estimate", local_reviews_estimate);
    router.add("fetch_local_reviews", fetch_local_reviews);
    router.add("local_reviews", local_reviews);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distances_are_in_kilometres() {
        // Kadıköy to Beşiktaş along the Asian shore and across the water: about 6 km.
        let km = distance_km((40.9901, 29.0291), (41.0422, 29.0083));
        assert!((km - 6.07).abs() < 0.2, "{km}");
        assert_eq!(distance_km((41.0, 29.0), (41.0, 29.0)), 0.0);
    }

    #[test]
    fn schedules_run_weekly_or_monthly_once_set_up() {
        let mut settings = LocalSettings {
            schedule: "weekly".to_owned(),
            ..LocalSettings::default()
        };
        assert_eq!(due_at(&settings), None, "no business or keywords yet");
        settings.business = Some(json!({ "title": "Örnek", "lat": 41.0, "lng": 29.0 }));
        settings.keywords = vec!["kahve".to_owned()];
        assert_eq!(due_at(&settings), Some(0), "never run: due now");
        settings.last_run_at = Some("2026-10-01T00:00:00Z".to_owned());
        let last = seconds_of("2026-10-01T00:00:00Z").unwrap();
        assert_eq!(due_at(&settings), Some(last + 7 * 86_400));
        settings.schedule = "manual".to_owned();
        assert_eq!(due_at(&settings), None);
        assert_eq!(per_month("monthly"), 1);
    }
}
