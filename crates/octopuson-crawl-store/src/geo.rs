//! GEO readiness: how ready a crawled site is to be fetched, read, quoted and cited by AI
//! answers (ChatGPT, Perplexity, Claude, Google AI Overviews).
//!
//! Read-only over a finished crawl: it adds no audit findings, so a crawl's checks and health
//! score stay what the SEO Crawler shows. Each signal is a share from 0 to 1 with a weight;
//! the score is their weighted sum (signals that cannot be measured are left out and the
//! weights renormalised).

use std::collections::{BTreeMap, HashMap, HashSet};

use duckdb::params;
use octopuson_audit::ai_crawler_access;
use serde::Serialize;

use crate::{CrawlReader, CrawlStoreError};

/// Failing URLs listed per signal.
const EXAMPLES: usize = 20;

/// Words of a page that has something to quote.
const SUBSTANTIAL_WORDS: u32 = 300;

/// A snippet limit shorter than this leaves nothing worth quoting.
const QUOTABLE_SNIPPET: i32 = 50;

/// How a signal stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum GeoStatus {
    /// Ready.
    Good,
    /// Partly ready.
    Warning,
    /// Not ready.
    Poor,
}

/// One readiness signal.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct GeoSignal {
    /// Stable id, for translations (`ai-search-access`).
    pub id: &'static str,
    /// Points it is worth out of 100.
    pub weight: f64,
    /// Share achieved, 0–1.
    pub value: f64,
    /// How it stands.
    pub status: GeoStatus,
    /// Pages, crawlers or items that pass.
    pub passed: u64,
    /// Pages, crawlers or items looked at.
    pub total: u64,
    /// What fails: page addresses, crawler names or missing types (at most 20).
    pub examples: Vec<String>,
}

/// An AI crawler and whether robots.txt lets it fetch the home page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GeoCrawler {
    /// Catalog id (`oai-searchbot`).
    pub id: &'static str,
    /// Display name (`OAI-SearchBot`).
    pub name: &'static str,
    /// Company operating it.
    pub operator: &'static str,
    /// `search`, `assistant` or `training`.
    pub purpose: &'static str,
    /// It may fetch `/`.
    pub allowed: bool,
    /// The deciding rule as written, if any.
    pub rule: Option<String>,
}

/// The GEO readiness of a crawl.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct GeoReadiness {
    /// 0–100 with one decimal; `None` without indexable pages.
    pub score: Option<f64>,
    /// Indexable HTML pages looked at.
    pub pages: u64,
    /// The signals, by weight.
    pub signals: Vec<GeoSignal>,
    /// AI crawlers under the home page's robots.txt.
    pub crawlers: Vec<GeoCrawler>,
    /// robots.txt: `found`, `missing` or `unreachable`; `None` when not read.
    pub robots: Option<String>,
    /// Structured data types and the pages carrying them, most used first.
    pub schema_types: Vec<(String, u64)>,
}

/// Words an English, German or Spanish question starts with (and Turkish "ne").
const QUESTION_STARTS: &[&str] = &[
    "ne", // Turkish, only first: elsewhere it is often "neither … nor"
    // English
    "how", "what", "why", "when", "where", "which", "who", "whom", "whose", "can", "does", "do",
    "is", "are", "should", "will", "could", "would", // German
    "wie", "was", "warum", "wann", "wo", "welche", "welcher", "welches", "wer", "wieso", "weshalb",
    "kann", "gibt", // Spanish
    "cómo", "qué", "cuándo", "dónde", "cuál", "cuáles", "quién", "quiénes", "cuánto", "cuántos",
    "por",
];

/// Turkish question words and particles, wherever they stand: the verb comes last, so the
/// question word is often in the middle ("Türk kahvesi nasıl pişirilir").
const QUESTION_ANYWHERE: &[&str] = &[
    "nasıl",
    "neden",
    "niçin",
    "niye",
    "nedir",
    "neler",
    "hangi",
    "hangisi",
    "kaç",
    "kim",
    "kimler",
    "nerede",
    "nereden",
    "nereye",
    "mi",
    "mı",
    "mu",
    "mü",
    "midir",
    "mıdır",
    "mudur",
    "müdür",
    "misiniz",
    "mısınız",
];

/// Whether a heading asks a question: a question mark, an English, German or Spanish question
/// word first, or a Turkish question word anywhere.
pub(crate) fn is_question(heading: &str) -> bool {
    let text = heading.trim();
    if text.ends_with('?') || text.starts_with('¿') {
        return true;
    }
    let lower = text.to_lowercase();
    let mut words = lower
        .split(|c: char| !c.is_alphanumeric())
        .filter(|word| !word.is_empty());
    let Some(first) = words.next() else {
        return false;
    };
    QUESTION_STARTS.contains(&first)
        || std::iter::once(first)
            .chain(words)
            .any(|word| QUESTION_ANYWHERE.contains(&word))
}

#[allow(clippy::cast_precision_loss)] // page counts
fn share(passed: u64, total: u64) -> f64 {
    if total == 0 {
        0.0
    } else {
        passed as f64 / total as f64
    }
}

fn signal(
    id: &'static str,
    weight: f64,
    (passed, total): (u64, u64),
    (good, warning): (f64, f64),
    examples: Vec<String>,
) -> GeoSignal {
    let value = share(passed, total);
    GeoSignal {
        id,
        weight,
        value,
        status: if value >= good {
            GeoStatus::Good
        } else if value >= warning {
            GeoStatus::Warning
        } else {
            GeoStatus::Poor
        },
        passed,
        total,
        examples: examples.into_iter().take(EXAMPLES).collect(),
    }
}

struct Page {
    id: u32,
    url: String,
    words: u32,
    nosnippet: bool,
    max_snippet: Option<i32>,
}

impl CrawlReader {
    /// The GEO readiness of the crawl.
    ///
    /// # Errors
    /// Database errors.
    #[allow(clippy::too_many_lines)] // one pass per signal
    pub fn geo_readiness(&self) -> Result<GeoReadiness, CrawlStoreError> {
        let connection = &self.connection;
        let mut statement = connection.prepare(
            "SELECT id, url, coalesce(word_count, 0), nosnippet, max_snippet FROM urls
             WHERE internal AND content_kind='html' AND status=200 AND indexability='indexable'
             ORDER BY depth NULLS LAST, url",
        )?;
        let pages: Vec<Page> = statement
            .query_map([], |row| {
                Ok(Page {
                    id: row.get(0)?,
                    url: row.get(1)?,
                    words: row.get(2)?,
                    nosnippet: row.get(3)?,
                    max_snippet: row.get(4)?,
                })
            })?
            .collect::<Result<_, _>>()?;
        let total = pages.len() as u64;

        let mut types: HashMap<u32, Vec<String>> = HashMap::new();
        let mut statement =
            connection.prepare("SELECT page, types FROM structured_data WHERE valid")?;
        let rows = statement.query_map([], |row| {
            Ok((row.get::<_, u32>(0)?, row.get::<_, String>(1)?))
        })?;
        for row in rows {
            let (page, text) = row?;
            types.entry(page).or_default().extend(
                text.split(',')
                    .map(str::trim)
                    .filter(|name| !name.is_empty())
                    .map(str::to_owned),
            );
        }
        let mut questions: HashSet<u32> = HashSet::new();
        let mut statement =
            connection.prepare("SELECT page, text FROM headings WHERE level IN (2, 3)")?;
        let rows = statement.query_map([], |row| {
            Ok((row.get::<_, u32>(0)?, row.get::<_, String>(1)?))
        })?;
        for row in rows {
            let (page, text) = row?;
            if is_question(&text) {
                questions.insert(page);
            }
        }

        // AI crawlers under the robots.txt of the home page's origin.
        let origin = pages.first().map(|page| origin_of(&page.url));
        let robots: Option<(String, Option<String>)> = match &origin {
            Some(origin) => connection
                .query_row(
                    "SELECT status, text FROM robots_files WHERE url = ?",
                    params![format!("{origin}/robots.txt")],
                    |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?)),
                )
                .ok(),
            None => None,
        };
        let crawlers: Vec<GeoCrawler> = match &robots {
            Some((status, text)) if status != "unreachable" => {
                let text = if status == "found" {
                    text.as_deref().unwrap_or_default()
                } else {
                    ""
                };
                ai_crawler_access(text)
                    .into_iter()
                    .map(|access| GeoCrawler {
                        id: access.crawler.id,
                        name: access.crawler.name,
                        operator: access.crawler.operator,
                        purpose: access.crawler.purpose.as_str(),
                        allowed: access.allowed,
                        rule: access.rule,
                    })
                    .collect()
            }
            Some(_) => ai_crawler_access("User-agent: *\nDisallow: /")
                .into_iter()
                .map(|access| GeoCrawler {
                    id: access.crawler.id,
                    name: access.crawler.name,
                    operator: access.crawler.operator,
                    purpose: access.crawler.purpose.as_str(),
                    allowed: false,
                    rule: Some("robots.txt unreachable".to_owned()),
                })
                .collect(),
            None => Vec::new(),
        };
        let access = |purpose: &str| {
            let group: Vec<&GeoCrawler> = crawlers
                .iter()
                .filter(|crawler| crawler.purpose == purpose)
                .collect();
            let blocked: Vec<String> = group
                .iter()
                .filter(|crawler| !crawler.allowed)
                .map(|crawler| crawler.name.to_owned())
                .collect();
            let count = group.len() as u64;
            (count - blocked.len() as u64, count, blocked)
        };

        let mut signals = Vec::new();
        if !crawlers.is_empty() {
            let (passed, count, blocked) = access("search");
            signals.push(signal(
                "ai-search-access",
                25.0,
                (passed, count),
                (1.0, 0.5),
                blocked,
            ));
            let (passed, count, blocked) = access("assistant");
            signals.push(signal(
                "ai-assistant-access",
                10.0,
                (passed, count),
                (1.0, 0.5),
                blocked,
            ));
        }
        if let Some(origin) = &origin {
            let llms = format!("{origin}/llms.txt");
            let found: Option<(Option<u16>, Option<String>)> = connection
                .query_row(
                    "SELECT status, text FROM site_files WHERE url = ?",
                    params![llms],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .ok();
            if let Some((status, text)) = found {
                let present =
                    status == Some(200) && text.is_some_and(|text| !text.trim().is_empty());
                signals.push(signal(
                    "llms-txt",
                    5.0,
                    (u64::from(present), 1),
                    (1.0, 1.0),
                    if present { Vec::new() } else { vec![llms] },
                ));
            }
        }
        if total > 0 {
            let restricted: Vec<String> = pages
                .iter()
                .filter(|page| {
                    page.nosnippet
                        || page
                            .max_snippet
                            .is_some_and(|limit| (0..QUOTABLE_SNIPPET).contains(&limit))
                })
                .map(|page| page.url.clone())
                .collect();
            signals.push(signal(
                "snippets-allowed",
                15.0,
                (total - restricted.len() as u64, total),
                (1.0, 0.8),
                restricted,
            ));
            let without: Vec<String> = pages
                .iter()
                .filter(|page| !types.contains_key(&page.id))
                .map(|page| page.url.clone())
                .collect();
            signals.push(signal(
                "structured-data",
                15.0,
                (total - without.len() as u64, total),
                (0.8, 0.4),
                without,
            ));
            let all_types: HashSet<&str> = types.values().flatten().map(String::as_str).collect();
            let organization = all_types.iter().any(|name| {
                name.contains("Organization")
                    || name.contains("LocalBusiness")
                    || name.contains("Corporation")
                    || name.contains("Store")
            });
            let website = all_types.contains("WebSite");
            let mut missing = Vec::new();
            if !organization {
                missing.push("Organization".to_owned());
            }
            if !website {
                missing.push("WebSite".to_owned());
            }
            signals.push(signal(
                "entity-schema",
                10.0,
                (u64::from(organization) + u64::from(website), 2),
                (1.0, 0.5),
                missing,
            ));
            let content: Vec<&Page> = pages
                .iter()
                .filter(|page| page.words >= SUBSTANTIAL_WORDS)
                .collect();
            if !content.is_empty() {
                let without: Vec<String> = content
                    .iter()
                    .filter(|page| !questions.contains(&page.id))
                    .map(|page| page.url.clone())
                    .collect();
                signals.push(signal(
                    "question-headings",
                    10.0,
                    (
                        content.len() as u64 - without.len() as u64,
                        content.len() as u64,
                    ),
                    (0.5, 0.2),
                    without,
                ));
            }
            let thin: Vec<String> = pages
                .iter()
                .filter(|page| page.words < SUBSTANTIAL_WORDS)
                .map(|page| page.url.clone())
                .collect();
            signals.push(signal(
                "substantial-content",
                10.0,
                (total - thin.len() as u64, total),
                (0.7, 0.4),
                thin,
            ));
        }
        signals.sort_by(|left, right| right.weight.total_cmp(&left.weight));

        let weights: f64 = signals.iter().map(|signal| signal.weight).sum();
        let score = (total > 0 && weights > 0.0).then(|| {
            let got: f64 = signals
                .iter()
                .map(|signal| signal.weight * signal.value)
                .sum();
            (got / weights * 1000.0).round() / 10.0
        });
        let mut counted: BTreeMap<String, u64> = BTreeMap::new();
        for names in types.values() {
            let unique: HashSet<&String> = names.iter().collect();
            for name in unique {
                *counted.entry(name.clone()).or_default() += 1;
            }
        }
        let mut schema_types: Vec<(String, u64)> = counted.into_iter().collect();
        schema_types.sort_by(|left, right| right.1.cmp(&left.1).then(left.0.cmp(&right.0)));
        Ok(GeoReadiness {
            score,
            pages: total,
            signals,
            crawlers,
            robots: robots.map(|(status, _)| status),
            schema_types,
        })
    }
}

/// `https://example.com` of `https://example.com/a/b`.
fn origin_of(url: &str) -> String {
    let rest = url.split_once("://").map_or(url, |(_, rest)| rest);
    let host = rest.split(['/', '?', '#']).next().unwrap_or_default();
    let scheme = url.split_once("://").map_or("https", |(scheme, _)| scheme);
    format!("{scheme}://{host}")
}
