//! Application use-cases shared by the `snx-log` CLI and the desktop Log Analyser.
//!
//! Front-ends translate user input into request structs, call a use-case and render the
//! returned report. No use-case prints, prompts or depends on a UI toolkit.

pub mod analysis;
pub mod archive;
pub mod bots;
mod cancel;
pub mod catalog;
mod format_choice;
pub mod geo;
pub mod import;
mod inputs;
pub mod inspect;
mod pipeline;
pub mod query;
mod sample;
pub mod saved;
pub mod settings;
mod site_access;
mod stats;
pub mod table_file;
#[cfg(test)]
mod test_support;
mod workspace;

pub use cancel::CancelToken;
pub use format_choice::{FormatChoice, FormatError, FormatInfo, builtin_formats};
pub use seonexus_core::table;
/// Streaming exporters, re-exported for front-ends that write results themselves.
pub use seonexus_report as report;
pub use seonexus_store::{ImportOptions, StoreError};
pub use stats::{ErrorSample, ParseSummary, StreamSummary, Throughput};
pub use workspace::Workspace;

/// `User-Agent` of every HTTP request the application makes (IP lists, geolocation data,
/// sitemaps, robots.txt), so site owners can identify and allow it.
pub(crate) const USER_AGENT: &str = concat!(
    "octopusSEO-LogAnalyser/",
    env!("CARGO_PKG_VERSION"),
    " (+https://octopusseo.com)"
);
