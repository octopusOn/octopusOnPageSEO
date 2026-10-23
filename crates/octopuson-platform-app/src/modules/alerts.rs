//! Alerts: rules look at what the modules hold after each piece of work (a rank check, an
//! audit, an AI run, a Search Console sync, a grid run, a snapshot) and once an hour, and keep
//! an alert for each change worth knowing: keywords falling or leaving the first ten,
//! visibility or clicks dropping, audit health falling or new high-priority problems, AI
//! mentions moving, the local grid weakening, new one- or two-star reviews, competitors growing
//! faster, and (checked weekly when asked, paid) linking sites won and lost.
//!
//! An alert's key comes from what it is about, so it is kept once however often the rules run
//! and on however many copies. When a desktop syncs with the account, only the web looks for
//! alerts; the desktop shows them as system notifications when they arrive. The web e-mails
//! them, at once or in a daily digest.

use std::collections::BTreeSet;
use std::sync::Arc;

use octopuson_dataforseo::Client;
use octopuson_dataforseo::backlinks::{self, BacklinkQuery, LinkStatus};
use octopuson_platform::{
    AlertRow, AlertSettings, CacheClass, LinkProspect, NewAlert, ProjectRow, unix_now,
};
use octopuson_serp_score::rank::seconds_of;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

use crate::fetch::{Estimate, Paid};
use crate::jobs::{Job, JobControl, JobDone, JobManager, JobStatus};
use crate::mail::Mail;
use crate::report::texts;
use crate::router::{Context, Router};
use crate::snapshot::ask;
use crate::{AppError, Host, Workspace, now};

/// Kind of the job checking linking sites.
pub const LINK_JOB: &str = "link-watch";

/// The rules a finished job can make true: only those are looked at after it.
fn parts_after(kind: &str) -> &'static [Part] {
    match kind {
        "rank-check" => &[Part::Rank],
        "site-audit" => &[Part::Audit],
        "ai-visibility" => &[Part::Ai],
        "search-console" => &[Part::Search],
        "local-grid" => &[Part::Local],
        "snapshot" => &[Part::Rivals],
        _ => &[],
    }
}

/// A group of rules, read from one module's data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Part {
    /// Rank tracking.
    Rank,
    /// Search Console.
    Search,
    /// The site audit.
    Audit,
    /// AI visibility.
    Ai,
    /// The local grid and reviews.
    Local,
    /// Competitors between snapshots.
    Rivals,
}

/// Every group of rules.
pub const ALL_PARTS: &[Part] = &[
    Part::Rank,
    Part::Search,
    Part::Audit,
    Part::Ai,
    Part::Local,
    Part::Rivals,
];

/// Seconds between looks at a project's data without work finishing: a safety net run when
/// the schedule wakes anyway (it does not wake it).
const EVALUATE_EVERY: i64 = 6 * 3_600;

/// Time a look waits after the work that called it, so pages read what changed first.
const SETTLE: std::time::Duration = std::time::Duration::from_secs(2);

/// Pause before each report a look reads, so pages waiting for the data get their turn.
const YIELD: std::time::Duration = std::time::Duration::from_millis(30);

/// Seconds after which data a sync brought is looked at again.
const SYNCED_EVERY: i64 = 600;

/// Seconds between link checks.
const LINKS_EVERY: i64 = 7 * 86_400;

/// Seconds between daily e-mails (a little under a day, so they keep their hour).
const DIGEST_EVERY: i64 = 20 * 3_600;

/// Hour (UTC) after which a day's digest goes out.
const DIGEST_HOUR: i64 = 6;

/// Alerts older than this are never sent (copies that synced late, rules turned on late).
const SEND_WITHIN: i64 = 2 * 86_400;

/// Reviews seen within this many days are reported.
const REVIEW_DAYS: i64 = 14;

/// Linking sites read per link check (new ones, lost ones).
const LINK_ROWS: u32 = 100;

/// Keywords named in an alert at most.
const NAMED: usize = 5;

/// System notifications shown one by one for a project; more become one notice.
const NOTIFY_EACH: usize = 2;

fn rfc3339(seconds: i64) -> String {
    octopuson_core::Timestamp::from_unix_micros(seconds.saturating_mul(1_000_000)).to_string()
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

fn round(value: f64) -> f64 {
    (value * 10.0).round() / 10.0
}

fn report(workspace: &Arc<Workspace>, command: &str, arguments: Value) -> Option<Value> {
    std::thread::sleep(YIELD);
    ask(workspace, command, arguments)
        .ok()
        .filter(|value| !value.is_null())
}

fn alert(
    key: String,
    kind: &str,
    severity: &str,
    page: &str,
    params: Value,
    detail: Value,
) -> NewAlert {
    NewAlert {
        key,
        kind: kind.to_owned(),
        severity: severity.to_owned(),
        page: page.to_owned(),
        params,
        detail,
        created_at: now(),
    }
}

/// `kw (6→14)` for a keyword's move; `—` when it is gone.
fn moved(keyword: &str, before: f64, now: Option<f64>) -> String {
    let now = now.map_or_else(|| "—".to_owned(), |now| format!("{now:.0}"));
    format!("{keyword} ({before:.0}→{now})")
}

/// Rank tracking: keywords falling or leaving the first ten, visibility dropping.
fn rank_alerts(
    workspace: &Arc<Workspace>,
    project: &ProjectRow,
    settings: &AlertSettings,
    out: &mut Vec<NewAlert>,
) -> Result<(), AppError> {
    let rules = &settings.rules;
    // Read first: the store is not held while the reports are read.
    let trackers = workspace.store()?.rank_trackers(project.id)?;
    for tracker in trackers {
        let label = match &tracker.location {
            Some(place) => format!("{} {}", tracker.country.to_uppercase(), place),
            None => format!("{} {}", tracker.country.to_uppercase(), tracker.device),
        };
        let Some(report) = report(
            workspace,
            "rank_report",
            json!({ "project_id": project.id, "tracker_id": tracker.id, "days": 30 }),
        ) else {
            continue;
        };
        let mut fell: Vec<(f64, String)> = Vec::new();
        let mut exits = 0_u32;
        let mut latest = String::new();
        for row in list(&report, "rows") {
            let (Some(keyword), Some(before)) = (text(row, "keyword"), num(row, "previous")) else {
                continue;
            };
            let position = num(row, "position");
            if let Some(at) = text(row, "checked_at")
                && at > latest
            {
                latest = at;
            }
            let drop = position.map_or(100.0 - before, |now| now - before);
            let left = rules.top10_exit && before <= 10.0 && position.is_none_or(|now| now > 10.0);
            if left {
                exits += 1;
            }
            if before <= 30.0
                && ((rules.rank_drop > 0 && drop >= f64::from(rules.rank_drop)) || left)
            {
                fell.push((drop, moved(&keyword, before, position)));
            }
        }
        if !fell.is_empty() && !latest.is_empty() {
            fell.sort_by(|a, b| b.0.total_cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
            let named: Vec<&str> = fell
                .iter()
                .take(NAMED)
                .map(|(_, text)| text.as_str())
                .collect();
            out.push(alert(
                format!("rank|{}|{}", tracker.id_key(), latest.chars().take(16).collect::<String>()),
                "rank_drop",
                "bad",
                "rank",
                json!({}),
                json!({ "count": fell.len(), "top10": exits, "keywords": named.join(", "), "tracker": label }),
            ));
        }
        if rules.visibility_drop > 0.0 {
            let points: Vec<(String, f64)> = list(&report, "visibility")
                .iter()
                .filter_map(|point| Some((text(point, "date")?, num(point, "visibility")?)))
                .collect();
            if let [.., (_, before), (date, now)] = points.as_slice()
                && *before > 0.0
            {
                let drop = (before - now) / before * 100.0;
                if drop >= rules.visibility_drop {
                    out.push(alert(
                        format!("visibility|{}|{date}", tracker.id_key()),
                        "visibility_drop",
                        "bad",
                        "rank",
                        json!({}),
                        json!({ "drop": round(drop), "before": round(*before), "now": round(*now), "tracker": label }),
                    ));
                }
            }
        }
    }
    Ok(())
}

/// A tracker named the same on every copy (its id differs).
trait TrackerKey {
    fn id_key(&self) -> String;
}

impl TrackerKey for octopuson_platform::TrackerRow {
    fn id_key(&self) -> String {
        format!(
            "{}-{}-{}-{}-{}",
            self.engine,
            self.country,
            self.language,
            self.device,
            self.location.as_deref().unwrap_or("")
        )
    }
}

/// Search Console: clicks dropping against the period before.
fn gsc_alerts(
    workspace: &Arc<Workspace>,
    project: &ProjectRow,
    settings: &AlertSettings,
    out: &mut Vec<NewAlert>,
) {
    if settings.rules.gsc_drop <= 0.0 {
        return;
    }
    let Some(report) = report(workspace, "gsc_report", json!({ "project_id": project.id })) else {
        return;
    };
    let (Some(now), Some(before)) = (
        report.get("now").and_then(|now| num(now, "clicks")),
        report
            .get("before")
            .and_then(|before| num(before, "clicks")),
    ) else {
        return;
    };
    let end = report
        .get("period")
        .and_then(|period| period.get(1))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    if before >= 20.0 && !end.is_empty() {
        let drop = (before - now) / before * 100.0;
        if drop >= settings.rules.gsc_drop {
            out.push(alert(
                format!("gsc|{end}"),
                "gsc_drop",
                "bad",
                "gsc",
                json!({}),
                json!({ "drop": round(drop), "before": before.round(), "clicks": now.round() }),
            ));
        }
    }
}

/// The latest audit against the one before: health falling, high-priority problems appearing.
fn audit_alerts(
    workspace: &Arc<Workspace>,
    project: &ProjectRow,
    settings: &AlertSettings,
    lang: &str,
    out: &mut Vec<NewAlert>,
) {
    let Some(report) = report(
        workspace,
        "audit_report",
        json!({ "project_id": project.id, "lang": lang }),
    ) else {
        return;
    };
    let (Some(audit), Some(previous)) = (
        report.get("audit"),
        report
            .get("previous")
            .filter(|previous| !previous.is_null()),
    ) else {
        return;
    };
    let started = text(audit, "started_at").unwrap_or_default();
    if let (Some(now), Some(before)) = (num(audit, "health"), num(previous, "health"))
        && settings.rules.health_drop > 0.0
        && before - now >= settings.rules.health_drop
    {
        out.push(alert(
            format!("audit_health|{started}"),
            "audit_health",
            "bad",
            "audit",
            json!({ "tab": "issues" }),
            json!({ "drop": round(before - now), "before": round(before), "now": round(now) }),
        ));
    }
    if settings.rules.audit_issues {
        let fresh: Vec<String> = list(&report, "categories")
            .iter()
            .flat_map(|category| list(category, "issues"))
            .filter(|issue| {
                text(issue, "priority").as_deref() == Some("high")
                    && num(issue, "count").unwrap_or(0.0) > 0.0
                    && num(issue, "previous") == Some(0.0)
            })
            .filter_map(|issue| text(issue, "title"))
            .collect();
        if !fresh.is_empty() {
            out.push(alert(
                format!("audit_issues|{started}"),
                "audit_issues",
                "bad",
                "audit",
                json!({ "tab": "issues" }),
                json!({ "count": fresh.len(), "titles": fresh.iter().take(NAMED).cloned().collect::<Vec<_>>().join(", ") }),
            ));
        }
    }
}

/// AI visibility: the mention rate moving between the last two runs.
fn ai_alerts(
    workspace: &Arc<Workspace>,
    project: &ProjectRow,
    settings: &AlertSettings,
    out: &mut Vec<NewAlert>,
) {
    if settings.rules.ai_change <= 0.0 {
        return;
    }
    let Some(report) = report(
        workspace,
        "ai_report",
        json!({ "project_id": project.id, "days": 90 }),
    ) else {
        return;
    };
    let points: Vec<(String, f64)> = list(&report, "history")
        .iter()
        .filter_map(|point| Some((text(point, "date")?, num(point, "mention_rate")?)))
        .collect();
    if let [.., (_, before), (date, now)] = points.as_slice() {
        let change = now - before;
        if change.abs() >= settings.rules.ai_change {
            out.push(alert(
                format!("ai|{date}"),
                if change < 0.0 { "ai_drop" } else { "ai_gain" },
                if change < 0.0 { "bad" } else { "good" },
                "ai",
                json!({}),
                json!({ "change": round(change.abs()), "before": round(*before), "now": round(*now) }),
            ));
        }
    }
}

/// The local grid: keywords whose share of voice fell between the last two runs; new low
/// reviews.
fn local_alerts(
    workspace: &Arc<Workspace>,
    project: &ProjectRow,
    settings: &AlertSettings,
    out: &mut Vec<NewAlert>,
) -> Result<(), AppError> {
    if settings.rules.local_drop > 0.0
        && let Some(report) = report(workspace, "local_grid", json!({ "project_id": project.id }))
    {
        let mut runs: Vec<String> = Vec::new();
        for point in list(&report, "trend") {
            if let Some(at) = text(point, "started_at")
                && !runs.contains(&at)
            {
                runs.push(at);
            }
        }
        runs.sort();
        if let [.., before, latest] = runs.as_slice() {
            let solv = |run: &str, keyword: &str| {
                list(&report, "trend")
                    .iter()
                    .find(|point| {
                        text(point, "started_at").as_deref() == Some(run)
                            && text(point, "keyword").as_deref() == Some(keyword)
                    })
                    .and_then(|point| num(point, "solv"))
            };
            let keywords: BTreeSet<String> = list(&report, "trend")
                .iter()
                .filter_map(|point| text(point, "keyword"))
                .collect();
            let fell: Vec<String> = keywords
                .iter()
                .filter_map(|keyword| {
                    let (was, now) = (solv(before, keyword)?, solv(latest, keyword)?);
                    (was - now >= settings.rules.local_drop)
                        .then(|| format!("{keyword} ({was:.0}→{now:.0})"))
                })
                .collect();
            if !fell.is_empty() {
                out.push(alert(
                    format!("local|{latest}"),
                    "local_drop",
                    "bad",
                    "local",
                    json!({}),
                    json!({ "count": fell.len(), "keywords": fell.iter().take(NAMED).cloned().collect::<Vec<_>>().join(", ") }),
                ));
            }
        }
    }
    if settings.rules.reviews {
        let since = rfc3339(unix_now() - REVIEW_DAYS * 86_400);
        let reviews = workspace.store()?.local_reviews(project.id, 200)?;
        for review in reviews {
            if review.rating.is_some_and(|stars| stars <= 2)
                && review.owner_answer.is_none()
                && review.first_seen >= since
            {
                let excerpt: String = review
                    .text
                    .as_deref()
                    .unwrap_or_default()
                    .chars()
                    .take(140)
                    .collect();
                out.push(alert(
                    format!("review|{}", review.review_id),
                    "review",
                    "bad",
                    "local",
                    json!({ "tab": "reviews" }),
                    json!({ "rating": review.rating.unwrap_or(0), "author": review.author.unwrap_or_default(), "text": excerpt }),
                ));
            }
        }
    }
    Ok(())
}

/// Competitors growing their keywords much faster than the site between the last two
/// snapshots.
fn rival_alerts(
    workspace: &Arc<Workspace>,
    project: &ProjectRow,
    settings: &AlertSettings,
    out: &mut Vec<NewAlert>,
) -> Result<(), AppError> {
    if !settings.rules.rivals {
        return Ok(());
    }
    let rows: Vec<(String, crate::snapshot::SnapshotData)> = workspace
        .store()?
        .snapshots(project.id)?
        .into_iter()
        .filter(|row| matches!(row.status.as_str(), "done" | "partial"))
        .filter_map(|row| {
            Some((
                row.taken_at.clone(),
                serde_json::from_value(row.data?).ok()?,
            ))
        })
        .take(2)
        .collect();
    let [(taken, now), (_, before)] = rows.as_slice() else {
        return Ok(());
    };
    let growth = |now: f64, before: f64| {
        if before > 0.0 {
            (now - before) / before
        } else {
            0.0
        }
    };
    let own = growth(
        now.overview
            .as_ref()
            .map_or(0.0, |metrics| metrics.keywords),
        before
            .overview
            .as_ref()
            .map_or(0.0, |metrics| metrics.keywords),
    );
    for rival in &now.competitors {
        let (Some(metrics), Some(was)) = (
            rival.overview.as_ref(),
            before
                .competitors
                .iter()
                .find(|other| other.domain == rival.domain)
                .and_then(|other| other.overview.as_ref()),
        ) else {
            continue;
        };
        let rate = growth(metrics.keywords, was.keywords);
        if rate >= 0.1 && metrics.keywords - was.keywords >= 50.0 && rate - own >= 0.05 {
            out.push(alert(
                format!("rival|{taken}|{}", rival.domain),
                "rival_growth",
                "info",
                "gap",
                json!({ "domain": rival.domain }),
                json!({ "domain": rival.domain, "growth": round(rate * 100.0), "gained": (metrics.keywords - was.keywords).round(), "own": round(own * 100.0) }),
            ));
        }
    }
    Ok(())
}

/// The language a project's alerts are written in.
fn lang_of(project: &ProjectRow, settings: &AlertSettings) -> String {
    if !settings.lang.is_empty() {
        return settings.lang.clone();
    }
    if octopuson_platform::ALERT_LANGUAGES.contains(&project.language.as_str()) {
        project.language.clone()
    } else {
        "en".to_owned()
    }
}

/// Looks for a project's alerts now, keeps the new ones and sends them where asked; the new
/// ones.
///
/// # Errors
/// Storage errors.
pub fn evaluate(workspace: &Arc<Workspace>, project_id: i64) -> Result<Vec<NewAlert>, AppError> {
    evaluate_parts(workspace, project_id, ALL_PARTS)
}

/// Looks for a project's alerts of some groups of rules; the new ones.
///
/// # Errors
/// Storage errors.
pub fn evaluate_parts(
    workspace: &Arc<Workspace>,
    project_id: i64,
    parts: &[Part],
) -> Result<Vec<NewAlert>, AppError> {
    let (project, settings) = {
        let store = workspace.store()?;
        (
            store.project(project_id)?,
            store.alert_settings(project_id)?,
        )
    };
    if parts.len() == ALL_PARTS.len() {
        workspace
            .store()?
            .note_alert_evaluation(project_id, &now())?;
    }
    if !settings.enabled || !workspace.schedules_here() {
        return Ok(Vec::new());
    }
    let lang = lang_of(&project, &settings);
    let mut found = Vec::new();
    for part in parts {
        match part {
            Part::Rank => rank_alerts(workspace, &project, &settings, &mut found)?,
            Part::Search => gsc_alerts(workspace, &project, &settings, &mut found),
            Part::Audit => audit_alerts(workspace, &project, &settings, &lang, &mut found),
            Part::Ai => ai_alerts(workspace, &project, &settings, &mut found),
            Part::Local => local_alerts(workspace, &project, &settings, &mut found)?,
            Part::Rivals => rival_alerts(workspace, &project, &settings, &mut found)?,
        }
    }
    let mut added = Vec::new();
    {
        let store = workspace.store()?;
        for alert in found {
            if store.add_alert(project_id, &alert)? {
                added.push(alert);
            }
        }
    }
    if !added.is_empty() {
        deliver(workspace, &project, &settings)?;
    }
    Ok(added)
}

/// Looks for alerts after a job that may raise them (the job manager's hook).
pub fn after_job(workspace: &Arc<Workspace>, job: &Job) {
    let Some(project_id) = job.project_id else {
        return;
    };
    let parts = parts_after(&job.kind);
    if parts.is_empty() || !matches!(job.status, JobStatus::Done | JobStatus::Partial) {
        return;
    }
    std::thread::sleep(SETTLE);
    if let Err(error) = crate::in_background(|| evaluate_parts(workspace, project_id, parts)) {
        tracing::warn!(project = project_id, %error, "alerts were not looked for");
    }
}

/// Looks for the alerts of every project not looked at for `older` seconds (after answers
/// were read from the provider's queue, after a sync brought data).
pub fn evaluate_all(workspace: &Arc<Workspace>, older: i64) {
    std::thread::sleep(SETTLE);
    let Ok(plans) = workspace.store().and_then(|store| Ok(store.alert_plans()?)) else {
        return;
    };
    let now_secs = unix_now();
    for (project_id, settings, evaluated) in plans {
        if settings.enabled
            && due(evaluated.as_deref(), older) <= now_secs
            && let Err(error) = evaluate(workspace, project_id)
        {
            tracing::warn!(project = project_id, %error, "alerts were not looked for");
        }
    }
}

/// Looks at data a sync or the provider's queue brought, in the background.
pub fn after_sync(workspace: &Arc<Workspace>) {
    let workspace = Arc::clone(workspace);
    let started = std::thread::Builder::new()
        .name("alerts-after-sync".to_owned())
        .spawn(move || crate::in_background(|| evaluate_all(&workspace, SYNCED_EVERY)));
    if let Err(error) = started {
        tracing::warn!(%error, "alerts were not looked for after a sync");
    }
}

// ----- Words ---------------------------------------------------------------------------------

/// An alert as one line in `lang`.
pub(crate) fn line(alert: &NewAlert, lang: &str) -> String {
    let values: Vec<(String, String)> = alert
        .detail
        .as_object()
        .map(|detail| {
            detail
                .iter()
                .map(|(name, value)| {
                    let shown = match value {
                        Value::Number(number) => {
                            let value = number.as_f64().unwrap_or_default();
                            texts::number(value, usize::from(value.fract() != 0.0), lang)
                        }
                        Value::String(text) => text.clone(),
                        other => other.to_string(),
                    };
                    (name.clone(), shown)
                })
                .collect()
        })
        .unwrap_or_default();
    let pairs: Vec<(&str, &str)> = values
        .iter()
        .map(|(name, value)| (name.as_str(), value.as_str()))
        .collect();
    texts::fill(lang, &format!("alert_{}", alert.kind), &pairs)
}

/// Sends a project's new alerts: at once by e-mail when asked (web), and as system
/// notifications (desktop).
fn deliver(
    workspace: &Arc<Workspace>,
    project: &ProjectRow,
    settings: &AlertSettings,
) -> Result<(), AppError> {
    if settings.email == "instant" {
        mail(workspace, project, settings)?;
    }
    announce(workspace)
}

/// E-mails a project's unsent alerts to its recipients (the web only).
fn mail(
    workspace: &Arc<Workspace>,
    project: &ProjectRow,
    settings: &AlertSettings,
) -> Result<usize, AppError> {
    let Some(mailer) = workspace
        .mailer
        .as_ref()
        .filter(|_| workspace.host() == Host::Web)
    else {
        return Ok(0);
    };
    if settings.recipients.is_empty() {
        return Ok(0);
    }
    let since = rfc3339(unix_now() - SEND_WITHIN);
    let unsent = workspace
        .store()?
        .alerts_unsent(Some(project.id), "mailed", &since)?;
    if unsent.is_empty() {
        return Ok(0);
    }
    let lang = lang_of(project, settings);
    let count = unsent.len().to_string();
    let mut body = texts::fill(
        &lang,
        "alert_email_intro",
        &[("project", &project.name), ("n", &count)],
    );
    body.push_str("\n\n");
    for row in &unsent {
        body.push_str("• ");
        body.push_str(&line(&row.alert, &lang));
        body.push('\n');
    }
    if let Some(base) = &workspace.public_url {
        body.push('\n');
        body.push_str(&texts::fill(
            &lang,
            "alert_email_link",
            &[(
                "link",
                &format!("{base}/#/project?project={}&tab=alerts", project.id),
            )],
        ));
        body.push('\n');
    }
    let reply_to = workspace
        .account_status()
        .ok()
        .and_then(|status| status.account)
        .and_then(|account| account.email);
    mailer
        .send(&Mail {
            to: settings.recipients.clone(),
            reply_to,
            subject: texts::fill(
                &lang,
                "alert_email_subject",
                &[("project", &project.name), ("n", &count)],
            ),
            text: body,
            attachment: None,
        })
        .map_err(AppError::Unavailable)?;
    let ids: Vec<i64> = unsent.iter().map(|row| row.id).collect();
    workspace
        .store()?
        .mark_alerts_sent(&ids, "mailed", &now())?;
    Ok(ids.len())
}

/// Shows alerts this copy has not shown yet as system notifications (the desktop): those it
/// found and those a sync brought.
///
/// # Errors
/// Storage errors.
pub fn announce(workspace: &Workspace) -> Result<(), AppError> {
    let Some(notifier) = workspace.notifier.as_ref() else {
        return Ok(());
    };
    let since = rfc3339(unix_now() - SEND_WITHIN);
    // Words first, shown after the store is let go.
    let mut shown: Vec<(String, String)> = Vec::new();
    let ids: Vec<i64> = {
        let store = workspace.store()?;
        let unsent: Vec<AlertRow> = store.alerts_unsent(None, "notified", &since)?;
        let mut projects: Vec<i64> = unsent.iter().map(|row| row.project_id).collect();
        projects.sort_unstable();
        projects.dedup();
        for project_id in projects {
            let project = store.project(project_id)?;
            let lang = lang_of(&project, &store.alert_settings(project_id)?);
            let rows: Vec<&AlertRow> = unsent
                .iter()
                .filter(|row| row.project_id == project_id)
                .collect();
            // A few one by one; more as one notice, the first of them as its text.
            if rows.len() <= NOTIFY_EACH {
                for row in rows {
                    shown.push((project.name.clone(), line(&row.alert, &lang)));
                }
            } else {
                let title = texts::fill(
                    &lang,
                    "alert_email_subject",
                    &[("project", &project.name), ("n", &rows.len().to_string())],
                );
                shown.push((title, line(&rows[0].alert, &lang)));
            }
        }
        let ids: Vec<i64> = unsent.iter().map(|row| row.id).collect();
        store.mark_alerts_sent(&ids, "notified", &now())?;
        ids
    };
    if !ids.is_empty() {
        for (title, body) in &shown {
            notifier.notify(title, body);
        }
    }
    Ok(())
}

// ----- Linking sites -------------------------------------------------------------------------

fn link_paid(project_id: i64, task: Value) -> Paid<'static> {
    Paid {
        travels: false,
        module: "alerts",
        provider: octopuson_dataforseo::ID,
        endpoint: backlinks::BACKLINKS,
        class: CacheClass::Backlinks,
        project_id: Some(project_id),
        task,
        estimate_usd: backlinks::estimate(LINK_ROWS),
    }
}

/// What a week's link check costs: new and lost linking sites.
fn links_usd() -> f64 {
    backlinks::estimate(LINK_ROWS) * 2.0
}

#[derive(Debug, Default, Serialize)]
struct LinksChecked {
    new: Vec<String>,
    lost: Vec<String>,
    cost_usd: f64,
}

/// Reads the linking sites won and lost in the last week, keeps an alert, and adds the lost
/// ones to the link prospects to win back.
fn check_links(
    workspace: &Arc<Workspace>,
    project_id: i64,
    control: &JobControl,
) -> Result<JobDone, AppError> {
    let project = workspace.store()?.project(project_id)?;
    let session = workspace.paid_run(octopuson_dataforseo::ID)?;
    let week_ago = rfc3339(unix_now() - 8 * 86_400);
    let mut checked = LinksChecked::default();
    for (index, status) in [LinkStatus::New, LinkStatus::Lost].into_iter().enumerate() {
        control.progress(if index == 0 { "new" } else { "lost" }, index as u64, 2);
        let query = BacklinkQuery {
            status,
            dofollow_only: false,
            one_per_domain: true,
        };
        let paid = link_paid(
            project_id,
            backlinks::backlinks_task(&project.domain, query, LINK_ROWS)?,
        );
        let answered = session.ask(&paid, |route| {
            let credentials = route.key(octopuson_dataforseo::SECRET)?;
            let client = Client::new(route.transport, &credentials)?;
            Ok(client.post(paid.endpoint, &paid.task)?.into())
        })?;
        checked.cost_usd += answered.spent_usd;
        control.spent(answered.spent_usd);
        let rows = backlinks::backlinks_from_result(&answered.result);
        let mut domains: Vec<String> = rows
            .iter()
            .filter(|row| {
                let seen = if index == 0 {
                    row.first_seen.as_deref()
                } else {
                    row.last_seen.as_deref()
                };
                seen.is_some_and(|seen| seen >= week_ago.as_str())
            })
            .filter_map(|row| row.domain_from.clone())
            .collect();
        domains.sort();
        domains.dedup();
        if index == 0 {
            checked.new = domains;
        } else {
            checked.lost = domains;
        }
    }
    let day = now().chars().take(10).collect::<String>();
    let store = workspace.store()?;
    store.note_link_watch(project_id, &now())?;
    let known: BTreeSet<String> = store
        .prospects(project_id)?
        .into_iter()
        .map(|prospect| prospect.domain)
        .collect();
    for domain in checked
        .lost
        .iter()
        .filter(|domain| !known.contains(*domain))
        .take(50)
    {
        store.save_prospect(
            project_id,
            &LinkProspect {
                domain: domain.clone(),
                url: None,
                source: "lost".to_owned(),
                status: "prospect".to_owned(),
                note: None,
                added_at: now(),
                updated_at: now(),
            },
            &now(),
        )?;
    }
    if !checked.new.is_empty() || !checked.lost.is_empty() {
        let added = store.add_alert(
            project_id,
            &alert(
                format!("links|{day}"),
                "links",
                if checked.lost.len() > checked.new.len() { "bad" } else { "good" },
                "backlinks",
                json!({ "tab": if checked.lost.is_empty() { "backlinks" } else { "prospects" } }),
                json!({
                    "new": checked.new.len(),
                    "lost": checked.lost.len(),
                    "domains": checked.new.iter().chain(&checked.lost).take(NAMED).cloned().collect::<Vec<_>>().join(", "),
                }),
            ),
        )?;
        drop(store);
        if added {
            let settings = workspace.store()?.alert_settings(project_id)?;
            deliver(workspace, &project, &settings)?;
        }
    }
    Ok(JobDone::with(&checked))
}

fn start_links(
    workspace: &Arc<Workspace>,
    jobs: &JobManager,
    project: &ProjectRow,
) -> Result<String, AppError> {
    let shared = Arc::clone(workspace);
    let id = project.id;
    jobs.start(LINK_JOB, &project.name, Some(id), move |control| {
        check_links(&shared, id, control)
    })
}

// ----- Schedule ------------------------------------------------------------------------------

fn due(at: Option<&str>, every: i64) -> i64 {
    at.and_then(seconds_of).map_or(0, |last| last + every)
}

/// When the daily e-mail of a project is due.
fn digest_due(settings: &AlertSettings) -> Option<i64> {
    if settings.email != "daily" || settings.recipients.is_empty() {
        return None;
    }
    let after = due(settings.last_digest_at.as_deref(), DIGEST_EVERY);
    // Not before the day's hour.
    let day_start = after - after.rem_euclid(86_400);
    Some(after.max(day_start + DIGEST_HOUR * 3_600))
}

/// Looks for alerts that are due, checks links and sends daily e-mails; the link jobs started.
///
/// # Errors
/// Storage errors.
pub fn run_due(workspace: &Arc<Workspace>, jobs: &JobManager) -> Result<Vec<String>, AppError> {
    let now_secs = unix_now();
    let mut started = Vec::new();
    // Read first: looking for alerts opens the store again.
    let plans = workspace.store()?.alert_plans()?;
    for (project_id, settings, evaluated) in plans {
        if !settings.enabled {
            continue;
        }
        if due(evaluated.as_deref(), EVALUATE_EVERY) <= now_secs
            && let Err(error) = crate::in_background(|| evaluate(workspace, project_id))
        {
            tracing::warn!(project = project_id, %error, "alerts were not looked for");
        }
        if settings.rules.links && due(settings.links_last_run.as_deref(), LINKS_EVERY) <= now_secs
        {
            let project = workspace.store()?.project(project_id)?;
            // Marked first, so a check that cannot be paid is not tried every minute.
            workspace.store()?.note_link_watch(project_id, &now())?;
            match start_links(workspace, jobs, &project) {
                Ok(id) => started.push(id),
                Err(error) => {
                    tracing::warn!(project = project_id, %error, "links were not checked");
                }
            }
        }
        if workspace.host() == Host::Web && digest_due(&settings).is_some_and(|at| at <= now_secs) {
            let project = workspace.store()?.project(project_id)?;
            if let Err(error) = mail(workspace, &project, &settings) {
                tracing::warn!(project = project_id, %error, "the alert digest was not sent");
            }
            workspace.store()?.note_alert_digest(project_id, &now())?;
        }
    }
    Ok(started)
}

/// When links are next checked or a digest sent, seconds since the Unix epoch (the periodic
/// look for alerts rides on other work and does not wake the schedule).
///
/// # Errors
/// Storage errors.
pub fn next_due(workspace: &Workspace) -> Result<Option<i64>, AppError> {
    let mut next: Option<i64> = None;
    for (_, settings, _) in workspace.store()?.alert_plans()? {
        if !settings.enabled {
            continue;
        }
        let mut times = Vec::new();
        if settings.rules.links {
            times.push(due(settings.links_last_run.as_deref(), LINKS_EVERY));
        }
        if workspace.host() == Host::Web
            && let Some(at) = digest_due(&settings)
        {
            times.push(at);
        }
        next = times.into_iter().chain(next).min();
    }
    Ok(next)
}

// ----- Commands ------------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct ProjectAsk {
    project_id: i64,
}

/// A project's alerting as its page shows it.
#[derive(Debug, Serialize)]
struct AlertSetup {
    settings: AlertSettings,
    /// E-mails can be sent from here (the web, with mail set up).
    can_email: bool,
    /// System notifications can be shown here (the desktop).
    can_notify: bool,
    /// This copy looks for alerts (a syncing desktop leaves it to the web).
    evaluates_here: bool,
    /// What the weekly link check costs, list price.
    links_usd: f64,
    links_estimate: Option<Estimate>,
}

fn setup(workspace: &Workspace, project_id: i64) -> Result<AlertSetup, AppError> {
    let settings = {
        let store = workspace.store()?;
        store.project(project_id)?;
        store.alert_settings(project_id)?
    };
    Ok(AlertSetup {
        settings,
        can_email: workspace.host() == Host::Web && workspace.mailer.is_some(),
        can_notify: workspace.notifier.is_some(),
        evaluates_here: workspace.schedules_here(),
        links_usd: links_usd(),
        links_estimate: workspace
            .estimate_spend(octopuson_dataforseo::ID, links_usd())
            .ok(),
    })
}

fn alert_setup(context: &Context<'_>, ask: ProjectAsk) -> Result<AlertSetup, AppError> {
    setup(context.workspace, ask.project_id)
}

#[derive(Debug, Deserialize)]
struct SettingsAsk {
    project_id: i64,
    settings: AlertSettings,
}

fn save_alert_settings(context: &Context<'_>, ask: SettingsAsk) -> Result<AlertSetup, AppError> {
    context
        .workspace
        .store()?
        .save_alert_settings(ask.project_id, &ask.settings)?;
    setup(context.workspace, ask.project_id)
}

#[derive(Debug, Deserialize)]
struct ListAsk {
    /// Every project's when absent.
    #[serde(default)]
    project_id: Option<i64>,
    #[serde(default)]
    unread: bool,
    #[serde(default)]
    limit: Option<u32>,
}

/// An alert with its project's name.
#[derive(Debug, Serialize)]
struct Listed {
    #[serde(flatten)]
    row: AlertRow,
    project: String,
}

#[derive(Debug, Serialize)]
struct AlertList {
    alerts: Vec<Listed>,
    /// Unread alerts in all.
    unread: u64,
    /// Unread alerts by project.
    by_project: Map<String, Value>,
}

fn list_alerts(context: &Context<'_>, ask: ListAsk) -> Result<AlertList, AppError> {
    let store = context.workspace.store()?;
    let names: std::collections::HashMap<i64, String> = store
        .projects()?
        .into_iter()
        .map(|project| (project.id, project.name))
        .collect();
    let alerts = store
        .alerts(
            ask.project_id,
            ask.unread,
            ask.limit.unwrap_or(50).clamp(1, 500),
        )?
        .into_iter()
        .map(|row| Listed {
            project: names.get(&row.project_id).cloned().unwrap_or_default(),
            row,
        })
        .collect();
    let unread = store.unread_alerts()?;
    Ok(AlertList {
        alerts,
        unread: unread.iter().map(|(_, n)| n).sum(),
        by_project: unread
            .into_iter()
            .map(|(id, n)| (id.to_string(), json!(n)))
            .collect(),
    })
}

#[derive(Debug, Deserialize)]
struct ReadAsk {
    #[serde(default)]
    project_id: Option<i64>,
    /// The alerts read; every unread one when absent.
    #[serde(default)]
    ids: Option<Vec<i64>>,
}

fn mark_alerts_read(context: &Context<'_>, ask: ReadAsk) -> Result<usize, AppError> {
    Ok(context
        .workspace
        .store()?
        .mark_alerts_read(ask.project_id, ask.ids.as_deref(), &now())?)
}

fn check_alerts(context: &Context<'_>, ask: ProjectAsk) -> Result<usize, AppError> {
    if !context.workspace.schedules_here() {
        return Err(AppError::Invalid(
            "alerts are looked for on the web while this computer syncs with the account"
                .to_owned(),
        ));
    }
    Ok(evaluate(context.workspace, ask.project_id)?.len())
}

fn start_link_watch(context: &Context<'_>, ask: ProjectAsk) -> Result<String, AppError> {
    let estimate = context
        .workspace
        .estimate_spend(octopuson_dataforseo::ID, links_usd())?;
    if let Some(blocked) = estimate.blocked {
        return Err(match blocked.code {
            crate::router::ErrorCode::Budget => AppError::Budget(blocked.message),
            _ => AppError::NoSource(blocked.message),
        });
    }
    let project = context.workspace.store()?.project(ask.project_id)?;
    start_links(context.workspace, context.jobs, &project)
}

/// Adds the module's commands.
pub fn register(router: &mut Router) {
    router.add("alert_setup", alert_setup);
    router.add("save_alert_settings", save_alert_settings);
    router.add("list_alerts", list_alerts);
    router.add("mark_alerts_read", mark_alerts_read);
    router.add("check_alerts", check_alerts);
    router.add("start_link_watch", start_link_watch);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn digests_wait_for_their_hour_and_lines_read_in_the_language() {
        let settings = AlertSettings {
            email: "daily".to_owned(),
            recipients: vec!["a@b.co".to_owned()],
            ..AlertSettings::default()
        };
        assert_eq!(digest_due(&settings), Some(DIGEST_HOUR * 3_600));
        let last = seconds_of("2026-10-22T23:30:00Z").unwrap();
        let after = AlertSettings {
            last_digest_at: Some("2026-10-22T23:30:00Z".to_owned()),
            ..settings.clone()
        };
        assert_eq!(digest_due(&after), Some(last + DIGEST_EVERY));
        let off = AlertSettings {
            email: "instant".to_owned(),
            ..settings
        };
        assert_eq!(digest_due(&off), None);
        let gsc = alert(
            "gsc|x".to_owned(),
            "gsc_drop",
            "bad",
            "gsc",
            json!({}),
            json!({ "drop": 31.5, "before": 1200, "clicks": 822 }),
        );
        assert_eq!(
            line(&gsc, "en"),
            "Search Console clicks fell 31.5%: 1,200 → 822"
        );
        assert_eq!(
            line(&gsc, "tr"),
            "Search Console tıklamaları %31,5 düştü: 1.200 → 822"
        );
        assert_eq!(moved("bot", 8.0, None), "bot (8→—)");
    }
}
