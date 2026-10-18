//! Higher-level SEO analyses built on query results.
//!
//! - [`urls`] matches a URL list (crawl export, sitemap) against logged URLs: pages bots
//!   never requested and logged pages missing from the list (orphans).
//! - [`periods`] compares two periods per key (URL, directory, bot, …).
//! - [`overview`] derives crawl-budget indicators from aggregate counts.
//! - [`anomalies`] finds unusual days: crawl drops and spikes, server-error bursts.
//!
//! Inputs and outputs are plain values; fetching data is the application's job.

pub mod anomalies;
pub mod overview;
pub mod periods;
pub mod urls;

mod share;

pub use share::Share;
