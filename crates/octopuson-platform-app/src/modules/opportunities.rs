//! Opportunities: the work that would bring the most for the least effort, worked out by rules
//! from what the other modules already hold (nothing is paid): tracked keywords and queries just
//! below the first places, results clicked less than their position earns, pages losing
//! clicks, queries split over several pages, lost keywords and links, high-priority audit
//! findings, orphan and weakly linked pages, Core Web Vitals, AI answers naming competitors
//! but not the site, weak content scores, the local grid, competitors growing faster.
//!
//! Each opportunity has an estimated gain (clicks a month) where one can be estimated, an
//! impact (0–100), an effort and a priority (impact weighed by effort). The rules read the
//! modules' own reports, so they are worked out again whenever they are shown; only what the
//! user decides (done, ignored, snoozed) is kept.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::Arc;

use octopuson_core::Timestamp;
use octopuson_platform::{ProjectRow, unix_now};
use octopuson_serp_score::rank::{ctr, seconds_of};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::router::{Context, Router};
use crate::snapshot::{SnapshotData, ask};
use crate::{AppError, Workspace, now};

/// Clicks a month at which the impact reaches 100.
const FULL_GAIN: f64 = 5_000.0;

/// A done opportunity comes back after this many days if its rule still finds it.
const DONE_DAYS: i64 = 30;

/// Opportunities of one rule kept at most.
const PER_RULE: usize = 25;

/// Keywords with fewer searches a month are left out.
const MIN_VOLUME: f64 = 10.0;

/// Something worth doing.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct Opportunity {
    /// Stable: the rule and what it is about.
    pub(crate) key: String,
    pub(crate) rule: &'static str,
    /// The module page that shows it.
    pub(crate) page: &'static str,
    /// Its tab and what it shows (`keyword`, `url`).
    pub(crate) params: BTreeMap<&'static str, String>,
    /// The keyword, address, check or question.
    pub(crate) subject: String,
    /// Numbers and names its text uses.
    pub(crate) detail: BTreeMap<&'static str, Value>,
    /// Estimated extra clicks a month.
    pub(crate) gain: Option<f64>,
    /// 0–100.
    pub(crate) impact: f64,
    /// `low`, `medium` or `high`.
    pub(crate) effort: &'static str,
    /// Impact weighed by effort.
    pub(crate) priority: f64,
    /// `open`, `done`, `ignored` or `snoozed`.
    pub(crate) state: String,
    pub(crate) until: Option<String>,
    /// Done more than a month ago and found again.
    pub(crate) returned: bool,
}

fn round(value: f64) -> f64 {
    (value * 10.0).round() / 10.0
}

/// Impact of a gain in clicks a month: logarithmic, 100 at [`FULL_GAIN`].
fn impact_of(gain: f64) -> f64 {
    round((100.0 * (1.0 + gain.max(0.0)).ln() / (1.0 + FULL_GAIN).ln()).clamp(5.0, 100.0))
}

fn effort_factor(effort: &str) -> f64 {
    match effort {
        "low" => 1.0,
        "medium" => 0.75,
        _ => 0.5,
    }
}

struct Draft {
    rule: &'static str,
    page: &'static str,
    subject: String,
    params: Vec<(&'static str, String)>,
    detail: Vec<(&'static str, Value)>,
    gain: Option<f64>,
    impact: f64,
    effort: &'static str,
}

impl Draft {
    fn with_gain(
        rule: &'static str,
        page: &'static str,
        subject: &str,
        gain: f64,
        effort: &'static str,
    ) -> Self {
        Self {
            rule,
            page,
            subject: subject.to_owned(),
            params: Vec::new(),
            detail: Vec::new(),
            gain: Some(gain.round()),
            impact: impact_of(gain),
            effort,
        }
    }

    fn with_impact(
        rule: &'static str,
        page: &'static str,
        subject: &str,
        impact: f64,
        effort: &'static str,
    ) -> Self {
        Self {
            rule,
            page,
            subject: subject.to_owned(),
            params: Vec::new(),
            detail: Vec::new(),
            gain: None,
            impact: round(impact.clamp(5.0, 100.0)),
            effort,
        }
    }

    fn param(mut self, name: &'static str, value: &str) -> Self {
        self.params.push((name, value.to_owned()));
        self
    }

    fn detail(mut self, name: &'static str, value: impl Into<Value>) -> Self {
        self.detail.push((name, value.into()));
        self
    }

    fn done(self) -> Opportunity {
        Opportunity {
            key: format!("{}|{}", self.rule, self.subject),
            rule: self.rule,
            page: self.page,
            params: self.params.into_iter().collect(),
            subject: self.subject,
            detail: self.detail.into_iter().collect(),
            gain: self.gain,
            priority: round(self.impact * effort_factor(self.effort)),
            impact: self.impact,
            effort: self.effort,
            state: "open".to_owned(),
            until: None,
            returned: false,
        }
    }
}

fn num(value: &Value, key: &str) -> Option<f64> {
    value.get(key).and_then(Value::as_f64)
}

fn text<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(Value::as_str)
}

fn list<'a>(value: &'a Value, key: &str) -> &'a [Value] {
    value
        .get(key)
        .and_then(Value::as_array)
        .map_or(&[], Vec::as_slice)
}

/// A place as a whole position (1–100).
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // clamped
fn place(position: f64) -> u16 {
    position.round().clamp(1.0, 100.0) as u16
}

/// Which sources had data, so the page can say what to connect for more.
#[derive(Debug, Default, Clone, Serialize)]
#[allow(clippy::struct_excessive_bools)] // one flag per source
pub(crate) struct Sources {
    rank: bool,
    gsc: bool,
    audit: bool,
    vitals: bool,
    ai: bool,
    content: bool,
    local: bool,
    snapshots: bool,
}

/// Tracked keywords: just below the first places, and dropped or lost.
fn rank_rules(report: &Value, out: &mut Vec<Draft>) -> bool {
    let rows = list(report, "rows");
    for row in rows {
        let Some(keyword) = text(row, "keyword") else {
            continue;
        };
        let volume = num(row, "volume").unwrap_or(0.0);
        if volume < MIN_VOLUME {
            continue;
        }
        let position = num(row, "position");
        let previous = num(row, "previous");
        if let Some(position) = position.filter(|position| (4.0..=20.0).contains(position)) {
            let gain = volume * (ctr(3) - ctr(place(position)));
            out.push(
                Draft::with_gain(
                    "near_top",
                    "rank",
                    keyword,
                    gain,
                    if position <= 10.0 { "medium" } else { "high" },
                )
                .param("keyword", keyword)
                .detail("position", position)
                .detail("volume", volume)
                .detail("source", "rank"),
            );
        }
        match (position, previous) {
            (None, Some(before)) if before <= 50.0 => out.push(
                Draft::with_gain(
                    "dropped",
                    "rank",
                    keyword,
                    volume * ctr(place(before)).max(0.005),
                    "medium",
                )
                .param("keyword", keyword)
                .detail("before", before)
                .detail("volume", volume)
                .detail("lost", true),
            ),
            (Some(now), Some(before)) if now - before >= 5.0 && before <= 20.0 => out.push(
                Draft::with_gain(
                    "dropped",
                    "rank",
                    keyword,
                    volume * (ctr(place(before)) - ctr(place(now))).max(0.0),
                    "medium",
                )
                .param("keyword", keyword)
                .detail("before", before)
                .detail("position", now)
                .detail("volume", volume)
                .detail("lost", false),
            ),
            _ => {}
        }
    }
    !rows.is_empty()
}

/// Days a Search Console period covers.
fn period_days(report: &Value) -> f64 {
    let day = |index: usize| {
        report
            .get("period")
            .and_then(|period| period.get(index))
            .and_then(Value::as_str)
            .and_then(octopuson_core::Date::parse)
            .map(octopuson_core::Date::days_since_epoch)
    };
    match (day(0), day(1)) {
        (Some(start), Some(end)) if end >= start => f64::from(end - start + 1),
        _ => 28.0,
    }
}

/// Search Console: queries near the top or clicked too little, pages losing clicks, queries
/// split over pages.
fn gsc_rules(report: &Value, tracked: &BTreeSet<String>, out: &mut Vec<Draft>) {
    let month = 30.0 / period_days(report);
    for row in list(report, "queries") {
        let Some(query) = text(row, "query") else {
            continue;
        };
        let impressions = num(row, "impressions").unwrap_or(0.0) * month;
        let position = num(row, "position").unwrap_or(100.0);
        let brand = row.get("brand").and_then(Value::as_bool).unwrap_or(false);
        if row.get("near_top").and_then(Value::as_bool) == Some(true)
            && !brand
            && !tracked.contains(&query.to_lowercase())
        {
            let gain = impressions * (ctr(3) - ctr(place(position)));
            out.push(
                Draft::with_gain(
                    "near_top",
                    "gsc",
                    query,
                    gain,
                    if position <= 10.0 { "medium" } else { "high" },
                )
                .param("keyword", query)
                .detail("position", round(position))
                .detail("impressions", impressions.round())
                .detail("source", "gsc"),
            );
        }
        if row.get("low_ctr").and_then(Value::as_bool) == Some(true) && !brand {
            let actual = num(row, "ctr").unwrap_or(0.0) / 100.0;
            let expected = ctr(place(position));
            out.push(
                Draft::with_gain(
                    "low_ctr",
                    "gsc",
                    query,
                    impressions * (expected - actual).max(0.0),
                    "low",
                )
                .param("keyword", query)
                .detail("position", round(position))
                .detail("ctr", num(row, "ctr").unwrap_or(0.0))
                .detail("expected", round(expected * 100.0)),
            );
        }
    }
    for row in list(report, "pages") {
        let (Some(page), Some(change), Some(clicks)) = (
            text(row, "page"),
            num(row, "clicks_change"),
            num(row, "clicks"),
        ) else {
            continue;
        };
        let before = clicks - change;
        if change <= -10.0 && before > 0.0 && change / before <= -0.2 {
            out.push(
                Draft::with_gain("losing_page", "gsc", page, -change * month, "medium")
                    .param("url", page)
                    .detail("clicks", clicks)
                    .detail("before", before)
                    .detail("drop", round(-change / before * 100.0)),
            );
        }
    }
    for row in list(report, "cannibalised") {
        let Some(query) = text(row, "query") else {
            continue;
        };
        let impressions = num(row, "impressions").unwrap_or(0.0) * month;
        let pages = list(row, "pages").len();
        if impressions >= 100.0 {
            out.push(
                Draft::with_gain("cannibal", "gsc", query, impressions * 0.02, "medium")
                    .param("keyword", query)
                    .detail("pages", f64::from(u32::try_from(pages).unwrap_or(u32::MAX)))
                    .detail("impressions", impressions.round()),
            );
        }
    }
}

/// The latest audit: high-priority checks with findings.
fn audit_rules(report: &Value, out: &mut Vec<Draft>) -> bool {
    let mut found = false;
    for category in list(report, "categories") {
        for issue in list(category, "issues") {
            found = true;
            let count = num(issue, "count").unwrap_or(0.0);
            if text(issue, "priority") != Some("high") || count <= 0.0 {
                continue;
            }
            let (Some(id), Some(title)) = (text(issue, "id"), text(issue, "title")) else {
                continue;
            };
            out.push(
                Draft::with_impact(
                    "audit_issue",
                    "audit",
                    id,
                    30.0 + 15.0 * (1.0 + count).log10(),
                    if text(issue, "kind") == Some("opportunity") {
                        "low"
                    } else {
                        "medium"
                    },
                )
                .param("tab", "issues")
                .detail("title", title)
                .detail("count", count),
            );
        }
    }
    found || report.get("audit").is_some()
}

/// The latest crawl's links: orphan and weakly linked pages.
fn link_rules(report: &Value, out: &mut Vec<Draft>) {
    let orphans = num(report, "orphan_total").unwrap_or(0.0);
    if orphans > 0.0 {
        out.push(
            Draft::with_impact(
                "orphans",
                "audit",
                "orphans",
                25.0 + 10.0 * (1.0 + orphans).log10(),
                "medium",
            )
            .param("tab", "links")
            .detail("count", orphans),
        );
    }
    let weak = num(report, "weak_total").unwrap_or(0.0);
    if weak > 0.0 {
        out.push(
            Draft::with_impact(
                "weak_links",
                "audit",
                "weak",
                20.0 + 10.0 * (1.0 + weak).log10(),
                "low",
            )
            .param("tab", "links")
            .detail("count", weak),
        );
    }
    let deep = num(report, "deep").unwrap_or(0.0);
    let pages = num(report, "pages").unwrap_or(0.0);
    if deep > 0.0 && pages > 0.0 && deep / pages >= 0.1 {
        out.push(
            Draft::with_impact(
                "deep_pages",
                "audit",
                "deep",
                20.0 + 10.0 * (1.0 + deep).log10(),
                "high",
            )
            .param("tab", "links")
            .detail("count", deep)
            .detail("share", round(deep / pages * 100.0)),
        );
    }
}

/// Core Web Vitals: a site failing on phones, slow pages.
fn vitals_rules(report: &Value, out: &mut Vec<Draft>) -> bool {
    let origins = list(report, "origins");
    let pages = list(report, "pages");
    if let Some(phone) = origins
        .iter()
        .find(|origin| text(origin, "device") == Some("phone"))
        && phone.get("passes").and_then(Value::as_bool) == Some(false)
    {
        let field = phone.get("field").unwrap_or(&Value::Null);
        let failing: Vec<String> = [("lcp", 2_500.0), ("inp", 200.0), ("cls", 0.1)]
            .iter()
            .filter(|(name, good)| {
                field
                    .get(*name)
                    .and_then(|metric| num(metric, "p75"))
                    .is_some_and(|p75| p75 > *good)
            })
            .map(|(name, _)| name.to_uppercase())
            .collect();
        out.push(
            Draft::with_impact(
                "vitals",
                "audit",
                text(phone, "origin").unwrap_or("site"),
                60.0,
                "high",
            )
            .param("tab", "vitals")
            .detail("metrics", failing.join(", ")),
        );
    }
    for page in pages {
        let (Some(url), Some(score)) = (
            text(page, "url"),
            page.get("lab").and_then(|lab| num(lab, "score")),
        ) else {
            continue;
        };
        if score < 50.0 && text(page, "device") == Some("phone") {
            out.push(
                Draft::with_impact(
                    "slow_page",
                    "audit",
                    url,
                    25.0 + (50.0 - score) / 2.0,
                    "medium",
                )
                .param("tab", "vitals")
                .detail("score", score),
            );
        }
    }
    !origins.is_empty() || !pages.is_empty()
}

/// AI answers naming a competitor but not the site.
fn ai_rules(report: &Value, out: &mut Vec<Draft>) -> bool {
    let prompts = list(report, "prompts");
    for prompt in prompts {
        let Some(question) = text(prompt, "prompt") else {
            continue;
        };
        let mut engines = 0_u32;
        let mut rivals: BTreeSet<String> = BTreeSet::new();
        for cell in prompt
            .get("results")
            .and_then(Value::as_object)
            .into_iter()
            .flat_map(|results| results.values())
        {
            if cell.get("mentioned").and_then(Value::as_bool) == Some(false)
                && let Some(rival) = cell.get("rival").and_then(|rival| text(rival, "key"))
            {
                engines += 1;
                rivals.insert(rival.to_owned());
            }
        }
        if engines > 0 {
            out.push(
                Draft::with_impact(
                    "ai_gap",
                    "ai",
                    question,
                    30.0 + 10.0 * f64::from(engines),
                    "high",
                )
                .detail("engines", f64::from(engines))
                .detail("rivals", rivals.into_iter().collect::<Vec<_>>().join(", ")),
            );
        }
    }
    !prompts.is_empty()
}

/// Pages whose content scored low against their keyword's ranking pages.
fn content_rules(pages: &Value, out: &mut Vec<Draft>) -> bool {
    let mut found = false;
    for page in pages.as_array().map_or(&[][..], Vec::as_slice) {
        let Some(score) = num(page, "score") else {
            continue;
        };
        found = true;
        let Some(url) = text(page, "url") else {
            continue;
        };
        if score < 50.0 {
            out.push(
                Draft::with_impact(
                    "low_content",
                    "content",
                    url,
                    25.0 + (50.0 - score) / 2.0,
                    "medium",
                )
                .param("tab", "checker")
                .detail("score", score)
                .detail("keyword", text(page, "scored_for").unwrap_or_default()),
            );
        }
    }
    found
}

/// The local grid: keywords where the business shows far down or rarely.
fn local_rules(report: &Value, out: &mut Vec<Draft>) -> bool {
    let keywords = list(report, "keywords");
    for keyword in keywords {
        let (Some(name), Some(summary)) = (text(keyword, "keyword"), keyword.get("summary")) else {
            continue;
        };
        let solv = num(summary, "solv").unwrap_or(0.0);
        let arp = num(summary, "arp");
        if solv < 30.0 || arp.is_some_and(|arp| arp > 10.0) {
            out.push(
                Draft::with_impact(
                    "local_weak",
                    "local",
                    name,
                    35.0 + (30.0 - solv).max(0.0) / 2.0,
                    "medium",
                )
                .param("keyword", name)
                .detail("solv", round(solv))
                .detail("arp", arp.map_or(Value::Null, |arp| json!(round(arp)))),
            );
        }
    }
    !keywords.is_empty()
}

/// Competitors whose ranked keywords grew much faster than the site's between the last two
/// snapshots.
fn snapshot_rules(
    workspace: &Workspace,
    project: &ProjectRow,
    out: &mut Vec<Draft>,
) -> Result<bool, AppError> {
    let finished: Vec<SnapshotData> = workspace
        .store()?
        .snapshots(project.id)?
        .into_iter()
        .filter(|row| matches!(row.status.as_str(), "done" | "partial"))
        .filter_map(|row| row.data.and_then(|data| serde_json::from_value(data).ok()))
        .take(2)
        .collect();
    let [now, before] = finished.as_slice() else {
        return Ok(false);
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
    let earlier: HashMap<&str, f64> = before
        .competitors
        .iter()
        .filter_map(|rival| Some((rival.domain.as_str(), rival.overview.as_ref()?.keywords)))
        .collect();
    for rival in &now.competitors {
        let (Some(metrics), Some(was)) =
            (rival.overview.as_ref(), earlier.get(rival.domain.as_str()))
        else {
            continue;
        };
        let rate = growth(metrics.keywords, *was);
        if rate >= 0.1 && metrics.keywords - was >= 50.0 && rate - own >= 0.05 {
            out.push(
                Draft::with_impact(
                    "rival_growth",
                    "gap",
                    &rival.domain,
                    30.0 + 40.0 * rate.min(1.0),
                    "high",
                )
                .param("domain", &rival.domain)
                .detail("gained", (metrics.keywords - was).round())
                .detail("growth", round(rate * 100.0))
                .detail("own", round(own * 100.0)),
            );
        }
    }
    Ok(true)
}

/// What an evaluation found.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct Evaluation {
    /// Shown ones, highest priority first (every one with `all`).
    pub(crate) items: Vec<Opportunity>,
    /// Open ones in all.
    pub(crate) open: usize,
    /// Hidden by decision: done, ignored, snoozed.
    pub(crate) hidden: BTreeMap<String, usize>,
    pub(crate) sources: Sources,
    /// Estimated clicks a month of the open ones together.
    pub(crate) gain: f64,
}

/// Works out a project's opportunities and applies what the user decided.
#[allow(clippy::too_many_lines)] // every source, then the decisions
pub(crate) fn evaluate(
    workspace: &Arc<Workspace>,
    project: &ProjectRow,
    lang: &str,
    all: bool,
) -> Result<Evaluation, AppError> {
    let id = project.id;
    let report = |command: &str, arguments: Value| {
        ask(workspace, command, arguments)
            .ok()
            .filter(|value| !value.is_null())
    };
    let mut drafts: Vec<Draft> = Vec::new();
    let mut sources = Sources::default();
    let tracked: BTreeSet<String> = workspace
        .store()?
        .tracked_keywords(id)?
        .into_iter()
        .map(|keyword| keyword.keyword.to_lowercase())
        .collect();
    if let Some(rank) = report("rank_report", json!({ "project_id": id, "days": 30 })) {
        sources.rank = rank_rules(&rank, &mut drafts);
    }
    if let Some(gsc) = report("gsc_report", json!({ "project_id": id })) {
        sources.gsc = true;
        gsc_rules(&gsc, &tracked, &mut drafts);
    }
    if let Some(audit) = report("audit_report", json!({ "project_id": id, "lang": lang })) {
        sources.audit = audit_rules(&audit, &mut drafts);
        if let Some(links) = report("audit_links", json!({ "project_id": id })) {
            link_rules(&links, &mut drafts);
        }
    }
    if let Some(vitals) = report("vitals_report", json!({ "project_id": id })) {
        sources.vitals = vitals_rules(&vitals, &mut drafts);
    }
    if let Some(ai) = report("ai_report", json!({ "project_id": id, "days": 30 })) {
        sources.ai = ai_rules(&ai, &mut drafts);
    }
    if let Some(pages) = report("content_pages", json!({ "project_id": id })) {
        sources.content = content_rules(&pages, &mut drafts);
    }
    if let Some(local) = report("local_grid", json!({ "project_id": id })) {
        sources.local = local_rules(&local, &mut drafts);
    }
    sources.snapshots = snapshot_rules(workspace, project, &mut drafts)?;
    let lost = workspace
        .store()?
        .prospects(id)?
        .into_iter()
        .filter(|prospect| prospect.source == "lost" && prospect.status == "prospect")
        .count();
    if lost > 0 {
        let lost = f64::from(u32::try_from(lost).unwrap_or(u32::MAX));
        drafts.push(
            Draft::with_impact(
                "lost_links",
                "backlinks",
                "lost",
                25.0 + 10.0 * (1.0 + lost).log10(),
                "medium",
            )
            .param("tab", "prospects")
            .detail("count", lost),
        );
    }

    // One opportunity per key (a keyword tracked and in Search Console counts once), the most
    // valuable of each rule.
    let mut seen = BTreeSet::new();
    let mut by_rule: HashMap<&'static str, usize> = HashMap::new();
    drafts.sort_by(|a, b| {
        (b.impact * effort_factor(b.effort))
            .total_cmp(&(a.impact * effort_factor(a.effort)))
            .then_with(|| a.subject.cmp(&b.subject))
    });
    let mut items: Vec<Opportunity> = Vec::new();
    for draft in drafts {
        let opportunity = draft.done();
        let count = by_rule.entry(opportunity.rule).or_default();
        if *count >= PER_RULE || !seen.insert(opportunity.key.clone()) {
            continue;
        }
        *count += 1;
        items.push(opportunity);
    }

    let decisions: HashMap<String, octopuson_platform::OpportunityState> = workspace
        .store()?
        .opportunity_states(id)?
        .into_iter()
        .map(|state| (state.key.clone(), state))
        .collect();
    let now_secs = unix_now();
    let mut hidden: BTreeMap<String, usize> = BTreeMap::new();
    let mut shown = Vec::new();
    for mut item in items {
        let visible = match decisions.get(&item.key) {
            None => true,
            Some(decision) => {
                item.state.clone_from(&decision.state);
                item.until.clone_from(&decision.until);
                match decision.state.as_str() {
                    "snoozed" => decision
                        .until
                        .as_deref()
                        .and_then(seconds_of)
                        .is_none_or(|until| until <= now_secs),
                    "done" => {
                        let back = seconds_of(&decision.changed_at)
                            .is_some_and(|at| now_secs - at > DONE_DAYS * 86_400);
                        item.returned = back;
                        back
                    }
                    _ => false,
                }
            }
        };
        if visible {
            "open".clone_into(&mut item.state);
            shown.push(item);
        } else {
            *hidden.entry(item.state.clone()).or_default() += 1;
            if all {
                shown.push(item);
            }
        }
    }
    let open: Vec<&Opportunity> = shown.iter().filter(|item| item.state == "open").collect();
    Ok(Evaluation {
        open: open.len(),
        gain: open.iter().filter_map(|item| item.gain).sum(),
        items: shown,
        hidden,
        sources,
    })
}

#[derive(Debug, Deserialize)]
struct ListAsk {
    project_id: i64,
    #[serde(default)]
    lang: Option<String>,
    /// Hidden ones too (done, ignored, snoozed).
    #[serde(default)]
    all: bool,
}

fn opportunities(context: &Context<'_>, ask: ListAsk) -> Result<Evaluation, AppError> {
    let project = context.workspace.store()?.project(ask.project_id)?;
    evaluate(
        context.workspace,
        &project,
        ask.lang.as_deref().unwrap_or("tr"),
        ask.all,
    )
}

#[derive(Debug, Deserialize)]
struct DecideAsk {
    project_id: i64,
    key: String,
    /// `open`, `done`, `ignored` or `snoozed`.
    state: String,
    /// Snoozed for this many days (1–365).
    #[serde(default)]
    days: Option<u32>,
}

fn set_opportunity(context: &Context<'_>, ask: DecideAsk) -> Result<(), AppError> {
    let until = (ask.state == "snoozed").then(|| {
        let days = i64::from(ask.days.unwrap_or(7).clamp(1, 365));
        let at = unix_now() + days * 86_400;
        Timestamp::from_unix_micros(at.saturating_mul(1_000_000)).to_string()
    });
    Ok(context.workspace.store()?.set_opportunity_state(
        ask.project_id,
        &ask.key,
        &ask.state,
        until.as_deref(),
        &now(),
    )?)
}

/// Adds the module's commands.
pub fn register(router: &mut Router) {
    router.add("opportunities", opportunities);
    router.add("set_opportunity", set_opportunity);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rules_find_keywords_near_the_top_drops_and_low_clicks() {
        let rank = json!({ "rows": [
            { "keyword": "spor ayakkabı", "volume": 9900, "position": 6, "previous": 7 },
            { "keyword": "bot", "volume": 2400, "position": null, "previous": 12 },
            { "keyword": "terlik", "volume": 1000, "position": 18, "previous": 9 },
            { "keyword": "nadir", "volume": 5, "position": 5, "previous": 5 }
        ]});
        let mut drafts = Vec::new();
        assert!(rank_rules(&rank, &mut drafts));
        let found: Vec<(&str, &str)> = drafts
            .iter()
            .map(|draft| (draft.rule, draft.subject.as_str()))
            .collect();
        assert_eq!(
            found,
            [
                ("near_top", "spor ayakkabı"),
                ("dropped", "bot"),
                ("near_top", "terlik"),
                ("dropped", "terlik")
            ]
        );
        let near = &drafts[0];
        assert_eq!(near.gain, Some((9900.0 * (ctr(3) - ctr(6))).round()));
        assert_eq!(near.effort, "medium");

        let gsc = json!({
            "period": ["2026-10-01", "2026-10-14"],
            "queries": [
                { "query": "beyaz sneaker", "impressions": 7000, "position": 8.2, "ctr": 0.4, "near_top": true, "low_ctr": true, "brand": false },
                { "query": "spor ayakkabı", "impressions": 7000, "position": 6, "ctr": 3, "near_top": true, "low_ctr": false }
            ],
            "pages": [{ "page": "https://a.com/x", "clicks": 40, "clicks_change": -60 }],
            "cannibalised": [{ "query": "bot", "impressions": 900, "pages": [{}, {}] }]
        });
        let tracked: BTreeSet<String> = ["spor ayakkabı".to_owned()].into();
        let mut drafts = Vec::new();
        gsc_rules(&gsc, &tracked, &mut drafts);
        let rules: Vec<&str> = drafts.iter().map(|draft| draft.rule).collect();
        assert_eq!(rules, ["near_top", "low_ctr", "losing_page", "cannibal"]);
        // Fourteen days count as a month's worth: twice the clicks.
        assert_eq!(drafts[2].gain, Some(120.0 + 9.0));
        assert_eq!(drafts[1].effort, "low");
        assert_eq!(impact_of(0.0), 5.0);
        assert_eq!(impact_of(FULL_GAIN), 100.0);
    }

    #[test]
    fn audit_ai_vitals_and_content_rules_read_their_reports() {
        let audit = json!({ "audit": {}, "categories": [{ "issues": [
            { "id": "title-missing", "title": "Title missing", "kind": "issue", "priority": "high", "count": 12 },
            { "id": "h2-missing", "title": "No H2", "kind": "warning", "priority": "low", "count": 40 }
        ]}]});
        let mut drafts = Vec::new();
        assert!(audit_rules(&audit, &mut drafts));
        assert_eq!(drafts.len(), 1);
        assert_eq!(drafts[0].subject, "title-missing");
        link_rules(
            &json!({ "orphan_total": 6, "weak_total": 0, "deep": 50, "pages": 200 }),
            &mut drafts,
        );
        assert_eq!(
            drafts.iter().map(|draft| draft.rule).collect::<Vec<_>>(),
            ["audit_issue", "orphans", "deep_pages"]
        );

        let ai = json!({ "prompts": [{ "prompt": "en iyi koşu ayakkabısı", "results": {
            "chatgpt": { "mentioned": false, "rival": { "key": "rakip.com", "position": 1 } },
            "claude": { "mentioned": true, "rival": null },
            "gemini": { "mentioned": false, "rival": { "key": "diger.com", "position": 2 } }
        }}]});
        let mut drafts = Vec::new();
        assert!(ai_rules(&ai, &mut drafts));
        assert_eq!(
            drafts[0].detail[1],
            ("rivals", json!("diger.com, rakip.com"))
        );
        assert_eq!(drafts[0].impact, 50.0);

        let vitals = json!({ "origins": [{ "origin": "https://a.com", "device": "phone", "passes": false, "field": { "lcp": { "p75": 3100 }, "cls": { "p75": 0.02 }, "inp": { "p75": 250 } } }],
            "pages": [{ "url": "https://a.com/", "device": "phone", "lab": { "score": 31 } }] });
        let mut drafts = Vec::new();
        assert!(vitals_rules(&vitals, &mut drafts));
        assert_eq!(drafts[0].detail[0], ("metrics", json!("LCP, INP")));
        assert_eq!(drafts[1].impact, 34.5);

        let mut drafts = Vec::new();
        assert!(content_rules(
            &json!([{ "url": "https://a.com/k", "score": 38, "scored_for": "koşu" }, { "url": "https://a.com/l", "score": null }]),
            &mut drafts
        ));
        assert_eq!(drafts.len(), 1);
        assert!(local_rules(
            &json!({ "keywords": [{ "keyword": "kahve", "summary": { "solv": 12, "arp": 14.2 } }] }),
            &mut drafts
        ));
        assert_eq!(drafts[1].rule, "local_weak");
    }
}
