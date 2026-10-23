//! The commands, one file per module. Each module adds its commands to the router in
//! `register`; [`router`] collects them all.

// Every command has the router's shape, `fn(&Context, Arguments) -> Result<Answer, AppError>`:
// arguments arrive owned from JSON and a command that cannot fail still returns a `Result`.
#![allow(clippy::needless_pass_by_value, clippy::unnecessary_wraps)]

pub mod account;
pub mod ai;
pub mod alerts;
pub mod app;
pub mod audit;
pub mod backlinks;
pub mod content;
pub mod crawl_views;
pub mod gap;
pub mod gsc;
pub mod history;
pub mod jobs;
pub mod keywords;
pub mod links;
pub mod local;
pub mod opportunities;
pub mod overview;
pub mod project;
pub mod projects;
pub mod rank;
pub mod reports;
pub mod research;
pub mod snapshots;
pub mod sources;
pub mod sync;
pub mod vitals;

use std::sync::Arc;

use crate::jobs::JobManager;
use crate::router::Router;
use crate::{AppError, Workspace};

/// Every command of the platform.
pub fn router() -> Router {
    let mut router = Router::new();
    app::register(&mut router);
    projects::register(&mut router);
    sources::register(&mut router);
    account::register(&mut router);
    jobs::register(&mut router);
    overview::register(&mut router);
    opportunities::register(&mut router);
    project::register(&mut router);
    keywords::register(&mut router);
    backlinks::register(&mut router);
    gap::register(&mut router);
    links::register(&mut router);
    history::register(&mut router);
    rank::register(&mut router);
    local::register(&mut router);
    content::register(&mut router);
    audit::register(&mut router);
    crawl_views::register(&mut router);
    vitals::register(&mut router);
    ai::register(&mut router);
    gsc::register(&mut router);
    reports::register(&mut router);
    alerts::register(&mut router);
    research::register(&mut router);
    snapshots::register(&mut router);
    sync::register(&mut router);
    router
}

/// Whether a command only reads: it cannot change what a sync carries, so it does not make the
/// desktop sync sooner. Unsure ones count as changes.
pub fn reads_only(command: &str) -> bool {
    const READS: &[&str] = &[
        "account_page",
        "account_status",
        "ai_answer",
        "app_info",
        "audit_findings_csv",
        "audit_hreflang",
        "audit_issue",
        "audit_links",
        "audit_outbound",
        "audit_page",
        "audit_pages",
        "disavow_file",
        "cancel_job",
        "clear_jobs",
        "content_brief",
        "content_briefs",
        "content_pages",
        "google_properties",
        "google_status",
        "gsc_export",
        "gsc_query",
        "keyword_autocomplete",
        "keyword_lists",
        "link_decisions",
        "link_prospects",
        "list_alerts",
        "opportunities",
        "project_dashboard",
        "metric_history",
        "open_url",
        "local_grid",
        "local_reviews",
        "local_runs",
        "local_setup",
        "rank_keyword",
        "rank_places",
        "report_deliveries",
        "report_html",
        "report_pdf",
        "report_problem",
        "report_xlsx",
        "serp_clusters",
        "suggest_ai_prompts",
        "sync_status",
        "usage",
    ];
    READS.contains(&command)
        || ["get_", "list_"]
            .iter()
            .any(|prefix| command.starts_with(prefix))
        || ["_estimate", "_report", "_setup", "_runs"]
            .iter()
            .any(|suffix| command.ends_with(suffix))
}

/// Whether a command may move when scheduled work is due next (the web server notes it again).
pub fn changes_schedule(command: &str) -> bool {
    rank::changes_schedule(command)
        || local::changes_schedule(command)
        || audit::changes_schedule(command)
        || ai::changes_schedule(command)
        || gsc::changes_schedule(command)
        || reports::changes_schedule(command)
        || snapshots::changes_schedule(command)
}

/// Starts the scheduled work that is due: rank checks, audits, AI runs, Search Console syncs,
/// snapshots and report e-mails.
///
/// # Errors
/// Storage errors.
pub fn run_due(workspace: &Arc<Workspace>, jobs: &JobManager) -> Result<Vec<String>, AppError> {
    let mut started = rank::run_due(workspace, jobs)?;
    started.extend(local::run_due(workspace, jobs)?);
    started.extend(audit::run_due(workspace, jobs)?);
    started.extend(ai::run_due(workspace, jobs)?);
    started.extend(gsc::run_due(workspace, jobs)?);
    started.extend(snapshots::run_due(workspace, jobs)?);
    started.extend(reports::run_due(workspace, jobs)?);
    started.extend(alerts::run_due(workspace, jobs)?);
    Ok(started)
}

/// Reads the answers waiting in the provider's queue: rank checks
/// ([`rank::collect_queued`]), grid points and reviews ([`local::collect_queued`]); each with
/// the kind of note pages get when it read something.
///
/// # Errors
/// Storage errors.
pub fn collect_queued(
    workspace: &Workspace,
) -> Result<Vec<(&'static str, rank::Collected)>, AppError> {
    Ok(vec![
        (rank::QUEUE_KIND, rank::collect_queued(workspace)?),
        (local::QUEUE_KIND, local::collect_queued(workspace)?),
    ])
}

/// When the provider's queue should be read next, seconds since the Unix epoch; `None` with
/// nothing waiting in it.
///
/// # Errors
/// Storage errors.
pub fn next_collect(workspace: &Workspace, jobs: &JobManager) -> Result<Option<i64>, AppError> {
    Ok([
        rank::next_collect(workspace, jobs)?,
        local::next_collect(workspace, jobs)?,
    ]
    .into_iter()
    .flatten()
    .min())
}

/// When scheduled work is due next, seconds since the Unix epoch.
///
/// # Errors
/// Storage errors.
pub fn next_due(workspace: &Workspace) -> Result<Option<i64>, AppError> {
    Ok([
        rank::next_due(workspace)?,
        local::next_due(workspace)?,
        audit::next_due(workspace)?,
        ai::next_due(workspace)?,
        gsc::next_due(workspace)?,
        snapshots::next_due(workspace)?,
        reports::next_due(workspace)?,
        alerts::next_due(workspace)?,
    ]
    .into_iter()
    .flatten()
    .min())
}

/// The market a report is asked for: `country-language` (`de-de`) when given, else the
/// project's.
///
/// # Errors
/// [`AppError::Invalid`] for a market that is not `country-language` or names a country
/// DataForSEO is not asked for.
pub(crate) fn market_of(
    project: &octopuson_platform::ProjectRow,
    market: Option<&str>,
) -> Result<octopuson_dataforseo::Market, AppError> {
    let Some(text) = market.map(str::trim).filter(|text| !text.is_empty()) else {
        return Ok(octopuson_dataforseo::Market::new(
            &project.country,
            &project.language,
        ));
    };
    let invalid = || AppError::Invalid(format!("market: `{text}` is not country-language"));
    let (country, language) = text.split_once('-').ok_or_else(invalid)?;
    let letters = |code: &str, most: usize| {
        (2..=most).contains(&code.len()) && code.bytes().all(|byte| byte.is_ascii_alphabetic())
    };
    if !letters(country, 2) || !letters(language, 3) {
        return Err(invalid());
    }
    let market = octopuson_dataforseo::Market::new(country, language);
    market.location_code()?;
    Ok(market)
}

/// Writes a list report's filters, order and offset into its task; `fields` are the ones the
/// report filters by, none for a report that is not a list. Returns whether the answer may
/// travel to the account's other copies: only whole, unfiltered lists do.
///
/// # Errors
/// [`AppError::Invalid`] for a filter the report does not take, or any query of a report
/// that is not filtered.
pub(crate) fn queried(
    task: &mut serde_json::Value,
    query: Option<&octopuson_dataforseo::query::ListQuery>,
    fields: Option<&[octopuson_dataforseo::query::FieldSpec]>,
) -> Result<bool, AppError> {
    let Some(query) = query.filter(|query| !query.is_empty()) else {
        return Ok(true);
    };
    let fields =
        fields.ok_or_else(|| AppError::Invalid("query: this report is not filtered".to_owned()))?;
    octopuson_dataforseo::query::apply(task, query, fields)?;
    Ok(false)
}
