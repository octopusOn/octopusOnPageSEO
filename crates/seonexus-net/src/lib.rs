//! Network adapters of the octopusSEO tools.
//!
//! - [`HttpClient`] downloads small documents such as crawler IP lists (rustls, no system
//!   TLS dependency).
//! - [`SystemDns`] implements [`seonexus_botverify::DnsResolver`] with the operating
//!   system's resolver, so it honours local DNS configuration on every platform.

mod dns;
mod http;

pub use dns::SystemDns;
pub use http::{HttpClient, NetError};
