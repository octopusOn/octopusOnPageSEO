//! Keyword Explorer from DataForSEO Labs (Google): the numbers of given keywords, ideas from
//! seeds, related keywords and suggestions containing a phrase.
//!
//! Labs answers hold keywords in two shapes: as the item itself (`keyword_overview`,
//! `keyword_ideas`, `keyword_suggestions`) or under `keyword_data` (`related_keywords`); one
//! reader takes both.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::{DfsError, Market, count, items_of, number, prices};

/// Endpoint of the numbers of given keywords.
pub const OVERVIEW: &str = "dataforseo_labs/google/keyword_overview/live";
/// Endpoint of ideas from seed keywords.
pub const IDEAS: &str = "dataforseo_labs/google/keyword_ideas/live";
/// Endpoint of keywords related to a seed (“searches related to”).
pub const RELATED: &str = "dataforseo_labs/google/related_keywords/live";
/// Endpoint of keywords containing a phrase.
pub const SUGGESTIONS: &str = "dataforseo_labs/google/keyword_suggestions/live";

/// Most keywords one overview measures.
pub const MAX_OVERVIEW: usize = 700;
/// Most seeds one ideas task takes.
pub const MAX_SEEDS: usize = 200;
/// Longest keyword DataForSEO accepts, in characters.
pub const MAX_KEYWORD: usize = 80;

/// Searches in one month.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Monthly {
    /// Year.
    pub year: u32,
    /// Month, 1–12.
    pub month: u32,
    /// Searches.
    pub volume: u64,
}

/// How the searches changed, in percent.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Trend {
    /// Against the previous month.
    pub monthly: Option<i64>,
    /// Against three months before.
    pub quarterly: Option<i64>,
    /// Against a year before.
    pub yearly: Option<i64>,
}

/// A keyword with its numbers.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct KeywordData {
    /// The keyword.
    pub keyword: String,
    /// Average monthly searches.
    pub volume: Option<u64>,
    /// Cost per click, US dollars.
    pub cpc: Option<f64>,
    /// Advertiser competition, 0–1.
    pub competition: Option<f64>,
    /// `LOW`, `MEDIUM` or `HIGH`.
    pub competition_level: Option<String>,
    /// Keyword difficulty, 0–100.
    pub difficulty: Option<u32>,
    /// Main search intent.
    pub intent: Option<String>,
    /// Other intents the keyword also has.
    pub other_intents: Vec<String>,
    /// Searches per month, oldest first.
    pub monthly: Vec<Monthly>,
    /// How the searches changed.
    pub trend: Trend,
    /// Elements of its result page (`featured_snippet`, `ai_overview` …), when asked for.
    pub serp_features: Vec<String>,
    /// Results the search engine reports, when asked for.
    pub results: Option<u64>,
    /// Average backlinks of the top-ten pages.
    pub top_backlinks: Option<u64>,
    /// Average referring domains of the top-ten pages.
    pub top_referring_domains: Option<u64>,
}

/// Keywords found from a seed.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct KeywordList {
    /// Every keyword DataForSEO has for the question.
    pub total: u64,
    /// The seed's own numbers, when asked for.
    pub seed: Option<KeywordData>,
    /// The keywords returned.
    pub keywords: Vec<KeywordData>,
}

#[allow(clippy::cast_possible_truncation)]
fn small(value: &Value, pointer: &str) -> Option<u32> {
    value
        .pointer(pointer)
        .and_then(count)
        .map(|n| n.min(u64::from(u32::MAX)) as u32)
}

#[allow(clippy::cast_possible_truncation)]
fn signed(value: &Value, pointer: &str) -> Option<i64> {
    value.pointer(pointer).and_then(|value| {
        value
            .as_i64()
            .or_else(|| number(value).map(|n| n.round() as i64))
    })
}

fn strings(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

/// A keyword from an item of either shape.
pub fn keyword_data(item: &Value) -> Option<KeywordData> {
    let data = item.get("keyword_data").unwrap_or(item);
    let keyword = data.get("keyword")?.as_str()?.trim().to_owned();
    if keyword.is_empty() {
        return None;
    }
    let info = data.get("keyword_info").unwrap_or(&Value::Null);
    let mut monthly: Vec<Monthly> = info
        .get("monthly_searches")
        .and_then(Value::as_array)
        .map(|months| {
            months
                .iter()
                .filter_map(|month| {
                    Some(Monthly {
                        year: small(month, "/year")?,
                        month: small(month, "/month")?,
                        volume: month
                            .get("search_volume")
                            .and_then(count)
                            .unwrap_or_default(),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    monthly.sort_by_key(|entry| (entry.year, entry.month));
    let other_intents = data
        .pointer("/search_intent_info/foreign_intent")
        .and_then(Value::as_array)
        .map(|intents| {
            intents
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default();
    Some(KeywordData {
        keyword,
        volume: info.get("search_volume").and_then(count),
        cpc: info.get("cpc").and_then(number),
        competition: info.get("competition").and_then(number),
        competition_level: info
            .get("competition_level")
            .and_then(Value::as_str)
            .map(str::to_owned),
        difficulty: small(data, "/keyword_properties/keyword_difficulty"),
        intent: data
            .pointer("/search_intent_info/main_intent")
            .and_then(Value::as_str)
            .map(str::to_owned),
        other_intents,
        monthly,
        trend: Trend {
            monthly: signed(info, "/search_volume_trend/monthly"),
            quarterly: signed(info, "/search_volume_trend/quarterly"),
            yearly: signed(info, "/search_volume_trend/yearly"),
        },
        serp_features: strings(data.pointer("/serp_info/serp_item_types"))
            .into_iter()
            .filter(|kind| kind != "organic")
            .collect(),
        results: data.pointer("/serp_info/se_results_count").and_then(count),
        top_backlinks: data
            .pointer("/avg_backlinks_info/backlinks")
            .and_then(count),
        top_referring_domains: data
            .pointer("/avg_backlinks_info/referring_domains")
            .and_then(count),
    })
}

/// Reads a keyword list answer (overview, ideas, related keywords, suggestions).
pub fn list_from_result(result: &[Value]) -> KeywordList {
    let first = result.first().unwrap_or(&Value::Null);
    let keywords: Vec<KeywordData> = items_of(result).iter().filter_map(keyword_data).collect();
    KeywordList {
        total: first
            .get("total_count")
            .and_then(count)
            .unwrap_or(keywords.len() as u64),
        seed: first.get("seed_keyword_data").and_then(keyword_data),
        keywords,
    }
}

/// Keywords as DataForSEO takes them: trimmed, lowercased, without duplicates or empties,
/// at most `most`.
///
/// # Errors
/// [`DfsError::Config`] when none is left or one is longer than 80 characters.
pub fn clean_keywords(keywords: &[String], most: usize) -> Result<Vec<String>, DfsError> {
    let mut clean: Vec<String> = Vec::new();
    for keyword in keywords {
        let keyword = keyword
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .to_lowercase();
        if keyword.is_empty() || clean.contains(&keyword) {
            continue;
        }
        if keyword.chars().count() > MAX_KEYWORD {
            return Err(DfsError::Config(format!(
                "keywords: `{keyword}` is longer than 80 characters"
            )));
        }
        clean.push(keyword);
    }
    clean.truncate(most);
    if clean.is_empty() {
        return Err(DfsError::Config(
            "keywords: enter at least one keyword".to_owned(),
        ));
    }
    Ok(clean)
}

/// The task measuring `keywords` (at most [`MAX_OVERVIEW`]), with their result-page elements.
///
/// # Errors
/// [`DfsError::Config`] without usable keywords or with an unknown country.
pub fn overview_task(keywords: &[String], market: &Market) -> Result<Value, DfsError> {
    let mut task = market.fields()?;
    task["keywords"] = json!(clean_keywords(keywords, MAX_OVERVIEW)?);
    task["include_serp_info"] = json!(true);
    Ok(task)
}

/// The task of ideas from `seeds` (at most [`MAX_SEEDS`]), most searched first.
///
/// # Errors
/// [`DfsError::Config`] without usable seeds or with an unknown country.
pub fn ideas_task(seeds: &[String], market: &Market, limit: u32) -> Result<Value, DfsError> {
    let mut task = market.fields()?;
    task["keywords"] = json!(clean_keywords(seeds, MAX_SEEDS)?);
    task["limit"] = json!(crate::site::rows(limit));
    task["order_by"] = json!(["keyword_info.search_volume,desc"]);
    Ok(task)
}

fn seed(keyword: &str) -> Result<String, DfsError> {
    clean_keywords(&[keyword.to_owned()], 1).map(|mut clean| clean.remove(0))
}

/// The task of keywords related to `keyword`, `depth` levels deep (1–4), most searched first.
///
/// # Errors
/// [`DfsError::Config`] without a keyword or with an unknown country.
pub fn related_task(
    keyword: &str,
    market: &Market,
    depth: u32,
    limit: u32,
) -> Result<Value, DfsError> {
    let mut task = market.fields()?;
    task["keyword"] = json!(seed(keyword)?);
    task["depth"] = json!(depth.clamp(1, 4));
    task["include_seed_keyword"] = json!(true);
    task["limit"] = json!(crate::site::rows(limit));
    task["order_by"] = json!(["keyword_data.keyword_info.search_volume,desc"]);
    Ok(task)
}

/// The task of keywords containing `keyword`, most searched first.
///
/// # Errors
/// [`DfsError::Config`] without a keyword or with an unknown country.
pub fn suggestions_task(keyword: &str, market: &Market, limit: u32) -> Result<Value, DfsError> {
    suggestions_match_task(keyword, market, limit, false)
}

/// The task of keywords containing `keyword`: anywhere (phrase match, the same task as
/// [`suggestions_task`]) or, with `exact`, as the whole phrase in its order.
///
/// # Errors
/// [`DfsError::Config`] without a keyword or with an unknown country.
pub fn suggestions_match_task(
    keyword: &str,
    market: &Market,
    limit: u32,
    exact: bool,
) -> Result<Value, DfsError> {
    let mut task = market.fields()?;
    task["keyword"] = json!(seed(keyword)?);
    task["include_seed_keyword"] = json!(true);
    task["limit"] = json!(crate::site::rows(limit));
    task["order_by"] = json!(["keyword_info.search_volume,desc"]);
    if exact {
        task["exact_match"] = json!(true);
    }
    Ok(task)
}

/// Estimated cost of measuring `keywords` keywords.
pub fn overview_estimate(keywords: usize) -> f64 {
    prices::labs(u32::try_from(keywords).unwrap_or(u32::MAX))
}
