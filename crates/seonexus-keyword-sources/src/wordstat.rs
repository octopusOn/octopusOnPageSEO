//! Yandex Wordstat through the Yandex Search API v2, with the Yandex Cloud `api_key` and
//! `folder_id` secrets.
//!
//! The statistics are limited to [`Wordstat::HOURLY_QUOTA`] requests per hour by default
//! (support can raise it); callers keep to it.

use seonexus_core::http::HttpRequest;
use seonexus_serp::locale::yandex_country_region;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::{
    Fetched, FoundKeyword, Market, Monthly, SourceContext, SourceError, count, parse, redact,
};

/// A Yandex region with the regions inside it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WordstatRegion {
    /// Region id.
    pub id: u32,
    /// Name.
    pub label: String,
    /// Regions inside it.
    pub children: Vec<WordstatRegion>,
}

impl WordstatRegion {
    /// This region and every region inside it, depth first.
    pub fn walk(&self) -> Vec<&Self> {
        let mut regions = vec![self];
        for child in &self.children {
            regions.extend(child.walk());
        }
        regions
    }
}

/// Yandex Wordstat.
#[derive(Debug, Clone, Copy, Default)]
pub struct Wordstat;

impl Wordstat {
    /// Source id.
    pub const ID: &'static str = "wordstat";
    /// Provider whose secrets it uses.
    pub const SECRETS: &'static str = "yandex-cloud";
    /// Address the secrets are bound to.
    pub const ORIGIN: &'static str = "https://searchapi.api.cloud.yandex.net";
    /// Most phrases one top-requests call returns.
    pub const MAX_PHRASES: u32 = 2_000;
    /// Default requests per hour.
    pub const HOURLY_QUOTA: u32 = 100;
    /// Estimated price of one request, US dollars.
    pub const COST_PER_REQUEST_USD: f64 = 0.000_16;

    fn request(
        context: &SourceContext<'_>,
        method: &str,
        mut body: Value,
    ) -> Result<Value, SourceError> {
        let key = context.secret(Self::SECRETS, "api_key")?;
        let folder = context.secret(Self::SECRETS, "folder_id")?;
        body["folderId"] = json!(folder);
        let request = HttpRequest::post(
            format!("{}/v2/wordstat/{method}", Self::ORIGIN),
            body.to_string().into_bytes(),
        )
        .header("Authorization", format!("Api-Key {key}"))
        .header("Content-Type", "application/json");
        let response = context.send(&request)?;
        let hide = |text: &str| redact(text, std::slice::from_ref(&key));
        if !response.is_success() {
            // Yandex Cloud answers an exhausted quota with 429 RESOURCE_EXHAUSTED.
            return Err(SourceError::from_status(&response, &hide));
        }
        parse(&response, "Wordstat")
    }

    fn regions(market: &Market) -> Option<Value> {
        market
            .yandex_region
            .or_else(|| yandex_country_region(&market.country))
            .map(|region| json!([region.to_string()]))
    }

    /// The most searched phrases containing `phrase` (up to [`Self::MAX_PHRASES`]) and its
    /// associations (similar searches), with monthly counts.
    ///
    /// # Errors
    /// [`SourceError`] when Wordstat cannot be asked.
    pub fn top_requests(
        context: &SourceContext<'_>,
        phrase: &str,
        market: &Market,
        phrases: u32,
    ) -> Result<Fetched, SourceError> {
        let mut body = json!({
            "phrase": phrase.trim(),
            "numPhrases": phrases.clamp(1, Self::MAX_PHRASES).to_string(),
            "devices": ["DEVICE_ALL"],
        });
        if let Some(regions) = Self::regions(market) {
            body["regions"] = regions;
        }
        let value = Self::request(context, "topRequests", body)?;
        let mut keywords: Vec<FoundKeyword> = Vec::new();
        for list in ["results", "associations"] {
            for item in value
                .get(list)
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                let Some(text) = item.get("phrase").and_then(Value::as_str) else {
                    continue;
                };
                keywords.push(FoundKeyword {
                    volume: item.get("count").and_then(count),
                    ..FoundKeyword::named(text)
                });
            }
        }
        Ok(Fetched {
            keywords,
            requests: 1,
            cost_usd: Self::COST_PER_REQUEST_USD,
        })
    }

    /// Monthly searches of `phrase` between two months (`YYYY-MM`, inclusive).
    ///
    /// # Errors
    /// [`SourceError`] when Wordstat cannot be asked.
    pub fn dynamics(
        context: &SourceContext<'_>,
        phrase: &str,
        market: &Market,
        from: &str,
        to: &str,
    ) -> Result<(Vec<Monthly>, Fetched), SourceError> {
        let mut body = json!({
            "phrase": phrase.trim(),
            "period": "PERIOD_MONTHLY",
            "fromDate": format!("{from}-01T00:00:00Z"),
            "toDate": format!("{}T23:59:59Z", last_day(to)),
            "devices": ["DEVICE_ALL"],
        });
        if let Some(regions) = Self::regions(market) {
            body["regions"] = regions;
        }
        let value = Self::request(context, "dynamics", body)?;
        let monthly = value
            .get("results")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|item| {
                let date = item.get("date")?.as_str()?;
                Some(Monthly {
                    month: date.get(..7)?.to_owned(),
                    volume: count(item.get("count")?)?,
                })
            })
            .collect();
        Ok((
            monthly,
            Fetched {
                keywords: Vec::new(),
                requests: 1,
                cost_usd: Self::COST_PER_REQUEST_USD,
            },
        ))
    }

    /// The regions Wordstat knows (free).
    ///
    /// # Errors
    /// [`SourceError`] when Wordstat cannot be asked.
    pub fn regions_tree(context: &SourceContext<'_>) -> Result<Vec<WordstatRegion>, SourceError> {
        let value = Self::request(context, "getRegionsTree", json!({}))?;
        Ok(value
            .get("regions")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(region)
            .collect())
    }
}

fn region(value: &Value) -> Option<WordstatRegion> {
    Some(WordstatRegion {
        id: u32::try_from(count(value.get("id")?)?).ok()?,
        label: value.get("label")?.as_str()?.to_owned(),
        children: value
            .get("children")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(region)
            .collect(),
    })
}

/// `YYYY-MM-DD` of the last day of a month (`YYYY-MM`).
fn last_day(month: &str) -> String {
    let (year, number) = month.split_once('-').unwrap_or((month, "12"));
    let year: u32 = year.parse().unwrap_or(2000);
    let number: u32 = number.parse().unwrap_or(12);
    let leap = (year.is_multiple_of(4) && !year.is_multiple_of(100)) || year.is_multiple_of(400);
    let day = match number {
        2 if leap => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    };
    format!("{year:04}-{number:02}-{day:02}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_last_day_of_a_month() {
        assert_eq!(last_day("2024-02"), "2024-02-29");
        assert_eq!(last_day("2026-02"), "2026-02-28");
        assert_eq!(last_day("2026-09"), "2026-09-30");
        assert_eq!(last_day("2026-12"), "2026-12-31");
    }
}
