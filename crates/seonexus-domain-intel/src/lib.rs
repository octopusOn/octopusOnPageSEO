//! Facts about domains that make up a site's strength.
//!
//! - Age: [`rdap`] for TLDs with an RDAP service (most generic TLDs), [`whois`] through
//!   IANA's referral for the others (`.tr`, many country codes), and the [`wayback`]
//!   Machine's first capture as a lower bound when neither answers.
//! - Authority: [`authority::open_pagerank`] (free with a key, from the Common Crawl link
//!   graph, with referring domains) and [`authority::backlink_summary`] (DataForSEO, paid).
//! - Popularity: [`tranco`] ranks from the Tranco top-million list, which the user downloads.
//!
//! Network access goes through the [`seonexus_core::http::HttpTransport`] and
//! [`whois::WhoisPort`] ports.

pub mod authority;
mod dates;
pub mod rdap;
pub mod tranco;
pub mod wayback;
pub mod whois;

use serde::{Deserialize, Serialize};

pub use dates::{Day, parse_date};

/// A lookup failed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum IntelError {
    /// No such domain or no data about it.
    #[error("no data about {0}")]
    NotFound(String),
    /// The source does not cover the domain's TLD.
    #[error("{0}")]
    Unsupported(String),
    /// The source refused the credentials.
    #[error("{0}")]
    Auth(String),
    /// The balance or quota of the source is exhausted.
    #[error("{0}")]
    Quota(String),
    /// Too many requests.
    #[error("too many requests; try again later")]
    RateLimited,
    /// No answer.
    #[error("network error: {0}")]
    Network(String),
    /// The answer could not be read.
    #[error("cannot read the answer: {0}")]
    Parse(String),
}

/// The last label of a domain (`ornek.com.tr` → `tr`), lowercase.
pub fn top_level(domain: &str) -> &str {
    domain
        .trim_end_matches('.')
        .rsplit('.')
        .next()
        .unwrap_or(domain)
}

/// Registrable domain (eTLD+1) of a host: `blog.ornek.com.tr` → `ornek.com.tr`.
pub fn registrable(host: &str) -> String {
    let host = host.trim().trim_end_matches('.').to_ascii_lowercase();
    if host.parse::<std::net::IpAddr>().is_ok() || host.starts_with('[') {
        return host;
    }
    psl::domain_str(&host).unwrap_or(&host).to_owned()
}

/// Where a domain's age comes from, most reliable first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AgeSource {
    /// The registry's RDAP record.
    Rdap,
    /// The registry's WHOIS record.
    Whois,
    /// The first capture in the Wayback Machine (the domain is at least this old).
    Wayback,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_tlds_and_registrable_domains() {
        assert_eq!(top_level("ornek.com.tr"), "tr");
        assert_eq!(top_level("example.com."), "com");
        assert_eq!(registrable("Blog.Ornek.COM.tr"), "ornek.com.tr");
        assert_eq!(registrable("127.0.0.1"), "127.0.0.1");
    }
}
