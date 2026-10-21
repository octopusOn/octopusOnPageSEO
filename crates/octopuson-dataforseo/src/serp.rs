//! Live Google result pages from the DataForSEO SERP API: who ranks for a keyword now and what
//! else the page shows.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::{DfsError, Market, count, items_of};

/// Endpoint of a live organic result page.
pub const ORGANIC: &str = "serp/google/organic/live/advanced";

/// A live result page of up to ten results, US dollars.
pub const LIVE_PAGE: f64 = 0.002;

/// One element of a result page.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SerpItem {
    /// Element type (`organic`, `featured_snippet`, `people_also_ask` …).
    pub kind: String,
    /// Position among elements of the same type.
    pub rank: Option<u32>,
    /// Position among every element of the page.
    pub absolute: Option<u32>,
    /// The site.
    pub domain: Option<String>,
    /// The page.
    pub url: Option<String>,
    /// The title.
    pub title: Option<String>,
    /// The snippet.
    pub description: Option<String>,
}

/// A result page.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SerpPage {
    /// The keyword.
    pub keyword: String,
    /// Results the search engine reports.
    pub results: Option<u64>,
    /// Element types on the page, in order.
    pub features: Vec<String>,
    /// Organic results, by rank.
    pub organic: Vec<SerpItem>,
    /// Other elements (snippets, questions, AI overview).
    pub other: Vec<SerpItem>,
    /// The search page to check it by hand.
    pub check_url: Option<String>,
}

/// The task of a live result page of `keyword` (`depth` results, 10–100) on `device`.
///
/// # Errors
/// [`DfsError::Config`] without a keyword or with an unknown country.
pub fn organic_task(
    keyword: &str,
    market: &Market,
    depth: u32,
    mobile: bool,
) -> Result<Value, DfsError> {
    let keyword = keyword.split_whitespace().collect::<Vec<_>>().join(" ");
    if keyword.is_empty() {
        return Err(DfsError::Config("keyword: enter a keyword".to_owned()));
    }
    let mut task = market.fields()?;
    task["keyword"] = json!(keyword);
    task["depth"] = json!(depth.clamp(10, 100));
    task["device"] = json!(if mobile { "mobile" } else { "desktop" });
    Ok(task)
}

#[allow(clippy::cast_possible_truncation)]
fn small(value: &Value, name: &str) -> Option<u32> {
    value
        .get(name)
        .and_then(count)
        .map(|n| n.min(u64::from(u32::MAX)) as u32)
}

fn text(value: &Value, name: &str) -> Option<String> {
    value
        .get(name)
        .and_then(Value::as_str)
        .filter(|text| !text.is_empty())
        .map(str::to_owned)
}

/// Reads a live organic result page.
pub fn organic_from_result(result: &[Value]) -> SerpPage {
    let first = result.first().unwrap_or(&Value::Null);
    let mut page = SerpPage {
        keyword: text(first, "keyword").unwrap_or_default(),
        results: first.get("se_results_count").and_then(count),
        features: first
            .get("item_types")
            .and_then(Value::as_array)
            .map(|types| {
                types
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default(),
        check_url: text(first, "check_url"),
        ..SerpPage::default()
    };
    for item in items_of(result) {
        let entry = SerpItem {
            kind: text(item, "type").unwrap_or_default(),
            rank: small(item, "rank_group"),
            absolute: small(item, "rank_absolute"),
            domain: text(item, "domain"),
            url: text(item, "url"),
            title: text(item, "title"),
            description: text(item, "description"),
        };
        if entry.kind == "organic" {
            page.organic.push(entry);
        } else {
            page.other.push(entry);
        }
    }
    page.organic
        .sort_by_key(|item| item.rank.unwrap_or(u32::MAX));
    page
}
