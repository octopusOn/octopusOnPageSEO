//! Where snapshots come from.
//!
//! A [`SerpProvider`] turns a [`SerpQuery`] into a [`SerpFetch`]: the snapshot plus the raw
//! responses (stored so pages can be re-read after a selector fix), the requests made and
//! what they cost. Providers talk to the network only through the
//! [`octopuson_core::http::HttpTransport`] port.
//!
//! Built-in providers:
//! - [`SpecProvider`]: any [`crate::spec::ProviderSpec`]; Serper, `SerpApi` and `DataForSEO` are
//!   embedded specs ([`builtin_specs`]) and user-defined providers run the same way;
//! - [`YandexCloud`]: the Yandex Search API (base64 XML inside JSON);
//! - [`BrowserProvider`]: free attended searches in a browser window ([`BrowserPort`]).
//!
//! [`VolumeProvider`]s report monthly searches: [`Wordstat`] for Yandex and
//! [`DataForSeoVolume`] for Google Ads data.
//!
//! [`search_chain`] tries providers in order and falls back on errors that another provider
//! may not have; a challenge (CAPTCHA) never falls back silently.

mod browser;
mod chain;
mod spec_provider;
mod volume;
mod yandex_cloud;

use std::fmt;

use octopuson_core::http::{HttpResponse, HttpTransport};
use serde::{Deserialize, Serialize};

pub use browser::{
    BROWSER_ID, BrowserPacing, BrowserPort, BrowserProvider, BrowserView, Clock, SystemClock,
};
pub use chain::{Attempt, AttemptOutcome, ChainFailure, search_chain};
pub use spec_provider::{SpecProvider, builtin_specs};
pub use volume::{DataForSeoVolume, KeywordVolume, VolumeProvider, VolumeQuery, Wordstat};
pub use yandex_cloud::YandexCloud;

use crate::challenge::{Challenge, ChallengeKind};
use crate::{Device, Engine, SerpError, SerpQuery, SerpSnapshot};

/// What a provider can do.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Capabilities {
    /// Engines it searches.
    pub engines: Vec<Engine>,
    /// Devices it returns results for.
    pub devices: Vec<Device>,
    /// Deepest position it can return.
    pub max_depth: u16,
    /// Whether it needs a person at hand (to solve challenges).
    pub interactive: bool,
}

impl Capabilities {
    /// Whether a query is within the capabilities.
    pub fn supports(&self, query: &SerpQuery) -> bool {
        self.engines.contains(&query.engine) && self.devices.contains(&query.device)
    }
}

/// Expected price of a query.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct CostEstimate {
    /// Requests the query takes.
    pub requests: u32,
    /// US dollars.
    pub usd: f64,
}

/// One raw response, kept with the snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawPage {
    /// Result page, from 1.
    pub page: u16,
    /// Request URL with secrets redacted.
    pub url: String,
    /// HTTP status (0 for pages read from a browser).
    pub status: u16,
    /// Media type of the body.
    pub content_type: Option<String>,
    /// Body.
    pub body: Vec<u8>,
}

/// A snapshot and how it was obtained.
#[derive(Debug, Clone, PartialEq)]
pub struct SerpFetch {
    /// The results.
    pub snapshot: SerpSnapshot,
    /// Raw responses, page by page.
    pub pages: Vec<RawPage>,
    /// Requests made.
    pub requests: u32,
    /// US dollars the provider reported, if it did.
    pub reported_cost_usd: Option<f64>,
    /// US dollars estimated from the provider's price list.
    pub estimated_cost_usd: f64,
}

impl SerpFetch {
    /// Reported cost, else the estimate.
    pub fn cost_usd(&self) -> f64 {
        self.reported_cost_usd.unwrap_or(self.estimated_cost_usd)
    }
}

/// Why a provider returned no snapshot.
#[derive(Debug, Clone, PartialEq)]
pub enum ProviderError {
    /// Credentials are missing or wrong.
    Auth(String),
    /// The balance or quota is exhausted.
    Quota {
        /// Provider message.
        message: String,
        /// Seconds until the quota resets, if known.
        retry_after: Option<u64>,
    },
    /// Too many requests.
    RateLimited {
        /// Provider message.
        message: String,
        /// Seconds to wait, if known.
        retry_after: Option<u64>,
    },
    /// The engine showed a consent page or CAPTCHA; a person must act.
    Blocked(Challenge),
    /// No response.
    Network(String),
    /// An unexpected HTTP status.
    Http {
        /// Status code.
        status: u16,
        /// Start of the body, secrets redacted.
        excerpt: String,
    },
    /// The provider answered with an error of its own.
    Rejected(String),
    /// The response could not be read.
    Parse {
        /// What went wrong.
        message: String,
        /// Share of matched results that were readable, when results matched at all.
        coverage: Option<f64>,
    },
    /// A page came back but no result could be read from it (outdated selectors); the page
    /// is kept so it can be read again after a fix.
    Unreadable {
        /// What went wrong.
        message: String,
        /// Share of matched results that were readable, when results matched at all.
        coverage: Option<f64>,
        /// The page.
        page: Box<RawPage>,
    },
    /// The provider is not set up (missing secret, invalid spec).
    Config(String),
    /// The query is outside the provider's capabilities.
    Unsupported(String),
    /// The run was cancelled.
    Cancelled,
}

impl ProviderError {
    /// Whether the next provider of a chain should be tried.
    pub fn falls_back(&self) -> bool {
        !matches!(self, Self::Blocked(_) | Self::Cancelled)
    }

    /// Stable short code for storage and the UI (`auth`, `quota`, ...).
    pub const fn code(&self) -> &'static str {
        match self {
            Self::Auth(_) => "auth",
            Self::Quota { .. } => "quota",
            Self::RateLimited { .. } => "rate-limited",
            Self::Blocked(_) => "blocked",
            Self::Network(_) => "network",
            Self::Http { .. } => "http",
            Self::Rejected(_) => "rejected",
            Self::Parse { .. } | Self::Unreadable { .. } => "parse",
            Self::Config(_) => "config",
            Self::Unsupported(_) => "unsupported",
            Self::Cancelled => "cancelled",
        }
    }

    /// Classifies a non-2xx response.
    pub fn from_status(response: &HttpResponse, redact: &dyn Fn(&str) -> String) -> Self {
        let excerpt: String = redact(&response.text()).chars().take(300).collect();
        match response.status {
            401 | 403 => Self::Auth(excerpt),
            402 => Self::Quota {
                message: excerpt,
                retry_after: response.retry_after(),
            },
            429 => Self::RateLimited {
                message: excerpt,
                retry_after: response.retry_after(),
            },
            status => Self::Http { status, excerpt },
        }
    }
}

impl fmt::Display for ProviderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Auth(message) => write!(f, "authentication failed: {message}"),
            Self::Quota { message, .. } => write!(f, "quota or balance exhausted: {message}"),
            Self::RateLimited { message, .. } => write!(f, "too many requests: {message}"),
            Self::Blocked(challenge) => write!(
                f,
                "the search engine asks for a check ({:?}): {}",
                challenge.kind, challenge.reason
            ),
            Self::Network(message) => write!(f, "network error: {message}"),
            Self::Http { status, excerpt } => write!(f, "HTTP {status}: {excerpt}"),
            Self::Rejected(message) => write!(f, "the provider reported an error: {message}"),
            Self::Parse { message, .. } | Self::Unreadable { message, .. } => {
                write!(f, "cannot read the results: {message}")
            }
            Self::Config(message) => write!(f, "provider not set up: {message}"),
            Self::Unsupported(message) => write!(f, "not supported: {message}"),
            Self::Cancelled => f.write_str("cancelled"),
        }
    }
}

impl std::error::Error for ProviderError {}

impl From<SerpError> for ProviderError {
    fn from(error: SerpError) -> Self {
        match error {
            SerpError::InvalidQuery(message) => Self::Unsupported(message),
            SerpError::InvalidSpec(message) | SerpError::Template(message) => Self::Config(message),
            SerpError::Parse(message) => Self::Parse {
                message,
                coverage: None,
            },
        }
    }
}

/// Progress a provider reports while it searches.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum SearchEvent {
    /// Waiting before the next request.
    Waiting {
        /// Provider id.
        provider: String,
        /// Seconds left.
        seconds: u64,
    },
    /// A result page is being requested.
    Page {
        /// Provider id.
        provider: String,
        /// Page, from 1.
        page: u16,
        /// Pages planned.
        pages: u16,
    },
    /// The engine shows a check page; a person must act in the browser window.
    Challenge {
        /// Provider id.
        provider: String,
        /// Kind of check.
        kind: ChallengeKind,
        /// The search address.
        url: String,
    },
    /// The check is gone; the search goes on.
    Resumed {
        /// Provider id.
        provider: String,
    },
}

/// What a search runs with.
pub struct SearchContext<'a> {
    /// The network.
    pub transport: &'a dyn HttpTransport,
    /// Secret value by provider id and secret name.
    pub secrets: &'a dyn Fn(&str, &str) -> Option<String>,
    /// RFC 3339 time recorded as the snapshot's fetch time.
    pub fetched_at: String,
    /// Whether the run was cancelled; checked between requests.
    pub cancelled: &'a dyn Fn() -> bool,
    /// Receives progress.
    pub events: &'a dyn Fn(&SearchEvent),
}

/// A source of result pages.
pub trait SerpProvider: Send + Sync {
    /// Identifier (`serper`, `yandex-cloud`, a custom provider's id).
    fn id(&self) -> &str;

    /// Display name.
    fn name(&self) -> &str;

    /// What it can do.
    fn capabilities(&self) -> Capabilities;

    /// Expected price of a query.
    fn estimate(&self, query: &SerpQuery) -> CostEstimate;

    /// Runs a query.
    ///
    /// # Errors
    /// [`ProviderError`] describing why there is no snapshot.
    fn search(
        &self,
        query: &SerpQuery,
        context: &SearchContext<'_>,
    ) -> Result<SerpFetch, ProviderError>;
}
