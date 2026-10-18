//! URL normalization for SEO log analysis.
//!
//! Logs record request targets (`/blog/post?utm_source=x`), crawlers and sitemaps record
//! absolute URLs (`https://www.example.com/blog/post`). [`normalize`] maps both onto a
//! comparable key so that lists from different tools can be matched against log data.
//!
//! Normalization is conservative by default: it only applies changes that never alter which
//! resource a server returns (fragment removal, RFC 3986 percent-encoding and dot-segment
//! normalization, default ports, host case) plus removal of well-known tracking parameters.
//! Everything else (case, trailing slashes, all parameters) is opt-in.
//!
//! [`robots`] parses robots.txt files and decides which URLs a crawler may fetch.

mod encoding;
mod normalize;
mod query;
pub mod robots;

pub use normalize::{NormalizeOptions, NormalizedUrl, QueryMode, normalize};
pub use query::{is_tracking_parameter, strip_parameters};
