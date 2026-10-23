//! The market around a site: the traffic of several domains month by month (each one's share of
//! their market over time), the product and service categories the site ranks in, and the
//! domains getting the most traffic in a category.
//!
//! Each report has a task builder, a reader of the answer's result list (so a kept copy reads
//! the same), the endpoint and an estimate.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::site::{self, HISTORY_START};
use crate::{DfsError, Market, count, items_of, number, prices, target_domain};

/// Endpoint of the traffic of many domains month by month.
pub const TRAFFIC_HISTORY: &str = "dataforseo_labs/google/historical_bulk_traffic_estimation/live";
/// Endpoint of the categories a domain ranks in.
pub const CATEGORIES_FOR_DOMAIN: &str = "dataforseo_labs/google/categories_for_domain/live";
/// Endpoint of the domains with the most traffic in categories.
pub const CATEGORY_DOMAINS: &str = "dataforseo_labs/google/domain_metrics_by_categories/live";
/// The list of category codes and names (free, read with GET).
pub const CATEGORY_NAMES: &str = "dataforseo_labs/categories";

/// Most domains a share history compares.
pub const MAX_SHARE_DOMAINS: usize = 20;

/// Categories of a domain read.
pub const CATEGORY_ROWS: u32 = 50;

/// Domains of a category read.
pub const PLAYER_ROWS: u32 = 50;

/// Most categories asked at once.
pub const MAX_CATEGORIES: usize = 5;

fn small(value: &Value, name: &str) -> Option<u32> {
    value
        .get(name)
        .and_then(count)
        .map(|n| u32::try_from(n).unwrap_or(u32::MAX))
}

// ----------------------------------------------------------------------------- Share history

/// A domain's organic traffic in one month.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct TrafficMonth {
    /// Year.
    pub year: u32,
    /// Month, 1–12.
    pub month: u32,
    /// Estimated organic traffic.
    pub traffic: f64,
    /// Organic keywords.
    pub keywords: u64,
}

/// A domain's organic traffic month by month.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct TrafficHistory {
    /// The domain.
    pub target: String,
    /// Months, oldest first.
    pub months: Vec<TrafficMonth>,
}

/// The task of the organic traffic of `targets` (at most [`MAX_SHARE_DOMAINS`]) month by month
/// since `date_from` (`yyyy-mm-dd`, not before October 2020).
///
/// # Errors
/// [`DfsError::Config`] without targets or with an unknown country.
pub fn traffic_history_task(
    targets: &[String],
    market: &Market,
    date_from: &str,
) -> Result<Value, DfsError> {
    let mut domains: Vec<String> = Vec::new();
    for target in targets {
        let domain = target_domain(target);
        if !domain.is_empty() && !domains.contains(&domain) {
            domains.push(domain);
        }
    }
    domains.truncate(MAX_SHARE_DOMAINS);
    if domains.is_empty() {
        return Err(DfsError::Config(
            "targets: no domains to compare".to_owned(),
        ));
    }
    let (year, month) = HISTORY_START;
    let start = format!("{year:04}-{month:02}-01");
    let mut task = market.fields()?;
    task["targets"] = json!(domains);
    task["date_from"] = json!(if date_from < start.as_str() {
        start.as_str()
    } else {
        date_from
    });
    task["item_types"] = json!(["organic"]);
    Ok(task)
}

/// Reads a traffic history answer, in the order of the targets asked.
pub fn traffic_history_from_result(result: &[Value]) -> Vec<TrafficHistory> {
    items_of(result)
        .iter()
        .filter_map(|item| {
            let target = item.get("target").and_then(Value::as_str)?.to_owned();
            let mut months: Vec<TrafficMonth> = item
                .pointer("/metrics/organic")
                .and_then(Value::as_array)
                .map(|months| {
                    months
                        .iter()
                        .filter_map(|month| {
                            Some(TrafficMonth {
                                year: small(month, "year")?,
                                month: small(month, "month")?,
                                traffic: month.get("etv").and_then(number).unwrap_or_default(),
                                keywords: month.get("count").and_then(count).unwrap_or_default(),
                            })
                        })
                        .collect()
                })
                .unwrap_or_default();
            months.sort_by_key(|month| (month.year, month.month));
            Some(TrafficHistory { target, months })
        })
        .collect()
}

/// Estimated cost of the traffic history of `targets` domains: ten times the default Labs price,
/// a row per domain.
pub fn traffic_history_estimate(targets: usize) -> f64 {
    prices::labs_history(u32::try_from(targets).unwrap_or(u32::MAX))
}

// -------------------------------------------------------------------------------- Categories

/// A category a domain ranks in.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct DomainCategory {
    /// The category's code, the most specific last of its path.
    pub code: u32,
    /// Its path of codes, from the top.
    pub path: Vec<u32>,
    /// The domain's organic traffic in it.
    pub traffic: f64,
    /// The domain's organic keywords in it.
    pub keywords: u64,
}

/// The task of the categories `domain` ranks in, most traffic first.
///
/// # Errors
/// [`DfsError::Config`] for an empty domain or an unknown country.
pub fn categories_task(domain: &str, market: &Market) -> Result<Value, DfsError> {
    let target = target_domain(domain);
    if target.is_empty() {
        return Err(DfsError::Config("the domain is empty".to_owned()));
    }
    let mut task = market.fields()?;
    task["target"] = json!(target);
    task["item_types"] = json!(["organic"]);
    task["limit"] = json!(CATEGORY_ROWS);
    task["order_by"] = json!(["metrics.organic.etv,desc"]);
    Ok(task)
}

/// Reads a categories answer.
pub fn categories_from_result(result: &[Value]) -> Vec<DomainCategory> {
    items_of(result)
        .iter()
        .filter_map(|item| {
            let path: Vec<u32> = item
                .get("categories")
                .and_then(Value::as_array)?
                .iter()
                .filter_map(|code| count(code).and_then(|code| u32::try_from(code).ok()))
                .collect();
            Some(DomainCategory {
                code: *path.last()?,
                traffic: item
                    .pointer("/metrics/organic/etv")
                    .and_then(number)
                    .unwrap_or_default(),
                keywords: item
                    .pointer("/metrics/organic/count")
                    .and_then(count)
                    .unwrap_or_default(),
                path,
            })
        })
        .collect()
}

/// A domain getting traffic in a category.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CategoryPlayer {
    /// The domain.
    pub domain: String,
    /// Its organic traffic now.
    pub traffic: f64,
    /// Its organic keywords now.
    pub keywords: u64,
    /// How its traffic changed since the earlier month.
    pub traffic_change: Option<f64>,
}

/// The task of the domains with the most organic traffic in `codes` (at most
/// [`MAX_CATEGORIES`]), compared between `earlier` and `later` (`yyyy-mm-dd`, different months).
///
/// # Errors
/// [`DfsError::Config`] without categories or with an unknown country.
pub fn players_task(
    codes: &[u32],
    market: &Market,
    earlier: &str,
    later: &str,
) -> Result<Value, DfsError> {
    let codes: Vec<u32> = codes.iter().copied().take(MAX_CATEGORIES).collect();
    if codes.is_empty() {
        return Err(DfsError::Config("categories: choose a category".to_owned()));
    }
    let mut task = market.fields()?;
    task["category_codes"] = json!(codes);
    task["first_date"] = json!(earlier);
    task["second_date"] = json!(later);
    task["item_types"] = json!(["organic"]);
    task["limit"] = json!(PLAYER_ROWS);
    Ok(task)
}

/// Reads a category players answer, most traffic first.
pub fn players_from_result(result: &[Value]) -> Vec<CategoryPlayer> {
    let mut players: Vec<CategoryPlayer> = items_of(result)
        .iter()
        .filter_map(|item| {
            Some(CategoryPlayer {
                domain: item.get("domain").and_then(Value::as_str)?.to_owned(),
                traffic: item.get("organic_etv").and_then(number).unwrap_or_default(),
                keywords: item
                    .get("organic_count")
                    .and_then(count)
                    .unwrap_or_default(),
                traffic_change: item
                    .pointer("/metrics_difference/organic/etv")
                    .and_then(number),
            })
        })
        .collect();
    players.sort_by(|left, right| right.traffic.total_cmp(&left.traffic));
    players
}

/// Estimated cost of the domains of a category: ten times the default Labs price, a row per
/// domain.
pub fn players_estimate() -> f64 {
    prices::labs_history(PLAYER_ROWS)
}

/// Estimated cost of a domain's categories.
pub fn categories_estimate() -> f64 {
    site::estimate(CATEGORY_ROWS)
}

/// A category's name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CategoryName {
    /// Its code.
    pub code: u32,
    /// Its English name.
    pub name: String,
    /// The code of its parent.
    pub parent: Option<u32>,
}

/// Reads the category list.
pub fn category_names_from_result(result: &[Value]) -> Vec<CategoryName> {
    result
        .iter()
        .filter_map(|item| {
            Some(CategoryName {
                code: small(item, "category_code")?,
                name: item
                    .get("category_name")
                    .and_then(Value::as_str)?
                    .to_owned(),
                parent: small(item, "category_code_parent"),
            })
        })
        .collect()
}
