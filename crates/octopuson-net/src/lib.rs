//! Network adapters of the OctopusOn SEO tools.
//!
//! - [`HttpClient`] downloads small documents such as crawler IP lists (rustls, no system
//!   TLS dependency).
//! - [`CrawlFetcher`] implements the Spider's [`octopuson_crawl::Fetcher`]: one GET per call,
//!   redirects and error statuses returned as responses, bodies cut at a limit.
//! - [`ApiClient`] implements the [`octopuson_core::http::HttpTransport`] port for web APIs:
//!   every status returned as a response, redirects followed only within the origin.
//! - [`whois`] asks WHOIS servers (TCP port 43) for registration data.
//! - [`SystemDns`] implements [`octopuson_botverify::DnsResolver`] with the operating
//!   system's resolver, so it honours local DNS configuration on every platform.

mod api;
mod crawl;
mod dns;
mod http;
mod whois;

pub use api::ApiClient;
pub use crawl::CrawlFetcher;
pub use dns::SystemDns;
pub use http::{HttpClient, NetError};
pub use whois::whois;
