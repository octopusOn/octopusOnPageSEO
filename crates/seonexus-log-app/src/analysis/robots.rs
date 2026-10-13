//! robots.txt compliance: which bots requested URLs the site's robots.txt disallows them,
//! and which bots the robots.txt blocks entirely (useful for AI crawler policies).

use std::collections::{BTreeMap, BTreeSet};
use std::time::Duration;

use seonexus_botverify::Classifier;
use seonexus_botverify::robots::{RobotsTokens, robots_tokens};
use seonexus_core::table::{Cell, Column, ColumnKind, TableSink};
use seonexus_net::HttpClient;
use seonexus_store::Catalog;
use seonexus_url::robots::Robots;
use serde::Serialize;

use super::AnalysisError;
use super::collect::{for_each_row, integer, key, with_filter};
use crate::USER_AGENT;
use crate::bots::classifier;
use crate::query::QueryError;
use crate::settings::{SiteSettings, effective_host};
use crate::workspace::Workspace;

const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(30);

/// Input of [`robots_compliance`].
#[derive(Debug, Clone)]
pub struct RobotsRequest {
    /// Site name.
    pub site: String,
    /// robots.txt file or URL; `None` downloads `https://<site host>/robots.txt`.
    pub robots: Option<String>,
    /// Extra conditions, e.g. `date:last30d verification:verified`.
    pub filter: Option<String>,
}

/// How one bot treats the robots.txt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RobotsBotSummary {
    /// Bot id.
    pub bot: String,
    /// Display name.
    pub name: String,
    /// Category.
    pub category: String,
    /// `User-agent` group the bot obeys (`*` for the default group, `None` if no group
    /// applies).
    pub group: Option<String>,
    /// Whether the robots.txt disallows the whole site for this bot.
    pub blocked: bool,
    /// The operator says user-triggered fetches may not follow robots.txt.
    pub user_triggered: bool,
    /// Requests in the logs.
    pub hits: u64,
    /// Requests for disallowed URLs.
    pub disallowed_hits: u64,
    /// Requests for disallowed URLs from verified IPs (the real bot ignoring the rules).
    pub verified_disallowed_hits: u64,
    /// Distinct disallowed URLs requested.
    pub disallowed_urls: u64,
}

/// Result of [`robots_compliance`]; disallowed requests went to the sink.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RobotsReport {
    /// Site name.
    pub site: String,
    /// Where the robots.txt was read.
    pub robots: String,
    /// Filter applied to logged hits.
    pub filter: String,
    /// `Sitemap:` lines.
    pub sitemaps: Vec<String>,
    /// Bots seen in the logs, most disallowed hits first.
    pub bots: Vec<RobotsBotSummary>,
    /// Rows written (disallowed URL × bot × verdict).
    pub rows: u64,
}

/// Checks logged bot requests against a robots.txt.
///
/// # Errors
/// [`AnalysisError::Read`] when the robots.txt cannot be read; query errors.
pub fn robots_compliance(
    workspace: &Workspace,
    request: &RobotsRequest,
    sink: &mut dyn TableSink,
) -> Result<RobotsReport, AnalysisError> {
    let source = match request
        .robots
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        Some(source) => source.to_owned(),
        None => default_robots_url(workspace, &request.site)?,
    };
    let robots = Robots::parse(&load(&source)?);
    let filter = request.filter.clone().unwrap_or_default();
    let text = with_filter(
        Some(&filter),
        "bot:* group:bot,url,verification metric:hits,last_seen",
    );
    let mut collector = Collector::new(&robots, classifier(workspace));
    for_each_row(workspace, &request.site, &text, |cells| {
        collector.add(cells);
    })?;
    let (bots, rows) = collector.finish();

    sink.begin(&columns()).map_err(AnalysisError::Output)?;
    for row in &rows {
        sink.row(row).map_err(AnalysisError::Output)?;
    }
    sink.finish().map_err(AnalysisError::Output)?;
    Ok(RobotsReport {
        site: request.site.clone(),
        robots: source,
        filter,
        sitemaps: robots.sitemaps.clone(),
        bots,
        rows: rows.len() as u64,
    })
}

/// Accumulates per-bot summaries and disallowed rows from `(bot, url, verdict)` groups.
struct Collector<'r> {
    robots: &'r Robots,
    classifier: Classifier,
    /// robots.txt tokens per bot id (`None` for ids the catalog no longer knows).
    identities: BTreeMap<String, Option<RobotsTokens>>,
    bots: BTreeMap<String, RobotsBotSummary>,
    disallowed_urls: BTreeMap<String, BTreeSet<String>>,
    rows: Vec<Vec<Cell>>,
}

impl<'r> Collector<'r> {
    fn new(robots: &'r Robots, classifier: Classifier) -> Self {
        Self {
            robots,
            classifier,
            identities: BTreeMap::new(),
            bots: BTreeMap::new(),
            disallowed_urls: BTreeMap::new(),
            rows: Vec::new(),
        }
    }

    fn add(&mut self, cells: &[Cell]) {
        let (Some(bot_id), Some(Cell::Text(url))) = (key(cells.first()), cells.get(1)) else {
            return;
        };
        let verdict = key(cells.get(2)).unwrap_or_default();
        let hits = integer(cells.get(3));
        let classifier = self.classifier;
        let identity = self
            .identities
            .entry(bot_id.clone())
            .or_insert_with(|| classifier.find(&bot_id).map(robots_tokens));
        let Some(identity) = identity else {
            return;
        };
        let tokens: Vec<&str> = identity.tokens.iter().map(String::as_str).collect();
        let rules = self.robots.group_for(&tokens, identity.uses_default);
        let user_triggered = identity.user_triggered;
        let summary = self.bots.entry(bot_id.clone()).or_insert_with(|| {
            let bot = classifier.find(&bot_id);
            RobotsBotSummary {
                bot: bot_id.clone(),
                name: bot.map_or_else(|| bot_id.clone(), |b| b.name.to_owned()),
                category: bot.map_or_else(String::new, |b| b.category.name().to_owned()),
                group: rules.group.map(str::to_owned),
                blocked: rules.blocks_everything(),
                user_triggered,
                hits: 0,
                disallowed_hits: 0,
                verified_disallowed_hits: 0,
                disallowed_urls: 0,
            }
        });
        summary.hits += hits;
        let decision = rules.decide(url);
        if decision.allowed {
            return;
        }
        summary.disallowed_hits += hits;
        if verdict == "verified" {
            summary.verified_disallowed_hits += hits;
        }
        self.disallowed_urls
            .entry(bot_id.clone())
            .or_default()
            .insert(url.clone());
        let rule = decision.rule.map_or_else(String::new, |rule| {
            let kind = if rule.allow { "Allow" } else { "Disallow" };
            format!("{kind}: {}", rule.pattern)
        });
        self.rows.push(vec![
            Cell::Text(url.clone()),
            Cell::Text(bot_id),
            if verdict.is_empty() {
                Cell::Null
            } else {
                Cell::Text(verdict)
            },
            Cell::Integer(i64::try_from(hits).unwrap_or(i64::MAX)),
            cells.get(4).cloned().unwrap_or(Cell::Null),
            Cell::Text(rules.group.unwrap_or("").to_owned()),
            Cell::Text(rule),
        ]);
    }

    /// Bots by disallowed hits and rows by hits, both descending.
    fn finish(mut self) -> (Vec<RobotsBotSummary>, Vec<Vec<Cell>>) {
        for (bot, urls) in &self.disallowed_urls {
            if let Some(summary) = self.bots.get_mut(bot) {
                summary.disallowed_urls = urls.len() as u64;
            }
        }
        let mut bots: Vec<RobotsBotSummary> = self.bots.into_values().collect();
        bots.sort_by(|a, b| {
            b.disallowed_hits
                .cmp(&a.disallowed_hits)
                .then(b.hits.cmp(&a.hits))
        });
        let hits = |row: &[Cell]| match row.get(3) {
            Some(Cell::Integer(n)) => *n,
            _ => 0,
        };
        self.rows.sort_by_key(|row| std::cmp::Reverse(hits(row)));
        (bots, self.rows)
    }
}

fn columns() -> Vec<Column> {
    vec![
        Column::new("url", ColumnKind::Text),
        Column::new("bot", ColumnKind::Text),
        Column::new("verification", ColumnKind::Text),
        Column::new("hits", ColumnKind::Integer),
        Column::new("last_seen", ColumnKind::Timestamp),
        Column::new("group", ColumnKind::Text),
        Column::new("rule", ColumnKind::Text),
    ]
}

fn default_robots_url(workspace: &Workspace, site: &str) -> Result<String, AnalysisError> {
    let record = Catalog::open(workspace.data_dir())
        .and_then(|catalog| catalog.site_by_name(site))
        .map_err(|e| AnalysisError::Query(QueryError::Store(e)))?;
    let settings = SiteSettings::from_json(&record.settings).unwrap_or_default();
    let host = settings
        .host
        .clone()
        .or_else(|| effective_host(&record.name, &settings))
        .ok_or_else(|| AnalysisError::Read {
            list: "robots.txt".into(),
            message: format!("`{site}` is not a host name; give the robots.txt file or URL"),
        })?;
    // The configured host has no `www.`; most sites redirect, which the client follows.
    Ok(format!("https://{host}/robots.txt"))
}

fn load(source: &str) -> Result<String, AnalysisError> {
    let error = |message: String| AnalysisError::Read {
        list: source.to_owned(),
        message,
    };
    let lower = source.to_ascii_lowercase();
    if lower.starts_with("http://") || lower.starts_with("https://") {
        HttpClient::new(USER_AGENT, DOWNLOAD_TIMEOUT)
            .get_text(source)
            .map_err(|e| error(e.to_string()))
    } else {
        std::fs::read_to_string(source).map_err(|e| error(e.to_string()))
    }
}
