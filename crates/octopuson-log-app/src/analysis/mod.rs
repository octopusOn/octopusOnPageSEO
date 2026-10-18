//! SEO analyses on top of queries: URL-list comparison, period comparison, robots.txt
//! compliance and the crawl budget overview.

mod collect;
mod overview;
mod periods;
mod robots;
mod url_compare;
mod url_lists;

pub use octopuson_analytics::Share;
pub use octopuson_analytics::anomalies::{Anomaly, AnomalyKind};
pub use octopuson_analytics::overview::{BotHits, CrawlOverview, SearchCrawl};
pub use octopuson_analytics::periods::PeriodSummary;
pub use octopuson_analytics::urls::ComparisonSummary;
pub use octopuson_url::{NormalizeOptions, QueryMode};
pub use overview::{OverviewRequest, SiteCrawlOverview, crawl_overview};
pub use periods::{PeriodReport, PeriodRequest, compare_periods};
pub use robots::{RobotsBotSummary, RobotsReport, RobotsRequest, robots_compliance};
pub use url_compare::{SourceCount, UrlListReport, UrlListRequest, compare_url_list};
pub use url_lists::UrlSource;

use crate::query::QueryError;

/// Filter applied to logged URLs when none is given: verified search-engine crawlers.
pub const DEFAULT_CRAWLER_FILTER: &str = "bot_category:search verification:verified";

/// Errors of the analyses.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum AnalysisError {
    /// A query failed (invalid filter, unknown site, storage error).
    #[error(transparent)]
    Query(#[from] QueryError),
    /// A URL list could not be read or downloaded.
    #[error("cannot read URL list `{list}`: {message}")]
    Read {
        /// File or URL.
        list: String,
        /// What went wrong.
        message: String,
    },
    /// A URL list contained no usable URL.
    #[error("URL list `{0}` contains no URLs")]
    EmptyList(String),
    /// Writing the result failed.
    #[error("writing results failed")]
    Output(#[source] std::io::Error),
}

#[cfg(test)]
mod tests;
