//! The commands, one file per module. Each module adds its commands to the router in
//! `register`; [`router`] collects them all.

// Every command has the router's shape, `fn(&Context, Arguments) -> Result<Answer, AppError>`:
// arguments arrive owned from JSON and a command that cannot fail still returns a `Result`.
#![allow(clippy::needless_pass_by_value, clippy::unnecessary_wraps)]

pub mod account;
pub mod ai;
pub mod app;
pub mod audit;
pub mod backlinks;
pub mod gap;
pub mod gsc;
pub mod jobs;
pub mod keywords;
pub mod overview;
pub mod projects;
pub mod rank;
pub mod reports;
pub mod sources;
pub mod sync;

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
    keywords::register(&mut router);
    backlinks::register(&mut router);
    gap::register(&mut router);
    rank::register(&mut router);
    audit::register(&mut router);
    ai::register(&mut router);
    gsc::register(&mut router);
    reports::register(&mut router);
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
        "audit_issue",
        "cancel_job",
        "clear_jobs",
        "google_properties",
        "google_status",
        "gsc_query",
        "keyword_autocomplete",
        "keyword_lists",
        "open_url",
        "rank_keyword",
        "report_deliveries",
        "report_html",
        "report_pdf",
        "report_problem",
        "report_xlsx",
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
        || audit::changes_schedule(command)
        || ai::changes_schedule(command)
        || gsc::changes_schedule(command)
        || reports::changes_schedule(command)
}

/// Starts the scheduled work that is due: rank checks, audits, AI runs, Search Console syncs
/// and report e-mails.
///
/// # Errors
/// Storage errors.
pub fn run_due(workspace: &Arc<Workspace>, jobs: &JobManager) -> Result<Vec<String>, AppError> {
    let mut started = rank::run_due(workspace, jobs)?;
    started.extend(audit::run_due(workspace, jobs)?);
    started.extend(ai::run_due(workspace, jobs)?);
    started.extend(gsc::run_due(workspace, jobs)?);
    started.extend(reports::run_due(workspace, jobs)?);
    Ok(started)
}

/// When scheduled work is due next, seconds since the Unix epoch.
///
/// # Errors
/// Storage errors.
pub fn next_due(workspace: &Workspace) -> Result<Option<i64>, AppError> {
    Ok([
        rank::next_due(workspace)?,
        audit::next_due(workspace)?,
        ai::next_due(workspace)?,
        gsc::next_due(workspace)?,
        reports::next_due(workspace)?,
    ]
    .into_iter()
    .flatten()
    .min())
}
