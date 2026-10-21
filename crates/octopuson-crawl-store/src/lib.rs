//! Storage of OctopusOn SEO Crawler crawls.
//!
//! ```text
//! <root>/crawls/<crawl-id>/crawl.json     name, settings, status and counters
//! <root>/crawls/<crawl-id>/crawl.duckdb   URLs, links, images, headings, hreflang,
//!                                         structured data, robots.txt files and sitemaps
//! ```
//!
//! A [`CrawlWriter`] is the [`octopuson_crawl::CrawlSink`] of a running crawl: it buffers
//! records and appends them in batches. A [`CrawlReader`] opens a finished (or running) crawl
//! read-only and streams its [`CrawlTab`]s or any SQL query into a
//! [`octopuson_core::table::TableSink`], which is how the exporters write CSV, XLSX and JSON.
//! [`CrawlReader::geo_readiness`] reads how ready the site is for AI answers without adding
//! findings.

mod anchor_context;
mod audit;
mod comparison;
mod content_match;
mod duplicates;
mod error;
mod geo;
mod hreflang_context;
mod inspect;
mod library;
mod link_queries;
mod reader;
mod redirects;
mod remediation;
mod responses;
mod schema;
mod schema_checks;
mod score_exclusions;
mod seo_score;
mod sitemap_export;
mod storage;
mod writer;

pub use comparison::{
    ComparisonFilter, ComparisonRequest, ComparisonSummary, CrawlComparison, FieldChanges,
    FindingChange, FindingComparisonRequest, FindingComparisonSummary, FindingRuleCount,
};
pub use content_match::ContentMatch;
pub use duplicates::{DuplicateGroup, DuplicateKind, DuplicateScope};
pub use error::CrawlStoreError;
pub use geo::{GeoCrawler, GeoReadiness, GeoSignal, GeoStatus};
pub use inspect::{
    CrawlOverview, IssueCount, LinkBreakdown, LinkClass, LinkCounts, LinkDirection, LinkScope,
    LinkSummary, PageRequest, TableExportRequest, TablePage, UrlDetail,
};
pub use library::{CrawlLibrary, CrawlMeta, CrawlStatus};
pub use reader::{CrawlReader, CrawlTab};
pub use redirects::{MAX_REDIRECT_TRACE_ROWS, RedirectHop, RedirectOutcome, RedirectTrace};
pub use remediation::{RemediationDetail, RemediationRequest};
pub use responses::{ResponseCount, ResponseGroup};
pub use schema_checks::SchemaCoverage;
pub use seo_score::{ScoreCategory, ScoreDeduction, ScoreIgnoredRule, SeoScore};
pub use sitemap_export::{
    SITEMAP_FOOTER, SITEMAP_HEADER, SITEMAP_MAX_BYTES, SITEMAP_MAX_URLS, SitemapExclusion,
    SitemapLayout, SitemapOrigin, SitemapPlan, SitemapReason, SitemapSelection, sitemap_escape,
    sitemap_url_entry,
};
pub use storage::CrawlStorage;
pub use writer::CrawlWriter;

#[cfg(test)]
mod anchor_context_tests;
#[cfg(test)]
mod content_match_tests;
#[cfg(test)]
mod duplicates_tests;
#[cfg(test)]
mod export_tests;
#[cfg(test)]
mod geo_tests;
#[cfg(test)]
mod hreflang_context_tests;
#[cfg(test)]
mod link_evidence_tests;
#[cfg(test)]
mod response_tests;
#[cfg(test)]
mod seo_score_tests;
#[cfg(test)]
mod sitemap_identity_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod url_findings_tests;
