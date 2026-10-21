//! Rank checks: where a site ranks for a keyword on a live Google, Bing or Yandex result page,
//! which of its pages rank, what else the page shows and whether an AI Overview cites it.
//!
//! DataForSEO prices a live page per ten results, so a check's depth is rounded up to whole
//! pages. Google may load an AI Overview after the page; asking for it costs a little more,
//! refunded when there is none.

use octopuson_serp::{Device, Engine};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::serp::LIVE_PAGE;
use crate::{DfsError, Market, count, items_of, target_domain};

/// Deepest check, in results.
pub const MAX_DEPTH: u32 = 100;

/// Results of each check kept for the shares (positions past 20 get no clicks).
pub const KEPT_RESULTS: usize = 20;

/// Extra price of an AI Overview Google loads after the page, US dollars.
pub const AI_OVERVIEW_USD: f64 = 0.002;

/// Longest keyword DataForSEO takes.
const MAX_KEYWORD: usize = 700;

/// The live endpoint of an engine's organic results.
pub const fn endpoint(engine: Engine) -> &'static str {
    match engine {
        Engine::Google => crate::serp::ORGANIC,
        Engine::Bing => "serp/bing/organic/live/advanced",
        Engine::Yandex => "serp/yandex/organic/live/advanced",
    }
}

/// Results looked at: 10–100, rounded up to whole pages of ten.
pub fn depth(depth: u32) -> u32 {
    depth.clamp(10, MAX_DEPTH).div_ceil(10) * 10
}

/// The list price of one check, US dollars: a live page per ten results, and on Google the AI
/// Overview loaded after the page when asked for.
pub fn estimate(engine: Engine, results: u32, ai_overview: bool) -> f64 {
    let pages = depth(results) / 10;
    let late = if ai_overview && engine == Engine::Google {
        AI_OVERVIEW_USD
    } else {
        0.0
    };
    LIVE_PAGE * f64::from(pages) + late
}

/// A city or region as DataForSEO names it (`Istanbul,Istanbul,Turkey`), checked to be in the
/// market's country.
///
/// # Errors
/// [`DfsError::Config`] for a location outside the country, or an unknown country.
pub fn location(location: &str, market: &Market) -> Result<String, DfsError> {
    let parts: Vec<&str> = location
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect();
    let country = market.location_name()?;
    let inside = parts.len() >= 2
        && parts
            .last()
            .is_some_and(|last| last.eq_ignore_ascii_case(country));
    if !inside {
        return Err(DfsError::Config(format!(
            "location: a city or region of {country}, written like `Istanbul,Istanbul,Turkey`"
        )));
    }
    Ok(parts.join(","))
}

/// Where and how a keyword is checked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RankQuery<'a> {
    /// The keyword.
    pub keyword: &'a str,
    /// The search engine.
    pub engine: Engine,
    /// Country and language.
    pub market: &'a Market,
    /// A city or region; the whole country when `None`.
    pub location: Option<&'a str>,
    /// Desktop or phone results.
    pub device: Device,
    /// Results looked at (10–100).
    pub depth: u32,
    /// Ask for an AI Overview Google loads after the page.
    pub ai_overview: bool,
}

/// The task of one check.
///
/// # Errors
/// [`DfsError::Config`] for an empty or too long keyword, an unknown country or a location
/// outside it.
pub fn task(query: &RankQuery<'_>) -> Result<Value, DfsError> {
    let keyword = query
        .keyword
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if keyword.is_empty() {
        return Err(DfsError::Config("keyword: enter a keyword".to_owned()));
    }
    if keyword.chars().count() > MAX_KEYWORD {
        return Err(DfsError::Config(
            "keyword: at most 700 characters".to_owned(),
        ));
    }
    let mut task = query.market.fields()?;
    if let Some(place) = query.location {
        task["location_name"] = json!(location(place, query.market)?);
    }
    task["keyword"] = json!(keyword);
    task["depth"] = json!(depth(query.depth));
    task["device"] = json!(query.device.as_str());
    if query.ai_overview && query.engine == Engine::Google {
        task["load_async_ai_overview"] = json!(true);
    }
    Ok(task)
}

/// One organic result of a check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RankResult {
    /// Organic position.
    pub position: u16,
    /// Host, without `www.`.
    pub domain: String,
    /// The page.
    pub url: String,
    /// Its title.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
}

/// What one check found.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RankCheck {
    /// Best organic position of the site.
    pub position: Option<u16>,
    /// Its page there.
    pub url: Option<String>,
    /// Every page of the site among the results, by position.
    pub pages: Vec<(u16, String)>,
    /// Element types on the page (`ai_overview`, `featured_snippet`, `local_pack` …).
    pub features: Vec<String>,
    /// An AI Overview is shown.
    pub ai_overview: bool,
    /// The AI Overview cites the site.
    pub ai_cited: bool,
    /// The site holds the featured snippet.
    pub snippet: bool,
    /// The first organic results, for the shares.
    pub results: Vec<RankResult>,
    /// Results the engine reports.
    pub total: Option<u64>,
    /// The search page, to look at it by hand.
    pub check_url: Option<String>,
}

fn text(value: &Value, name: &str) -> Option<String> {
    value
        .get(name)
        .and_then(Value::as_str)
        .filter(|text| !text.is_empty())
        .map(str::to_owned)
}

/// A host without `www.`, lowercase.
fn host(value: &Value) -> Option<String> {
    let domain = value.get("domain")?.as_str()?.trim().to_ascii_lowercase();
    let domain = domain.strip_prefix("www.").unwrap_or(&domain);
    (!domain.is_empty()).then(|| domain.trim_end_matches('.').to_owned())
}

/// Whether a host (without `www.`) belongs to a site: the domain itself or one of its
/// subdomains.
pub fn belongs(host: &str, site: &str) -> bool {
    !site.is_empty()
        && (host == site
            || host
                .strip_suffix(site)
                .is_some_and(|rest| rest.ends_with('.')))
}

/// The domains an AI Overview cites: its references and those of its parts, and its links.
fn cited(item: &Value) -> Vec<String> {
    let mut domains = Vec::new();
    let mut add = |list: Option<&Value>| {
        for entry in list.and_then(Value::as_array).into_iter().flatten() {
            if let Some(domain) = host(entry) {
                domains.push(domain);
            }
        }
    };
    add(item.get("references"));
    for part in item
        .get("items")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        add(part.get("references"));
        add(part.get("links"));
    }
    domains
}

/// Reads a check's result page for `site` (a domain; its subdomains count as the site).
pub fn rank_from_result(result: &[Value], site: &str) -> RankCheck {
    let site = target_domain(site);
    let first = result.first().unwrap_or(&Value::Null);
    let features: Vec<String> = first
        .get("item_types")
        .and_then(Value::as_array)
        .map(|types| {
            types
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default();
    let mut check = RankCheck {
        ai_overview: features.iter().any(|kind| kind == "ai_overview"),
        features,
        total: first.get("se_results_count").and_then(count),
        check_url: text(first, "check_url"),
        ..RankCheck::default()
    };
    let mut organic: Vec<RankResult> = Vec::new();
    for item in items_of(result) {
        let kind = item.get("type").and_then(Value::as_str).unwrap_or_default();
        let ours = host(item).is_some_and(|host| belongs(&host, &site));
        match kind {
            "organic" => {
                let (Some(position), Some(domain), Some(url)) = (
                    item.get("rank_group")
                        .and_then(count)
                        .and_then(|n| u16::try_from(n).ok()),
                    host(item),
                    text(item, "url"),
                ) else {
                    continue;
                };
                if ours && item.get("is_featured_snippet") == Some(&Value::Bool(true)) {
                    check.snippet = true;
                }
                organic.push(RankResult {
                    position,
                    domain,
                    url,
                    title: text(item, "title"),
                });
            }
            "featured_snippet" if ours => check.snippet = true,
            "ai_overview" => {
                check.ai_overview = true;
                if cited(item).iter().any(|domain| belongs(domain, &site)) {
                    check.ai_cited = true;
                }
            }
            _ => {}
        }
    }
    organic.sort_by_key(|result| result.position);
    check.pages = organic
        .iter()
        .filter(|result| belongs(&result.domain, &site))
        .map(|result| (result.position, result.url.clone()))
        .collect();
    if let Some((position, url)) = check.pages.first() {
        check.position = Some(*position);
        check.url = Some(url.clone());
    }
    organic.truncate(KEPT_RESULTS);
    check.results = organic;
    check
}
