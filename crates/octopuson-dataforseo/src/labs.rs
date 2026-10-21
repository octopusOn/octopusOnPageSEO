//! DataForSEO Labs (Google): what a domain ranks for and how much traffic that brings.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{Answer, Client, DfsError, Market, count, items_of, number, prices, target_domain};

/// Rankings of a domain in one kind of result, organic or paid.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RankMetrics {
    /// Keywords the domain ranks for.
    pub keywords: u64,
    /// Estimated monthly traffic.
    pub traffic: f64,
    /// What that traffic would cost in ads, US dollars a month.
    pub traffic_value_usd: f64,
    /// Keywords in position 1.
    pub pos_1: u64,
    /// Positions 2–3.
    pub pos_2_3: u64,
    /// Positions 4–10.
    pub pos_4_10: u64,
    /// Positions 11–20.
    pub pos_11_20: u64,
    /// Positions 21–100.
    pub pos_21_100: u64,
    /// Keywords new since the last update.
    pub new: u64,
    /// Keywords that moved up.
    pub up: u64,
    /// Keywords that moved down.
    pub down: u64,
    /// Keywords lost.
    pub lost: u64,
}

impl RankMetrics {
    pub(crate) fn from_value(value: &Value) -> Self {
        let field = |name: &str| value.get(name).and_then(count).unwrap_or_default();
        let deep = [
            "pos_21_30",
            "pos_31_40",
            "pos_41_50",
            "pos_51_60",
            "pos_61_70",
            "pos_71_80",
            "pos_81_90",
            "pos_91_100",
        ]
        .iter()
        .map(|name| field(name))
        .sum();
        Self {
            keywords: field("count"),
            traffic: value.get("etv").and_then(number).unwrap_or_default(),
            traffic_value_usd: value
                .get("estimated_paid_traffic_cost")
                .and_then(number)
                .unwrap_or_default(),
            pos_1: field("pos_1"),
            pos_2_3: field("pos_2_3"),
            pos_4_10: field("pos_4_10"),
            pos_11_20: field("pos_11_20"),
            pos_21_100: deep,
            new: field("is_new"),
            up: field("is_up"),
            down: field("is_down"),
            lost: field("is_lost"),
        }
    }
}

/// A domain's rankings in a market.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct DomainOverview {
    /// The domain asked for.
    pub target: String,
    /// Organic results.
    pub organic: RankMetrics,
    /// Paid results.
    pub paid: RankMetrics,
}

impl DomainOverview {
    /// Reads a `domain_rank_overview` answer; an answer without items is a domain DataForSEO
    /// knows no rankings of, so every number is zero.
    pub fn from_answer(target: &str, answer: &Answer) -> Self {
        Self::from_result(target, &answer.result)
    }

    /// Reads the result list of a `domain_rank_overview` answer (as kept).
    pub fn from_result(target: &str, result: &[Value]) -> Self {
        let metrics = items_of(result)
            .first()
            .and_then(|item| item.get("metrics"))
            .cloned()
            .unwrap_or(Value::Null);
        let part = |name: &str| {
            metrics
                .get(name)
                .map(RankMetrics::from_value)
                .unwrap_or_default()
        };
        Self {
            target: target.to_owned(),
            organic: part("organic"),
            paid: part("paid"),
        }
    }
}

/// The task of a domain overview.
///
/// # Errors
/// [`DfsError::Config`] for an empty domain or an unknown country.
pub fn domain_overview_task(domain: &str, market: &Market) -> Result<Value, DfsError> {
    let target = target_domain(domain);
    if target.is_empty() {
        return Err(DfsError::Config("the domain is empty".to_owned()));
    }
    let mut task = market.fields()?;
    task["target"] = Value::String(target);
    Ok(task)
}

/// Path of the domain overview endpoint.
pub const DOMAIN_OVERVIEW: &str = "dataforseo_labs/google/domain_rank_overview/live";

/// Estimated cost of a domain overview, US dollars.
pub fn domain_overview_estimate() -> f64 {
    prices::labs(1)
}

/// A domain's organic and paid rankings in a market.
///
/// # Errors
/// [`DfsError`] when DataForSEO cannot be asked.
pub fn domain_overview(
    client: &Client<'_>,
    domain: &str,
    market: &Market,
) -> Result<(DomainOverview, Answer), DfsError> {
    let task = domain_overview_task(domain, market)?;
    let answer = client.post(DOMAIN_OVERVIEW, &task)?;
    let target = task["target"].as_str().unwrap_or_default().to_owned();
    Ok((DomainOverview::from_answer(&target, &answer), answer))
}
