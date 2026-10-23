//! A project's dashboard: the headline numbers of every module with their change and a short
//! series each, the opportunities worth doing first, what the schedule does next and what the
//! project spent this month. Everything comes from what the modules already hold; nothing is
//! paid.

use std::sync::Arc;

use octopuson_platform::ProjectRow;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::opportunities::{Opportunity, evaluate};
use crate::router::{Context, Router};
use crate::snapshot::{LinkPoint, Metrics, ask, links_of, metrics_of};
use crate::{AppError, Workspace, month};

/// Opportunities on the dashboard.
const TOP_OPPORTUNITIES: usize = 10;

/// Points of each series at most.
const SERIES: usize = 60;

/// A dated value.
#[derive(Debug, Clone, Serialize)]
struct Point {
    date: String,
    value: Option<f64>,
}

/// The domain's organic numbers (the kept overview) and their history.
#[derive(Debug, Serialize)]
struct OrganicCard {
    metrics: Metrics,
    fetched_at: String,
    /// Estimated visits, from the days the overview was fetched.
    traffic: Vec<Point>,
    keywords: Vec<Point>,
}

/// The link profile (the kept summary) and its history.
#[derive(Debug, Serialize)]
struct LinksCard {
    links: LinkPoint,
    fetched_at: String,
    referring_domains: Vec<Point>,
}

#[derive(Debug, Serialize)]
struct RankCard {
    visibility: Option<f64>,
    visibility_change: Option<f64>,
    average_position: Option<f64>,
    top3: f64,
    top10: f64,
    ranked: f64,
    checked: f64,
    improved: f64,
    declined: f64,
    lost: f64,
    traffic: f64,
    series: Vec<Point>,
}

#[derive(Debug, Serialize)]
struct SearchCard {
    period: (String, String),
    clicks: f64,
    clicks_before: f64,
    impressions: f64,
    impressions_before: f64,
    ctr: f64,
    position: Option<f64>,
    position_before: Option<f64>,
    series: Vec<Point>,
}

#[derive(Debug, Serialize)]
struct AuditCard {
    audit_id: i64,
    health: Option<f64>,
    health_before: Option<f64>,
    geo: Option<f64>,
    issues: f64,
    issues_before: Option<f64>,
    pages: f64,
    finished_at: Option<String>,
    series: Vec<Point>,
}

#[derive(Debug, Serialize)]
struct AiCard {
    answers: f64,
    mention_rate: f64,
    citation_rate: f64,
    share_of_voice: f64,
    series: Vec<Point>,
}

#[derive(Debug, Serialize)]
struct LocalCard {
    arp: Option<f64>,
    solv: f64,
    top3: f64,
    points: f64,
    series: Vec<Point>,
}

#[derive(Debug, Serialize)]
struct VitalsCard {
    /// Phones, then desktops: the assessment and LCP, INP, CLS.
    devices: Vec<VitalsDevice>,
}

#[derive(Debug, Serialize)]
struct VitalsDevice {
    device: String,
    passes: Option<bool>,
    lcp: Option<f64>,
    inp: Option<f64>,
    cls: Option<f64>,
    day: String,
}

#[derive(Debug, Default, Serialize)]
struct ContentCard {
    briefs: usize,
    scored_pages: usize,
    average_score: Option<f64>,
}

/// When a module's schedule runs next, RFC 3339.
#[derive(Debug, Serialize)]
struct Scheduled {
    module: &'static str,
    at: String,
}

#[derive(Debug, Serialize)]
struct Dashboard {
    project: ProjectRow,
    organic: Option<OrganicCard>,
    links: Option<LinksCard>,
    rank: Option<RankCard>,
    search: Option<SearchCard>,
    audit: Option<AuditCard>,
    ai: Option<AiCard>,
    local: Option<LocalCard>,
    vitals: Option<VitalsCard>,
    content: ContentCard,
    opportunities: Vec<Opportunity>,
    opportunities_open: usize,
    /// Estimated clicks a month of the open opportunities.
    opportunities_gain: f64,
    schedule: Vec<Scheduled>,
    /// This month's spending by module, US dollars.
    spend: Vec<(String, f64)>,
    spend_total: f64,
}

fn num(value: &Value, key: &str) -> Option<f64> {
    value.get(key).and_then(Value::as_f64)
}

fn text(value: &Value, key: &str) -> Option<String> {
    value.get(key).and_then(Value::as_str).map(str::to_owned)
}

fn list<'a>(value: &'a Value, key: &str) -> &'a [Value] {
    value
        .get(key)
        .and_then(Value::as_array)
        .map_or(&[], Vec::as_slice)
}

/// The last [`SERIES`] points of a series.
fn last(mut points: Vec<Point>) -> Vec<Point> {
    if points.len() > SERIES {
        points.drain(..points.len() - SERIES);
    }
    points
}

/// A series from rows: the date under `date` and the value under `path` (a dotted path).
fn series(rows: &[Value], date: &str, path: &str) -> Vec<Point> {
    last(
        rows.iter()
            .filter_map(|row| {
                let value = path
                    .split('.')
                    .try_fold(row, |value, key| value.get(key))
                    .and_then(Value::as_f64);
                Some(Point {
                    date: text(row, date)?.chars().take(10).collect(),
                    value,
                })
            })
            .collect(),
    )
}

/// What a command answers, or `None` when it fails or has nothing.
fn report(workspace: &Arc<Workspace>, command: &str, arguments: Value) -> Option<Value> {
    ask(workspace, command, arguments)
        .ok()
        .filter(|value| !value.is_null())
}

fn organic(workspace: &Arc<Workspace>, id: i64) -> Option<OrganicCard> {
    let kept = report(
        workspace,
        "get_overview",
        json!({ "project_id": id, "report": "summary" }),
    )?;
    let metrics = metrics_of(kept.get("data")?)?;
    let history = report(
        workspace,
        "metric_history",
        json!({ "project_id": id, "kind": "domain" }),
    )
    .unwrap_or(Value::Null);
    let points = history.as_array().map_or(&[][..], Vec::as_slice);
    Some(OrganicCard {
        metrics,
        fetched_at: text(&kept, "fetched_at").unwrap_or_default(),
        traffic: series(points, "day", "metrics.organic.traffic"),
        keywords: series(points, "day", "metrics.organic.keywords"),
    })
}

fn links(workspace: &Arc<Workspace>, id: i64) -> Option<LinksCard> {
    let kept = report(
        workspace,
        "get_backlinks",
        json!({ "project_id": id, "report": "summary" }),
    )?;
    let links = links_of(kept.get("data")?)?;
    let history = report(
        workspace,
        "metric_history",
        json!({ "project_id": id, "kind": "backlinks" }),
    )
    .unwrap_or(Value::Null);
    Some(LinksCard {
        links,
        fetched_at: text(&kept, "fetched_at").unwrap_or_default(),
        referring_domains: series(
            history.as_array().map_or(&[][..], Vec::as_slice),
            "day",
            "metrics.referring_domains",
        ),
    })
}

fn rank(workspace: &Arc<Workspace>, id: i64) -> Option<RankCard> {
    let report = report(
        workspace,
        "rank_report",
        json!({ "project_id": id, "days": 90 }),
    )?;
    let summary = report.get("summary")?;
    let figure = |key: &str| num(summary, key).unwrap_or(0.0);
    (figure("checked") > 0.0).then(|| RankCard {
        visibility: num(summary, "visibility"),
        visibility_change: num(summary, "visibility_change"),
        average_position: num(summary, "average_position"),
        top3: figure("top3"),
        top10: figure("top10"),
        ranked: figure("ranked"),
        checked: figure("checked"),
        improved: figure("improved"),
        declined: figure("declined"),
        lost: figure("lost"),
        traffic: figure("traffic"),
        series: series(list(&report, "visibility"), "date", "visibility"),
    })
}

fn search(workspace: &Arc<Workspace>, id: i64) -> Option<SearchCard> {
    let report = report(workspace, "gsc_report", json!({ "project_id": id }))?;
    let (now, before) = (report.get("now")?, report.get("before")?);
    let day = |index: usize| {
        report
            .get("period")
            .and_then(|period| period.get(index))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned()
    };
    let current: Vec<Value> = list(&report, "daily")
        .iter()
        .filter(|row| text(row, "period").as_deref() == Some("now"))
        .cloned()
        .collect();
    Some(SearchCard {
        period: (day(0), day(1)),
        clicks: num(now, "clicks").unwrap_or(0.0),
        clicks_before: num(before, "clicks").unwrap_or(0.0),
        impressions: num(now, "impressions").unwrap_or(0.0),
        impressions_before: num(before, "impressions").unwrap_or(0.0),
        ctr: num(now, "ctr").unwrap_or(0.0),
        position: num(now, "position"),
        position_before: num(before, "position"),
        series: series(&current, "date", "clicks"),
    })
}

fn audit(workspace: &Arc<Workspace>, id: i64) -> Option<AuditCard> {
    let setup = report(workspace, "audit_setup", json!({ "project_id": id }))?;
    let finished: Vec<&Value> = list(&setup, "audits")
        .iter()
        .filter(|row| {
            num(row, "health").is_some()
                && !matches!(text(row, "status").as_deref(), Some("running" | "failed"))
        })
        .collect();
    let latest = finished.first()?;
    let previous = finished.get(1);
    let mut trend: Vec<Value> = finished.iter().map(|row| (*row).clone()).collect();
    trend.reverse();
    Some(AuditCard {
        audit_id: latest.get("id").and_then(Value::as_i64).unwrap_or_default(),
        health: num(latest, "health"),
        health_before: previous.and_then(|row| num(row, "health")),
        geo: num(latest, "geo"),
        issues: num(latest, "issues").unwrap_or(0.0),
        issues_before: previous.and_then(|row| num(row, "issues")),
        pages: num(latest, "pages").unwrap_or(0.0),
        finished_at: text(latest, "finished_at"),
        series: series(&trend, "started_at", "health"),
    })
}

fn ai(workspace: &Arc<Workspace>, id: i64) -> Option<AiCard> {
    let report = report(
        workspace,
        "ai_report",
        json!({ "project_id": id, "days": 90 }),
    )?;
    let summary = report.get("summary")?;
    let answers = num(summary, "answers").unwrap_or(0.0);
    (answers > 0.0).then(|| AiCard {
        answers,
        mention_rate: num(summary, "mention_rate").unwrap_or(0.0),
        citation_rate: num(summary, "citation_rate").unwrap_or(0.0),
        share_of_voice: num(summary, "share_of_voice").unwrap_or(0.0),
        series: series(list(&report, "history"), "date", "mention_rate"),
    })
}

fn local(workspace: &Arc<Workspace>, id: i64) -> Option<LocalCard> {
    let report = report(workspace, "local_grid", json!({ "project_id": id }))?;
    report.get("run").filter(|run| !run.is_null())?;
    let summary = report.get("summary")?;
    // The trend has a point per keyword and run: the runs' average.
    let mut runs: Vec<(String, Vec<f64>)> = Vec::new();
    for point in list(&report, "trend") {
        let (Some(at), Some(solv)) = (text(point, "started_at"), num(point, "solv")) else {
            continue;
        };
        match runs.iter_mut().find(|(started, _)| *started == at) {
            Some((_, values)) => values.push(solv),
            None => runs.push((at, vec![solv])),
        }
    }
    #[allow(clippy::cast_precision_loss)] // a few keywords
    let points: Vec<Point> = runs
        .into_iter()
        .map(|(date, values)| Point {
            date: date.chars().take(10).collect(),
            value: Some(values.iter().sum::<f64>() / values.len() as f64),
        })
        .collect();
    Some(LocalCard {
        arp: num(summary, "arp"),
        solv: num(summary, "solv").unwrap_or(0.0),
        top3: num(summary, "top3").unwrap_or(0.0),
        points: num(summary, "points").unwrap_or(0.0),
        series: last(points),
    })
}

fn vitals(workspace: &Arc<Workspace>, id: i64) -> Option<VitalsCard> {
    let report = report(workspace, "vitals_report", json!({ "project_id": id }))?;
    let p75 = |field: Option<&Value>, name: &str| {
        field
            .and_then(|field| field.get(name))
            .and_then(|metric| num(metric, "p75"))
    };
    let mut devices: Vec<VitalsDevice> = list(&report, "origins")
        .iter()
        .map(|origin| {
            let field = origin.get("field").filter(|field| !field.is_null());
            VitalsDevice {
                device: text(origin, "device").unwrap_or_default(),
                passes: origin.get("passes").and_then(Value::as_bool),
                lcp: p75(field, "lcp"),
                inp: p75(field, "inp"),
                cls: p75(field, "cls"),
                day: text(origin, "day").unwrap_or_default(),
            }
        })
        .collect();
    devices.sort_by_key(|device| device.device != "phone");
    (!devices.is_empty()).then_some(VitalsCard { devices })
}

fn content(workspace: &Arc<Workspace>, id: i64) -> ContentCard {
    let briefs = report(workspace, "content_briefs", json!({ "project_id": id }))
        .and_then(|value| value.as_array().map(Vec::len))
        .unwrap_or(0);
    let scores: Vec<f64> = report(workspace, "content_pages", json!({ "project_id": id }))
        .and_then(|value| value.as_array().cloned())
        .unwrap_or_default()
        .iter()
        .filter_map(|page| num(page, "score"))
        .collect();
    #[allow(clippy::cast_precision_loss)] // a few pages
    let average_score = (!scores.is_empty())
        .then(|| (scores.iter().sum::<f64>() / scores.len() as f64 * 10.0).round() / 10.0);
    ContentCard {
        briefs,
        scored_pages: scores.len(),
        average_score,
    }
}

fn schedule(workspace: &Arc<Workspace>, id: i64) -> Vec<Scheduled> {
    let setups: [(&'static str, &str, &str); 5] = [
        ("rank", "rank_setup", "next_check_at"),
        ("audit", "audit_setup", "next_audit_at"),
        ("ai", "ai_setup", "next_run_at"),
        ("gsc", "gsc_setup", "next_sync_at"),
        ("local", "local_setup", "next_run_at"),
    ];
    let mut next: Vec<Scheduled> = setups
        .iter()
        .filter_map(|(module, command, field)| {
            let setup = report(workspace, command, json!({ "project_id": id }))?;
            Some(Scheduled {
                module,
                at: text(&setup, field)?,
            })
        })
        .collect();
    if let Some(at) = report(workspace, "snapshot_setup", json!({ "project_id": id }))
        .and_then(|setup| setup.get("next_at").and_then(Value::as_i64))
    {
        next.push(Scheduled {
            module: "snapshots",
            at: octopuson_core::Timestamp::from_unix_micros(at.saturating_mul(1_000_000))
                .to_string(),
        });
    }
    next.sort_by(|a, b| a.at.cmp(&b.at));
    next
}

#[derive(Debug, Deserialize)]
struct DashboardAsk {
    project_id: i64,
    #[serde(default)]
    lang: Option<String>,
}

fn project_dashboard(context: &Context<'_>, ask: DashboardAsk) -> Result<Dashboard, AppError> {
    let workspace = context.workspace;
    let project = workspace.store()?.project(ask.project_id)?;
    let id = project.id;
    let found = evaluate(
        workspace,
        &project,
        ask.lang.as_deref().unwrap_or("tr"),
        false,
    )?;
    let spend = workspace.store()?.project_spend(id, &month())?;
    Ok(Dashboard {
        organic: organic(workspace, id),
        links: links(workspace, id),
        rank: rank(workspace, id),
        search: search(workspace, id),
        audit: audit(workspace, id),
        ai: ai(workspace, id),
        local: local(workspace, id),
        vitals: vitals(workspace, id),
        content: content(workspace, id),
        opportunities_open: found.open,
        opportunities_gain: found.gain,
        opportunities: found.items.into_iter().take(TOP_OPPORTUNITIES).collect(),
        schedule: schedule(workspace, id),
        spend_total: spend.iter().map(|(_, usd)| usd).sum(),
        spend,
        project,
    })
}

/// Adds the module's commands.
pub fn register(router: &mut Router) {
    router.add("project_dashboard", project_dashboard);
}
