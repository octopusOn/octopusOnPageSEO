//! Search Console and Google Analytics: connecting Google, choosing a project's properties,
//! reading their numbers (free, as a job, on a schedule) and the report that puts them beside
//! the rank tracker's keywords and the latest site audit.
//!
//! A sync reads the chosen period (ending three days ago, Google's final data) and the period
//! it is compared with (as many days before it, or the same days a year before): clicks, impressions and positions by day, query, page, query on page,
//! device and country from Search Console; organic search sessions, engaged sessions, key
//! events and revenue by day and landing page from Analytics, when a property is chosen. It
//! replaces the project's numbers; Google keeps the history.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use octopuson_core::table::{Cell, TableBuffer};
use octopuson_core::{Date, Timestamp};
use octopuson_google::GoogleError;
use octopuson_google::analytics::{self, Breakdown, Organic};
use octopuson_google::search_console::{self, Dimension, Search};
use octopuson_platform::{
    GscData, GscSettings, GscSyncEnd, GscSyncRow, Period, ProjectRow, QueryPage, SearchKind,
    SearchRow, TrafficKind, TrafficRow, unix_now,
};
use octopuson_serp_score::rank::{ctr, seconds_of};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::google::google_error;
use crate::jobs::{JobControl, JobDone, JobManager, JobStatus};
use crate::router::{Context, NoArgs, Router};
use crate::{AppError, GoogleStatus, Host, Workspace, now};

/// Kind of the job of a sync.
pub const JOB_KIND: &str = "search-console";

/// Kind of the job of a desktop sign-in.
const LOGIN_KIND: &str = "google-login";

/// Days Search Console's final data lags behind.
const LAG_DAYS: i32 = 3;

/// Queries and pages read per period.
const TOP_ROWS: u32 = 10_000;

/// Queries on pages read.
const QUERY_PAGES: u32 = 25_000;

/// Countries read.
const COUNTRIES: u32 = 250;

/// Rows a report lists (every row is in `gsc_export`).
const REPORT_ROWS: u32 = 5_000;

/// Syncs listed.
const SYNCS_SHOWN: u32 = 10;

/// Queries split over pages listed.
const CANNIBALISED: usize = 50;

/// Pages of the report looked up in the latest audit.
const AUDITED_PAGES: usize = 300;

/// Commands after which a sync may be due at another time.
const SCHEDULE_COMMANDS: &[&str] = &["save_gsc_settings", "start_gsc_sync", "delete_project"];

/// Whether a command may move when a sync is due next.
pub fn changes_schedule(command: &str) -> bool {
    SCHEDULE_COMMANDS.contains(&command)
}

fn rfc3339(seconds: i64) -> String {
    Timestamp::from_unix_micros(seconds.saturating_mul(1_000_000)).to_string()
}

/// When a project's sync is due, seconds since the Unix epoch.
fn due_at(settings: &GscSettings) -> Option<i64> {
    let interval = match settings.schedule.as_str() {
        "daily" => 20 * 3_600,
        "weekly" => 6 * 86_400 + 12 * 3_600,
        _ => return None,
    };
    settings.property.as_ref()?;
    Some(
        settings
            .last_sync_at
            .as_deref()
            .and_then(seconds_of)
            .map_or(0, |last| last + interval),
    )
}

/// A day `back` days before today (UTC), `YYYY-MM-DD`.
fn day(back: i32) -> String {
    let today = Date::of(Timestamp::now()).days_since_epoch();
    Date::from_days_since_epoch(today - back).map_or_else(String::new, |date| date.to_string())
}

/// Days in a year, for comparing with the same days a year before.
const YEAR_DAYS: i32 = 364;

/// The sync's days and the days they are compared with: as many days before them, or (with
/// `year`) the same weekdays a year before.
fn periods(days: u32, compare: &str) -> ((String, String), (String, String)) {
    let days = i32::try_from(days).unwrap_or(28);
    let end = LAG_DAYS;
    let start = end + days - 1;
    let before = if compare == "year" {
        (day(start + YEAR_DAYS), day(end + YEAR_DAYS))
    } else {
        (day(start + days), day(start + 1))
    };
    ((day(start), day(end)), before)
}

// ----- Google ------------------------------------------------------------------------------

fn google_status(context: &Context<'_>, _: NoArgs) -> Result<GoogleStatus, AppError> {
    context.workspace.google_status()
}

#[derive(Debug, Deserialize)]
struct ClientAsk {
    client_id: String,
    #[serde(default)]
    client_secret: String,
}

fn set_google_client(context: &Context<'_>, ask: ClientAsk) -> Result<GoogleStatus, AppError> {
    context
        .workspace
        .set_google_client(&ask.client_id, &ask.client_secret)
}

#[derive(Debug, Default, Deserialize)]
struct LoginAsk {
    #[serde(default)]
    analytics: bool,
}

/// How a sign-in goes on: a job on the desktop, the consent page to open on the web.
#[derive(Debug, Serialize)]
struct GoogleLogin {
    job_id: Option<String>,
    url: Option<String>,
}

fn start_google_login(context: &Context<'_>, ask: LoginAsk) -> Result<GoogleLogin, AppError> {
    let workspace = context.workspace;
    if workspace.host() == Host::Web {
        return Ok(GoogleLogin {
            job_id: None,
            url: Some(workspace.begin_google_login(ask.analytics)?),
        });
    }
    if !workspace.google_status()?.configured {
        return Err(AppError::NoSource(
            "enter the client id and secret of your Google OAuth client (Desktop app type) first"
                .to_owned(),
        ));
    }
    let workspace = Arc::clone(workspace);
    let open = Arc::clone(context.open);
    let id = context
        .jobs
        .start(LOGIN_KIND, "Google", None, move |control| {
            control.progress("browser", 0, 0);
            let status = workspace
                .connect_google(ask.analytics, &|url| open(url), &|| control.cancelled())?;
            Ok(JobDone::with(status))
        })?;
    Ok(GoogleLogin {
        job_id: Some(id),
        url: None,
    })
}

fn google_logout(context: &Context<'_>, _: NoArgs) -> Result<GoogleStatus, AppError> {
    context.workspace.google_logout()
}

/// Asks Google with a valid token, renewing it once when Google refuses it.
fn with_token<T>(
    workspace: &Workspace,
    ask: impl Fn(&dyn octopuson_core::http::HttpTransport, &str) -> Result<T, GoogleError>,
) -> Result<T, GoogleError> {
    let transport = workspace
        .free_transport()
        .map_err(|error| GoogleError::Unavailable(error.to_string()))?;
    let token = workspace
        .google_token()
        .map_err(|error| GoogleError::Auth(error.to_string()))?;
    match ask(&transport, &token) {
        Err(GoogleError::Auth(_)) => {
            workspace.drop_google_token();
            let token = workspace
                .google_token()
                .map_err(|error| GoogleError::Auth(error.to_string()))?;
            ask(&transport, &token)
        }
        other => other,
    }
}

/// The token, or why there is none, before any request.
fn signed_in(workspace: &Workspace) -> Result<(), AppError> {
    workspace.google_token().map(drop)
}

#[derive(Debug, Default, Deserialize)]
struct PropertiesAsk {
    #[serde(default)]
    project_id: Option<i64>,
}

#[derive(Debug, Serialize)]
struct GoogleProperties {
    search_console: Vec<search_console::Site>,
    analytics: Vec<analytics::Property>,
    /// Why Analytics properties could not be listed (not granted, API off).
    analytics_error: Option<String>,
    /// The Search Console property that covers the project's domain.
    suggested: Option<String>,
}

fn google_properties(
    context: &Context<'_>,
    ask: PropertiesAsk,
) -> Result<GoogleProperties, AppError> {
    let workspace = context.workspace;
    signed_in(workspace)?;
    let sites = with_token(workspace, search_console::sites).map_err(google_error)?;
    let (analytics, analytics_error) = if workspace.google_status()?.analytics {
        match with_token(workspace, analytics::properties) {
            Ok(properties) => (properties, None),
            Err(error) => (Vec::new(), Some(google_error(error).to_string())),
        }
    } else {
        (Vec::new(), None)
    };
    let suggested = match ask.project_id {
        Some(id) => {
            let project = workspace.store()?.project(id)?;
            search_console::matching(&sites, &project.domain).map(|site| site.site_url.clone())
        }
        None => None,
    };
    Ok(GoogleProperties {
        search_console: sites,
        analytics,
        analytics_error,
        suggested,
    })
}

// ----- Setup -------------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct ProjectAsk {
    project_id: i64,
}

#[derive(Debug, Serialize)]
struct GscSetup {
    settings: GscSettings,
    google: GoogleStatus,
    syncs: Vec<GscSyncRow>,
    /// When the schedule syncs next, RFC 3339; `None` when only by hand.
    next_sync_at: Option<String>,
}

fn setup(workspace: &Workspace, project_id: i64) -> Result<GscSetup, AppError> {
    let (settings, syncs) = {
        let store = workspace.store()?;
        store.project(project_id)?;
        (
            store.gsc_settings(project_id)?,
            store.gsc_syncs(project_id, SYNCS_SHOWN)?,
        )
    };
    Ok(GscSetup {
        next_sync_at: due_at(&settings).map(|at| rfc3339(at.max(unix_now()))),
        google: workspace.google_status()?,
        settings,
        syncs,
    })
}

fn gsc_setup(context: &Context<'_>, ask: ProjectAsk) -> Result<GscSetup, AppError> {
    setup(context.workspace, ask.project_id)
}

#[derive(Debug, Deserialize)]
struct SettingsAsk {
    project_id: i64,
    #[serde(default)]
    property: Option<String>,
    #[serde(default)]
    ga4_property: Option<String>,
    #[serde(default)]
    ga4_name: Option<String>,
    days: u32,
    schedule: String,
    /// `previous` or `year`.
    #[serde(default = "previous_period")]
    compare: String,
}

fn previous_period() -> String {
    "previous".to_owned()
}

fn save_gsc_settings(context: &Context<'_>, ask: SettingsAsk) -> Result<GscSetup, AppError> {
    context.workspace.store()?.save_gsc_settings(
        ask.project_id,
        &GscSettings {
            property: ask.property,
            ga4_property: ask.ga4_property,
            ga4_name: ask.ga4_name,
            days: ask.days,
            schedule: ask.schedule,
            last_sync_at: None,
            compare: ask.compare,
        },
    )?;
    setup(context.workspace, ask.project_id)
}

// ----- Sync --------------------------------------------------------------------------------

/// Starts a sync of a project as a job, or returns the one running.
fn start(
    workspace: &Arc<Workspace>,
    jobs: &JobManager,
    project: &ProjectRow,
    scheduled: bool,
) -> Result<String, AppError> {
    if let Some(id) = jobs.running_for(JOB_KIND, Some(project.id)) {
        return Ok(id);
    }
    let settings = workspace.store()?.gsc_settings(project.id)?;
    if settings.property.is_none() {
        return Err(AppError::Invalid(
            "choose the project's Search Console property first".to_owned(),
        ));
    }
    signed_in(workspace)?;
    let sync_id = workspace
        .store()?
        .begin_gsc_sync(project.id, scheduled, &now())?;
    let workspace = Arc::clone(workspace);
    let project_id = project.id;
    jobs.start(JOB_KIND, &project.name, Some(project.id), move |control| {
        sync(&workspace, project_id, sync_id, &settings, control)
    })
}

fn start_gsc_sync(context: &Context<'_>, ask: ProjectAsk) -> Result<String, AppError> {
    let project = context.workspace.store()?.project(ask.project_id)?;
    start(context.workspace, context.jobs, &project, false)
}

/// The search rows of one question, split by one dimension.
fn search_rows(
    workspace: &Workspace,
    property: &str,
    (start, end): (&str, &str),
    dimension: Dimension,
    max_rows: u32,
) -> Result<Vec<SearchRow>, GoogleError> {
    let rows = with_token(workspace, |transport, token| {
        search_console::search(
            transport,
            token,
            &Search {
                site_url: property,
                start,
                end,
                dimensions: &[dimension],
                max_rows,
            },
        )
    })?;
    Ok(rows
        .into_iter()
        .filter_map(|row| {
            Some(SearchRow {
                key: row.keys.into_iter().next()?,
                clicks: row.clicks,
                impressions: row.impressions,
                position: row.position,
            })
        })
        .collect())
}

fn traffic_rows(report: Organic) -> Vec<TrafficRow> {
    report
        .rows
        .into_iter()
        .map(|row| TrafficRow {
            key: row.key,
            sessions: row.sessions,
            engaged_sessions: row.engaged_sessions,
            key_events: row.key_events,
            revenue: row.revenue,
        })
        .collect()
}

/// Where the rows split by a dimension are kept.
const fn kind_of(dimension: Dimension) -> SearchKind {
    match dimension {
        Dimension::Date => SearchKind::Date,
        Dimension::Query => SearchKind::Query,
        Dimension::Page => SearchKind::Page,
        Dimension::Country => SearchKind::Country,
        Dimension::Device => SearchKind::Device,
    }
}

/// Reads Search Console into `data`, step by step.
fn read_search(
    workspace: &Workspace,
    property: &str,
    (now, before): (&(String, String), &(String, String)),
    data: &mut GscData,
    step: &mut dyn FnMut(&str) -> bool,
) -> Result<(), GoogleError> {
    let now = (now.0.as_str(), now.1.as_str());
    let before = (before.0.as_str(), before.1.as_str());
    let mut put = |kind: SearchKind, period: Period, rows: Vec<SearchRow>| {
        data.search
            .extend(rows.into_iter().map(|row| (kind, period, row)));
    };
    let questions = [
        (Dimension::Date, Period::Days, now, 400),
        (Dimension::Date, Period::Days, before, 400),
        (Dimension::Query, Period::Now, now, TOP_ROWS),
        (Dimension::Query, Period::Before, before, TOP_ROWS),
        (Dimension::Page, Period::Now, now, TOP_ROWS),
        (Dimension::Page, Period::Before, before, TOP_ROWS),
        (Dimension::Device, Period::Now, now, 10),
        (Dimension::Country, Period::Now, now, COUNTRIES),
    ];
    for (dimension, period, range, max_rows) in questions {
        if !step(dimension.as_str()) {
            return Ok(());
        }
        let rows = search_rows(workspace, property, range, dimension, max_rows)?;
        put(kind_of(dimension), period, rows);
    }
    if !step("query-pages") {
        return Ok(());
    }
    let rows = with_token(workspace, |transport, token| {
        search_console::search(
            transport,
            token,
            &Search {
                site_url: property,
                start: now.0,
                end: now.1,
                dimensions: &[Dimension::Query, Dimension::Page],
                max_rows: QUERY_PAGES,
            },
        )
    })?;
    data.query_pages = rows
        .into_iter()
        .filter_map(|row| {
            let mut keys = row.keys.into_iter();
            Some(QueryPage {
                query: keys.next()?,
                page: keys.next()?,
                clicks: row.clicks,
                impressions: row.impressions,
                position: row.position,
            })
        })
        .collect();
    Ok(())
}

/// Reads Analytics into `data`; the property's currency and whether numbers were limited.
fn read_traffic(
    workspace: &Workspace,
    property: &str,
    (now, before): (&(String, String), &(String, String)),
    data: &mut GscData,
    step: &mut dyn FnMut(&str) -> bool,
) -> Result<(Option<String>, bool), GoogleError> {
    let questions = [
        (
            TrafficKind::Date,
            Period::Days,
            (now.0.as_str(), now.1.as_str()),
            Breakdown::Date,
            400,
        ),
        (
            TrafficKind::Date,
            Period::Days,
            (before.0.as_str(), before.1.as_str()),
            Breakdown::Date,
            400,
        ),
        (
            TrafficKind::Landing,
            Period::Now,
            (now.0.as_str(), now.1.as_str()),
            Breakdown::LandingPage,
            TOP_ROWS,
        ),
        (
            TrafficKind::Landing,
            Period::Before,
            (before.0.as_str(), before.1.as_str()),
            Breakdown::LandingPage,
            TOP_ROWS,
        ),
    ];
    let mut currency = None;
    let mut limited = false;
    for (kind, period, range, breakdown, max_rows) in questions {
        if !step("analytics") {
            break;
        }
        let report = with_token(workspace, |transport, token| {
            analytics::organic(transport, token, property, range, breakdown, max_rows)
        })?;
        limited |= report.limited;
        currency = currency.or_else(|| report.currency.clone());
        data.traffic.extend(
            traffic_rows(report)
                .into_iter()
                .map(|row| (kind, period, row)),
        );
    }
    Ok((currency, limited))
}

/// Requests of a sync, for its progress.
const STEPS: u64 = 9;
const ANALYTICS_STEPS: u64 = 4;

fn sync(
    workspace: &Workspace,
    project_id: i64,
    sync_id: i64,
    settings: &GscSettings,
    control: &JobControl,
) -> Result<JobDone, AppError> {
    let property = settings.property.clone().unwrap_or_default();
    let (now_days, before_days) = periods(settings.days, &settings.compare);
    let total = STEPS
        + if settings.ga4_property.is_some() {
            ANALYTICS_STEPS
        } else {
            0
        };
    let mut done = 0;
    let mut step = |name: &str| {
        control.progress(name, done, total);
        done += 1;
        !control.cancelled()
    };
    let mut data = GscData::default();
    let searched = read_search(
        workspace,
        &property,
        (&now_days, &before_days),
        &mut data,
        &mut step,
    );
    let mut status = "done";
    let mut error = None;
    let mut currency = None;
    let mut limited = false;
    if let Err(failure) = searched {
        let failure = google_error(failure);
        workspace.store()?.finish_gsc_sync(
            sync_id,
            &GscSyncEnd {
                status: "failed",
                period: None,
                rows: 0,
                currency: None,
                limited: false,
                error: Some(&failure.to_string()),
            },
            &now(),
        )?;
        return Err(failure);
    }
    if let Some(ga4) = settings.ga4_property.as_deref() {
        match read_traffic(
            workspace,
            ga4,
            (&now_days, &before_days),
            &mut data,
            &mut step,
        ) {
            Ok((found, cut)) => {
                currency = found;
                limited = cut;
            }
            Err(failure) => {
                status = "partial";
                error = Some(format!("Google Analytics: {}", google_error(failure)));
                data.traffic.clear();
            }
        }
    }
    let cancelled = control.cancelled();
    if cancelled {
        status = "cancelled";
    } else {
        workspace.store()?.replace_gsc_data(project_id, &data)?;
    }
    control.progress("", total, total);
    workspace.store()?.finish_gsc_sync(
        sync_id,
        &GscSyncEnd {
            status,
            period: (!cancelled).then_some((now_days.0.as_str(), now_days.1.as_str())),
            rows: data.rows(),
            currency: currency.as_deref(),
            limited,
            error: error.as_deref(),
        },
        &now(),
    )?;
    Ok(JobDone {
        status: if status == "partial" {
            JobStatus::Partial
        } else {
            JobStatus::Done
        },
        result: Some(json!({ "sync_id": sync_id, "rows": data.rows(), "status": status })),
    })
}

/// Starts the syncs whose schedule is due; returns the jobs started (or already running).
///
/// # Errors
/// Storage errors.
pub fn run_due(workspace: &Arc<Workspace>, jobs: &JobManager) -> Result<Vec<String>, AppError> {
    let now_secs = unix_now();
    let store = workspace.store()?;
    let mut projects = Vec::new();
    for (project_id, settings) in store.gsc_plans()? {
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
                tracing::warn!(project = project.id, %error, "a scheduled Search Console sync did not start");
            }
        }
    }
    Ok(started)
}

/// When the next scheduled sync is due, seconds since the Unix epoch. Without a Google
/// sign-in nothing is due.
///
/// # Errors
/// Storage errors.
pub fn next_due(workspace: &Workspace) -> Result<Option<i64>, AppError> {
    if !workspace.google_status()?.connected {
        return Ok(None);
    }
    Ok(workspace
        .store()?
        .gsc_plans()?
        .iter()
        .filter_map(|(_, settings)| due_at(settings))
        .min())
}

// ----- Report ------------------------------------------------------------------------------

/// Clicks, impressions, CTR and position of a period.
#[derive(Debug, Clone, Copy, Default, Serialize)]
struct Totals {
    clicks: f64,
    impressions: f64,
    /// 0–100.
    ctr: f64,
    /// Weighted by impressions.
    position: Option<f64>,
}

fn totals<'a>(rows: impl Iterator<Item = &'a SearchRow>) -> Totals {
    let (mut clicks, mut impressions, mut weighted) = (0.0, 0.0, 0.0);
    for row in rows {
        clicks += row.clicks;
        impressions += row.impressions;
        weighted += row.position * row.impressions;
    }
    Totals {
        clicks,
        impressions,
        ctr: if impressions > 0.0 {
            (clicks / impressions * 1000.0).round() / 10.0
        } else {
            0.0
        },
        position: (impressions > 0.0).then(|| (weighted / impressions * 10.0).round() / 10.0),
    }
}

/// Organic sessions, key events and revenue of a period.
#[derive(Debug, Clone, Copy, Default, Serialize)]
struct Traffic {
    sessions: f64,
    engaged_sessions: f64,
    key_events: f64,
    revenue: f64,
}

fn traffic<'a>(rows: impl Iterator<Item = &'a TrafficRow>) -> Traffic {
    rows.fold(Traffic::default(), |sum, row| Traffic {
        sessions: sum.sessions + row.sessions,
        engaged_sessions: sum.engaged_sessions + row.engaged_sessions,
        key_events: sum.key_events + row.key_events,
        revenue: sum.revenue + row.revenue,
    })
}

#[derive(Debug, Serialize)]
struct DayPoint {
    date: String,
    /// `now` or `before`.
    period: &'static str,
    clicks: f64,
    impressions: f64,
    position: f64,
    sessions: Option<f64>,
    key_events: Option<f64>,
}

#[derive(Debug, Serialize)]
#[allow(clippy::struct_excessive_bools)] // flags a page shows
struct QueryRow {
    query: String,
    clicks: f64,
    impressions: f64,
    ctr: f64,
    position: f64,
    /// Clicks gained or lost against the days before; `None` when it was not seen then.
    clicks_change: Option<f64>,
    /// Places gained (positive) or lost.
    position_change: Option<f64>,
    /// Pages seen for it.
    pages: u32,
    /// The rank tracker follows it.
    tracked: bool,
    /// Within reach of the first places: average position 4–20 with enough impressions.
    near_top: bool,
    /// It names the brand (the project's brand names or its domain's name).
    brand: bool,
    /// Its CTR is less than half of what its position usually gets.
    low_ctr: bool,
}

/// How the latest site audit saw a page.
#[derive(Debug, Clone, Serialize)]
struct PageAudit {
    status: Option<i64>,
    indexability: String,
    findings: u64,
}

#[derive(Debug, Serialize)]
struct PageRow {
    page: String,
    clicks: f64,
    impressions: f64,
    ctr: f64,
    position: f64,
    clicks_change: Option<f64>,
    sessions: Option<f64>,
    key_events: Option<f64>,
    revenue: Option<f64>,
    audit: Option<PageAudit>,
}

#[derive(Debug, Serialize)]
struct CannibalPage {
    page: String,
    clicks: f64,
    impressions: f64,
    position: f64,
}

/// A query several pages of the site share.
#[derive(Debug, Serialize)]
struct Cannibalised {
    query: String,
    impressions: f64,
    pages: Vec<CannibalPage>,
}

#[derive(Debug, Serialize)]
struct AuditLink {
    audit_id: i64,
    finished_at: Option<String>,
    /// Pages with clicks that the audit found broken, redirected or not indexable.
    problem_pages: u32,
}

/// Searches naming the brand and the others, in both periods (from the queries Search
/// Console lists, so a little under the totals, which include anonymised queries).
#[derive(Debug, Serialize)]
struct BrandSplit {
    /// What counts as the brand, folded.
    terms: Vec<String>,
    brand: Totals,
    other: Totals,
    brand_before: Totals,
    other_before: Totals,
}

#[derive(Debug, Serialize)]
struct GscReport {
    /// The sync the numbers come from.
    sync: GscSyncRow,
    period: (String, String),
    previous: (String, String),
    /// `previous` (the days before) or `year` (the same days a year before).
    compare: &'static str,
    brand: BrandSplit,
    now: Totals,
    before: Totals,
    traffic_now: Option<Traffic>,
    traffic_before: Option<Traffic>,
    currency: Option<String>,
    daily: Vec<DayPoint>,
    queries: Vec<QueryRow>,
    pages: Vec<PageRow>,
    devices: Vec<SearchRow>,
    countries: Vec<SearchRow>,
    cannibalised: Vec<Cannibalised>,
    audit: Option<AuditLink>,
    /// Tracked keywords Search Console has numbers for, and tracked keywords in all.
    tracked: (u32, u32),
}

fn round(value: f64) -> f64 {
    (value * 10.0).round() / 10.0
}

fn ctr_of(clicks: f64, impressions: f64) -> f64 {
    if impressions > 0.0 {
        round(clicks / impressions * 100.0)
    } else {
        0.0
    }
}

/// The day before `date` by `days` days.
fn shift(date: &str, days: i32) -> String {
    Date::parse(date)
        .and_then(|date| Date::from_days_since_epoch(date.days_since_epoch() - days))
        .map_or_else(String::new, |date| date.to_string())
}

fn fold(text: &str) -> String {
    text.trim().to_lowercase()
}

/// Text folded for brand matching: lowercase, Turkish and Latin accents dropped, without
/// spaces or punctuation (`Örnek Mağaza` → `ornekmagaza`).
fn compact(text: &str) -> String {
    text.to_lowercase()
        .chars()
        .filter_map(|ch| match ch {
            'ı' | 'î' | 'ì' | 'í' | 'ï' => Some('i'),
            'ö' | 'ô' | 'ò' | 'ó' => Some('o'),
            'ü' | 'û' | 'ù' | 'ú' => Some('u'),
            'ş' => Some('s'),
            'ç' => Some('c'),
            'ğ' => Some('g'),
            'â' | 'à' | 'á' | 'ä' => Some('a'),
            'é' | 'è' | 'ê' | 'ë' => Some('e'),
            ch if ch.is_alphanumeric() => Some(ch),
            _ => None,
        })
        .collect()
}

/// What counts as the brand: the project's brand names and its domain's name.
fn brand_terms(project: &ProjectRow) -> Vec<String> {
    let domain = project.domain.trim_start_matches("www.");
    let name = domain.split('.').next().unwrap_or(domain);
    let mut terms: Vec<String> = project
        .brands
        .iter()
        .map(String::as_str)
        .chain(std::iter::once(name))
        .map(compact)
        .filter(|term| term.chars().count() >= 3)
        .collect();
    terms.sort();
    terms.dedup();
    terms
}

fn is_brand(query: &str, terms: &[String]) -> bool {
    let query = compact(query);
    terms.iter().any(|term| query.contains(term.as_str()))
}

/// The path of an address (`/shoes/`), for Analytics' landing pages.
fn path_of(page: &str) -> String {
    url::Url::parse(page).map_or_else(|_| page.to_owned(), |url| url.path().to_owned())
}

/// How the latest audit with a crawl saw `pages`.
fn audited(
    workspace: &Workspace,
    project_id: i64,
    pages: &[&str],
) -> Option<(AuditLink, HashMap<String, PageAudit>)> {
    let audits = workspace.store().ok()?.audits(project_id, 10).ok()?;
    let audit = audits.into_iter().find(|row| {
        row.crawl_id.is_some() && !matches!(row.status.as_str(), "running" | "failed")
    })?;
    let reader = super::audit::open(workspace, &audit)?;
    let list = pages
        .iter()
        .take(AUDITED_PAGES)
        .map(|page| format!("'{}'", page.replace('\'', "''")))
        .collect::<Vec<_>>()
        .join(",");
    let mut buffer = TableBuffer::default();
    if !list.is_empty() {
        let sql = format!(
            "SELECT u.url, u.status, u.indexability,
                    (SELECT count(*) FROM audit_findings f WHERE f.url = u.url)
             FROM urls u WHERE u.url IN ({list})"
        );
        reader.query(&sql, &mut buffer).ok()?;
    }
    let mut seen = HashMap::new();
    for row in &buffer.rows {
        let text = |cell: Option<&Cell>| match cell {
            Some(Cell::Text(text)) => text.clone(),
            _ => String::new(),
        };
        let number = |cell: Option<&Cell>| match cell {
            Some(Cell::Integer(value)) => Some(*value),
            _ => None,
        };
        seen.insert(
            text(row.first()),
            PageAudit {
                status: number(row.get(1)),
                indexability: text(row.get(2)),
                findings: number(row.get(3))
                    .and_then(|n| u64::try_from(n).ok())
                    .unwrap_or_default(),
            },
        );
    }
    Some((
        AuditLink {
            audit_id: audit.id,
            finished_at: audit.finished_at.clone(),
            problem_pages: 0,
        },
        seen,
    ))
}

#[allow(
    clippy::too_many_lines,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)] // one report; places are 1–100
fn gsc_report(context: &Context<'_>, ask: ProjectAsk) -> Result<Option<GscReport>, AppError> {
    let workspace = context.workspace;
    let store = workspace.store()?;
    let project = store.project(ask.project_id)?;
    let Some(sync) = store
        .gsc_syncs(ask.project_id, SYNCS_SHOWN)?
        .into_iter()
        .find(|sync| matches!(sync.status.as_str(), "done" | "partial"))
    else {
        return Ok(None);
    };
    let id = ask.project_id;
    let (start, end) = (
        sync.period_start.clone().unwrap_or_default(),
        sync.period_end.clone().unwrap_or_default(),
    );
    let days = Date::parse(&end)
        .zip(Date::parse(&start))
        .map_or(28, |(end, start)| {
            end.days_since_epoch() - start.days_since_epoch() + 1
        });
    let day_rows = store.gsc_rows(id, SearchKind::Date, Period::Days, 1_000)?;
    // The compared days: as many days before the period, or the same days a year before,
    // whichever holds the days read before it.
    let before_days = (shift(&start, days), shift(&start, 1));
    let year_days = (shift(&start, YEAR_DAYS), shift(&end, YEAR_DAYS));
    let within = |(low, high): &(String, String)| {
        day_rows
            .iter()
            .filter(|row| row.key.as_str() >= low.as_str() && row.key.as_str() <= high.as_str())
            .count()
    };
    let (compare, previous) = if within(&year_days) > within(&before_days) {
        ("year", year_days)
    } else {
        ("previous", before_days)
    };
    let traffic_days = store.ga4_rows(id, TrafficKind::Date, Period::Days, 1_000)?;
    let mut queries_now = store.gsc_rows(id, SearchKind::Query, Period::Now, TOP_ROWS)?;
    let queries_before: HashMap<String, SearchRow> = store
        .gsc_rows(id, SearchKind::Query, Period::Before, 25_000)?
        .into_iter()
        .map(|row| (row.key.clone(), row))
        .collect();
    let terms = brand_terms(&project);
    let brand = BrandSplit {
        brand: totals(queries_now.iter().filter(|row| is_brand(&row.key, &terms))),
        other: totals(queries_now.iter().filter(|row| !is_brand(&row.key, &terms))),
        brand_before: totals(
            queries_before
                .values()
                .filter(|row| is_brand(&row.key, &terms)),
        ),
        other_before: totals(
            queries_before
                .values()
                .filter(|row| !is_brand(&row.key, &terms)),
        ),
        terms: terms.clone(),
    };
    queries_now.truncate(REPORT_ROWS as usize);
    let pages_now = store.gsc_rows(id, SearchKind::Page, Period::Now, REPORT_ROWS)?;
    let pages_before: HashMap<String, SearchRow> = store
        .gsc_rows(id, SearchKind::Page, Period::Before, 25_000)?
        .into_iter()
        .map(|row| (row.key.clone(), row))
        .collect();
    let landing: HashMap<String, TrafficRow> = store
        .ga4_rows(id, TrafficKind::Landing, Period::Now, 25_000)?
        .into_iter()
        .map(|row| (row.key.clone(), row))
        .collect();
    let query_pages = store.gsc_query_pages(id, None, 25_000)?;
    let tracked: HashSet<String> = store
        .tracked_keywords(id)?
        .into_iter()
        .map(|keyword| fold(&keyword.keyword))
        .collect();
    let devices = store.gsc_rows(id, SearchKind::Device, Period::Now, 10)?;
    let countries = store.gsc_rows(id, SearchKind::Country, Period::Now, 20)?;
    drop(store);

    let in_now = |date: &str| date >= start.as_str();
    let now_totals = totals(day_rows.iter().filter(|row| in_now(&row.key)));
    let before_totals = totals(day_rows.iter().filter(|row| !in_now(&row.key)));
    let has_traffic = !traffic_days.is_empty() || !landing.is_empty();
    let traffic_by_day: HashMap<&str, &TrafficRow> = traffic_days
        .iter()
        .map(|row| (row.key.as_str(), row))
        .collect();
    let daily = day_rows
        .iter()
        .map(|row| DayPoint {
            date: row.key.clone(),
            period: if in_now(&row.key) { "now" } else { "before" },
            clicks: row.clicks,
            impressions: row.impressions,
            position: round(row.position),
            sessions: traffic_by_day.get(row.key.as_str()).map(|day| day.sessions),
            key_events: traffic_by_day
                .get(row.key.as_str())
                .map(|day| day.key_events),
        })
        .collect();

    let mut pages_of_query: HashMap<&str, Vec<&QueryPage>> = HashMap::new();
    for row in &query_pages {
        pages_of_query
            .entry(row.query.as_str())
            .or_default()
            .push(row);
    }
    let most_impressions = queries_now
        .iter()
        .map(|row| row.impressions)
        .fold(0.0, f64::max);
    let enough = (most_impressions * 0.01).max(20.0);
    let queries: Vec<QueryRow> = queries_now
        .iter()
        .map(|row| {
            let before = queries_before.get(&row.key);
            let rate = ctr_of(row.clicks, row.impressions);
            let place = row.position.round().clamp(1.0, 100.0) as u16;
            QueryRow {
                clicks_change: before.map(|before| row.clicks - before.clicks),
                position_change: before.map(|before| round(before.position - row.position)),
                pages: pages_of_query
                    .get(row.key.as_str())
                    .map_or(0, |pages| u32::try_from(pages.len()).unwrap_or(u32::MAX)),
                tracked: tracked.contains(&fold(&row.key)),
                near_top: row.position >= 3.5 && row.position <= 20.0 && row.impressions >= enough,
                brand: is_brand(&row.key, &terms),
                low_ctr: row.impressions >= enough * 2.0
                    && place <= 10
                    && rate < ctr(place) * 100.0 * 0.5,
                query: row.key.clone(),
                clicks: row.clicks,
                impressions: row.impressions,
                ctr: rate,
                position: round(row.position),
            }
        })
        .collect();
    let tracked_seen = u32::try_from(
        queries_before
            .keys()
            .chain(queries_now.iter().map(|row| &row.key))
            .map(|query| fold(query))
            .filter(|query| tracked.contains(query))
            .collect::<HashSet<_>>()
            .len(),
    )
    .unwrap_or(u32::MAX);

    let page_names: Vec<&str> = pages_now.iter().map(|row| row.key.as_str()).collect();
    let audit = audited(workspace, id, &page_names);
    let mut pages: Vec<PageRow> = pages_now
        .iter()
        .map(|row| {
            let visits = landing.get(&path_of(&row.key));
            PageRow {
                clicks_change: pages_before
                    .get(&row.key)
                    .map(|before| row.clicks - before.clicks),
                sessions: visits.map(|visits| visits.sessions),
                key_events: visits.map(|visits| visits.key_events),
                revenue: visits.map(|visits| visits.revenue),
                audit: audit
                    .as_ref()
                    .and_then(|(_, seen)| seen.get(&row.key).cloned()),
                page: row.key.clone(),
                clicks: row.clicks,
                impressions: row.impressions,
                ctr: ctr_of(row.clicks, row.impressions),
                position: round(row.position),
            }
        })
        .collect();
    let audit = audit.map(|(mut link, _)| {
        link.problem_pages = u32::try_from(
            pages
                .iter()
                .filter(|page| page.clicks > 0.0)
                .filter(|page| {
                    page.audit.as_ref().is_some_and(|seen| {
                        seen.indexability != "indexable"
                            || seen.status.is_none_or(|status| status >= 300)
                    })
                })
                .count(),
        )
        .unwrap_or(u32::MAX);
        link
    });
    pages.truncate(REPORT_ROWS as usize);

    let mut cannibalised: Vec<Cannibalised> = pages_of_query
        .iter()
        .filter_map(|(query, rows)| {
            let shown: Vec<&&QueryPage> =
                rows.iter().filter(|row| row.impressions >= 5.0).collect();
            (shown.len() >= 2).then(|| Cannibalised {
                query: (*query).to_owned(),
                impressions: shown.iter().map(|row| row.impressions).sum(),
                pages: shown
                    .iter()
                    .map(|row| CannibalPage {
                        page: row.page.clone(),
                        clicks: row.clicks,
                        impressions: row.impressions,
                        position: round(row.position),
                    })
                    .collect(),
            })
        })
        .collect();
    cannibalised.sort_by(|left, right| {
        right
            .impressions
            .total_cmp(&left.impressions)
            .then_with(|| left.query.cmp(&right.query))
    });
    cannibalised.truncate(CANNIBALISED);

    let traffic_now =
        has_traffic.then(|| traffic(traffic_days.iter().filter(|row| in_now(&row.key))));
    let traffic_before =
        has_traffic.then(|| traffic(traffic_days.iter().filter(|row| !in_now(&row.key))));
    Ok(Some(GscReport {
        currency: sync.currency.clone(),
        period: (start.clone(), end.clone()),
        previous,
        compare,
        brand,
        now: now_totals,
        before: before_totals,
        traffic_now,
        traffic_before,
        daily,
        queries,
        pages,
        devices,
        countries,
        cannibalised,
        audit,
        tracked: (
            tracked_seen,
            u32::try_from(tracked.len()).unwrap_or(u32::MAX),
        ),
        sync,
    }))
}

#[derive(Debug, Deserialize)]
struct QueryAsk {
    project_id: i64,
    query: String,
}

/// The pages Search Console showed for a query.
fn gsc_query(context: &Context<'_>, ask: QueryAsk) -> Result<Vec<QueryPage>, AppError> {
    let store = context.workspace.store()?;
    store.project(ask.project_id)?;
    Ok(store.gsc_query_pages(ask.project_id, Some(ask.query.trim()), 100)?)
}

#[derive(Debug, Deserialize)]
struct ExportAsk {
    project_id: i64,
    /// `queries`, `pages` or `query_pages`.
    kind: String,
}

/// Every row a sync read, for a CSV file: a header and the rows.
#[derive(Debug, Serialize)]
struct Export {
    columns: Vec<&'static str>,
    rows: Vec<Vec<serde_json::Value>>,
}

fn gsc_export(context: &Context<'_>, ask: ExportAsk) -> Result<Export, AppError> {
    let store = context.workspace.store()?;
    let project = store.project(ask.project_id)?;
    let id = ask.project_id;
    let before_of = |kind: SearchKind| -> Result<HashMap<String, SearchRow>, AppError> {
        Ok(store
            .gsc_rows(id, kind, Period::Before, 50_000)?
            .into_iter()
            .map(|row| (row.key.clone(), row))
            .collect())
    };
    Ok(match ask.kind.as_str() {
        "queries" => {
            let terms = brand_terms(&project);
            let before = before_of(SearchKind::Query)?;
            Export {
                columns: vec![
                    "query",
                    "clicks",
                    "impressions",
                    "ctr",
                    "position",
                    "clicks_before",
                    "position_before",
                    "brand",
                ],
                rows: store
                    .gsc_rows(id, SearchKind::Query, Period::Now, 50_000)?
                    .iter()
                    .map(|row| {
                        let earlier = before.get(&row.key);
                        vec![
                            json!(row.key),
                            json!(row.clicks),
                            json!(row.impressions),
                            json!(ctr_of(row.clicks, row.impressions)),
                            json!(round(row.position)),
                            json!(earlier.map(|before| before.clicks)),
                            json!(earlier.map(|before| round(before.position))),
                            json!(is_brand(&row.key, &terms)),
                        ]
                    })
                    .collect(),
            }
        }
        "pages" => {
            let before = before_of(SearchKind::Page)?;
            Export {
                columns: vec![
                    "page",
                    "clicks",
                    "impressions",
                    "ctr",
                    "position",
                    "clicks_before",
                ],
                rows: store
                    .gsc_rows(id, SearchKind::Page, Period::Now, 50_000)?
                    .iter()
                    .map(|row| {
                        vec![
                            json!(row.key),
                            json!(row.clicks),
                            json!(row.impressions),
                            json!(ctr_of(row.clicks, row.impressions)),
                            json!(round(row.position)),
                            json!(before.get(&row.key).map(|before| before.clicks)),
                        ]
                    })
                    .collect(),
            }
        }
        "query_pages" => Export {
            columns: vec!["query", "page", "clicks", "impressions", "position"],
            rows: store
                .gsc_query_pages(id, None, 100_000)?
                .iter()
                .map(|row| {
                    vec![
                        json!(row.query),
                        json!(row.page),
                        json!(row.clicks),
                        json!(row.impressions),
                        json!(round(row.position)),
                    ]
                })
                .collect(),
        },
        other => {
            return Err(AppError::Invalid(format!(
                "kind: queries, pages or query_pages, not `{other}`"
            )));
        }
    })
}

/// Adds the commands.
pub fn register(router: &mut Router) {
    router.add("gsc_export", gsc_export);
    router.add("google_status", google_status);
    router.add("set_google_client", set_google_client);
    router.add("start_google_login", start_google_login);
    router.add("google_logout", google_logout);
    router.add("google_properties", google_properties);
    router.add("gsc_setup", gsc_setup);
    router.add("save_gsc_settings", save_gsc_settings);
    router.add("start_gsc_sync", start_gsc_sync);
    router.add("gsc_report", gsc_report);
    router.add("gsc_query", gsc_query);
}
