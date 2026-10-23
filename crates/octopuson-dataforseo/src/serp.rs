//! Live Google result pages from the DataForSEO SERP API: who ranks for a keyword now and what
//! else the page shows.

use octopuson_serp::{
    Device, Engine, Feature, FeatureItem, FeatureKind, Organic, SerpQuery, SerpSnapshot,
    registrable_domain,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::{DfsError, Market, count, items_of, target_domain};

/// Endpoint of a live organic result page.
pub const ORGANIC: &str = "serp/google/organic/live/advanced";

/// A live result page of up to ten results, US dollars.
pub const LIVE_PAGE: f64 = crate::prices::SERP_LIVE_PAGE;

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

/// The kind of block a DataForSEO element type is, as the SERP Checker's measures read it.
fn feature_kind(kind: &str) -> FeatureKind {
    match kind {
        "featured_snippet" => FeatureKind::FeaturedSnippet,
        "people_also_ask" => FeatureKind::PeopleAlsoAsk,
        "local_pack" | "map" => FeatureKind::LocalPack,
        "video" | "short_videos" => FeatureKind::Videos,
        "images" => FeatureKind::Images,
        "ai_overview" => FeatureKind::AiOverview,
        "related_searches" | "people_also_search" => FeatureKind::RelatedSearches,
        "top_stories" => FeatureKind::TopStories,
        "knowledge_graph" => FeatureKind::KnowledgePanel,
        "shopping" | "popular_products" => FeatureKind::Shopping,
        "paid" => FeatureKind::AdsTop,
        _ => FeatureKind::Other,
    }
}

/// The entries of a block: "People also ask" questions, related searches (plain strings), the
/// snippet's page.
fn feature_items(item: &Value) -> Vec<FeatureItem> {
    let entry = |value: &Value| -> Option<FeatureItem> {
        if let Some(text) = value.as_str().filter(|text| !text.trim().is_empty()) {
            return Some(FeatureItem {
                text: Some(text.trim().to_owned()),
                url: None,
                domain: None,
            });
        }
        let url = text(value, "url");
        let label = text(value, "title").or_else(|| text(value, "text"));
        (url.is_some() || label.is_some()).then(|| FeatureItem {
            domain: url
                .as_deref()
                .map(|url| registrable_domain(&target_domain(url))),
            text: label,
            url,
        })
    };
    match item.get("items").and_then(Value::as_array) {
        Some(items) => items.iter().filter_map(entry).collect(),
        None => entry(item).into_iter().collect(),
    }
}

/// A result page as the SERP Checker's measures read it: its organic results and the blocks
/// around them, with the "People also ask" questions and related searches.
pub fn snapshot_of(
    result: &[Value],
    keyword: &str,
    market: &Market,
    mobile: bool,
    fetched_at: &str,
) -> SerpSnapshot {
    let first = result.first().unwrap_or(&Value::Null);
    let position = |value: &Value, name: &str| {
        value
            .get(name)
            .and_then(count)
            .and_then(|n| u16::try_from(n).ok())
    };
    let mut organic = Vec::new();
    let mut features = Vec::new();
    for item in items_of(result) {
        let kind = item.get("type").and_then(Value::as_str).unwrap_or_default();
        if kind == "organic" {
            let (Some(rank), Some(url)) = (position(item, "rank_group"), text(item, "url")) else {
                continue;
            };
            let host = target_domain(&url);
            organic.push(Organic {
                position: rank,
                absolute_position: position(item, "rank_absolute"),
                page: 1,
                domain: registrable_domain(&host),
                host,
                title: text(item, "title").unwrap_or_default(),
                snippet: text(item, "description"),
                displayed_url: text(item, "breadcrumb"),
                date: None,
                sitelinks: Vec::new(),
                site_only: false,
                url,
            });
        } else if !kind.is_empty() {
            features.push(Feature {
                kind: feature_kind(kind),
                absolute_position: position(item, "rank_absolute"),
                page: 1,
                items: feature_items(item),
            });
        }
    }
    organic.sort_by_key(|result| result.position);
    SerpSnapshot {
        query: SerpQuery {
            engine: Engine::Google,
            keyword: keyword.split_whitespace().collect::<Vec<_>>().join(" "),
            country: market.country.clone(),
            language: market.language.clone(),
            location: None,
            device: if mobile {
                Device::Mobile
            } else {
                Device::Desktop
            },
            depth: 10,
        },
        provider: crate::ID.to_owned(),
        provider_rev: None,
        parser_rev: None,
        fetched_at: fetched_at.to_owned(),
        total_results: first.get("se_results_count").and_then(count),
        organic,
        features,
        warnings: Vec::new(),
    }
}
