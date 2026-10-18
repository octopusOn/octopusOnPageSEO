//! Keyword sources of the OctopusOn SEO Keyword Research.
//!
//! Each call sends one request through the [`HttpTransport`] port and returns the keywords it
//! found or measured with the requests and cost it took, so the caller can keep budgets,
//! quotas and cancellation between requests. Keys come from a lookup and never appear in
//! errors.
//!
//! - [`wordstat`]: Yandex Wordstat through the Yandex Search API (top requests with their
//!   associations, monthly dynamics, the region tree).
//! - [`dataforseo`]: Google Ads volumes and ideas, DataForSEO Labs ideas, related keywords,
//!   suggestions, a competitor's ranking keywords, difficulty, intent, and Google
//!   autocomplete.
//! - [`serpapi`]: Google autocomplete through SerpApi.
//! - [`suggest`]: the free Google and Yandex suggestion endpoints, off unless the user
//!   switches them on.
//! - [`search_console`] and [`google_ads`]: the user's own Google data, with an OAuth access
//!   token.

pub mod countries;
pub mod dataforseo;
pub mod google_ads;
pub mod search_console;
pub mod serpapi;
pub mod suggest;
pub mod wordstat;

use octopuson_core::http::{HttpRequest, HttpResponse, HttpTransport};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// What a source call needs: the transport and the secret lookup (`provider`, `name`).
#[derive(Clone, Copy)]
pub struct SourceContext<'a> {
    /// Sends requests.
    pub transport: &'a dyn HttpTransport,
    /// Secret lookup.
    pub secrets: &'a (dyn Fn(&str, &str) -> Option<String> + Sync),
}

impl std::fmt::Debug for SourceContext<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SourceContext").finish_non_exhaustive()
    }
}

impl SourceContext<'_> {
    fn secret(&self, provider: &str, name: &str) -> Result<String, SourceError> {
        (self.secrets)(provider, name)
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty())
            .ok_or_else(|| SourceError::Config(format!("the {provider} key `{name}` is not set")))
    }

    fn send(&self, request: &HttpRequest) -> Result<HttpResponse, SourceError> {
        self.transport
            .send(request)
            .map_err(|error| SourceError::Network(error.to_string()))
    }
}

/// Why a source could not be asked.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum SourceError {
    /// Not set up (missing key).
    #[error("{0}")]
    Config(String),
    /// The key was refused.
    #[error("the key was refused: {0}")]
    Auth(String),
    /// Out of balance or quota.
    #[error("out of balance or quota: {message}")]
    Quota {
        /// The source's message.
        message: String,
        /// Seconds until it may be asked again.
        retry_after: Option<u64>,
    },
    /// Too many requests.
    #[error("too many requests")]
    RateLimited {
        /// Seconds until it may be asked again.
        retry_after: Option<u64>,
    },
    /// The request was refused.
    #[error("refused: {0}")]
    Rejected(String),
    /// No response.
    #[error("network: {0}")]
    Network(String),
    /// The response could not be read.
    #[error("unreadable response: {0}")]
    Parse(String),
}

impl SourceError {
    /// Whether asking again later can help (quota, rate limit, network).
    pub const fn is_temporary(&self) -> bool {
        matches!(
            self,
            Self::Quota { .. } | Self::RateLimited { .. } | Self::Network(_)
        )
    }

    /// The error of a non-2xx response, secrets hidden.
    fn from_status(response: &HttpResponse, hide: &dyn Fn(&str) -> String) -> Self {
        let body: String = hide(&response.text()).chars().take(300).collect();
        match response.status {
            401 | 403 => Self::Auth(body),
            402 => Self::Quota {
                message: body,
                retry_after: response.retry_after(),
            },
            429 => Self::RateLimited {
                retry_after: response.retry_after(),
            },
            _ => Self::Rejected(format!("HTTP {}: {body}", response.status)),
        }
    }
}

/// Searches in one month.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Monthly {
    /// `YYYY-MM`.
    pub month: String,
    /// Searches.
    pub volume: u64,
}

/// A keyword a source found or measured.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct FoundKeyword {
    /// The keyword as the source wrote it.
    pub keyword: String,
    /// Average monthly searches.
    pub volume: Option<u64>,
    /// Lower end, where only a range is known.
    pub volume_low: Option<u64>,
    /// Upper end, where only a range is known.
    pub volume_high: Option<u64>,
    /// Cost per click, US dollars.
    pub cpc_usd: Option<f64>,
    /// Advertiser competition, 0–1.
    pub competition: Option<f64>,
    /// Keyword difficulty, 0–100.
    pub difficulty: Option<f64>,
    /// Searches per month, oldest first.
    pub monthly: Vec<Monthly>,
    /// Main intent (`informational`, `navigational`, `commercial`, `transactional`).
    pub intent: Option<String>,
    /// Second intent.
    pub secondary_intent: Option<String>,
    /// The page that ranks for it (a competitor's keywords).
    pub url: Option<String>,
    /// The rank of that page.
    pub position: Option<u32>,
}

impl FoundKeyword {
    /// A keyword without numbers.
    pub fn named(keyword: impl Into<String>) -> Self {
        Self {
            keyword: keyword.into(),
            ..Self::default()
        }
    }

    /// Whether it carries any number.
    pub fn has_numbers(&self) -> bool {
        self.volume.is_some()
            || self.volume_low.is_some()
            || self.cpc_usd.is_some()
            || self.competition.is_some()
            || self.difficulty.is_some()
            || !self.monthly.is_empty()
    }
}

/// What one request returned.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Fetched {
    /// Keywords, in the source's order.
    pub keywords: Vec<FoundKeyword>,
    /// Requests sent.
    pub requests: u32,
    /// US dollars, as reported by the source where it reports it, else estimated.
    pub cost_usd: f64,
}

/// Where keywords are looked for.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Market {
    /// Country code (ISO 3166-1 alpha-2, lower case).
    pub country: String,
    /// Language code (ISO 639-1).
    pub language: String,
    /// Yandex region narrowing Wordstat (the whole country when absent).
    pub yandex_region: Option<u32>,
}

impl Market {
    /// A market of a country and language.
    pub fn new(country: &str, language: &str) -> Self {
        Self {
            country: country.to_owned(),
            language: language.to_owned(),
            yandex_region: None,
        }
    }
}

/// `text` with every secret replaced by `***`.
fn redact(text: &str, secrets: &[String]) -> String {
    secrets
        .iter()
        .filter(|secret| secret.len() >= 4)
        .fold(text.to_owned(), |text, secret| {
            text.replace(secret.as_str(), "***")
        })
}

/// A count written as a number or a string.
fn count(value: &Value) -> Option<u64> {
    value
        .as_u64()
        .or_else(|| {
            value.as_f64().filter(|value| *value >= 0.0).map(|value| {
                // Counts are integers far below 2^53.
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                let value = value.round() as u64;
                value
            })
        })
        .or_else(|| value.as_str().and_then(|text| text.trim().parse().ok()))
}

/// A JSON body.
fn parse(response: &HttpResponse, source: &str) -> Result<Value, SourceError> {
    serde_json::from_slice(&response.body)
        .map_err(|error| SourceError::Parse(format!("{source}: {error}")))
}

#[cfg(test)]
mod tests;
