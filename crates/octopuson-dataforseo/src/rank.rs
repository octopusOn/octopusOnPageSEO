//! Rank checks: where a site ranks for a keyword on a live Google, Bing or Yandex result page,
//! which of its pages rank, what else the page shows and whether an AI Overview cites it.
//!
//! DataForSEO prices a live page per ten results, so a check's depth is rounded up to whole
//! pages. Google may load an AI Overview after the page; asking for it costs a little more,
//! refunded when there is none.
//!
//! Checks that need not answer at once (the schedule's) go through the standard queue instead:
//! up to a hundred tasks a request, each read back by its id within minutes, for less than a
//! third of the live price.

use octopuson_serp::{Device, Engine};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::prices::{SERP_STANDARD_AI_OVERVIEW, SERP_STANDARD_PAGE};
pub use crate::queue::{Posted, QUEUE_BATCH, posted_from_tasks};
use crate::serp::LIVE_PAGE;
use crate::{DfsError, Market, count, items_of, target_domain};

/// Deepest check, in results.
pub const MAX_DEPTH: u32 = 100;

/// Results of each check kept for the shares (positions past 20 get no clicks).
pub const KEPT_RESULTS: usize = 20;

/// Extra price of an AI Overview Google loads after the page, US dollars.
pub const AI_OVERVIEW_USD: f64 = crate::prices::SERP_AI_OVERVIEW;

/// Longest keyword DataForSEO takes.
const MAX_KEYWORD: usize = 700;

/// Radius of a place given by coordinates without one (DataForSEO's example).
const DEFAULT_RADIUS: f64 = 200.0;

/// Smallest and largest radius DataForSEO takes with coordinates.
const RADII: std::ops::RangeInclusive<f64> = 199.0..=199_999.0;

/// The live endpoint of an engine's organic results.
pub const fn endpoint(engine: Engine) -> &'static str {
    match engine {
        Engine::Google => crate::serp::ORGANIC,
        Engine::Bing => "serp/bing/organic/live/advanced",
        Engine::Yandex => "serp/yandex/organic/live/advanced",
    }
}

/// The standard queue's endpoint taking an engine's checks.
pub const fn queue_post(engine: Engine) -> &'static str {
    match engine {
        Engine::Google => "serp/google/organic/task_post",
        Engine::Bing => "serp/bing/organic/task_post",
        Engine::Yandex => "serp/yandex/organic/task_post",
    }
}

/// Where a queued check's result page is read, free, by the id the queue gave it.
pub fn queue_get(engine: Engine, id: &str) -> String {
    format!("serp/{}/organic/task_get/advanced/{id}", engine.as_str())
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

/// The list price of one check through the standard queue, US dollars.
pub fn queue_estimate(engine: Engine, results: u32, ai_overview: bool) -> f64 {
    let pages = depth(results) / 10;
    let late = if ai_overview && engine == Engine::Google {
        SERP_STANDARD_AI_OVERVIEW
    } else {
        0.0
    };
    SERP_STANDARD_PAGE * f64::from(pages) + late
}

/// Whether a place is written as coordinates (`latitude,longitude` and maybe a radius) rather
/// than named.
pub fn is_coordinate(location: &str) -> bool {
    let parts: Vec<&str> = location.split(',').map(str::trim).collect();
    (2..=3).contains(&parts.len()) && parts.iter().all(|part| part.parse::<f64>().is_ok())
}

/// A number with at most seven decimals and no trailing zeros.
fn decimal(value: f64) -> String {
    let text = format!("{value:.7}");
    text.trim_end_matches('0').trim_end_matches('.').to_owned()
}

/// Coordinates as DataForSEO takes them: `latitude,longitude,radius`, seven decimals at most.
fn coordinates(location: &str) -> Result<String, DfsError> {
    let numbers: Vec<f64> = location
        .split(',')
        .filter_map(|part| part.trim().parse().ok())
        .collect();
    let (latitude, longitude) = (numbers[0], numbers[1]);
    let radius = numbers.get(2).copied().unwrap_or(DEFAULT_RADIUS);
    if !(-90.0..=90.0).contains(&latitude)
        || !(-180.0..=180.0).contains(&longitude)
        || !RADII.contains(&radius)
    {
        return Err(DfsError::Config(
            "location: coordinates as latitude,longitude with an optional radius of 199–199999"
                .to_owned(),
        ));
    }
    Ok(format!(
        "{},{},{}",
        decimal(latitude),
        decimal(longitude),
        decimal(radius.round())
    ))
}

/// A city, region or postal code as DataForSEO names it (`Istanbul,Istanbul,Turkey`), checked
/// to be in the market's country; or coordinates (`41.0082,28.9784` with an optional radius),
/// which DataForSEO takes as they are.
///
/// # Errors
/// [`DfsError::Config`] for a location outside the country, unusable coordinates, or an unknown
/// country.
pub fn location(location: &str, market: &Market) -> Result<String, DfsError> {
    if is_coordinate(location) {
        market.location_name()?;
        return coordinates(location);
    }
    let parts: Vec<&str> = location
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect();
    let country = market.location_name()?;
    let inside = parts.len() >= 2
        && parts
            .last()
            .is_some_and(|last| octopuson_serp::locale::is_country_name(&market.country, last));
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
    // A city, region or point replaces the country's code: DataForSEO takes one location.
    if let Some(place) = query.location {
        let place = location(place, query.market)?;
        if let Some(fields) = task.as_object_mut() {
            fields.remove("location_code");
        }
        if is_coordinate(&place) {
            task["location_coordinate"] = json!(place);
        } else {
            task["location_name"] = json!(place);
        }
    }
    task["keyword"] = json!(keyword);
    task["depth"] = json!(depth(query.depth));
    task["device"] = json!(query.device.as_str());
    if query.ai_overview && query.engine == Engine::Google {
        task["load_async_ai_overview"] = json!(true);
    }
    Ok(task)
}

/// The task of a check sent to the standard queue (normal priority), named by `tag` so its
/// answer finds its keyword and place.
///
/// # Errors
/// As [`task`].
pub fn queued_task(query: &RankQuery<'_>, tag: &str) -> Result<Value, DfsError> {
    let mut task = task(query)?;
    task["priority"] = json!(1);
    task["tag"] = json!(tag.chars().take(255).collect::<String>());
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
    /// Element types of the page that show the site (its featured snippet, an AI Overview
    /// citing it, its place in the local pack, a video or a question's answer from it).
    pub owned: Vec<String>,
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

/// Every host an element of the page names: its own, its parts' and their references', a few
/// levels down.
fn hosts_in(value: &Value, levels: u8, hosts: &mut Vec<String>) {
    if levels == 0 {
        return;
    }
    match value {
        Value::Object(fields) => {
            if let Some(domain) = host(value) {
                hosts.push(domain);
            } else if let Some(url) = fields.get("url").and_then(Value::as_str) {
                let domain = target_domain(url);
                if !domain.is_empty() {
                    hosts.push(domain);
                }
            }
            for child in fields
                .values()
                .filter(|child| child.is_array() || child.is_object())
            {
                hosts_in(child, levels - 1, hosts);
            }
        }
        Value::Array(list) => {
            for child in list {
                hosts_in(child, levels - 1, hosts);
            }
        }
        _ => {}
    }
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
    let mut owned: Vec<String> = Vec::new();
    for item in items_of(result) {
        let kind = item.get("type").and_then(Value::as_str).unwrap_or_default();
        let ours = host(item).is_some_and(|host| belongs(&host, &site));
        if !kind.is_empty() && kind != "organic" && kind != "paid" {
            let mut hosts = Vec::new();
            hosts_in(item, 5, &mut hosts);
            if hosts.iter().any(|host| belongs(host, &site))
                && !owned.iter().any(|known| known == kind)
            {
                owned.push(kind.to_owned());
            }
        }
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
    // A snippet shown as an organic result is held the same way.
    if check.snippet && !owned.iter().any(|kind| kind == "featured_snippet") {
        owned.push("featured_snippet".to_owned());
    }
    check.owned = owned;
    organic.truncate(KEPT_RESULTS);
    check.results = organic;
    check
}
