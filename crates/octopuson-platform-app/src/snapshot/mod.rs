//! Snapshots (ADR 0080): a project's whole state kept at a moment, compared with another moment
//! ([`compare`]) and told in words ([`summary`]).
//!
//! A snapshot asks the modules' own commands, so what it pays for is what their pages would pay
//! for, and what they already kept is reused:
//! - paid, through `fetch_overview` and `fetch_backlinks`: the domain's numbers, its first 1,000
//!   ranked keywords and 100 pages, its backlink profile and first 1,000 referring domains, and
//!   the chosen competitors' domain and backlink numbers. A fresh kept answer costs nothing; the
//!   gateway's shared cache sells some at a discount (ADR 0079);
//! - free: rank tracking, the latest site audit, AI visibility and Search Console.
//!
//! What it keeps is compact ([`SnapshotData`]): only what a comparison needs.

pub mod compare;
pub mod summary;

use std::sync::{Arc, OnceLock};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::fetch::Blocked;
use crate::jobs::{JobManager, NoEvents};
use crate::router::{CallError, Context, ErrorCode, Opener, Router};
use crate::{AppError, Workspace};
use octopuson_cloud::Via;

/// Version of what a snapshot keeps.
pub const DATA_VERSION: u32 = 2;

/// Ranked keywords and referring domains kept.
pub const LIST_ROWS: u32 = 1000;

/// Top pages kept.
pub const PAGE_ROWS: u32 = 100;

/// Search Console queries and pages kept.
const GSC_ROWS: usize = 100;

/// A domain's organic numbers.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Metrics {
    /// Ranked keywords.
    pub keywords: f64,
    /// Estimated monthly visits.
    pub traffic: f64,
    /// Their value, US dollars.
    pub traffic_value_usd: f64,
    /// Keywords at position 1.
    pub pos_1: f64,
    /// Keywords at positions 2–3.
    pub pos_2_3: f64,
    /// Keywords at positions 4–10.
    pub pos_4_10: f64,
    /// Keywords at positions 11–20.
    pub pos_11_20: f64,
    /// Keywords at positions 21–100.
    pub pos_21_100: f64,
}

impl Metrics {
    /// Keywords in the first three results.
    pub fn top3(&self) -> f64 {
        self.pos_1 + self.pos_2_3
    }

    /// Keywords in the first ten results.
    pub fn top10(&self) -> f64 {
        self.pos_1 + self.pos_2_3 + self.pos_4_10
    }
}

/// A ranked keyword.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KeywordPoint {
    /// The keyword.
    pub keyword: String,
    /// The position; none when not ranked or not named.
    pub position: Option<u32>,
    /// The address.
    pub url: Option<String>,
    /// Monthly searches.
    pub volume: Option<u64>,
    /// Estimated monthly visits.
    pub traffic: f64,
}

/// A page with its organic numbers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PagePoint {
    /// The address.
    pub url: String,
    /// Estimated monthly visits.
    pub traffic: f64,
    /// Keywords.
    pub keywords: f64,
}

/// A backlink profile.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct LinkPoint {
    /// OA, 0–100.
    pub rank: Option<f64>,
    /// Backlinks.
    pub backlinks: f64,
    /// Referring domains.
    pub referring_domains: f64,
    /// Broken backlinks.
    pub broken_backlinks: f64,
}

/// A referring domain.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DomainPoint {
    /// The domain.
    pub domain: String,
    /// Authority (OA), 0–100.
    pub rank: Option<f64>,
    /// Backlinks.
    pub backlinks: f64,
}

/// A competitor's numbers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompetitorPoint {
    /// The domain.
    pub domain: String,
    /// Organic numbers.
    pub overview: Option<Metrics>,
    /// Backlink profile.
    pub links: Option<LinkPoint>,
}

/// A tracked keyword's place.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TrackedPoint {
    /// The keyword.
    pub keyword: String,
    /// The position; none when not ranked or not named.
    pub position: Option<u32>,
    /// The address.
    pub url: Option<String>,
}

/// A competitor in rank tracking.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ShareRow {
    /// The domain.
    pub domain: String,
    /// Whether it is the project's own.
    pub own: bool,
    /// Visibility, 0–100.
    pub visibility: f64,
    /// Keywords in the first ten results.
    pub top10: f64,
}

/// Rank tracking.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RankPoint {
    /// Visibility, 0–100.
    pub visibility: Option<f64>,
    /// Average position.
    pub average_position: Option<f64>,
    /// Keywords in the first three results.
    pub top3: f64,
    /// Keywords in the first ten results.
    pub top10: f64,
    /// Tracked keywords that rank.
    pub ranked: f64,
    /// Tracked keywords checked.
    pub checked: f64,
    /// Keywords.
    pub keywords: Vec<TrackedPoint>,
    /// Competitors.
    pub competitors: Vec<ShareRow>,
}

/// A check of the site audit with its findings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IssuePoint {
    /// Its id.
    pub id: String,
    /// Its title.
    pub title: String,
    /// Findings.
    pub count: f64,
}

/// The latest site audit.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AuditPoint {
    /// Technical health, 0–100.
    pub health: Option<f64>,
    /// GEO readiness, 0–100.
    pub geo: Option<f64>,
    /// Pages.
    pub pages: f64,
    /// Findings.
    pub issues: f64,
    /// When it finished, RFC 3339.
    pub finished_at: Option<String>,
    /// Every check with its findings.
    pub checks: Vec<IssuePoint>,
}

/// One question to one assistant.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CellPoint {
    /// The question.
    pub prompt: String,
    /// The assistant.
    pub engine: String,
    /// Whether the answer names the brand.
    pub mentioned: bool,
    /// The position; none when not ranked or not named.
    pub position: Option<f64>,
}

/// A brand's share of AI answers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BrandPoint {
    /// Its key.
    pub key: String,
    /// Whether it is the project's own.
    pub own: bool,
    /// Share of answers, 0–100.
    pub share: f64,
}

/// AI visibility.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AiPoint {
    /// Answers read.
    pub answers: f64,
    /// Answers naming the brand, 0–100.
    pub mention_rate: f64,
    /// Answers citing the site, 0–100.
    pub citation_rate: f64,
    /// Share of voice, 0–100.
    pub share_of_voice: f64,
    /// Average position.
    pub average_position: Option<f64>,
    /// Every question to every assistant.
    pub cells: Vec<CellPoint>,
    /// Brands.
    pub brands: Vec<BrandPoint>,
}

/// A Search Console query or page.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SearchPoint {
    /// Its key.
    pub key: String,
    /// Clicks.
    pub clicks: f64,
    /// Impressions.
    pub impressions: f64,
    /// The position; none when not ranked or not named.
    pub position: Option<f64>,
}

/// Search Console and Analytics.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct GscPoint {
    /// The period, first and last day.
    pub period: Option<(String, String)>,
    /// Clicks.
    pub clicks: f64,
    /// Impressions.
    pub impressions: f64,
    /// Click-through rate, 0–100.
    pub ctr: f64,
    /// The position; none when not ranked or not named.
    pub position: Option<f64>,
    /// Analytics sessions.
    pub sessions: Option<f64>,
    /// First queries.
    pub queries: Vec<SearchPoint>,
    /// Pages.
    pub pages: Vec<SearchPoint>,
}

/// A map keyword's place across the grid.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LocalKeywordPoint {
    /// The keyword.
    pub keyword: String,
    /// Average map rank (20 and over where it does not show).
    pub arp: Option<f64>,
    /// Share of local voice, 0–100.
    pub solv: f64,
}

/// Local SEO: the latest map grid and the Google rating.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct LocalPoint {
    /// Average map rank over every keyword.
    pub arp: Option<f64>,
    /// Share of local voice, 0–100.
    pub solv: f64,
    /// Grid points it shows at.
    pub found: f64,
    /// Grid points checked.
    pub points: f64,
    /// Google rating, 1–5.
    pub rating: Option<f64>,
    /// Reviews counted.
    pub votes: Option<f64>,
    /// Each keyword.
    pub keywords: Vec<LocalKeywordPoint>,
}

/// The toxic-link audit and the decisions taken on it.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct LinkAuditPoint {
    /// Linking sites judged.
    pub judged: f64,
    /// Sites that look like spam.
    pub high: f64,
    /// Sites worth a look.
    pub medium: f64,
    /// Sites disavowed.
    pub disavowed: f64,
    /// The high-risk sites not disavowed, the riskiest first.
    pub risky: Vec<String>,
}

/// A part that could not be taken, and why.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Missing {
    /// The part.
    pub part: String,
    /// Why, in words.
    pub reason: String,
    /// The error's code (`no_source`, `budget` …), for pages to explain.
    pub code: Option<ErrorCode>,
}

/// What a snapshot keeps.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SnapshotData {
    /// Version of what is kept.
    pub version: u32,
    /// The domain.
    pub domain: String,
    /// `language-country`.
    pub market: String,
    /// Organic numbers.
    #[serde(default)]
    pub overview: Option<Metrics>,
    /// Keywords.
    #[serde(default)]
    pub keywords: Option<Vec<KeywordPoint>>,
    /// Ranked keywords in all.
    #[serde(default)]
    pub keywords_total: Option<f64>,
    /// Pages.
    #[serde(default)]
    pub pages: Option<Vec<PagePoint>>,
    /// Backlink profile.
    #[serde(default)]
    pub links: Option<LinkPoint>,
    /// Referring domains.
    #[serde(default)]
    pub domains: Option<Vec<DomainPoint>>,
    /// Competitors.
    #[serde(default)]
    pub competitors: Vec<CompetitorPoint>,
    /// Rank tracking.
    #[serde(default)]
    pub rank: Option<RankPoint>,
    /// The site audit.
    #[serde(default)]
    pub audit: Option<AuditPoint>,
    /// AI visibility.
    #[serde(default)]
    pub ai: Option<AiPoint>,
    /// Search Console and Analytics.
    #[serde(default)]
    pub gsc: Option<GscPoint>,
    /// Local SEO (from version 2).
    #[serde(default)]
    pub local: Option<LocalPoint>,
    /// The toxic-link audit (from version 2).
    #[serde(default)]
    pub link_audit: Option<LinkAuditPoint>,
    /// Parts not taken, and why.
    #[serde(default)]
    pub missing: Vec<Missing>,
}

/// A snapshot's main numbers, for a timeline.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Headline {
    /// Organic traffic.
    pub traffic: Option<f64>,
    /// Ranked keywords.
    pub keywords: Option<f64>,
    /// Keywords in the first ten results.
    pub top10: Option<f64>,
    /// Authority (OA), 0–100.
    pub oa: Option<f64>,
    /// Referring domains.
    pub referring_domains: Option<f64>,
    /// Rank tracking visibility, 0–100.
    pub visibility: Option<f64>,
    /// Site audit health, 0–100.
    pub health: Option<f64>,
    /// AI mention rate, 0–100.
    pub mention_rate: Option<f64>,
    /// Search Console clicks.
    pub clicks: Option<f64>,
    /// Share of local voice, 0–100.
    pub solv: Option<f64>,
    /// High-risk linking sites not disavowed.
    pub risky_links: Option<f64>,
    /// Parts that could not be taken.
    pub missing: usize,
}

impl SnapshotData {
    /// Its main numbers.
    pub fn headline(&self) -> Headline {
        Headline {
            traffic: self.overview.as_ref().map(|m| m.traffic),
            keywords: self.overview.as_ref().map(|m| m.keywords),
            top10: self.overview.as_ref().map(Metrics::top10),
            oa: self.links.as_ref().and_then(|l| l.rank),
            referring_domains: self.links.as_ref().map(|l| l.referring_domains),
            visibility: self.rank.as_ref().and_then(|r| r.visibility),
            health: self.audit.as_ref().and_then(|a| a.health),
            mention_rate: self.ai.as_ref().map(|a| a.mention_rate),
            clicks: self.gsc.as_ref().map(|g| g.clicks),
            solv: self.local.as_ref().map(|l| l.solv),
            risky_links: self.link_audit.as_ref().map(|a| a.high - a.disavowed),
            missing: self.missing.len(),
        }
    }
}

fn number(value: &Value, key: &str) -> f64 {
    value.get(key).and_then(Value::as_f64).unwrap_or_default()
}

fn maybe(value: &Value, key: &str) -> Option<f64> {
    value.get(key).and_then(Value::as_f64)
}

fn text(value: &Value, key: &str) -> Option<String> {
    value.get(key).and_then(Value::as_str).map(str::to_owned)
}

fn rows(value: &Value) -> &[Value] {
    value.as_array().map_or(&[], Vec::as_slice)
}

/// A domain's organic numbers from a domain overview.
pub fn metrics_of(overview: &Value) -> Option<Metrics> {
    let organic = overview.get("organic")?;
    Some(Metrics {
        keywords: number(organic, "keywords"),
        traffic: number(organic, "traffic"),
        traffic_value_usd: number(organic, "traffic_value_usd"),
        pos_1: number(organic, "pos_1"),
        pos_2_3: number(organic, "pos_2_3"),
        pos_4_10: number(organic, "pos_4_10"),
        pos_11_20: number(organic, "pos_11_20"),
        pos_21_100: number(organic, "pos_21_100"),
    })
}

/// A backlink profile from a backlink summary.
pub fn links_of(summary: &Value) -> Option<LinkPoint> {
    summary.is_object().then(|| LinkPoint {
        rank: maybe(summary, "rank"),
        backlinks: number(summary, "backlinks"),
        referring_domains: number(summary, "referring_domains"),
        broken_backlinks: number(summary, "broken_backlinks"),
    })
}

fn keywords_of(ranked: &Value) -> (Vec<KeywordPoint>, Option<f64>) {
    let keywords = rows(ranked.get("keywords").unwrap_or(&Value::Null))
        .iter()
        .filter_map(|row| {
            Some(KeywordPoint {
                keyword: text(row, "keyword")?,
                position: row
                    .get("position")
                    .and_then(Value::as_u64)
                    .and_then(|position| u32::try_from(position).ok()),
                url: text(row, "url"),
                volume: row.get("volume").and_then(Value::as_u64),
                traffic: number(row, "traffic"),
            })
        })
        .collect();
    (keywords, maybe(ranked, "total"))
}

fn pages_of(pages: &Value) -> Vec<PagePoint> {
    rows(pages)
        .iter()
        .filter_map(|row| {
            let organic = row.get("organic").unwrap_or(&Value::Null);
            Some(PagePoint {
                url: text(row, "url")?,
                traffic: number(organic, "traffic"),
                keywords: number(organic, "keywords"),
            })
        })
        .collect()
}

fn domains_of(domains: &Value) -> Vec<DomainPoint> {
    rows(domains)
        .iter()
        .filter_map(|row| {
            Some(DomainPoint {
                domain: text(row, "domain")?,
                rank: maybe(row, "rank"),
                backlinks: number(row, "backlinks"),
            })
        })
        .collect()
}

/// Rank tracking from a rank report.
pub fn rank_of(report: &Value) -> Option<RankPoint> {
    let summary = report.get("summary")?;
    if number(summary, "checked") <= 0.0 && rows(report.get("rows")?).is_empty() {
        return None;
    }
    Some(RankPoint {
        visibility: maybe(summary, "visibility"),
        average_position: maybe(summary, "average_position"),
        top3: number(summary, "top3"),
        top10: number(summary, "top10"),
        ranked: number(summary, "ranked"),
        checked: number(summary, "checked"),
        keywords: rows(report.get("rows").unwrap_or(&Value::Null))
            .iter()
            .filter_map(|row| {
                Some(TrackedPoint {
                    keyword: text(row, "keyword")?,
                    position: row
                        .get("position")
                        .and_then(Value::as_u64)
                        .and_then(|position| u32::try_from(position).ok()),
                    url: text(row, "url"),
                })
            })
            .collect(),
        competitors: rows(report.get("competitors").unwrap_or(&Value::Null))
            .iter()
            .filter_map(|row| {
                Some(ShareRow {
                    domain: text(row, "domain")?,
                    own: row.get("own").and_then(Value::as_bool).unwrap_or(false),
                    visibility: number(row, "visibility"),
                    top10: number(row, "top10"),
                })
            })
            .collect(),
    })
}

/// The latest site audit from an audit report.
pub fn audit_of(report: &Value) -> Option<AuditPoint> {
    let audit = report.get("audit")?;
    let checks = rows(report.get("categories").unwrap_or(&Value::Null))
        .iter()
        .flat_map(|category| rows(category.get("issues").unwrap_or(&Value::Null)))
        .filter_map(|issue| {
            Some(IssuePoint {
                id: text(issue, "id")?,
                title: text(issue, "title").unwrap_or_default(),
                count: number(issue, "count"),
            })
        })
        .collect();
    Some(AuditPoint {
        health: maybe(audit, "health"),
        geo: maybe(audit, "geo"),
        pages: number(audit, "pages"),
        issues: number(audit, "issues"),
        finished_at: text(audit, "finished_at"),
        checks,
    })
}

/// AI visibility from an AI report.
pub fn ai_of(report: &Value) -> Option<AiPoint> {
    let summary = report.get("summary")?;
    if number(summary, "answers") <= 0.0 {
        return None;
    }
    let mut cells = Vec::new();
    for prompt in rows(report.get("prompts").unwrap_or(&Value::Null)) {
        let Some(question) = text(prompt, "prompt") else {
            continue;
        };
        if let Some(results) = prompt.get("results").and_then(Value::as_object) {
            for (engine, cell) in results {
                if cell.get("error").is_some_and(|error| !error.is_null()) {
                    continue;
                }
                cells.push(CellPoint {
                    prompt: question.clone(),
                    engine: engine.clone(),
                    mentioned: cell
                        .get("mentioned")
                        .and_then(Value::as_bool)
                        .unwrap_or(false),
                    position: maybe(cell, "position"),
                });
            }
        }
    }
    Some(AiPoint {
        answers: number(summary, "answers"),
        mention_rate: number(summary, "mention_rate"),
        citation_rate: number(summary, "citation_rate"),
        share_of_voice: number(summary, "share_of_voice"),
        average_position: maybe(summary, "average_position"),
        cells,
        brands: rows(report.get("brands").unwrap_or(&Value::Null))
            .iter()
            .filter_map(|brand| {
                Some(BrandPoint {
                    key: text(brand, "key")?,
                    own: brand.get("own").and_then(Value::as_bool).unwrap_or(false),
                    share: number(brand, "share"),
                })
            })
            .collect(),
    })
}

/// Search Console and Analytics from a Search Console report.
pub fn gsc_of(report: &Value) -> Option<GscPoint> {
    let now = report.get("now")?;
    let points = |list: &str, key: &str| -> Vec<SearchPoint> {
        rows(report.get(list).unwrap_or(&Value::Null))
            .iter()
            .take(GSC_ROWS)
            .filter_map(|row| {
                Some(SearchPoint {
                    key: text(row, key)?,
                    clicks: number(row, "clicks"),
                    impressions: number(row, "impressions"),
                    position: maybe(row, "position"),
                })
            })
            .collect()
    };
    let period = report
        .get("period")
        .and_then(Value::as_array)
        .and_then(|period| {
            Some((
                period.first()?.as_str()?.to_owned(),
                period.get(1)?.as_str()?.to_owned(),
            ))
        });
    Some(GscPoint {
        period,
        clicks: number(now, "clicks"),
        impressions: number(now, "impressions"),
        ctr: number(now, "ctr"),
        position: maybe(now, "position"),
        sessions: report
            .get("traffic_now")
            .and_then(|traffic| maybe(traffic, "sessions")),
        queries: points("queries", "query"),
        pages: points("pages", "page"),
    })
}

/// Local SEO from the module's grid report and its reviews.
pub fn local_of(grid: &Value, reviews: Option<&Value>) -> Option<LocalPoint> {
    if grid.get("run").is_none_or(Value::is_null) {
        return None;
    }
    let summary = grid.get("summary").unwrap_or(&Value::Null);
    let last = reviews
        .and_then(|reviews| reviews.get("ratings"))
        .and_then(Value::as_array)
        .and_then(|days| days.last());
    Some(LocalPoint {
        arp: maybe(summary, "arp"),
        solv: number(summary, "solv"),
        found: number(summary, "found"),
        points: number(summary, "points"),
        rating: last.and_then(|day| maybe(day, "rating")),
        votes: last.and_then(|day| maybe(day, "votes")),
        keywords: rows(grid.get("keywords").unwrap_or(&Value::Null))
            .iter()
            .filter_map(|keyword| {
                let figures = keyword.get("summary").unwrap_or(&Value::Null);
                Some(LocalKeywordPoint {
                    keyword: text(keyword, "keyword")?,
                    arp: maybe(figures, "arp"),
                    solv: number(figures, "solv"),
                })
            })
            .collect(),
    })
}

/// High-risk sites kept by name.
const RISKY_KEPT: usize = 200;

/// The toxic-link audit from the kept audit (`data`) and the decisions taken.
pub fn link_audit_of(audit: &Value, decisions: &Value) -> Option<LinkAuditPoint> {
    let judged = rows(audit);
    if judged.is_empty() {
        return None;
    }
    let disavowed: Vec<&str> = rows(decisions)
        .iter()
        .filter(|decision| decision.get("decision").and_then(Value::as_str) == Some("disavow"))
        .filter_map(|decision| decision.get("domain").and_then(Value::as_str))
        .collect();
    let level = |risk: &str| {
        judged
            .iter()
            .filter(|row| row.get("risk").and_then(Value::as_str) == Some(risk))
            .count()
    };
    let mut risky: Vec<(&str, f64)> = judged
        .iter()
        .filter(|row| row.get("risk").and_then(Value::as_str) == Some("high"))
        .filter_map(|row| Some((row.get("domain")?.as_str()?, number(row, "score"))))
        .filter(|(domain, _)| !disavowed.contains(domain))
        .collect();
    risky.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.cmp(b.0)));
    #[allow(clippy::cast_precision_loss)] // counts of linking sites
    Some(LinkAuditPoint {
        judged: judged.len() as f64,
        high: level("high") as f64,
        medium: level("medium") as f64,
        disavowed: disavowed.len() as f64,
        risky: risky
            .into_iter()
            .take(RISKY_KEPT)
            .map(|(domain, _)| domain.to_owned())
            .collect(),
    })
}

/// A paid part of a snapshot: the module's command arguments.
#[derive(Debug, Clone, PartialEq)]
pub struct Part {
    /// What it fills (`overview`, `keywords`, `pages`, `links`, `domains`, `competitor`).
    pub name: &'static str,
    /// `overview` or `backlinks`.
    pub module: &'static str,
    /// For a competitor, its domain.
    pub domain: Option<String>,
    /// The command's arguments.
    pub arguments: Value,
}

/// The paid parts of a snapshot of `project_id` with `competitors`.
pub fn parts(project_id: i64, competitors: &[String]) -> Vec<Part> {
    let part = |name, module, arguments: Value| Part {
        name,
        module,
        domain: None,
        arguments,
    };
    let mut parts = vec![
        part(
            "overview",
            "overview",
            json!({ "project_id": project_id, "report": "summary" }),
        ),
        part(
            "keywords",
            "overview",
            json!({ "project_id": project_id, "report": "keywords", "limit": LIST_ROWS }),
        ),
        part(
            "pages",
            "overview",
            json!({ "project_id": project_id, "report": "pages", "limit": PAGE_ROWS }),
        ),
        part(
            "links",
            "backlinks",
            json!({ "project_id": project_id, "report": "summary" }),
        ),
        part(
            "domains",
            "backlinks",
            json!({ "project_id": project_id, "report": "referring_domains", "limit": LIST_ROWS }),
        ),
    ];
    for competitor in competitors {
        parts.push(Part {
            name: "competitor",
            module: "overview",
            domain: Some(competitor.clone()),
            arguments: json!({ "project_id": project_id, "domain": competitor, "report": "summary" }),
        });
        parts.push(Part {
            name: "competitor",
            module: "backlinks",
            domain: Some(competitor.clone()),
            arguments: json!({ "project_id": project_id, "target": competitor, "report": "summary" }),
        });
    }
    parts
}

/// What taking a snapshot would cost now.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SnapshotEstimate {
    /// Provider list price of the parts to pay for, US dollars.
    pub usd: f64,
    /// On credits: what they cost, with the gateway's discount where it keeps the answer.
    pub credits: Option<f64>,
    /// On credits: what they would cost without that discount.
    pub full_credits: Option<f64>,
    /// How it is paid.
    pub via: Option<Via>,
    /// Paid parts kept fresh here: free.
    pub kept: u32,
    /// Paid parts to pay for.
    pub paid: u32,
    /// Why the paid parts cannot be taken now; the free parts still are.
    pub blocked: Option<Blocked>,
}

/// The modules' commands, called from inside the program.
fn data_router() -> &'static Router {
    static ROUTER: OnceLock<Router> = OnceLock::new();
    ROUTER.get_or_init(crate::modules::router)
}

/// Calls a module's command from inside the program.
pub(crate) fn ask(
    workspace: &Arc<Workspace>,
    command: &str,
    arguments: Value,
) -> Result<Value, CallError> {
    let jobs = JobManager::new(Arc::new(NoEvents));
    let open: Opener = Arc::new(|_| false);
    let context = Context {
        workspace,
        jobs: &jobs,
        open: &open,
    };
    data_router().call(&context, command, arguments)
}

/// What taking a snapshot with `competitors` would cost now.
///
/// # Errors
/// [`AppError`] when an estimate cannot be read.
pub fn estimate(
    workspace: &Arc<Workspace>,
    project_id: i64,
    competitors: &[String],
) -> Result<SnapshotEstimate, AppError> {
    let mut total = SnapshotEstimate::default();
    for part in parts(project_id, competitors) {
        let estimate = ask(
            workspace,
            &format!("{}_estimate", part.module),
            part.arguments,
        )
        .map_err(|error| AppError::Internal(error.message))?;
        if estimate.pointer("/kept/fresh").and_then(Value::as_bool) == Some(true) {
            total.kept += 1;
            continue;
        }
        total.paid += 1;
        total.usd += number(&estimate, "usd");
        total.via = total
            .via
            .or_else(|| serde_json::from_value(estimate.get("via").cloned()?).ok());
        if let Some(credits) = maybe(&estimate, "credits") {
            let shared = estimate
                .pointer("/shared/credits")
                .and_then(Value::as_f64)
                .unwrap_or(credits);
            total.credits = Some(total.credits.unwrap_or(0.0) + shared);
            total.full_credits = Some(total.full_credits.unwrap_or(0.0) + credits);
        }
        if total.blocked.is_none()
            && let Some(blocked) = estimate.get("blocked").filter(|blocked| !blocked.is_null())
        {
            total.blocked = serde_json::from_value(blocked.clone()).ok();
        }
    }
    Ok(total)
}

/// How a snapshot went.
#[derive(Debug, Clone, PartialEq)]
pub struct Taken {
    /// What it holds.
    pub data: SnapshotData,
    /// US dollars paid.
    pub spent_usd: f64,
}

/// Takes a snapshot of a project now: the paid parts first (each reusing what its module keeps
/// fresh), then the free ones. `progress` hears each step; `cancelled` stops between parts.
///
/// # Errors
/// [`AppError`] when the project cannot be read.
pub fn take(
    workspace: &Arc<Workspace>,
    project_id: i64,
    competitors: &[String],
    progress: &dyn Fn(&str, u64, u64),
    cancelled: &dyn Fn() -> bool,
) -> Result<Taken, AppError> {
    let project = workspace.store()?.project(project_id)?;
    let mut data = SnapshotData {
        version: DATA_VERSION,
        domain: project.domain.clone(),
        market: format!("{}-{}", project.language, project.country),
        ..SnapshotData::default()
    };
    let mut spent = 0.0;
    let paid = parts(project_id, competitors);
    let total = u64::try_from(paid.len() + 6).unwrap_or(u64::MAX);
    let mut step = 0_u64;
    for part in paid {
        if cancelled() {
            return Err(AppError::Invalid("cancelled".to_owned()));
        }
        progress(part.name, step, total);
        step += 1;
        // Once a source cannot pay (no key, no credits, budget), the other parts cannot either.
        if let Some(missing) = data
            .missing
            .iter()
            .find(|missing| {
                matches!(
                    missing.code,
                    Some(ErrorCode::NoSource | ErrorCode::Budget | ErrorCode::Quota)
                )
            })
            .cloned()
        {
            data.missing.push(Missing {
                part: part_name(&part),
                ..missing
            });
            continue;
        }
        match ask(
            workspace,
            &format!("fetch_{}", part.module),
            part.arguments.clone(),
        ) {
            Ok(fetched) => {
                spent += number(&fetched, "spent_usd");
                let answer = fetched.get("data").unwrap_or(&Value::Null);
                fill(&mut data, &part, answer);
            }
            Err(error) => data.missing.push(Missing {
                part: part_name(&part),
                reason: error.message,
                code: Some(error.code),
            }),
        }
    }
    let free: [(&str, &str, Value); 4] = [
        (
            "rank",
            "rank_report",
            json!({ "project_id": project_id, "days": 30 }),
        ),
        ("audit", "audit_report", json!({ "project_id": project_id })),
        (
            "ai",
            "ai_report",
            json!({ "project_id": project_id, "days": 30 }),
        ),
        ("gsc", "gsc_report", json!({ "project_id": project_id })),
    ];
    for (name, command, arguments) in free {
        progress(name, step, total);
        step += 1;
        let Ok(report) = ask(workspace, command, arguments) else {
            continue;
        };
        match name {
            "rank" => data.rank = rank_of(&report),
            "audit" => data.audit = audit_of(&report),
            "ai" => data.ai = ai_of(&report),
            _ => data.gsc = gsc_of(&report),
        }
    }
    let at = json!({ "project_id": project_id });
    progress("local", step, total);
    step += 1;
    if let Ok(grid) = ask(workspace, "local_grid", at.clone()) {
        let reviews = ask(workspace, "local_reviews", at.clone()).ok();
        data.local = local_of(&grid, reviews.as_ref());
    }
    progress("link_audit", step, total);
    if let Ok(kept) = ask(
        workspace,
        "get_backlinks",
        json!({ "project_id": project_id, "report": "audit" }),
    ) {
        let decisions = ask(workspace, "link_decisions", at).unwrap_or(Value::Null);
        data.link_audit = link_audit_of(kept.get("data").unwrap_or(&Value::Null), &decisions);
    }
    progress("", total, total);
    Ok(Taken {
        data,
        spent_usd: spent,
    })
}

fn part_name(part: &Part) -> String {
    match &part.domain {
        Some(domain) => format!("{}:{}:{domain}", part.name, part.module),
        None => part.name.to_owned(),
    }
}

/// Puts a paid part's answer into the snapshot.
fn fill(data: &mut SnapshotData, part: &Part, answer: &Value) {
    match (part.name, &part.domain) {
        ("overview", _) => data.overview = metrics_of(answer),
        ("keywords", _) => {
            let (keywords, total) = keywords_of(answer);
            data.keywords = Some(keywords);
            data.keywords_total = total;
        }
        ("pages", _) => data.pages = Some(pages_of(answer)),
        ("links", _) => data.links = links_of(answer),
        ("domains", _) => data.domains = Some(domains_of(answer)),
        ("competitor", Some(domain)) => {
            let known = data
                .competitors
                .iter()
                .position(|competitor| &competitor.domain == domain);
            let index = if let Some(index) = known {
                index
            } else {
                data.competitors.push(CompetitorPoint {
                    domain: domain.clone(),
                    overview: None,
                    links: None,
                });
                data.competitors.len() - 1
            };
            if part.module == "overview" {
                data.competitors[index].overview = metrics_of(answer);
            } else {
                data.competitors[index].links = links_of(answer);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
pub(crate) mod tests;
