//! Technical SEO checks of the OctopusOn SEO Crawler.
//!
//! - [`catalog`] describes every check (`issues.json`): category, kind, priority and an
//!   English and Turkish title, description and fix. The desktop app reads the same file.
//! - [`limits`] holds the thresholds the checks use (title length, slow response, …).
//! - [`pixel_width`] estimates how wide a title or description renders in search results.
//! - [`is_valid_hreflang`], [`is_generic_anchor`] and [`ai_crawler_access`] are the checks
//!   that need more than SQL; the crawl store evaluates the rest over its tables.

mod ai;
mod anchors;
mod catalog;
mod lang;
pub mod limits;
mod pixels;

pub use ai::{AiAccess, AiCrawler, ai_crawler_access};
pub use anchors::{GENERIC_ANCHORS, is_generic_anchor};
pub use catalog::{Catalog, CategoryDef, IssueDef, IssueKind, IssueText, Lang, Priority, catalog};
pub use lang::is_valid_hreflang;
pub use pixels::pixel_width;
