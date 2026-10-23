//! Reports for agencies: the agency's branding, each project's report definitions, the report
//! drawn from the modules' data as a page, a PDF and a workbook, read-only share links and
//! scheduled e-mails (web).
//!
//! A report never pays a source: it shows what rank tracking, Search Console, the site audit,
//! AI visibility, the kept domain and backlink answers and the project's snapshots already hold.
//! A report can be e-mailed after each new snapshot, with what moved since. PDFs are printed by an
//! installed Chrome, Chromium or Edge; on the web each PDF is metered (`report.pdf`) once the
//! gateway prices it. Share links and e-mails exist only on the web.

use std::sync::{Arc, OnceLock};

use base64::Engine as _;
use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use octopuson_cloud::Metered;
use octopuson_core::{Date, Timestamp};
use octopuson_platform::{
    Branding, DeliveryRow, REPORT_DESIGNS, REPORT_WIDGETS, ReportRow, ReportSpec, unix_now,
};
use octopuson_serp_score::rank::seconds_of;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::snapshots::{self, CompareAsk, Compared};
use crate::jobs::{JobDone, JobManager, NoEvents};
use crate::mail::{Attachment, Mail};
use crate::report::{self, Sources, texts};
use crate::router::{Context, Opener, Router};
use crate::{AppError, Host, Workspace, now};

/// Kind of the job of an e-mail.
pub const JOB_KIND: &str = "report-email";

/// What a PDF is metered as on the web.
const PDF_SKU: &str = "report.pdf";

/// Deliveries listed.
const DELIVERIES_SHOWN: u32 = 10;

/// Commands after which an e-mail may be due at another time.
const SCHEDULE_COMMANDS: &[&str] = &[
    "save_report",
    "delete_report",
    "send_report",
    "delete_project",
    "start_snapshot",
    "delete_snapshot",
];

/// Whether a command may move when a report e-mail is due next.
pub fn changes_schedule(command: &str) -> bool {
    SCHEDULE_COMMANDS.contains(&command)
}

/// The modules' commands a report is drawn from.
fn data_router() -> &'static Router {
    static ROUTER: OnceLock<Router> = OnceLock::new();
    ROUTER.get_or_init(super::router)
}

/// `YYYY-MM-DD` of `back` days ago (UTC).
fn day(back: u32) -> String {
    let today = Date::of(Timestamp::now()).days_since_epoch();
    Date::from_days_since_epoch(today - i32::try_from(back).unwrap_or(0))
        .map_or_else(String::new, |date| date.to_string())
}

/// The modules' reports a report needs.
#[allow(clippy::too_many_lines)] // one question per part
fn sources(workspace: &Arc<Workspace>, report: &ReportRow) -> Sources {
    let jobs = JobManager::new(Arc::new(NoEvents));
    let open: Opener = Arc::new(|_| false);
    let context = Context {
        workspace,
        jobs: &jobs,
        open: &open,
    };
    let wants = |parts: &[&str]| {
        report
            .spec
            .widgets
            .iter()
            .any(|widget| parts.contains(&widget.as_str()))
    };
    let ask = |command: &str, arguments: Value| {
        data_router()
            .call(&context, command, arguments)
            .ok()
            .filter(|value| !value.is_null())
    };
    let project = report.project_id;
    let days = report.spec.days;
    Sources {
        rank: wants(&["summary", "rankings", "competitors"])
            .then(|| {
                ask(
                    "rank_report",
                    json!({ "project_id": project, "days": days }),
                )
            })
            .flatten(),
        gsc: wants(&["summary", "search_console"])
            .then(|| ask("gsc_report", json!({ "project_id": project })))
            .flatten(),
        audit: wants(&["summary", "site_audit"])
            .then(|| {
                ask(
                    "audit_report",
                    json!({ "project_id": project, "lang": report.spec.language }),
                )
            })
            .flatten(),
        ai: wants(&["summary", "ai_visibility"])
            .then(|| ask("ai_report", json!({ "project_id": project, "days": days })))
            .flatten(),
        overview: wants(&["backlinks"])
            .then(|| {
                ask(
                    "get_overview",
                    json!({ "project_id": project, "report": "summary" }),
                )
            })
            .flatten(),
        backlinks: wants(&["backlinks"])
            .then(|| {
                ask(
                    "get_backlinks",
                    json!({ "project_id": project, "report": "summary" }),
                )
            })
            .flatten(),
        comparison: wants(&["comparison"])
            .then(|| compared(workspace, report, false))
            .flatten(),
        comparison_previous: wants(&["comparison_previous"])
            .then(|| compared(workspace, report, true))
            .flatten(),
        organic: wants(&["organic"])
            .then(|| organic(&ask, project))
            .flatten(),
        links: wants(&["link_history"])
            .then(|| links(&ask, project))
            .flatten(),
        gap: wants(&["gap"])
            .then(|| {
                ask(
                    "get_gap",
                    json!({ "project_id": project, "report": "keywords" }),
                )
                .and_then(|kept| kept.get("data").cloned())
            })
            .flatten(),
        lists: wants(&["keyword_lists"])
            .then(|| keyword_lists(&ask, project))
            .flatten(),
        opportunities: wants(&["opportunities"])
            .then(|| {
                ask(
                    "opportunities",
                    json!({ "project_id": project, "lang": report.spec.language }),
                )
            })
            .flatten(),
        local: wants(&["local"])
            .then(|| {
                let at = json!({ "project_id": project });
                Some(json!({
                    "setup": ask("local_setup", at.clone())?,
                    "grid": ask("local_grid", at.clone())?,
                    "reviews": ask("local_reviews", at),
                }))
            })
            .flatten(),
    }
}

/// The data of a kept answer and when it was fetched.
fn kept_data(kept: Option<Value>) -> (Value, Value) {
    kept.map_or((Value::Null, Value::Null), |kept| {
        (
            kept.get("data").cloned().unwrap_or(Value::Null),
            kept.get("fetched_at").cloned().unwrap_or(Value::Null),
        )
    })
}

/// Organic search as the Site Explorer keeps it: the summary, the keywords (the snapshot's
/// thousand, else the page's hundred), the lost ones, the history and the program's own points.
fn organic(ask: &dyn Fn(&str, Value) -> Option<Value>, project: i64) -> Option<Value> {
    let overview = |extra: Value| {
        let mut arguments = json!({ "project_id": project });
        if let (Some(fields), Some(extra)) = (arguments.as_object_mut(), extra.as_object()) {
            fields.extend(extra.clone());
        }
        ask("get_overview", arguments)
    };
    let (summary, fetched_at) = kept_data(overview(json!({ "report": "summary" })));
    let (keywords, _) = kept_data(
        overview(json!({ "report": "keywords", "limit": crate::snapshot::LIST_ROWS }))
            .or_else(|| overview(json!({ "report": "keywords" }))),
    );
    if summary.is_null() && keywords.is_null() {
        return None;
    }
    let (lost, _) = kept_data(overview(json!({ "report": "keywords", "set": "lost" })));
    let (history, _) = kept_data(overview(json!({ "report": "history" })));
    Some(json!({
        "summary": summary,
        "fetched_at": fetched_at,
        "keywords": keywords,
        "lost": lost,
        "history": history,
        "points": ask("metric_history", json!({ "project_id": project, "kind": "domain" })),
    }))
}

/// Backlinks over time as the Backlinks page keeps them, with the user's own link work.
fn links(ask: &dyn Fn(&str, Value) -> Option<Value>, project: i64) -> Option<Value> {
    let backlinks = |report: &str| {
        ask(
            "get_backlinks",
            json!({ "project_id": project, "report": report }),
        )
    };
    let (summary, fetched_at) = kept_data(backlinks("summary"));
    let (history, _) = kept_data(backlinks("history"));
    let (new_lost, _) = kept_data(backlinks("new_lost"));
    let (audit, _) = kept_data(backlinks("audit"));
    let at = json!({ "project_id": project });
    let data = json!({
        "summary": summary,
        "fetched_at": fetched_at,
        "history": history,
        "new_lost": new_lost,
        "audit": audit,
        "points": ask("metric_history", json!({ "project_id": project, "kind": "backlinks" })),
        "decisions": ask("link_decisions", at.clone()),
        "prospects": ask("link_prospects", at),
    });
    let something = [
        "summary",
        "history",
        "new_lost",
        "audit",
        "decisions",
        "prospects",
        "points",
    ]
    .iter()
    .any(|key| match data.get(key) {
        Some(Value::Array(rows)) => !rows.is_empty(),
        Some(value) => !value.is_null(),
        None => false,
    });
    something.then_some(data)
}

/// The project's keyword lists with their keywords, the largest first.
fn keyword_lists(ask: &dyn Fn(&str, Value) -> Option<Value>, project: i64) -> Option<Value> {
    let lists = ask("keyword_lists", json!({ "project_id": project }))?;
    let mut rows: Vec<&Value> = lists.as_array()?.iter().collect();
    rows.sort_by_key(|list| {
        std::cmp::Reverse(
            list.get("keywords")
                .and_then(Value::as_u64)
                .unwrap_or_default(),
        )
    });
    let filled: Vec<Value> = rows
        .into_iter()
        .take(report::research::LISTS_SHOWN)
        .filter_map(|list| {
            let id = list.get("id")?.as_i64()?;
            Some(json!({
                "name": list.get("name"),
                "keywords": ask("list_keywords", json!({ "list_id": id }))?,
            }))
        })
        .collect();
    (!filled.is_empty()).then_some(Value::Array(filled))
}

/// The project's latest snapshot against its first (or, `previous`, the one before it).
fn compared(workspace: &Workspace, report: &ReportRow, previous: bool) -> Option<Compared> {
    let ask = CompareAsk {
        project_id: report.project_id,
        from: None,
        to: None,
        lang: Some(report.spec.language.clone()),
        previous,
    };
    snapshots::compared(workspace, &ask)
        .inspect_err(|error| tracing::warn!(report = report.id, %error, "snapshots not compared"))
        .ok()
        .flatten()
}

/// A report drawn now, as a page.
fn draw(workspace: &Arc<Workspace>, report: &ReportRow) -> Result<report::ReportView, AppError> {
    let (project, branding) = {
        let store = workspace.store()?;
        (store.project(report.project_id)?, store.report_branding()?)
    };
    let data = sources(workspace, report);
    let period = (day(report.spec.days.saturating_sub(1)), day(0));
    Ok(report::build(
        report,
        &project,
        branding,
        &data,
        (period, day(0)),
    ))
}

/// A file name from a report's name (`aylik-rapor-2026-10-21.pdf`).
fn file_name(report: &ReportRow, extension: &str) -> String {
    let mut slug = String::new();
    for c in report.spec.name.to_lowercase().chars() {
        let c = match c {
            'ç' => 'c',
            'ğ' => 'g',
            'ı' | 'í' => 'i',
            'ö' | 'ó' => 'o',
            'ş' | 'ß' => 's',
            'ü' | 'ú' => 'u',
            'ä' | 'á' | 'à' => 'a',
            'é' | 'è' | 'ê' => 'e',
            'ñ' => 'n',
            other => other,
        };
        if c.is_ascii_alphanumeric() {
            slug.push(c);
        } else if !slug.ends_with('-') && !slug.is_empty() {
            slug.push('-');
        }
    }
    let slug = slug.trim_end_matches('-');
    let slug = if slug.is_empty() { "report" } else { slug };
    format!("{slug}-{}.{extension}", day(0))
}

impl Workspace {
    /// Whether PDFs can be printed here.
    fn can_print(&self) -> bool {
        self.printer.is_some() || octopuson_render::find_browser(None).is_some()
    }

    /// Prints a page to PDF.
    fn print(&self, html: &str) -> Result<Vec<u8>, AppError> {
        match &self.printer {
            Some(printer) => printer(html).map_err(AppError::Unavailable),
            None => octopuson_render::print_pdf(html, None).map_err(|error| match error {
                octopuson_render::RenderError::BrowserMissing => AppError::Unsupported(
                    "install Chrome, Chromium or Edge to print reports as PDF".to_owned(),
                ),
                other => AppError::Unavailable(other.to_string()),
            }),
        }
    }

    /// Meters a PDF on the web, once the gateway prices PDFs; free otherwise.
    fn meter_pdf(&self, key: &str) -> Result<(), AppError> {
        if self.host != Host::Web || self.meter_price(PDF_SKU)?.is_none() {
            return Ok(());
        }
        match self.meter(PDF_SKU, 1, key)? {
            Metered::Charged(_) => Ok(()),
            Metered::NoCredits { .. } => Err(AppError::Quota(
                "the month's free PDFs are used and your credits do not cover another".to_owned(),
            )),
        }
    }
}

/// A random link token, 32 bytes in URL-safe base64.
fn new_token() -> Result<String, AppError> {
    let mut bytes = [0_u8; 32];
    getrandom::fill(&mut bytes)
        .map_err(|_| AppError::Internal("no secure random numbers available".to_owned()))?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}

// ----- Commands ----------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct ProjectAsk {
    project_id: i64,
}

#[derive(Debug, Deserialize)]
struct ReportAsk {
    project_id: i64,
    report_id: i64,
}

/// What the Reports page needs.
#[derive(Debug, Serialize)]
struct ReportsSetup {
    reports: Vec<ReportRow>,
    branding: Branding,
    /// Parts a report can show, in order.
    widgets: Vec<&'static str>,
    /// Designs a report can have, the default first.
    designs: Vec<&'static str>,
    /// PDFs can be printed here.
    can_print: bool,
    /// Read-only links can be made (web).
    can_share: bool,
    /// Reports can be e-mailed (web with SMTP).
    can_email: bool,
    /// The web version's address, for share links.
    public_url: Option<String>,
}

fn reports_setup(context: &Context<'_>, ask: ProjectAsk) -> Result<ReportsSetup, AppError> {
    let workspace = context.workspace;
    let (reports, branding) = {
        let store = workspace.store()?;
        store.project(ask.project_id)?;
        (store.reports(ask.project_id)?, store.report_branding()?)
    };
    Ok(ReportsSetup {
        reports,
        branding,
        widgets: REPORT_WIDGETS.to_vec(),
        designs: REPORT_DESIGNS.to_vec(),
        can_print: workspace.can_print(),
        can_share: workspace.host() == Host::Web,
        can_email: workspace.mailer.is_some(),
        public_url: workspace.public_url.clone(),
    })
}

#[derive(Debug, Deserialize)]
struct BrandingAsk {
    branding: Branding,
}

fn save_report_branding(context: &Context<'_>, ask: BrandingAsk) -> Result<Branding, AppError> {
    Ok(context
        .workspace
        .store()?
        .save_report_branding(&ask.branding)?)
}

#[derive(Debug, Deserialize)]
struct SaveAsk {
    project_id: i64,
    #[serde(default)]
    report_id: Option<i64>,
    #[serde(flatten)]
    spec: ReportSpec,
}

fn save_report(context: &Context<'_>, ask: SaveAsk) -> Result<ReportRow, AppError> {
    let workspace = context.workspace;
    if ask.spec.schedule != "manual" && workspace.mailer.is_none() {
        return Err(AppError::Unsupported(match workspace.host() {
            Host::Desktop => "reports are e-mailed by the web version only".to_owned(),
            Host::Web => "e-mail is not set up on this server".to_owned(),
        }));
    }
    Ok(workspace
        .store()?
        .save_report(ask.project_id, ask.report_id, &ask.spec, &now())?)
}

fn delete_report(context: &Context<'_>, ask: ReportAsk) -> Result<(), AppError> {
    Ok(context
        .workspace
        .store()?
        .delete_report(ask.project_id, ask.report_id)?)
}

#[derive(Debug, Serialize)]
struct Page {
    name: String,
    html: String,
}

fn report_html(context: &Context<'_>, ask: ReportAsk) -> Result<Page, AppError> {
    let report = context
        .workspace
        .store()?
        .report(ask.project_id, ask.report_id)?;
    let view = draw(context.workspace, &report)?;
    Ok(Page {
        name: file_name(&report, "html"),
        html: report::html::render(&view),
    })
}

#[derive(Debug, Deserialize)]
struct PreviewAsk {
    project_id: i64,
    /// The report being edited; none for a new one.
    #[serde(default)]
    report_id: Option<i64>,
    #[serde(flatten)]
    spec: ReportSpec,
}

/// A report drawn with settings not saved yet (the edit dialog's preview); nothing is stored.
fn report_preview(context: &Context<'_>, ask: PreviewAsk) -> Result<Page, AppError> {
    let workspace = context.workspace;
    let report = {
        let store = workspace.store()?;
        let (id, now_at) = if let Some(id) = ask.report_id {
            let saved = store.report(ask.project_id, id)?;
            (saved.id, saved.updated_at)
        } else {
            store.project(ask.project_id)?;
            (0, now())
        };
        ReportRow {
            id,
            project_id: ask.project_id,
            spec: ask.spec,
            share_token: None,
            share_views: 0,
            last_sent_at: None,
            created_at: now_at.clone(),
            updated_at: now_at,
        }
    };
    let view = draw(workspace, &report)?;
    Ok(Page {
        name: file_name(&report, "html"),
        html: report::html::render(&view),
    })
}

/// A file for the page to save, base64.
#[derive(Debug, Serialize)]
struct File {
    name: String,
    content_type: &'static str,
    data: String,
}

fn report_pdf(context: &Context<'_>, ask: ReportAsk) -> Result<File, AppError> {
    let workspace = context.workspace;
    let report = workspace.store()?.report(ask.project_id, ask.report_id)?;
    let html = report::html::render(&draw(workspace, &report)?);
    workspace.meter_pdf(&format!("pdf:{}:{}", report.id, now()))?;
    let pdf = workspace.print(&html)?;
    Ok(File {
        name: file_name(&report, "pdf"),
        content_type: "application/pdf",
        data: STANDARD.encode(pdf),
    })
}

fn report_xlsx(context: &Context<'_>, ask: ReportAsk) -> Result<File, AppError> {
    let workspace = context.workspace;
    let report = workspace.store()?.report(ask.project_id, ask.report_id)?;
    let view = draw(workspace, &report)?;
    let bytes =
        report::xlsx::workbook(&view).map_err(|error| AppError::Internal(error.to_string()))?;
    Ok(File {
        name: file_name(&report, "xlsx"),
        content_type: "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        data: STANDARD.encode(bytes),
    })
}

fn share_report(context: &Context<'_>, ask: ReportAsk) -> Result<ReportRow, AppError> {
    if context.workspace.host() != Host::Web {
        return Err(AppError::Unsupported(
            "read-only links are made by the web version".to_owned(),
        ));
    }
    let token = new_token()?;
    Ok(context
        .workspace
        .store()?
        .set_report_share(ask.project_id, ask.report_id, Some(&token))?)
}

fn unshare_report(context: &Context<'_>, ask: ReportAsk) -> Result<ReportRow, AppError> {
    Ok(context
        .workspace
        .store()?
        .set_report_share(ask.project_id, ask.report_id, None)?)
}

fn report_deliveries(context: &Context<'_>, ask: ReportAsk) -> Result<Vec<DeliveryRow>, AppError> {
    let store = context.workspace.store()?;
    store.report(ask.project_id, ask.report_id)?;
    Ok(store.report_deliveries(ask.report_id, DELIVERIES_SHOWN)?)
}

// ----- E-mail ------------------------------------------------------------------------------

/// Sends a report to its recipients: a PDF when one can be printed, else the page itself.
fn deliver(
    workspace: &Arc<Workspace>,
    report: &ReportRow,
    scheduled: bool,
) -> Result<DeliveryRow, AppError> {
    let mailer = workspace
        .mailer
        .clone()
        .ok_or_else(|| AppError::Unsupported("e-mail is not set up here".to_owned()))?;
    let lang = report.spec.language.as_str();
    let view = draw(workspace, report)?;
    let html = report::html::render(&view);
    let attachment = if workspace.can_print() {
        workspace
            .meter_pdf(&format!("mail:{}:{}", report.id, now()))
            .and_then(|()| workspace.print(&html))
            .map(|pdf| Attachment {
                name: file_name(report, "pdf"),
                content_type: "application/pdf".to_owned(),
                bytes: pdf,
            })
            .ok()
    } else {
        None
    };
    let printed = attachment.is_some();
    let attachment = attachment.unwrap_or_else(|| Attachment {
        name: file_name(report, "html"),
        content_type: "text/html; charset=utf-8".to_owned(),
        bytes: html.into_bytes(),
    });
    let title = format!("{} · {}", report.spec.name, view.domain);
    let mut text = texts::fill(
        lang,
        if printed {
            "email_attached"
        } else {
            "email_html_attached"
        },
        &[("name", &title)],
    );
    if let (Some(token), Some(base)) = (&report.share_token, &workspace.public_url) {
        text.push_str("\n\n");
        text.push_str(&texts::fill(
            lang,
            "email_online",
            &[("link", &format!("{base}/r/{token}"))],
        ));
    }
    let reply_to = workspace
        .account_status()
        .ok()
        .and_then(|status| status.account)
        .and_then(|account| account.email);
    let error = if report.spec.recipients.is_empty() {
        Some("the report has no recipients".to_owned())
    } else {
        mailer
            .send(&Mail {
                to: report.spec.recipients.clone(),
                reply_to,
                subject: title,
                text,
                attachment: Some(attachment),
            })
            .err()
    };
    Ok(workspace
        .store()?
        .add_report_delivery(report, scheduled, error.as_deref(), &now())?)
}

fn start(
    workspace: &Arc<Workspace>,
    jobs: &JobManager,
    report: ReportRow,
    scheduled: bool,
) -> Result<String, AppError> {
    if workspace.mailer.is_none() {
        return Err(AppError::Unsupported(
            "reports are e-mailed by the web version when e-mail is set up".to_owned(),
        ));
    }
    if report.spec.recipients.is_empty() {
        return Err(AppError::Invalid(
            "add the report's recipients first".to_owned(),
        ));
    }
    let workspace = Arc::clone(workspace);
    jobs.start(
        JOB_KIND,
        &report.spec.name.clone(),
        Some(report.project_id),
        move |control| {
            control.progress("", 0, 1);
            let delivery = deliver(&workspace, &report, scheduled)?;
            if let Some(error) = &delivery.error {
                return Err(AppError::Unavailable(error.clone()));
            }
            Ok(JobDone::with(delivery))
        },
    )
}

fn send_report(context: &Context<'_>, ask: ReportAsk) -> Result<String, AppError> {
    let report = context
        .workspace
        .store()?
        .report(ask.project_id, ask.report_id)?;
    start(context.workspace, context.jobs, report, false)
}

/// Seconds before a report sent after each snapshot looks again while one is being taken.
const SNAPSHOT_WAIT: i64 = 600;

/// When a report's e-mail is due, seconds since the Unix epoch.
fn due_at(workspace: &Workspace, report: &ReportRow) -> Result<Option<i64>, AppError> {
    let interval = match report.spec.schedule.as_str() {
        "weekly" => 6 * 86_400 + 12 * 3_600,
        "monthly" => 29 * 86_400,
        "snapshot" => return after_snapshot(workspace, report),
        _ => return Ok(None),
    };
    if report.spec.recipients.is_empty() {
        return Ok(None);
    }
    Ok(Some(
        report
            .last_sent_at
            .as_deref()
            .and_then(seconds_of)
            .map_or(0, |last| last + interval),
    ))
}

/// When a report sent after each new snapshot is due: when a snapshot finished since it was
/// last sent or saved (and there is one before it to compare with); a little later while one is
/// being taken; else not until one is.
fn after_snapshot(workspace: &Workspace, report: &ReportRow) -> Result<Option<i64>, AppError> {
    if report.spec.recipients.is_empty() {
        return Ok(None);
    }
    let rows = workspace.store()?.snapshots(report.project_id)?;
    let since = [
        report.last_sent_at.as_deref(),
        Some(report.updated_at.as_str()),
    ]
    .into_iter()
    .flatten()
    .filter_map(seconds_of)
    .max()
    .unwrap_or(0);
    let finished: Vec<i64> = rows
        .iter()
        .filter(|row| matches!(row.status.as_str(), "done" | "partial"))
        .filter_map(|row| row.finished_at.as_deref().and_then(seconds_of))
        .collect();
    if finished.len() > 1
        && let Some(latest) = finished.iter().copied().max().filter(|at| *at > since)
    {
        return Ok(Some(latest));
    }
    let now_secs = unix_now();
    let taking = rows.iter().any(|row| {
        row.status == "running"
            && seconds_of(&row.taken_at).is_some_and(|at| now_secs - at < 6 * 3_600)
    });
    Ok(taking.then_some(now_secs + SNAPSHOT_WAIT))
}

/// Starts the report e-mails that are due; returns the jobs started.
///
/// # Errors
/// Storage errors.
pub fn run_due(workspace: &Arc<Workspace>, jobs: &JobManager) -> Result<Vec<String>, AppError> {
    if workspace.mailer.is_none() {
        return Ok(Vec::new());
    }
    let now_secs = unix_now();
    let plans = workspace.store()?.report_plans()?;
    let mut due: Vec<ReportRow> = Vec::new();
    for report in plans {
        if due_at(workspace, &report)?.is_some_and(|at| at <= now_secs) {
            due.push(report);
        }
    }
    let mut started = Vec::new();
    for report in due {
        let id = report.id;
        match start(workspace, jobs, report, true) {
            Ok(job) => started.push(job),
            Err(error) => {
                tracing::warn!(report = id, %error, "a scheduled report e-mail did not start");
            }
        }
    }
    Ok(started)
}

/// When the next report e-mail is due, seconds since the Unix epoch.
///
/// # Errors
/// Storage errors.
pub fn next_due(workspace: &Workspace) -> Result<Option<i64>, AppError> {
    if workspace.mailer.is_none() {
        return Ok(None);
    }
    let plans = workspace.store()?.report_plans()?;
    let mut next: Option<i64> = None;
    for report in &plans {
        if let Some(at) = due_at(workspace, report)? {
            next = Some(next.map_or(at, |next| next.min(at)));
        }
    }
    Ok(next)
}

/// The page a read-only link shows, drawn now; `None` for a token that opens nothing.
///
/// # Errors
/// Storage errors.
pub fn shared_report_html(
    workspace: &Arc<Workspace>,
    token: &str,
) -> Result<Option<String>, AppError> {
    let Some(report) = workspace.store()?.open_shared_report(token)? else {
        return Ok(None);
    };
    let view = draw(workspace, &report)?;
    Ok(Some(report::html::render(&view)))
}

/// Adds the commands.
pub fn register(router: &mut Router) {
    router.add("reports_setup", reports_setup);
    router.add("save_report_branding", save_report_branding);
    router.add("save_report", save_report);
    router.add("delete_report", delete_report);
    router.add("report_html", report_html);
    router.add("report_preview", report_preview);
    router.add("report_pdf", report_pdf);
    router.add("report_xlsx", report_xlsx);
    router.add("share_report", share_report);
    router.add("unshare_report", unshare_report);
    router.add("report_deliveries", report_deliveries);
    router.add("send_report", send_report);
}
