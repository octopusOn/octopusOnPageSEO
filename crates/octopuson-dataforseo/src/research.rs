//! Keyword research beyond the lists: a keyword's searches month by month (since 2021), its
//! result pages month by month, the keywords of one page (for the parent topic and traffic
//! potential), and the links and traffic of many pages at once (to weigh a result page).
//!
//! Each report has a task builder, a reader of the answer's result list (so a kept copy reads
//! the same), the endpoint and an estimate.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::keywords::{MAX_OVERVIEW, Monthly, clean_keywords};
use crate::{DfsError, Market, count, items_of, number, prices};

/// Endpoint of keywords' searches month by month.
pub const KEYWORD_HISTORY: &str = "dataforseo_labs/google/historical_keyword_data/live";
/// Endpoint of a keyword's result pages month by month.
pub const SERP_HISTORY: &str = "dataforseo_labs/google/historical_serps/live";
/// Endpoint of the referring domains of many targets.
pub const BULK_REFERRING_DOMAINS: &str = "backlinks/bulk_referring_domains/live";

/// Results of a page compared and weighed.
pub const TOP_RESULTS: usize = 10;

/// Result pages a month of SERP history holds at most, for estimates.
const SERPS_PER_MONTH: u32 = 2;

fn text(value: &Value, pointer: &str) -> Option<String> {
    value
        .pointer(pointer)
        .and_then(Value::as_str)
        .filter(|text| !text.is_empty())
        .map(str::to_owned)
}

#[allow(clippy::cast_possible_truncation)]
fn small(value: &Value, pointer: &str) -> Option<u32> {
    value
        .pointer(pointer)
        .and_then(count)
        .map(|n| n.min(u64::from(u32::MAX)) as u32)
}

// ------------------------------------------------------------------------- Search history

/// A keyword's searches month by month.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeywordHistory {
    /// The keyword.
    pub keyword: String,
    /// Searches per month, oldest first.
    pub months: Vec<Monthly>,
}

/// The task of `keywords`' search history (at most 700).
///
/// # Errors
/// [`DfsError::Config`] without usable keywords or with an unknown country.
pub fn keyword_history_task(keywords: &[String], market: &Market) -> Result<Value, DfsError> {
    let mut task = market.fields()?;
    task["keywords"] = json!(clean_keywords(keywords, MAX_OVERVIEW)?);
    Ok(task)
}

/// Reads a search history answer: each monthly snapshot holds the twelve months before it, so
/// the months are joined, a later snapshot's figure winning.
pub fn keyword_history_from_result(result: &[Value]) -> Vec<KeywordHistory> {
    items_of(result)
        .iter()
        .filter_map(|item| {
            let keyword = text(item, "/keyword")?;
            let mut months: std::collections::BTreeMap<(u32, u32), (u32, u32, u64)> =
                std::collections::BTreeMap::new();
            let mut snapshots: Vec<&Value> = item
                .get("history")
                .and_then(Value::as_array)
                .map(|history| history.iter().collect())
                .unwrap_or_default();
            snapshots.sort_by_key(|snapshot| (small(snapshot, "/year"), small(snapshot, "/month")));
            for snapshot in snapshots {
                let taken = (
                    small(snapshot, "/year").unwrap_or_default(),
                    small(snapshot, "/month").unwrap_or_default(),
                );
                let searches = snapshot
                    .pointer("/keyword_info/monthly_searches")
                    .and_then(Value::as_array)
                    .map(Vec::as_slice)
                    .unwrap_or_default();
                for month in searches {
                    let (Some(year), Some(number), Some(volume)) = (
                        small(month, "/year"),
                        small(month, "/month"),
                        month.get("search_volume").and_then(count),
                    ) else {
                        continue;
                    };
                    let entry = months.entry((year, number)).or_insert((0, 0, volume));
                    if taken >= (entry.0, entry.1) {
                        *entry = (taken.0, taken.1, volume);
                    }
                }
            }
            Some(KeywordHistory {
                keyword,
                months: months
                    .into_iter()
                    .map(|((year, month), (_, _, volume))| Monthly {
                        year,
                        month,
                        volume,
                    })
                    .collect(),
            })
        })
        .collect()
}

/// Estimated cost of the search history of `keywords` keywords.
pub fn keyword_history_estimate(keywords: usize) -> f64 {
    prices::labs(u32::try_from(keywords).unwrap_or(u32::MAX))
}

// ---------------------------------------------------------------------------- SERP history

/// One organic result of a past result page.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PastResult {
    /// Position among organic results.
    pub rank: u32,
    /// The site.
    pub domain: Option<String>,
    /// The page.
    pub url: String,
    /// Its title.
    pub title: Option<String>,
}

/// A result page as it was on a day.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PastSerp {
    /// `yyyy-mm-dd`.
    pub date: String,
    /// Results found.
    pub results_count: Option<u64>,
    /// Elements of the page (`ai_overview`, `people_also_ask` …).
    pub features: Vec<String>,
    /// The first organic results.
    pub results: Vec<PastResult>,
}

/// The task of `keyword`'s result pages since `date_from` (`yyyy-mm-dd`; DataForSEO keeps a
/// year).
///
/// # Errors
/// [`DfsError::Config`] for an empty keyword or an unknown country.
pub fn serp_history_task(
    keyword: &str,
    market: &Market,
    date_from: &str,
) -> Result<Value, DfsError> {
    let keyword = clean_keywords(&[keyword.to_owned()], 1)?.remove(0);
    let mut task = market.fields()?;
    task["keyword"] = json!(keyword);
    task["date_from"] = json!(date_from);
    Ok(task)
}

/// Reads a SERP history answer, oldest page first.
pub fn serp_history_from_result(result: &[Value]) -> Vec<PastSerp> {
    let mut pages: Vec<PastSerp> = result
        .iter()
        .filter_map(|month| month.get("items").and_then(Value::as_array))
        .flatten()
        .filter_map(|page| {
            let date: String = text(page, "/datetime")?.chars().take(10).collect();
            let elements = page
                .get("items")
                .and_then(Value::as_array)
                .map(Vec::as_slice)
                .unwrap_or_default();
            let mut results: Vec<PastResult> = elements
                .iter()
                .filter(|element| element.get("type").and_then(Value::as_str) == Some("organic"))
                .filter_map(|element| {
                    Some(PastResult {
                        rank: small(element, "/rank_group")?,
                        domain: text(element, "/domain"),
                        url: text(element, "/url")?,
                        title: text(element, "/title"),
                    })
                })
                .collect();
            results.sort_by_key(|result| result.rank);
            results.truncate(TOP_RESULTS);
            let features = page
                .get("item_types")
                .and_then(Value::as_array)
                .map(|types| {
                    types
                        .iter()
                        .filter_map(Value::as_str)
                        .filter(|kind| *kind != "organic")
                        .map(str::to_owned)
                        .collect()
                })
                .unwrap_or_default();
            Some(PastSerp {
                date,
                results_count: page.get("se_results_count").and_then(count),
                features,
                results,
            })
        })
        .collect();
    pages.sort_by(|left, right| left.date.cmp(&right.date));
    pages
}

/// The addresses of the latest result page of a SERP history answer, from the top.
pub fn latest_results(result: &[Value]) -> Option<Vec<String>> {
    serp_history_from_result(result)
        .pop()
        .map(|page| page.results.into_iter().map(|result| result.url).collect())
}

/// Estimated cost of `months` months of result pages: a price per page and no task price.
pub fn serp_history_estimate(months: u32) -> f64 {
    prices::HISTORICAL_SERP * f64::from(months.max(1) * SERPS_PER_MONTH)
}

// ---------------------------------------------------------------------------- Page keywords

/// What a page gets from search: its traffic and keywords over every keyword it ranks for.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct PageReach {
    /// Estimated monthly organic traffic.
    pub traffic: f64,
    /// Keywords it ranks for.
    pub keywords: u64,
}

/// The task of a page's organic keywords, most traffic first (the page as DataForSEO takes it:
/// a full address).
///
/// # Errors
/// [`DfsError::Config`] for an address that is not a web page, or an unknown country.
pub fn page_keywords_task(url: &str, market: &Market, limit: u32) -> Result<Value, DfsError> {
    let url = url.trim();
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err(DfsError::Config(format!("`{url}` is not a web page")));
    }
    let mut task = market.fields()?;
    task["target"] = json!(url);
    task["item_types"] = json!(["organic"]);
    task["limit"] = json!(crate::site::rows(limit));
    task["order_by"] = json!(["ranked_serp_element.serp_item.etv,desc"]);
    Ok(task)
}

/// The page's reach over all its keywords, from a `ranked_keywords` answer.
pub fn page_reach(result: &[Value]) -> PageReach {
    let first = result.first().unwrap_or(&Value::Null);
    PageReach {
        traffic: first
            .pointer("/metrics/organic/etv")
            .and_then(number)
            .unwrap_or_default(),
        keywords: first
            .pointer("/metrics/organic/count")
            .and_then(count)
            .unwrap_or_default(),
    }
}

// ------------------------------------------------------------------- Pages' links at once

/// The sites linking to one target.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReferringRow {
    /// The target.
    pub target: String,
    /// Domains linking.
    pub referring_domains: u64,
    /// Registrable domains linking.
    pub referring_main_domains: u64,
}

/// The task of the referring domains of `targets` (pages or domains, at most 1,000).
///
/// # Errors
/// [`DfsError::Config`] without targets.
pub fn bulk_referring_domains_task(targets: &[String]) -> Result<Value, DfsError> {
    let mut clean: Vec<String> = Vec::new();
    for entry in targets {
        if let Ok(entry) = crate::backlinks::target(entry)
            && !clean.contains(&entry)
        {
            clean.push(entry);
        }
    }
    clean.truncate(crate::backlinks::MAX_BULK);
    if clean.is_empty() {
        return Err(DfsError::Config("targets: nothing to look up".to_owned()));
    }
    Ok(json!({ "targets": clean }))
}

/// Reads a bulk referring domains answer.
pub fn bulk_referring_domains_from_result(result: &[Value]) -> Vec<ReferringRow> {
    items_of(result)
        .iter()
        .filter_map(|item| {
            Some(ReferringRow {
                target: text(item, "/target")?,
                referring_domains: item
                    .get("referring_domains")
                    .and_then(count)
                    .unwrap_or_default(),
                referring_main_domains: item
                    .get("referring_main_domains")
                    .and_then(count)
                    .unwrap_or_default(),
            })
        })
        .collect()
}

/// The task of the traffic of pages (full addresses kept) or domains, at most 1,000.
///
/// # Errors
/// [`DfsError::Config`] without targets or with an unknown country.
pub fn bulk_page_traffic_task(targets: &[String], market: &Market) -> Result<Value, DfsError> {
    let mut clean: Vec<String> = Vec::new();
    for entry in targets {
        let entry = entry.trim();
        let entry = if entry.starts_with("https://") || entry.starts_with("http://") {
            entry.to_owned()
        } else {
            crate::target_domain(entry)
        };
        if !entry.is_empty() && !clean.contains(&entry) {
            clean.push(entry);
        }
    }
    clean.truncate(crate::site::MAX_COMPARED);
    if clean.is_empty() {
        return Err(DfsError::Config("targets: nothing to look up".to_owned()));
    }
    let mut task = market.fields()?;
    task["targets"] = json!(clean);
    task["item_types"] = json!(["organic"]);
    Ok(task)
}
