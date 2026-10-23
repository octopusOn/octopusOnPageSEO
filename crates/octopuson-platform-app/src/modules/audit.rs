//! Site Audit: crawls the project's site with the SEO Crawler's engine and its 118 checks, and
//! reads the crawl's GEO readiness (AI crawler access, llms.txt, quotable snippets, structured
//! data, question-led sections, substantial content).
//!
//! Each audit is a crawl in the workspace's `crawls/` folder plus a row with its scores; the
//! report compares every check with the audit before. On the desktop the crawl runs on this
//! computer and costs nothing. On the web it runs on the server: only public addresses are
//! fetched (no private, local or metadata address, on any redirect), a few crawls run at a
//! time, and every page fetched is metered on the account, a month's free pages first.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use octopuson_audit::{Lang, catalog};
use octopuson_cloud::{MeterPrice, Metered};
use octopuson_core::Timestamp;
use octopuson_crawl::{
    CrawlActivity, CrawlConfig, CrawlControl, CrawlPlan, CrawlProgress, CrawlSink, Crawler,
    Fetcher, Follow, Limits, LinkRecord, RobotsRecord, SiteFileRecord, SitemapRecord, Speed,
    UrlRecord,
};
use octopuson_crawl_store::{
    CrawlLibrary, CrawlOverview, CrawlReader, CrawlStoreError, CrawlWriter, GeoReadiness,
    PageRequest,
};
use octopuson_net::CrawlFetcher;
use octopuson_platform::{AuditEnd, AuditRow, AuditSettings, ProjectRow, unix_now};
use octopuson_serp_score::rank::seconds_of;
use serde::{Deserialize, Serialize};

use crate::jobs::{JobControl, JobDone, JobManager, JobStatus};
use crate::router::{Context, Router};
use crate::{AppError, Host, Workspace, now};

/// Kind of the job of an audit.
pub const JOB_KIND: &str = "site-audit";

/// What crawled pages are metered as.
const METER_SKU: &str = "crawl.page";

/// Pages metered at a time while a crawl runs on the server.
const METER_BATCH: u32 = 100;

/// Fewest URLs an audit fetches.
const MIN_PAGES: u32 = 10;

/// Most URLs an audit fetches on the server.
const WEB_MAX_PAGES: u32 = 5_000;

/// Most URLs an audit fetches on the desktop.
const DESKTOP_MAX_PAGES: u32 = 100_000;

/// Crawls running on the server at once; the others wait.
const WEB_CRAWL_SLOTS: usize = 3;

/// Audits listed.
const AUDITS_SHOWN: u32 = 20;

/// Rows of a check's URL list.
const ISSUE_ROWS: u32 = 100;

/// Crawls running on the server now.
static WEB_CRAWLS: AtomicUsize = AtomicUsize::new(0);

/// A place among the server's running crawls, freed when dropped.
struct Slot;

impl Slot {
    fn take() -> Option<Self> {
        let mut running = WEB_CRAWLS.load(Ordering::SeqCst);
        loop {
            if running >= WEB_CRAWL_SLOTS {
                return None;
            }
            match WEB_CRAWLS.compare_exchange(
                running,
                running + 1,
                Ordering::SeqCst,
                Ordering::SeqCst,
            ) {
                Ok(_) => return Some(Self),
                Err(now) => running = now,
            }
        }
    }
}

impl Drop for Slot {
    fn drop(&mut self) {
        WEB_CRAWLS.fetch_sub(1, Ordering::SeqCst);
    }
}

/// How often a site is audited.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
enum Schedule {
    Weekly,
    Monthly,
    Manual,
}

impl Schedule {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Weekly => "weekly",
            Self::Monthly => "monthly",
            Self::Manual => "manual",
        }
    }

    /// Seconds after an audit when the next one is due (a little early, like rank checks).
    fn interval_secs(schedule: &str) -> Option<i64> {
        match schedule {
            "weekly" => Some(6 * 86_400 + 12 * 3_600),
            "monthly" => Some(29 * 86_400),
            _ => None,
        }
    }
}

fn library(workspace: &Workspace) -> CrawlLibrary {
    CrawlLibrary::new(workspace.data_dir())
}

const fn max_pages_limit(host: Host) -> u32 {
    match host {
        Host::Web => WEB_MAX_PAGES,
        Host::Desktop => DESKTOP_MAX_PAGES,
    }
}

fn rfc3339(seconds: i64) -> String {
    Timestamp::from_unix_micros(seconds.saturating_mul(1_000_000)).to_string()
}

/// When a project's audit schedule is due, seconds since the Unix epoch.
fn due_at(settings: &AuditSettings) -> Option<i64> {
    let interval = Schedule::interval_secs(&settings.schedule)?;
    Some(
        settings
            .last_run_at
            .as_deref()
            .and_then(seconds_of)
            .map_or(0, |last| last + interval),
    )
}

pub(crate) fn store_error(error: &CrawlStoreError) -> AppError {
    AppError::Internal(error.to_string())
}

// ----- Setup -------------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct ProjectAsk {
    project_id: i64,
}

/// What crawling costs on the web.
#[derive(Debug, Serialize)]
struct MeterInfo {
    /// Micro-dollars a page after the free ones.
    micros_per_unit: i64,
    /// Pages free each month.
    free_per_month: i64,
    /// Micro-dollars in a credit.
    credit_unit_micros: i64,
}

#[derive(Debug, Serialize)]
struct AuditSetup {
    settings: AuditSettings,
    /// Most URLs an audit may fetch here.
    max_pages_limit: u32,
    /// When the schedule audits next, RFC 3339; `None` when audited only by hand.
    next_audit_at: Option<String>,
    /// On the web: what crawled pages cost.
    meter: Option<MeterInfo>,
    audits: Vec<AuditRow>,
}

fn setup(workspace: &Workspace, project_id: i64) -> Result<AuditSetup, AppError> {
    let (settings, audits) = {
        let store = workspace.store()?;
        store.project(project_id)?;
        (
            store.audit_settings(project_id)?,
            store.audits(project_id, AUDITS_SHOWN)?,
        )
    };
    let meter = match workspace.host() {
        Host::Web => workspace
            .meter_price(METER_SKU)?
            .map(|(price, unit): (MeterPrice, i64)| MeterInfo {
                micros_per_unit: price.micros_per_unit,
                free_per_month: price.free_per_month,
                credit_unit_micros: unit,
            }),
        Host::Desktop => None,
    };
    Ok(AuditSetup {
        next_audit_at: due_at(&settings).map(|at| rfc3339(at.max(unix_now()))),
        max_pages_limit: max_pages_limit(workspace.host()),
        meter,
        settings,
        audits,
    })
}

fn audit_setup(context: &Context<'_>, ask: ProjectAsk) -> Result<AuditSetup, AppError> {
    setup(context.workspace, ask.project_id)
}

#[derive(Debug, Deserialize)]
struct SettingsAsk {
    project_id: i64,
    schedule: Schedule,
    max_pages: u32,
    /// Check links to other sites (each counts as a page).
    #[serde(default)]
    check_outbound: bool,
}

fn save_audit_settings(context: &Context<'_>, ask: SettingsAsk) -> Result<AuditSetup, AppError> {
    let limit = max_pages_limit(context.workspace.host());
    context.workspace.store()?.save_audit_settings(
        ask.project_id,
        &AuditSettings {
            schedule: ask.schedule.as_str().to_owned(),
            max_pages: ask.max_pages.clamp(MIN_PAGES, limit),
            last_run_at: None,
            check_outbound: ask.check_outbound,
        },
    )?;
    setup(context.workspace, ask.project_id)
}

// ----- Running -----------------------------------------------------------------------------

/// The crawl of a project's site: its home page, the pages and images it links to (other
/// sites only checked once each when asked, never crawled), as the OctopusOn crawler that obeys
/// robots.txt.
fn crawl_config(
    project: &ProjectRow,
    max_pages: u32,
    host: Host,
    check_outbound: bool,
) -> CrawlConfig {
    let speed = match host {
        Host::Web => Speed {
            threads: 3,
            max_requests_per_second: 4.0,
        },
        Host::Desktop => Speed::default(),
    };
    CrawlConfig {
        start_urls: vec![format!("https://{}/", project.domain)],
        follow: Follow {
            check_external: check_outbound,
            stylesheets: false,
            scripts: false,
            ..Follow::default()
        },
        limits: Limits {
            max_urls: max_pages,
            ..Limits::default()
        },
        speed,
        ..CrawlConfig::default()
    }
}

/// Starts an audit of a project as a job, or returns the one running.
fn start(
    workspace: &Arc<Workspace>,
    jobs: &JobManager,
    project: &ProjectRow,
    scheduled: bool,
) -> Result<String, AppError> {
    if let Some(id) = jobs.running_for(JOB_KIND, Some(project.id)) {
        return Ok(id);
    }
    if workspace.host() == Host::Web && workspace.account_token().is_none() {
        return Err(AppError::Account(
            "connect your OctopusOn SEO account to crawl on the server".to_owned(),
        ));
    }
    let start_url = format!("https://{}/", project.domain);
    let audit_id = workspace
        .store()?
        .begin_audit(project.id, scheduled, &start_url, &now())?;
    let workspace = Arc::clone(workspace);
    let project_id = project.id;
    jobs.start(JOB_KIND, &project.name, Some(project.id), move |control| {
        run_audit(&workspace, project_id, audit_id, control)
    })
}

fn start_audit(context: &Context<'_>, ask: ProjectAsk) -> Result<String, AppError> {
    let project = context.workspace.store()?.project(ask.project_id)?;
    start(context.workspace, context.jobs, &project, false)
}

/// Pages metered on the account while a server crawl runs.
struct Meter<'w> {
    workspace: &'w Workspace,
    crawl_id: String,
    metered: u32,
    batches: u32,
    charged_micros: i64,
    /// The credits ran out.
    refused: bool,
    /// The account service could not be reached.
    failed: Option<AppError>,
}

impl Meter<'_> {
    /// Meters whole batches of what was crawled (with `all`, the rest too); `false` when the
    /// crawl must stop.
    fn settle(&mut self, crawled: u32, all: bool) -> bool {
        while !self.refused && self.failed.is_none() {
            let left = crawled.saturating_sub(self.metered);
            let units = if left >= METER_BATCH {
                METER_BATCH
            } else if all && left > 0 {
                left
            } else {
                break;
            };
            let key = format!("{}:{}", self.crawl_id, self.batches);
            match self.workspace.meter(METER_SKU, units, &key) {
                Ok(Metered::Charged(receipt)) => {
                    self.metered += units;
                    self.batches += 1;
                    self.charged_micros += receipt.charged_micros;
                }
                Ok(Metered::NoCredits { .. }) => self.refused = true,
                Err(error) => self.failed = Some(error),
            }
        }
        !self.refused && self.failed.is_none()
    }
}

/// Writes the crawl and reports its progress.
struct AuditSink<F> {
    writer: CrawlWriter,
    on_progress: F,
}

impl<F: FnMut(&CrawlProgress)> CrawlSink for AuditSink<F> {
    type Error = CrawlStoreError;

    fn site_file(&mut self, record: &SiteFileRecord) -> Result<(), Self::Error> {
        self.writer.site_file(record)
    }

    fn url(&mut self, record: &UrlRecord, links: &[LinkRecord]) -> Result<(), Self::Error> {
        self.writer.url(record, links)
    }

    fn robots(&mut self, record: &RobotsRecord) -> Result<(), Self::Error> {
        self.writer.robots(record)
    }

    fn sitemap(&mut self, record: &SitemapRecord) -> Result<(), Self::Error> {
        self.writer.sitemap(record)
    }

    fn progress(&mut self, progress: &CrawlProgress) {
        self.writer.progress(progress);
        (self.on_progress)(progress);
    }

    fn activity(&mut self, _: &CrawlActivity) {}
}

/// Findings of every check.
fn findings(overview: &CrawlOverview) -> u32 {
    u32::try_from(overview.issues.iter().map(|issue| issue.count).sum::<u64>()).unwrap_or(u32::MAX)
}

#[allow(clippy::too_many_lines)] // one crawl with its metering and bookkeeping
fn run_audit(
    workspace: &Workspace,
    project_id: i64,
    audit_id: i64,
    control: &JobControl,
) -> Result<JobDone, AppError> {
    let host = workspace.host();
    let (project, settings) = {
        let store = workspace.store()?;
        (
            store.project(project_id)?,
            store.audit_settings(project_id)?,
        )
    };
    let max_pages = settings.max_pages.clamp(MIN_PAGES, max_pages_limit(host));
    let fail = |message: &str| -> Result<JobDone, AppError> {
        workspace.store()?.finish_audit(
            audit_id,
            &AuditEnd {
                status: "failed",
                pages: 0,
                health: None,
                geo: None,
                issues: 0,
                metered_pages: 0,
                charged_micros: 0,
                error: Some(message),
            },
            &now(),
        )?;
        Err(AppError::Invalid(message.to_owned()))
    };
    let mut config = crawl_config(&project, max_pages, host, settings.check_outbound);
    if workspace.test_fetcher().is_some() {
        // A site in memory needs no politeness.
        config.speed = Speed::default();
    }
    let plan = match CrawlPlan::new(config) {
        Ok(plan) => plan,
        Err(error) => return fail(&error.to_string()),
    };

    // A few crawls at a time on the server.
    let _slot = match host {
        Host::Web => loop {
            if let Some(slot) = Slot::take() {
                break Some(slot);
            }
            control.progress("waiting", 0, u64::from(max_pages));
            if !control.sleep(Duration::from_secs(2)) {
                workspace.store()?.finish_audit(
                    audit_id,
                    &AuditEnd {
                        status: "stopped",
                        pages: 0,
                        health: None,
                        geo: None,
                        issues: 0,
                        metered_pages: 0,
                        charged_micros: 0,
                        error: None,
                    },
                    &now(),
                )?;
                return Ok(JobDone {
                    status: JobStatus::Done,
                    result: None,
                });
            }
        },
        Host::Desktop => None,
    };

    let network = match host {
        Host::Web => CrawlFetcher::public_only(plan.user_agent(), plan.timeout(), None),
        Host::Desktop => CrawlFetcher::new(plan.user_agent(), plan.timeout(), None),
    };
    let fetcher: &dyn Fetcher = match workspace.test_fetcher() {
        Some(fetcher) => fetcher.as_ref(),
        None => &network,
    };
    let library = library(workspace);
    let writer = library.create(&plan).map_err(|error| store_error(&error))?;
    let crawl_id = writer.meta().id.clone();
    workspace.store()?.set_audit_crawl(audit_id, &crawl_id)?;
    let crawl_control = CrawlControl::new();
    let mut meter = Meter {
        workspace,
        crawl_id: crawl_id.clone(),
        metered: 0,
        batches: 0,
        charged_micros: 0,
        refused: false,
        failed: None,
    };
    let metering = host == Host::Web;
    let mut sink = AuditSink {
        writer,
        on_progress: |progress: &CrawlProgress| {
            control.progress(
                "crawling",
                u64::from(progress.crawled),
                u64::from(max_pages),
            );
            if control.cancelled() || (metering && !meter.settle(progress.crawled, false)) {
                crawl_control.stop();
            }
        },
    };
    let outcome = Crawler::new(&plan, fetcher).run(&mut sink, &crawl_control);
    let AuditSink { writer, .. } = sink;
    let summary = match outcome {
        Ok(summary) => summary,
        Err(error) => {
            let _ = writer.fail(&error.to_string());
            return fail(&error.to_string());
        }
    };
    control.progress(
        "finalizing",
        u64::from(summary.progress.crawled),
        u64::from(max_pages),
    );
    if metering {
        meter.settle(summary.progress.crawled, true);
    }
    let meta = writer
        .finish(&summary)
        .map_err(|error| store_error(&error))?;
    let reader = library
        .open(&meta.id)
        .map_err(|error| store_error(&error))?;
    let overview = reader.overview().map_err(|error| store_error(&error))?;
    let geo = reader
        .geo_readiness()
        .map_err(|error| store_error(&error))?;
    drop(reader);
    let stop_reason = if meter.refused {
        Some(format!(
            "the credits ran out after {} pages; add credits to audit the rest",
            meter.metered
        ))
    } else {
        meter
            .failed
            .as_ref()
            .map(|error| format!("crawled pages could not be metered: {error}"))
    };
    let status = if stop_reason.is_some() || control.cancelled() {
        "stopped"
    } else {
        "done"
    };
    workspace.store()?.finish_audit(
        audit_id,
        &AuditEnd {
            status,
            pages: summary.progress.crawled,
            health: overview.seo_score.value,
            geo: geo.score,
            issues: findings(&overview),
            metered_pages: meter.metered,
            charged_micros: meter.charged_micros,
            error: stop_reason.as_deref(),
        },
        &now(),
    )?;
    Ok(JobDone {
        status: if stop_reason.is_some() {
            JobStatus::Partial
        } else {
            JobStatus::Done
        },
        result: Some(serde_json::json!({
            "audit_id": audit_id,
            "pages": summary.progress.crawled,
            "health": overview.seo_score.value,
            "geo": geo.score,
        })),
    })
}

/// Starts the audits whose schedule is due; returns the jobs started (or already running).
///
/// # Errors
/// Storage errors.
pub fn run_due(workspace: &Arc<Workspace>, jobs: &JobManager) -> Result<Vec<String>, AppError> {
    let now_secs = unix_now();
    let store = workspace.store()?;
    let mut projects = Vec::new();
    for (project_id, settings) in store.audit_plans()? {
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
                tracing::warn!(project = project.id, %error, "a scheduled audit did not start");
            }
        }
    }
    Ok(started)
}

/// When the next scheduled audit is due, seconds since the Unix epoch.
///
/// # Errors
/// Storage errors.
pub fn next_due(workspace: &Workspace) -> Result<Option<i64>, AppError> {
    Ok(workspace
        .store()?
        .audit_plans()?
        .iter()
        .filter_map(|(_, settings)| due_at(settings))
        .min())
}

// ----- Reports -----------------------------------------------------------------------------

pub(crate) fn lang_of(text: Option<&str>) -> Lang {
    if text == Some("tr") {
        Lang::Tr
    } else {
        Lang::En
    }
}

#[derive(Debug, Deserialize)]
struct ReportAsk {
    project_id: i64,
    /// The latest audit when empty.
    #[serde(default)]
    audit_id: Option<i64>,
    /// `tr`, or English texts for every other language.
    #[serde(default)]
    lang: Option<String>,
}

/// Figures of a crawl.
#[derive(Debug, Serialize)]
struct Overview {
    health: Option<f64>,
    internal: u64,
    external: u64,
    indexable: u64,
    html: u64,
    blocked: u64,
    errors: u64,
    redirects: u64,
    average_response_ms: f64,
    affected_urls: u64,
}

/// One check with findings now or in the audit before.
#[derive(Debug, Serialize)]
struct IssueReport {
    id: String,
    title: String,
    kind: &'static str,
    priority: &'static str,
    count: u64,
    /// Its count in the audit before.
    previous: Option<u64>,
}

#[derive(Debug, Serialize)]
struct CategoryReport {
    id: String,
    name: String,
    issues: Vec<IssueReport>,
}

#[derive(Debug, Serialize)]
struct AuditReport {
    audit: AuditRow,
    /// The finished audit before it, compared with.
    previous: Option<AuditRow>,
    /// `None` while the crawl runs or when it left nothing to read.
    overview: Option<Overview>,
    categories: Vec<CategoryReport>,
    geo: Option<GeoReadiness>,
}

/// The crawl of a finished audit.
pub(crate) fn open(workspace: &Workspace, audit: &AuditRow) -> Option<CrawlReader> {
    if audit.status == "running" {
        return None;
    }
    library(workspace).open(audit.crawl_id.as_deref()?).ok()
}

fn audit_report(context: &Context<'_>, ask: ReportAsk) -> Result<Option<AuditReport>, AppError> {
    let workspace = context.workspace;
    let audits = workspace.store()?.audits(ask.project_id, AUDITS_SHOWN)?;
    let audit = match ask.audit_id {
        Some(id) => workspace.store()?.audit(ask.project_id, id)?,
        None => match audits.first() {
            Some(latest) => latest.clone(),
            None => return Ok(None),
        },
    };
    let previous = audits
        .iter()
        .find(|row| {
            (row.started_at.as_str(), row.id) < (audit.started_at.as_str(), audit.id)
                && row.crawl_id.is_some()
                && row.status != "running"
                && row.status != "failed"
        })
        .cloned();
    let lang = lang_of(ask.lang.as_deref());
    let Some(reader) = open(workspace, &audit) else {
        return Ok(Some(AuditReport {
            audit,
            previous,
            overview: None,
            categories: Vec::new(),
            geo: None,
        }));
    };
    let overview = reader.overview().map_err(|error| store_error(&error))?;
    let geo = reader
        .geo_readiness()
        .map_err(|error| store_error(&error))?;
    drop(reader);
    let before: Option<HashMap<String, u64>> = previous
        .as_ref()
        .and_then(|row| open(workspace, row))
        .and_then(|reader| reader.overview().ok())
        .map(|overview| {
            overview
                .issues
                .into_iter()
                .map(|issue| (issue.id, issue.count))
                .collect()
        });
    let now_counts: HashMap<&str, u64> = overview
        .issues
        .iter()
        .map(|issue| (issue.id.as_str(), issue.count))
        .collect();
    let checks = catalog();
    let categories = checks
        .categories
        .iter()
        .filter_map(|category| {
            let issues: Vec<IssueReport> = checks
                .issues
                .iter()
                .filter(|issue| issue.category == category.id)
                .filter_map(|issue| {
                    let count = now_counts.get(issue.id.as_str()).copied().unwrap_or(0);
                    let previous = before
                        .as_ref()
                        .map(|counts| counts.get(&issue.id).copied().unwrap_or(0));
                    (count > 0 || previous.is_some_and(|before| before > 0)).then(|| IssueReport {
                        id: issue.id.clone(),
                        title: issue.text(lang).title.clone(),
                        kind: issue.kind.as_str(),
                        priority: issue.priority.as_str(),
                        count,
                        previous,
                    })
                })
                .collect();
            (!issues.is_empty()).then(|| CategoryReport {
                id: category.id.clone(),
                name: category.name(lang).to_owned(),
                issues,
            })
        })
        .collect();
    Ok(Some(AuditReport {
        overview: Some(Overview {
            health: overview.seo_score.value,
            internal: overview.internal,
            external: overview.external,
            indexable: overview.indexable,
            html: overview.html,
            blocked: overview.blocked,
            errors: overview.errors,
            redirects: overview.redirects,
            average_response_ms: overview.average_response_ms,
            affected_urls: overview.seo_score.affected_urls,
        }),
        audit,
        previous,
        categories,
        geo: Some(geo),
    }))
}

#[derive(Debug, Deserialize)]
struct IssueAsk {
    project_id: i64,
    audit_id: i64,
    issue_id: String,
    #[serde(default)]
    offset: u64,
    #[serde(default)]
    lang: Option<String>,
}

/// A check: what it means, how to fix it and where it was found.
#[derive(Debug, Serialize)]
struct IssueDetail {
    id: String,
    title: String,
    description: String,
    fix: String,
    kind: &'static str,
    priority: &'static str,
    total: u64,
    offset: u64,
    columns: Vec<String>,
    rows: Vec<Vec<String>>,
}

fn audit_issue(context: &Context<'_>, ask: IssueAsk) -> Result<IssueDetail, AppError> {
    let check = catalog()
        .issue(&ask.issue_id)
        .ok_or_else(|| AppError::NotFound(format!("check `{}`", ask.issue_id)))?;
    let audit = context
        .workspace
        .store()?
        .audit(ask.project_id, ask.audit_id)?;
    let reader = open(context.workspace, &audit)
        .ok_or_else(|| AppError::Invalid("the audit has no finished crawl".to_owned()))?;
    let page = reader
        .page(&PageRequest {
            tab: "issues".to_owned(),
            issue_id: check.id.clone(),
            offset: ask.offset,
            limit: ISSUE_ROWS,
            ..PageRequest::default()
        })
        .map_err(|error| store_error(&error))?;
    let text = check.text(lang_of(ask.lang.as_deref()));
    Ok(IssueDetail {
        id: check.id.clone(),
        title: text.title.clone(),
        description: text.description.clone(),
        fix: text.fix.clone(),
        kind: check.kind.as_str(),
        priority: check.priority.as_str(),
        total: page.total,
        offset: ask.offset,
        columns: page
            .columns
            .iter()
            .map(|column| column.name.clone())
            .collect(),
        rows: page
            .rows
            .iter()
            .map(|row| row.iter().map(ToString::to_string).collect())
            .collect(),
    })
}

#[derive(Debug, Deserialize)]
struct OutboundAsk {
    project_id: i64,
    /// The latest audit with a crawl when absent.
    #[serde(default)]
    audit_id: Option<i64>,
    #[serde(default)]
    offset: u64,
    /// Only links answering with this status class (4 or 5); every link with 0.
    #[serde(default)]
    status_class: u16,
}

/// Links from the site to other sites in an audit's crawl, most linked first.
#[derive(Debug, Serialize)]
struct Outbound {
    audit_id: i64,
    /// Whether the audit checked them (otherwise they have no status).
    checked: bool,
    total: u64,
    offset: u64,
    columns: Vec<String>,
    rows: Vec<Vec<String>>,
}

/// Rows of outbound links shown at a time.
const OUTBOUND_ROWS: u32 = 200;

fn audit_outbound(context: &Context<'_>, ask: OutboundAsk) -> Result<Option<Outbound>, AppError> {
    let store = context.workspace.store()?;
    let audit = match ask.audit_id {
        Some(id) => store.audit(ask.project_id, id)?,
        None => match store
            .audits(ask.project_id, AUDITS_SHOWN)?
            .into_iter()
            .find(|audit| audit.status != "running" && audit.crawl_id.is_some())
        {
            Some(audit) => audit,
            None => return Ok(None),
        },
    };
    drop(store);
    let Some(reader) = open(context.workspace, &audit) else {
        return Ok(None);
    };
    let page = reader
        .page(&PageRequest {
            tab: "external".to_owned(),
            status_class: if matches!(ask.status_class, 4 | 5) {
                ask.status_class
            } else {
                0
            },
            offset: ask.offset,
            limit: OUTBOUND_ROWS,
            sort: "inlinks".to_owned(),
            descending: true,
            ..PageRequest::default()
        })
        .map_err(|error| store_error(&error))?;
    let status = page
        .columns
        .iter()
        .position(|column| column.name == "status_code");
    // Unchecked links have no status.
    let checked = status.is_some_and(|index| {
        page.rows.iter().any(|row| {
            row.get(index)
                .map(ToString::to_string)
                .is_some_and(|cell| !cell.is_empty() && cell != "null")
        })
    });
    Ok(Some(Outbound {
        audit_id: audit.id,
        checked,
        total: page.total,
        offset: ask.offset,
        columns: page
            .columns
            .iter()
            .map(|column| column.name.clone())
            .collect(),
        rows: page
            .rows
            .iter()
            .map(|row| row.iter().map(ToString::to_string).collect())
            .collect(),
    }))
}

#[derive(Debug, Deserialize)]
struct DeleteAsk {
    project_id: i64,
    audit_id: i64,
}

fn delete_audit(context: &Context<'_>, ask: DeleteAsk) -> Result<(), AppError> {
    let store = context.workspace.store()?;
    if store.audit(ask.project_id, ask.audit_id)?.status == "running" {
        return Err(AppError::Busy("the audit is still running".to_owned()));
    }
    let crawl = store.delete_audit(ask.project_id, ask.audit_id)?;
    drop(store);
    if let Some(crawl) = crawl {
        // A crawl folder already gone is fine.
        let _ = library(context.workspace).delete(&crawl);
    }
    Ok(())
}

/// Deletes the crawl folders of a project's audits (the project is being deleted).
///
/// # Errors
/// Storage errors.
pub fn delete_crawls(workspace: &Workspace, project_id: i64) -> Result<(), AppError> {
    let crawls = workspace.store()?.audit_crawls(project_id)?;
    let library = library(workspace);
    for crawl in crawls {
        let _ = library.delete(&crawl);
    }
    Ok(())
}

/// Commands after which the audit schedule may be due at another time.
const SCHEDULE_COMMANDS: &[&str] = &["save_audit_settings", "start_audit", "delete_project"];

/// Whether a command may move when an audit is due next.
pub fn changes_schedule(command: &str) -> bool {
    SCHEDULE_COMMANDS.contains(&command)
}

/// Adds the commands.
pub fn register(router: &mut Router) {
    router.add("audit_setup", audit_setup);
    router.add("save_audit_settings", save_audit_settings);
    router.add("start_audit", start_audit);
    router.add("audit_report", audit_report);
    router.add("audit_issue", audit_issue);
    router.add("audit_outbound", audit_outbound);
    router.add("delete_audit", delete_audit);
}
