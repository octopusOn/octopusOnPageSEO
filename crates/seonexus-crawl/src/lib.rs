//! The crawl engine of the OctopusOn SEO Crawler.
//!
//! A [`Crawler`] walks a site breadth-first from its start URLs (spider mode) or fetches a
//! fixed list of URLs (list mode). Worker threads fetch through a [`Fetcher`] port, so the
//! engine never touches the network itself; the HTTP adapter lives in `seonexus-net` and
//! tests use an in-memory fetcher. Every fetched URL becomes a [`UrlRecord`] with the
//! [`LinkRecord`]s it contains, handed to a [`CrawlSink`] (the crawl store) as soon as it is
//! known, so memory stays flat on large sites.
//!
//! The engine follows what search engines follow: hyperlinks, redirects (one hop per fetch,
//! so chains are visible), canonicals, hreflang, pagination and meta refreshes. It obeys
//! robots.txt like Google (RFC 9309), reads XML sitemaps from robots.txt and
//! `/sitemap.xml`, and crawls URLs that only sitemaps list after the link graph is exhausted,
//! which leaves their crawl depth empty and marks them as orphans.

mod activity;
mod config;
mod control;
mod engine;
mod exclusions;
mod fetch;
mod link_header;
mod rate;
mod record;
mod render;
mod robots;
mod scope;
mod sitemap;

pub use activity::{ActivityKind, ActivityState, CrawlActivity, RECENT_URL_LIMIT, UrlActivity};
pub use config::{
    AgentPreset, ConfigError, CrawlConfig, CrawlMode, CrawlPlan, Follow, Limits, RenderMode,
    Rendering, RobotsPolicy, Speed, Storage, UrlRules, Viewport,
};
pub use control::CrawlControl;
pub use engine::{CrawlProgress, CrawlSink, CrawlSummary, Crawler};
pub use exclusions::DEFAULT_URL_EXCLUSIONS;
pub use fetch::{FetchError, FetchRequest, FetchResponse, Fetcher, follows_location};
pub use record::{
    ContentKind, CrawlLinkKind, Indexability, LinkRecord, RenderRecord, RobotsRecord, RobotsStatus,
    SiteFileRecord, SitemapEntry, SitemapKind, SitemapRecord, UrlId, UrlRecord,
};
pub use render::{BrowserRequest, RenderRequest, RenderedPage, Renderer, RequestPolicy};
pub use sitemap::{ParsedSitemap, parse_sitemap};
