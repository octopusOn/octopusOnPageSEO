//! Google Ads keyword planning (REST v26), with an OAuth access token and the user's
//! customer id: keyword ideas and historical metrics.
//!
//! Keyword planning needs at least Basic access for the Google Cloud project; its terms
//! describe tools for Ads campaigns. Low-spending accounts may get volume ranges only.

use seonexus_core::http::HttpRequest;
use serde_json::{Value, json};

use crate::countries::{geo_target, language_constant};
use crate::{
    Fetched, FoundKeyword, Market, Monthly, SourceContext, SourceError, count, parse, redact,
};

/// Google Ads.
#[derive(Debug, Clone, Copy, Default)]
pub struct GoogleAds;

/// The account an Ads call runs under.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdsAccount<'a> {
    /// OAuth access token.
    pub token: &'a str,
    /// Customer id, digits only.
    pub customer_id: &'a str,
    /// Manager account id, when the customer is reached through one.
    pub login_customer_id: Option<&'a str>,
    /// Developer token, for projects that still have one.
    pub developer_token: Option<&'a str>,
}

const MONTHS: [&str; 12] = [
    "JANUARY",
    "FEBRUARY",
    "MARCH",
    "APRIL",
    "MAY",
    "JUNE",
    "JULY",
    "AUGUST",
    "SEPTEMBER",
    "OCTOBER",
    "NOVEMBER",
    "DECEMBER",
];

impl GoogleAds {
    /// Source id.
    pub const ID: &'static str = "google-ads";
    /// API base.
    pub const BASE: &'static str = "https://googleads.googleapis.com/v26";
    /// Seeds per ideas request.
    pub const SEEDS: usize = 20;
    /// Keywords per historical metrics request.
    pub const BATCH: usize = 10_000;

    /// Digits of a customer id (`123-456-7890` → `1234567890`).
    pub fn customer(id: &str) -> Option<String> {
        let digits: String = id.chars().filter(char::is_ascii_digit).collect();
        (digits.len() == 10).then_some(digits)
    }

    fn post(
        context: &SourceContext<'_>,
        account: &AdsAccount<'_>,
        method: &str,
        body: &Value,
    ) -> Result<Value, SourceError> {
        let customer = Self::customer(account.customer_id).ok_or_else(|| {
            SourceError::Config("the Google Ads customer id must have ten digits".to_owned())
        })?;
        let mut request = HttpRequest::post(
            format!("{}/customers/{customer}:{method}", Self::BASE),
            body.to_string().into_bytes(),
        )
        .header("Authorization", format!("Bearer {}", account.token))
        .header("Content-Type", "application/json");
        if let Some(manager) = account.login_customer_id.and_then(Self::customer) {
            request = request.header("login-customer-id", manager);
        }
        if let Some(token) = account.developer_token.filter(|token| !token.is_empty()) {
            request = request.header("developer-token", token);
        }
        let response = context.send(&request)?;
        let mut secrets = vec![account.token.to_owned()];
        secrets.extend(account.developer_token.map(str::to_owned));
        let hide = |text: &str| redact(text, &secrets);
        if !response.is_success() {
            // A project without Basic access is refused with 403 PERMISSION_DENIED.
            return Err(SourceError::from_status(&response, &hide));
        }
        parse(&response, "Google Ads")
    }

    fn market(market: &Market) -> Result<(String, String), SourceError> {
        let geo = geo_target(&market.country).ok_or_else(|| {
            SourceError::Config(format!("Google Ads: unknown country `{}`", market.country))
        })?;
        let language = language_constant(&market.language).ok_or_else(|| {
            SourceError::Config(format!(
                "Google Ads: unknown language `{}`",
                market.language
            ))
        })?;
        Ok((
            format!("geoTargetConstants/{geo}"),
            format!("languageConstants/{language}"),
        ))
    }

    /// Keyword ideas for up to [`Self::SEEDS`] seeds (one page of up to 1,000).
    ///
    /// # Errors
    /// [`SourceError`] when Google Ads cannot be asked.
    pub fn keyword_ideas(
        context: &SourceContext<'_>,
        account: &AdsAccount<'_>,
        seeds: &[String],
        market: &Market,
    ) -> Result<Fetched, SourceError> {
        let (geo, language) = Self::market(market)?;
        let seeds = &seeds[..seeds.len().min(Self::SEEDS)];
        let value = Self::post(
            context,
            account,
            "generateKeywordIdeas",
            &json!({
                "language": language,
                "geoTargetConstants": [geo],
                "keywordPlanNetwork": "GOOGLE_SEARCH",
                "includeAdultKeywords": false,
                "keywordSeed": {"keywords": seeds},
                "pageSize": 1000,
                "historicalMetricsOptions": {"includeAverageCpc": true},
            }),
        )?;
        let keywords = value
            .get("results")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|result| idea(result.get("text")?, result.get("keywordIdeaMetrics")))
            .collect();
        Ok(Fetched {
            keywords,
            requests: 1,
            cost_usd: 0.0,
        })
    }

    /// Volumes, competition, bids and monthly searches of up to [`Self::BATCH`] keywords.
    ///
    /// # Errors
    /// [`SourceError`] when Google Ads cannot be asked.
    pub fn historical_metrics(
        context: &SourceContext<'_>,
        account: &AdsAccount<'_>,
        keywords: &[String],
        market: &Market,
    ) -> Result<Fetched, SourceError> {
        let (geo, language) = Self::market(market)?;
        let keywords = &keywords[..keywords.len().min(Self::BATCH)];
        let value = Self::post(
            context,
            account,
            "generateKeywordHistoricalMetrics",
            &json!({
                "keywords": keywords,
                "language": language,
                "geoTargetConstants": [geo],
                "keywordPlanNetwork": "GOOGLE_SEARCH",
                "historicalMetricsOptions": {"includeAverageCpc": true},
            }),
        )?;
        let keywords = value
            .get("results")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|result| idea(result.get("text")?, result.get("keywordMetrics")))
            .collect();
        Ok(Fetched {
            keywords,
            requests: 1,
            cost_usd: 0.0,
        })
    }
}

/// A keyword with Google Ads metrics (int64 values arrive as strings).
fn idea(text: &Value, metrics: Option<&Value>) -> Option<FoundKeyword> {
    let mut keyword = FoundKeyword::named(text.as_str()?);
    let Some(metrics) = metrics else {
        return Some(keyword);
    };
    keyword.volume = metrics.get("avgMonthlySearches").and_then(count);
    keyword.competition = metrics
        .get("competitionIndex")
        .and_then(count)
        .and_then(|index| u32::try_from(index).ok())
        .map(|index| f64::from(index) / 100.0);
    let micros = |name: &str| {
        metrics
            .get(name)
            .and_then(count)
            .and_then(|micros| u32::try_from(micros / 10_000).ok())
            .map(|cents| f64::from(cents) / 100.0)
    };
    keyword.cpc_usd = micros("averageCpcMicros").or_else(|| micros("highTopOfPageBidMicros"));
    let mut monthly: Vec<Monthly> = metrics
        .get("monthlySearchVolumes")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|month| {
            let name = month.get("month")?.as_str()?;
            let number = MONTHS.iter().position(|candidate| *candidate == name)? + 1;
            Some(Monthly {
                month: format!("{:04}-{number:02}", count(month.get("year")?)?),
                volume: count(month.get("monthlySearches")?)?,
            })
        })
        .collect();
    monthly.sort_by(|a, b| a.month.cmp(&b.month));
    keyword.monthly = monthly;
    Some(keyword)
}
