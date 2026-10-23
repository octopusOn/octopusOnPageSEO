//! Snapshots (ADR 0080): a project's whole state kept at a moment, taken by hand or on a
//! schedule, and two of them compared — where the site stood when the work began and where it
//! stands now — with a written summary.

use std::sync::Arc;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use octopuson_platform::{SnapshotEnd, SnapshotRow, SnapshotSettings, unix_now};
use octopuson_serp_score::rank::seconds_of;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::jobs::{JobDone, JobManager, JobStatus};
use crate::report;
use crate::router::{Context, NoArgs, Router};
use crate::snapshot::compare::{self, Comparison, Side};
use crate::snapshot::summary::{self, Sentence};
use crate::snapshot::{self, Headline, SnapshotData, SnapshotEstimate};
use crate::{AppError, Workspace, now};

/// Kind of the job taking a snapshot.
pub const JOB_KIND: &str = "snapshot";

/// Commands after which a snapshot may be due at another time.
const SCHEDULE_COMMANDS: &[&str] = &["save_snapshot_settings", "start_snapshot", "delete_project"];

/// Whether a command may move when a snapshot is due next.
pub fn changes_schedule(command: &str) -> bool {
    SCHEDULE_COMMANDS.contains(&command)
}

#[derive(Debug, Deserialize)]
struct ProjectAsk {
    project_id: i64,
}

/// A snapshot as listed: with its main numbers once finished.
#[derive(Debug, Serialize)]
struct Listed {
    #[serde(flatten)]
    row: SnapshotRow,
    headline: Option<Headline>,
}

/// A project's snapshots, newest first, with their main numbers.
fn listed(workspace: &Workspace, project_id: i64) -> Result<Vec<Listed>, AppError> {
    let store = workspace.store()?;
    let rows = store.snapshots(project_id)?;
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        let headline = if finished(&row) {
            store
                .snapshot(project_id, row.id)?
                .data
                .and_then(|data| serde_json::from_value::<SnapshotData>(data).ok())
                .map(|data| data.headline())
        } else {
            None
        };
        out.push(Listed { row, headline });
    }
    Ok(out)
}

/// What the Snapshots page shows first.
#[derive(Debug, Serialize)]
struct Setup {
    settings: SnapshotSettings,
    /// The project's competitors to choose from.
    competitors: Vec<String>,
    /// Newest first.
    snapshots: Vec<Listed>,
    /// The first finished snapshot: where the work began.
    baseline_id: Option<i64>,
    /// When the next one is taken on its own, seconds since the Unix epoch.
    next_at: Option<i64>,
}

/// Whether a snapshot can be compared.
fn finished(row: &SnapshotRow) -> bool {
    matches!(row.status.as_str(), "done" | "partial")
}

fn snapshot_setup(context: &Context<'_>, ask: ProjectAsk) -> Result<Setup, AppError> {
    let (project, settings) = {
        let store = context.workspace.store()?;
        (
            store.project(ask.project_id)?,
            store.snapshot_settings(ask.project_id)?,
        )
    };
    let snapshots = listed(context.workspace, ask.project_id)?;
    let baseline_id = snapshots
        .iter()
        .rev()
        .find(|listed| finished(&listed.row))
        .map(|listed| listed.row.id);
    Ok(Setup {
        next_at: due_at(&settings),
        settings,
        competitors: project.competitors,
        snapshots,
        baseline_id,
    })
}

#[derive(Debug, Deserialize)]
struct SettingsAsk {
    project_id: i64,
    settings: SnapshotSettings,
}

fn save_snapshot_settings(
    context: &Context<'_>,
    ask: SettingsAsk,
) -> Result<SnapshotSettings, AppError> {
    Ok(context
        .workspace
        .store()?
        .save_snapshot_settings(ask.project_id, &ask.settings)?)
}

#[derive(Debug, Deserialize)]
struct TakeAsk {
    project_id: i64,
    /// Competitors to keep too; the saved choice when absent.
    #[serde(default)]
    competitors: Option<Vec<String>>,
    #[serde(default)]
    label: Option<String>,
}

/// The competitors a snapshot keeps: those asked for, else the saved choice, among the
/// project's own.
fn chosen(
    workspace: &Workspace,
    project_id: i64,
    asked: Option<Vec<String>>,
) -> Result<Vec<String>, AppError> {
    let store = workspace.store()?;
    let project = store.project(project_id)?;
    let wanted = match asked {
        Some(list) => list,
        None => store.snapshot_settings(project_id)?.competitors,
    };
    Ok(wanted
        .into_iter()
        .filter(|domain| project.competitors.contains(domain))
        .collect())
}

fn snapshot_estimate(context: &Context<'_>, ask: TakeAsk) -> Result<SnapshotEstimate, AppError> {
    let competitors = chosen(context.workspace, ask.project_id, ask.competitors)?;
    snapshot::estimate(context.workspace, ask.project_id, &competitors)
}

/// Starts taking a snapshot as a job; its id.
fn start(
    workspace: &Arc<Workspace>,
    jobs: &JobManager,
    project_id: i64,
    scheduled: bool,
    label: Option<String>,
    competitors: Vec<String>,
) -> Result<String, AppError> {
    if let Some(job) = jobs.running_for(JOB_KIND, Some(project_id)) {
        return Ok(job);
    }
    // Begun before the job runs, so what waits for snapshots (reports sent after each) sees
    // it at once.
    let (project, id) = {
        let store = workspace.store()?;
        let project = store.project(project_id)?;
        let id = store.begin_snapshot(project_id, scheduled, label.as_deref(), &now())?;
        (project, id)
    };
    let shared = Arc::clone(workspace);
    let started = jobs.start(JOB_KIND, &project.name, Some(project_id), move |control| {
        let workspace = shared;
        let taken = snapshot::take(
            &workspace,
            project_id,
            &competitors,
            &|step, done, total| control.progress(step, done, total),
            &|| control.cancelled(),
        );
        let (status, cost, error, data) = match &taken {
            Ok(taken) => {
                let data = &taken.data;
                let nothing = data.overview.is_none()
                    && data.links.is_none()
                    && data.rank.is_none()
                    && data.audit.is_none()
                    && data.ai.is_none()
                    && data.gsc.is_none();
                let reasons: Vec<String> = data
                    .missing
                    .iter()
                    .map(|missing| missing.reason.clone())
                    .collect::<std::collections::BTreeSet<_>>()
                    .into_iter()
                    .collect();
                let status = if nothing {
                    "failed"
                } else if data.missing.is_empty() {
                    "done"
                } else {
                    "partial"
                };
                (
                    status,
                    taken.spent_usd,
                    (!reasons.is_empty()).then(|| reasons.join("; ")),
                    serde_json::to_value(data).unwrap_or_default(),
                )
            }
            Err(error) => (
                if control.cancelled() {
                    "interrupted"
                } else {
                    "failed"
                },
                0.0,
                Some(error.to_string()),
                serde_json::Value::Null,
            ),
        };
        workspace.store()?.finish_snapshot(
            id,
            &SnapshotEnd {
                status,
                cost_usd: cost,
                error: error.as_deref(),
                data: &data,
            },
            &now(),
        )?;
        taken?;
        Ok(JobDone {
            status: if status == "done" {
                JobStatus::Done
            } else {
                JobStatus::Partial
            },
            result: Some(json!({ "snapshot_id": id, "status": status, "spent_usd": cost })),
        })
    });
    if let Err(error) = &started {
        workspace.store()?.finish_snapshot(
            id,
            &SnapshotEnd {
                status: "failed",
                cost_usd: 0.0,
                error: Some(&error.to_string()),
                data: &serde_json::Value::Null,
            },
            &now(),
        )?;
    }
    started
}

fn start_snapshot(context: &Context<'_>, ask: TakeAsk) -> Result<String, AppError> {
    let competitors = chosen(context.workspace, ask.project_id, ask.competitors)?;
    start(
        context.workspace,
        context.jobs,
        ask.project_id,
        false,
        ask.label,
        competitors,
    )
}

fn list_snapshots(context: &Context<'_>, ask: ProjectAsk) -> Result<Vec<Listed>, AppError> {
    listed(context.workspace, ask.project_id)
}

#[derive(Debug, Deserialize)]
struct SnapshotAsk {
    project_id: i64,
    snapshot_id: i64,
}

fn get_snapshot(context: &Context<'_>, ask: SnapshotAsk) -> Result<SnapshotRow, AppError> {
    Ok(context
        .workspace
        .store()?
        .snapshot(ask.project_id, ask.snapshot_id)?)
}

fn delete_snapshot(context: &Context<'_>, ask: SnapshotAsk) -> Result<(), AppError> {
    Ok(context
        .workspace
        .store()?
        .delete_snapshot(ask.project_id, ask.snapshot_id)?)
}

#[derive(Debug, Deserialize)]
pub(crate) struct CompareAsk {
    pub(crate) project_id: i64,
    /// The earlier snapshot; the first finished one (where the work began) when absent.
    #[serde(default)]
    pub(crate) from: Option<i64>,
    /// The later snapshot; the latest finished one when absent.
    #[serde(default)]
    pub(crate) to: Option<i64>,
    /// Language of the summary.
    #[serde(default)]
    pub(crate) lang: Option<String>,
    /// With `from` absent: the snapshot before `to` instead of the first.
    #[serde(default)]
    pub(crate) previous: bool,
}

/// A comparison with its summary.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct Compared {
    #[serde(flatten)]
    pub(crate) comparison: Comparison,
    pub(crate) summary: Vec<Sentence>,
}

/// Two snapshots of a project compared; `None` with fewer than two finished.
pub(crate) fn compared(
    workspace: &Workspace,
    ask: &CompareAsk,
) -> Result<Option<Compared>, AppError> {
    let store = workspace.store()?;
    let rows: Vec<SnapshotRow> = store
        .snapshots(ask.project_id)?
        .into_iter()
        .filter(finished)
        .collect();
    let to_id = match ask.to {
        Some(id) => id,
        None => match rows.first() {
            Some(row) => row.id,
            None => return Ok(None),
        },
    };
    let from_id = if let Some(id) = ask.from {
        id
    } else {
        let older = rows.iter().skip_while(|row| row.id != to_id).skip(1);
        let found = if ask.previous {
            older.clone().next()
        } else {
            older.last()
        };
        let Some(row) = found else {
            return Ok(None);
        };
        row.id
    };
    if from_id == to_id {
        return Err(AppError::Invalid(
            "choose two different snapshots".to_owned(),
        ));
    }
    let load = |id: i64| -> Result<(Side, SnapshotData), AppError> {
        let row = store.snapshot(ask.project_id, id)?;
        let data: SnapshotData = row
            .data
            .map(serde_json::from_value)
            .transpose()
            .map_err(|error| AppError::Internal(format!("snapshot {id}: {error}")))?
            .unwrap_or_default();
        Ok((
            Side {
                id: row.id,
                label: row.label,
                taken_at: row.taken_at,
            },
            data,
        ))
    };
    let (mut from, mut to) = (load(from_id)?, load(to_id)?);
    // The earlier one is always "before".
    if to.0.taken_at < from.0.taken_at {
        std::mem::swap(&mut from, &mut to);
    }
    let comparison = compare::compare((&from.0, &from.1), (&to.0, &to.1));
    let lang = ask.lang.as_deref().unwrap_or("tr");
    Ok(Some(Compared {
        summary: summary::summarize(&comparison, lang),
        comparison,
    }))
}

fn compare_snapshots(context: &Context<'_>, ask: CompareAsk) -> Result<Option<Compared>, AppError> {
    compared(context.workspace, &ask)
}

/// How far a project has come since its first snapshot.
#[derive(Debug, Serialize)]
struct Progress {
    project_id: i64,
    from: Side,
    to: Side,
    days: i64,
    /// Traffic, keywords, authority, site health and AI mentions, where kept.
    kpis: Vec<compare::Kpi>,
}

/// Numbers shown on the projects' cards.
const PROGRESS_KPIS: &[&str] = &["traffic", "keywords", "oa", "health", "mention_rate"];

/// Every project with two finished snapshots: its first against its latest.
fn snapshot_progress(context: &Context<'_>, _: NoArgs) -> Result<Vec<Progress>, AppError> {
    let projects = context.workspace.store()?.projects()?;
    let mut out = Vec::new();
    for project in projects {
        let ask = CompareAsk {
            project_id: project.id,
            from: None,
            to: None,
            lang: None,
            previous: false,
        };
        let Some(compared) = compared(context.workspace, &ask)? else {
            continue;
        };
        let comparison = compared.comparison;
        out.push(Progress {
            project_id: project.id,
            kpis: PROGRESS_KPIS
                .iter()
                .filter_map(|key| comparison.kpis.iter().find(|kpi| kpi.key == *key))
                .cloned()
                .collect(),
            from: comparison.from,
            to: comparison.to,
            days: comparison.days,
        });
    }
    Ok(out)
}

/// A file for the page to save, base64.
#[derive(Debug, Serialize)]
struct File {
    name: String,
    content_type: &'static str,
    data: String,
}

/// A comparison as a workbook: the summary and headline numbers first, then a sheet per list.
fn snapshot_xlsx(context: &Context<'_>, ask: CompareAsk) -> Result<File, AppError> {
    let workspace = context.workspace;
    let mut ask = ask;
    let lang = ask.lang.get_or_insert_with(|| "tr".to_owned()).clone();
    let compared = compared(workspace, &ask)?
        .ok_or_else(|| AppError::Invalid("comparing needs two finished snapshots".to_owned()))?;
    let (project, branding) = {
        let store = workspace.store()?;
        (store.project(ask.project_id)?, store.report_branding()?)
    };
    let mut widget = report::comparison::widget("comparison", Some(&compared), &lang);
    widget.title = report::texts::text(&lang, "snapshot_comparison");
    let day = |at: &str| at.get(..10).unwrap_or(at).to_owned();
    let period = (
        day(&compared.comparison.from.taken_at),
        day(&compared.comparison.to.taken_at),
    );
    let name = format!(
        "snapshots-{}-{}-{}.xlsx",
        project.domain, period.0, period.1
    );
    let view = report::ReportView {
        name: widget.title.clone(),
        language: lang,
        design: octopuson_platform::DEFAULT_DESIGN.to_owned(),
        colors: octopuson_platform::ReportColors::default(),
        project: project.name,
        domain: project.domain,
        period,
        generated: day(&now()),
        branding,
        widgets: vec![widget],
    };
    let bytes =
        report::xlsx::workbook(&view).map_err(|error| AppError::Internal(error.to_string()))?;
    Ok(File {
        name,
        content_type: "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        data: STANDARD.encode(bytes),
    })
}

/// When a project's next snapshot is due, seconds since the Unix epoch.
fn due_at(settings: &SnapshotSettings) -> Option<i64> {
    let interval = match settings.schedule.as_str() {
        "weekly" => 6 * 86_400 + 12 * 3_600,
        "monthly" => 29 * 86_400,
        _ => return None,
    };
    Some(
        settings
            .last_run_at
            .as_deref()
            .and_then(seconds_of)
            .map_or(0, |last| last + interval),
    )
}

/// Starts the snapshots that are due; returns the jobs started.
///
/// # Errors
/// Storage errors.
pub fn run_due(workspace: &Arc<Workspace>, jobs: &JobManager) -> Result<Vec<String>, AppError> {
    let now_secs = unix_now();
    let due: Vec<(i64, SnapshotSettings)> = workspace
        .store()?
        .snapshot_plans()?
        .into_iter()
        .filter(|(_, settings)| due_at(settings).is_some_and(|at| at <= now_secs))
        .collect();
    let mut started = Vec::new();
    for (project_id, settings) in due {
        let job = chosen(workspace, project_id, Some(settings.competitors))
            .and_then(|competitors| start(workspace, jobs, project_id, true, None, competitors));
        match job {
            Ok(job) => started.push(job),
            Err(error) => {
                tracing::warn!(project = project_id, %error, "a scheduled snapshot did not start");
            }
        }
    }
    Ok(started)
}

/// When the next snapshot of any project is due.
///
/// # Errors
/// Storage errors.
pub fn next_due(workspace: &Workspace) -> Result<Option<i64>, AppError> {
    Ok(workspace
        .store()?
        .snapshot_plans()?
        .iter()
        .filter_map(|(_, settings)| due_at(settings))
        .min())
}

/// Adds the commands.
pub fn register(router: &mut Router) {
    router.add("snapshot_setup", snapshot_setup);
    router.add("save_snapshot_settings", save_snapshot_settings);
    router.add("snapshot_estimate", snapshot_estimate);
    router.add("start_snapshot", start_snapshot);
    router.add("list_snapshots", list_snapshots);
    router.add("get_snapshot", get_snapshot);
    router.add("delete_snapshot", delete_snapshot);
    router.add("compare_snapshots", compare_snapshots);
    router.add("snapshot_xlsx", snapshot_xlsx);
    router.add("snapshot_progress", snapshot_progress);
}
