//! Site Explorer reports from DataForSEO Labs (Google): a domain's rankings over time, the
//! keywords it ranks for, its best pages, its competitors and subdomains, and the traffic of
//! several domains side by side.
//!
//! Each report has a task builder (the request), a reader of the answer's result list (so a kept
//! copy reads the same), the endpoint and an estimate.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::labs::RankMetrics;
use crate::{DfsError, Market, count, items_of, number, prices, target_domain};

/// Months of history asked for unless more are wanted.
pub const HISTORY_MONTHS: u32 = 24;

/// The first month of DataForSEO's ranking history.
pub const HISTORY_START: (u32, u32) = (2020, 10);

/// Most rows one report returns.
pub const MAX_ROWS: u32 = 1_000;

/// Most domains compared at once (`bulk_traffic_estimation` takes up to 1,000).
pub const MAX_COMPARED: usize = 1_000;

/// Endpoint of the ranking history.
pub const HISTORY: &str = "dataforseo_labs/google/historical_rank_overview/live";
/// Endpoint of the keywords a domain ranks for.
pub const RANKED_KEYWORDS: &str = "dataforseo_labs/google/ranked_keywords/live";
/// Endpoint of a domain's pages by traffic.
pub const RELEVANT_PAGES: &str = "dataforseo_labs/google/relevant_pages/live";
/// Endpoint of a domain's organic competitors.
pub const COMPETITORS: &str = "dataforseo_labs/google/competitors_domain/live";
/// Endpoint of a domain's subdomains.
pub const SUBDOMAINS: &str = "dataforseo_labs/google/subdomains/live";
/// Endpoint of the traffic of many domains.
pub const BULK_TRAFFIC: &str = "dataforseo_labs/google/bulk_traffic_estimation/live";

/// A row count within what DataForSEO returns (at least 10).
pub fn rows(limit: u32) -> u32 {
    limit.clamp(10, MAX_ROWS)
}

fn target_task(domain: &str, market: &Market) -> Result<(String, Value), DfsError> {
    let target = target_domain(domain);
    if target.is_empty() {
        return Err(DfsError::Config("the domain is empty".to_owned()));
    }
    let mut task = market.fields()?;
    task["target"] = Value::String(target.clone());
    Ok((target, task))
}

fn text(value: &Value, pointer: &str) -> Option<String> {
    value
        .pointer(pointer)
        .and_then(Value::as_str)
        .filter(|text| !text.is_empty())
        .map(str::to_owned)
}

fn whole(value: &Value, pointer: &str) -> Option<u64> {
    value.pointer(pointer).and_then(count)
}

fn real(value: &Value, pointer: &str) -> Option<f64> {
    value.pointer(pointer).and_then(number)
}

#[allow(clippy::cast_possible_truncation)]
fn small(value: &Value, pointer: &str) -> Option<u32> {
    whole(value, pointer).map(|n| n.min(u64::from(u32::MAX)) as u32)
}

// ------------------------------------------------------------------------------------ History

/// A domain's rankings in one month.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct HistoryPoint {
    /// Year.
    pub year: u32,
    /// Month, 1–12.
    pub month: u32,
    /// Organic results.
    pub organic: RankMetrics,
    /// Paid results.
    pub paid: RankMetrics,
}

/// The first day of the month `months` before `year`-`month`, as `yyyy-mm-01`, never before
/// 2020-10-01 (where DataForSEO's history starts).
pub fn months_before(year: u32, month: u32, months: u32) -> String {
    let index = (year * 12 + month.clamp(1, 12) - 1).saturating_sub(months);
    let (year, month) = (index / 12, index % 12 + 1);
    if (year, month) < (2020, 10) {
        return "2020-10-01".to_owned();
    }
    format!("{year:04}-{month:02}-01")
}

/// The task of a ranking history from `date_from` (`yyyy-mm-dd`).
///
/// # Errors
/// [`DfsError::Config`] for an empty domain or an unknown country.
pub fn history_task(domain: &str, market: &Market, date_from: &str) -> Result<Value, DfsError> {
    let (_, mut task) = target_task(domain, market)?;
    task["date_from"] = Value::String(date_from.to_owned());
    Ok(task)
}

/// The months of a ranking history answer, oldest first.
pub fn history_from_result(result: &[Value]) -> Vec<HistoryPoint> {
    let mut points: Vec<HistoryPoint> = items_of(result)
        .iter()
        .filter_map(|item| {
            Some(HistoryPoint {
                year: small(item, "/year")?,
                month: small(item, "/month")?,
                organic: item
                    .pointer("/metrics/organic")
                    .map(RankMetrics::from_value)
                    .unwrap_or_default(),
                paid: item
                    .pointer("/metrics/paid")
                    .map(RankMetrics::from_value)
                    .unwrap_or_default(),
            })
        })
        .collect();
    points.sort_by_key(|point| (point.year, point.month));
    points
}

/// Months from the start of the history to `year`-`month`, both counted.
pub fn months_since_start(year: u32, month: u32) -> u32 {
    let (start_year, start_month) = HISTORY_START;
    (year * 12 + month.clamp(1, 12)).saturating_sub(start_year * 12 + start_month) + 1
}

/// Estimated cost of a ranking history: this endpoint costs ten times the default Labs price,
/// with one row per month.
pub fn history_estimate() -> f64 {
    history_estimate_for(HISTORY_MONTHS)
}

/// Estimated cost of a ranking history of `months` months.
pub fn history_estimate_for(months: u32) -> f64 {
    prices::labs_history(months)
}

// ------------------------------------------------------------------------------- Ranked keywords

/// How a keyword's position changed since the previous update.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Change {
    /// It ranks now and did not before.
    New,
    /// It moved up.
    Up,
    /// It moved down.
    Down,
    /// It stayed.
    Same,
}

/// The text of an ad a domain shows for a keyword.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdText {
    /// The headline.
    pub title: String,
    /// The description.
    pub description: Option<String>,
    /// The address shown (breadcrumb).
    pub breadcrumb: Option<String>,
}

/// Which of a domain's keywords a list holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KeywordSet {
    /// Organic results it ranks in now.
    #[default]
    Organic,
    /// Organic results it ranked in before and not in the latest check.
    Lost,
    /// Ads it shows.
    Paid,
}

/// A keyword a domain ranks for.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RankedKeyword {
    /// The keyword.
    pub keyword: String,
    /// Position among organic results (`rank_group`).
    pub position: Option<u32>,
    /// Position among every element of the page (`rank_absolute`).
    pub absolute_position: Option<u32>,
    /// The previous absolute position.
    pub previous_position: Option<u32>,
    /// How the position changed.
    pub change: Change,
    /// The ranking address.
    pub url: Option<String>,
    /// Monthly searches.
    pub volume: Option<u64>,
    /// Keyword difficulty, 0–100.
    pub difficulty: Option<u32>,
    /// Cost per click, US dollars.
    pub cpc: Option<f64>,
    /// Advertiser competition, 0–1.
    pub competition: Option<f64>,
    /// Main search intent (`informational`, `navigational`, `commercial`, `transactional`).
    pub intent: Option<String>,
    /// Estimated monthly traffic from this keyword.
    pub traffic: f64,
    /// What that traffic would cost in ads, US dollars.
    pub traffic_value_usd: f64,
    /// Elements of the result page (`featured_snippet`, `ai_overview`, `people_also_ask` …).
    pub serp_features: Vec<String>,
    /// The ad shown, for paid keywords.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ad: Option<AdText>,
}

/// The keywords a domain ranks for.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RankedKeywords {
    /// Every keyword DataForSEO knows for the domain.
    pub total: u64,
    /// The rows returned (by traffic).
    pub keywords: Vec<RankedKeyword>,
}

/// The task of a domain's organic keywords, most traffic first.
///
/// # Errors
/// [`DfsError::Config`] for an empty domain or an unknown country.
pub fn ranked_keywords_task(domain: &str, market: &Market, limit: u32) -> Result<Value, DfsError> {
    ranked_set_task(domain, market, limit, KeywordSet::Organic)
}

/// The task of one set of a domain's keywords: organic ones by traffic (the same task as
/// [`ranked_keywords_task`]), lost ones by search volume, or paid ones by ad traffic.
///
/// # Errors
/// [`DfsError::Config`] for an empty domain or an unknown country.
pub fn ranked_set_task(
    domain: &str,
    market: &Market,
    limit: u32,
    set: KeywordSet,
) -> Result<Value, DfsError> {
    let (_, mut task) = target_task(domain, market)?;
    task["item_types"] = json!([if set == KeywordSet::Paid {
        "paid"
    } else {
        "organic"
    }]);
    task["limit"] = json!(rows(limit));
    if set == KeywordSet::Lost {
        task["historical_serp_mode"] = json!("lost");
        task["order_by"] = json!(["keyword_data.keyword_info.search_volume,desc"]);
    } else {
        task["order_by"] = json!(["ranked_serp_element.serp_item.etv,desc"]);
    }
    Ok(task)
}

fn ranked_keyword(item: &Value) -> Option<RankedKeyword> {
    let keyword = text(item, "/keyword_data/keyword")?;
    let serp = item
        .pointer("/ranked_serp_element/serp_item")
        .unwrap_or(&Value::Null);
    let changes = serp.get("rank_changes").unwrap_or(&Value::Null);
    let flag = |name: &str| changes.get(name).and_then(Value::as_bool).unwrap_or(false);
    let change = if flag("is_new") {
        Change::New
    } else if flag("is_up") {
        Change::Up
    } else if flag("is_down") {
        Change::Down
    } else {
        Change::Same
    };
    let serp_features = item
        .pointer("/keyword_data/serp_info/serp_item_types")
        .or_else(|| item.pointer("/ranked_serp_element/serp_item_types"))
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
    Some(RankedKeyword {
        keyword,
        position: small(serp, "/rank_group"),
        absolute_position: small(serp, "/rank_absolute"),
        previous_position: small(changes, "/previous_rank_absolute"),
        change,
        url: text(serp, "/url"),
        volume: whole(item, "/keyword_data/keyword_info/search_volume"),
        difficulty: small(item, "/keyword_data/keyword_properties/keyword_difficulty")
            .or_else(|| small(item, "/ranked_serp_element/keyword_difficulty")),
        cpc: real(item, "/keyword_data/keyword_info/cpc"),
        competition: real(item, "/keyword_data/keyword_info/competition"),
        intent: text(item, "/keyword_data/search_intent_info/main_intent"),
        traffic: real(serp, "/etv").unwrap_or_default(),
        traffic_value_usd: real(serp, "/estimated_paid_traffic_cost").unwrap_or_default(),
        serp_features,
        ad: (serp.get("type").and_then(Value::as_str) == Some("paid"))
            .then(|| {
                Some(AdText {
                    title: text(serp, "/title")?,
                    description: text(serp, "/description"),
                    breadcrumb: text(serp, "/breadcrumb"),
                })
            })
            .flatten(),
    })
}

/// Reads a `ranked_keywords` answer.
pub fn ranked_keywords_from_result(result: &[Value]) -> RankedKeywords {
    RankedKeywords {
        total: result
            .first()
            .and_then(|first| whole(first, "/total_count"))
            .unwrap_or_default(),
        keywords: items_of(result).iter().filter_map(ranked_keyword).collect(),
    }
}

// ----------------------------------------------------------------------------------- Pages

/// One page of a domain and the traffic it gets.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PageRow {
    /// The page.
    pub url: String,
    /// Organic rankings of the page.
    pub organic: RankMetrics,
}

/// The task of a domain's pages, most organic traffic first.
///
/// # Errors
/// [`DfsError::Config`] for an empty domain or an unknown country.
pub fn pages_task(domain: &str, market: &Market, limit: u32) -> Result<Value, DfsError> {
    let (_, mut task) = target_task(domain, market)?;
    task["item_types"] = json!(["organic"]);
    task["limit"] = json!(rows(limit));
    task["order_by"] = json!(["metrics.organic.etv,desc"]);
    Ok(task)
}

/// Reads a `relevant_pages` answer.
pub fn pages_from_result(result: &[Value]) -> Vec<PageRow> {
    items_of(result)
        .iter()
        .filter_map(|item| {
            Some(PageRow {
                url: text(item, "/page_address")?,
                organic: item
                    .pointer("/metrics/organic")
                    .map(RankMetrics::from_value)
                    .unwrap_or_default(),
            })
        })
        .collect()
}

// ------------------------------------------------------------------------------ Competitors

/// A domain competing for the same keywords.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CompetitorRow {
    /// The competitor.
    pub domain: String,
    /// Keywords both rank for.
    pub shared_keywords: u64,
    /// The competitor's average position on the shared keywords.
    pub average_position: Option<f64>,
    /// The competitor's traffic from the shared keywords.
    pub shared_traffic: f64,
    /// All the competitor's organic keywords.
    pub keywords: u64,
    /// All the competitor's organic traffic.
    pub traffic: f64,
    /// What that traffic would cost in ads, US dollars.
    pub traffic_value_usd: f64,
}

/// The task of a domain's organic competitors, most shared keywords first, without the giants
/// every site shares results with (Wikipedia, video sites …).
///
/// # Errors
/// [`DfsError::Config`] for an empty domain or an unknown country.
pub fn competitors_task(domain: &str, market: &Market, limit: u32) -> Result<Value, DfsError> {
    let (_, mut task) = target_task(domain, market)?;
    task["item_types"] = json!(["organic"]);
    task["limit"] = json!(rows(limit));
    task["exclude_top_domains"] = json!(true);
    task["order_by"] = json!(["metrics.organic.count,desc"]);
    Ok(task)
}

/// Reads a `competitors_domain` answer, leaving out the domain itself.
pub fn competitors_from_result(target: &str, result: &[Value]) -> Vec<CompetitorRow> {
    items_of(result)
        .iter()
        .filter_map(|item| {
            let domain = text(item, "/domain")?;
            if target_domain(&domain) == target_domain(target) {
                return None;
            }
            Some(CompetitorRow {
                shared_keywords: whole(item, "/intersections").unwrap_or_default(),
                average_position: real(item, "/avg_position"),
                shared_traffic: real(item, "/metrics/organic/etv").unwrap_or_default(),
                keywords: whole(item, "/full_domain_metrics/organic/count").unwrap_or_default(),
                traffic: real(item, "/full_domain_metrics/organic/etv").unwrap_or_default(),
                traffic_value_usd: real(
                    item,
                    "/full_domain_metrics/organic/estimated_paid_traffic_cost",
                )
                .unwrap_or_default(),
                domain,
            })
        })
        .collect()
}

// ------------------------------------------------------------------------------- Subdomains

/// A subdomain and its organic reach.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SubdomainRow {
    /// The subdomain.
    pub subdomain: String,
    /// Organic rankings.
    pub organic: RankMetrics,
}

/// The task of a domain's subdomains, most organic traffic first.
///
/// # Errors
/// [`DfsError::Config`] for an empty domain or an unknown country.
pub fn subdomains_task(domain: &str, market: &Market, limit: u32) -> Result<Value, DfsError> {
    let (_, mut task) = target_task(domain, market)?;
    task["limit"] = json!(rows(limit));
    task["order_by"] = json!(["metrics.organic.etv,desc"]);
    Ok(task)
}

/// Reads a `subdomains` answer.
pub fn subdomains_from_result(result: &[Value]) -> Vec<SubdomainRow> {
    items_of(result)
        .iter()
        .filter_map(|item| {
            Some(SubdomainRow {
                subdomain: text(item, "/subdomain")?,
                organic: item
                    .pointer("/metrics/organic")
                    .map(RankMetrics::from_value)
                    .unwrap_or_default(),
            })
        })
        .collect()
}

// -------------------------------------------------------------------------------- Countries

/// Rows of the traffic by country: every location DataForSEO measures.
pub const COUNTRY_ROWS: u32 = 100;

/// A domain's rankings in one country and language.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CountryRow {
    /// ISO country code (`tr`), when the platform knows the location.
    pub country: Option<String>,
    /// DataForSEO's location code.
    pub location_code: u32,
    /// Language code.
    pub language: Option<String>,
    /// Organic results.
    pub organic: RankMetrics,
    /// Paid results.
    pub paid: RankMetrics,
}

/// The task of a domain's rankings in every country (`domain_rank_overview` without a
/// location).
///
/// # Errors
/// [`DfsError::Config`] for an empty domain.
pub fn countries_task(domain: &str) -> Result<Value, DfsError> {
    let target = target_domain(domain);
    if target.is_empty() {
        return Err(DfsError::Config("the domain is empty".to_owned()));
    }
    Ok(json!({ "target": target, "limit": COUNTRY_ROWS }))
}

/// Reads a `domain_rank_overview` answer over every country, most organic traffic first.
pub fn countries_from_result(result: &[Value]) -> Vec<CountryRow> {
    let mut rows: Vec<CountryRow> = items_of(result)
        .iter()
        .filter_map(|item| {
            let location_code = small(item, "/location_code")?;
            Some(CountryRow {
                country: octopuson_serp::locale::country_of_location(location_code)
                    .map(str::to_owned),
                location_code,
                language: text(item, "/language_code"),
                organic: item
                    .pointer("/metrics/organic")
                    .map(RankMetrics::from_value)
                    .unwrap_or_default(),
                paid: item
                    .pointer("/metrics/paid")
                    .map(RankMetrics::from_value)
                    .unwrap_or_default(),
            })
        })
        .collect();
    rows.sort_by(|left, right| right.organic.traffic.total_cmp(&left.organic.traffic));
    rows
}

// ------------------------------------------------------------------------------ Bulk traffic

/// The traffic of one domain among several.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct TrafficRow {
    /// The domain.
    pub target: String,
    /// Estimated monthly organic traffic.
    pub organic_traffic: f64,
    /// Organic keywords.
    pub organic_keywords: u64,
    /// Estimated monthly paid traffic.
    pub paid_traffic: f64,
    /// Paid keywords.
    pub paid_keywords: u64,
}

/// The task comparing the traffic of `targets` (at most [`MAX_COMPARED`], duplicates dropped).
///
/// # Errors
/// [`DfsError::Config`] without targets or with an unknown country.
pub fn bulk_traffic_task(targets: &[String], market: &Market) -> Result<Value, DfsError> {
    let mut domains: Vec<String> = Vec::new();
    for target in targets {
        let domain = target_domain(target);
        if !domain.is_empty() && !domains.contains(&domain) {
            domains.push(domain);
        }
    }
    domains.truncate(MAX_COMPARED);
    if domains.is_empty() {
        return Err(DfsError::Config("no domains to compare".to_owned()));
    }
    let mut task = market.fields()?;
    task["targets"] = json!(domains);
    task["item_types"] = json!(["organic", "paid"]);
    Ok(task)
}

/// Reads a `bulk_traffic_estimation` answer, in the order of the targets asked.
pub fn bulk_traffic_from_result(result: &[Value]) -> Vec<TrafficRow> {
    items_of(result)
        .iter()
        .filter_map(|item| {
            Some(TrafficRow {
                target: text(item, "/target")?,
                organic_traffic: real(item, "/metrics/organic/etv").unwrap_or_default(),
                organic_keywords: whole(item, "/metrics/organic/count").unwrap_or_default(),
                paid_traffic: real(item, "/metrics/paid/etv").unwrap_or_default(),
                paid_keywords: whole(item, "/metrics/paid/count").unwrap_or_default(),
            })
        })
        .collect()
}

/// Estimated cost of a report returning up to `rows` rows.
pub fn estimate(rows: u32) -> f64 {
    prices::labs(rows)
}
