//! Search engine result pages.
//!
//! A [`SerpQuery`] names a keyword, an engine (Google, Bing or Yandex), a country, a language,
//! an optional location, a device and how deep to look. Whatever fetches the results (the
//! user's browser, a SERP API or a provider the user defined) produces a [`SerpSnapshot`]: the
//! organic results by position and the other blocks of the page (ads, featured snippets,
//! "People also ask", local packs, AI overviews, related searches, ...).
//!
//! Results are read by declarative [`mapping`]s: `JSONPath` for JSON APIs, CSS selectors for
//! HTML pages and regular expressions for anything else. The built-in Google, Bing and Yandex
//! HTML parsers are such mappings ([`profile`]), embedded and versioned, so a user can patch a
//! selector when an engine changes its markup and re-read stored pages without a new request.
//! The same engine runs the providers users add themselves ([`spec`]): a request template
//! and a response mapping in JSON.
//!
//! [`provider`] runs queries on SERP APIs and user-defined providers, with fallback chains;
//! [`ranking`] finds where a domain, host or URL prefix ranks in a snapshot.

pub mod challenge;
pub mod locale;
pub mod mapping;
mod model;
pub mod profile;
pub mod provider;
pub mod ranking;
pub mod snapshot;
pub mod spec;
pub mod template;
mod text;
pub mod yandex_xml;

pub use model::{
    Device, Engine, Feature, FeatureItem, FeatureKind, Location, MAX_DEPTH, Organic,
    RESULTS_PER_PAGE, SerpQuery, SerpSnapshot, Sitelink, keyword_key,
};
pub use ranking::{MatchMode, Rank, Target, find_rank, registrable_domain};
pub use snapshot::SnapshotBuilder;

/// Errors of this crate.
#[derive(Debug, thiserror::Error)]
pub enum SerpError {
    /// The query cannot be sent.
    #[error("invalid query: {0}")]
    InvalidQuery(String),
    /// A mapping, profile or provider spec is invalid.
    #[error("invalid specification: {0}")]
    InvalidSpec(String),
    /// A request template cannot be filled in.
    #[error("cannot build the request: {0}")]
    Template(String),
    /// A response could not be read.
    #[error("cannot read the response: {0}")]
    Parse(String),
}
