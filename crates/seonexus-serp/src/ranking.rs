//! Where a site ranks in a result page.

use serde::{Deserialize, Serialize};
use url::Url;

use crate::{SerpError, SerpSnapshot};

/// Registrable domain (eTLD+1) of a lowercase host, by the Public Suffix List:
/// `blog.example.com.tr` → `example.com.tr`. Hosts without a known suffix (IP addresses,
/// `localhost`) are returned as they are.
pub fn registrable_domain(host: &str) -> String {
    let host = host.trim_end_matches('.');
    if host.parse::<std::net::IpAddr>().is_ok() || host.starts_with('[') {
        return host.to_owned();
    }
    psl::domain_str(host).unwrap_or(host).to_owned()
}

/// How a target is compared with result URLs.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MatchMode {
    /// Any host of the registrable domain (`example.com` matches `shop.example.com`).
    #[default]
    Domain,
    /// Exactly this host, ignoring a leading `www.`.
    Host,
    /// URLs starting with this address, ignoring scheme and a leading `www.`.
    Prefix,
}

/// A site whose rankings are tracked.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Target {
    /// Domain, host or URL prefix, normalised (lowercase ASCII host, no scheme for prefixes).
    pub value: String,
    /// How it is compared.
    pub mode: MatchMode,
}

impl Target {
    /// Normalises a domain, host or URL as typed (`Örnek.com.tr`, `https://www.x.com/blog`).
    ///
    /// # Errors
    /// [`SerpError::InvalidQuery`] when no host can be read from `input`.
    pub fn parse(input: &str, mode: MatchMode) -> Result<Self, SerpError> {
        let trimmed = input.trim();
        let with_scheme = if trimmed.contains("://") {
            trimmed.to_owned()
        } else {
            format!("http://{trimmed}")
        };
        let url = Url::parse(&with_scheme)
            .map_err(|_| SerpError::InvalidQuery(format!("`{trimmed}` is not a domain or URL")))?;
        let host = url
            .host_str()
            .filter(|host| !host.is_empty())
            .ok_or_else(|| SerpError::InvalidQuery(format!("`{trimmed}` has no host")))?
            .trim_end_matches('.')
            .to_ascii_lowercase();
        let value = match mode {
            MatchMode::Domain => registrable_domain(strip_www(&host)),
            MatchMode::Host => strip_www(&host).to_owned(),
            MatchMode::Prefix => prefix_key(&url),
        };
        Ok(Self { value, mode })
    }

    /// Whether a result URL belongs to the target.
    pub fn matches(&self, url: &str) -> bool {
        let Ok(url) = Url::parse(url) else {
            return false;
        };
        let Some(host) = url.host_str() else {
            return false;
        };
        let host = host.trim_end_matches('.').to_ascii_lowercase();
        match self.mode {
            MatchMode::Domain => registrable_domain(&host) == self.value,
            MatchMode::Host => strip_www(&host) == self.value,
            MatchMode::Prefix => prefix_key(&url).starts_with(&self.value),
        }
    }
}

fn strip_www(host: &str) -> &str {
    host.strip_prefix("www.").unwrap_or(host)
}

/// `host/path?query` without scheme, `www.` and a trailing slash of the root.
fn prefix_key(url: &Url) -> String {
    let host = url
        .host_str()
        .map(|host| strip_www(&host.trim_end_matches('.').to_ascii_lowercase()).to_owned())
        .unwrap_or_default();
    let mut key = format!("{host}{}", url.path());
    if let Some(query) = url.query() {
        key.push('?');
        key.push_str(query);
    }
    if key.ends_with('/') && url.path() == "/" && url.query().is_none() {
        key.pop();
    }
    key
}

/// Where a target ranks in a snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rank {
    /// Best organic position.
    pub position: u16,
    /// URL at that position.
    pub url: String,
    /// Every position the target holds, ascending (more than one means several URLs rank).
    pub positions: Vec<u16>,
}

/// The target's best organic position, if it ranks within the snapshot.
pub fn find_rank(snapshot: &SerpSnapshot, target: &Target) -> Option<Rank> {
    let mut hits: Vec<(u16, &str)> = snapshot
        .organic
        .iter()
        .filter(|result| target.matches(&result.url))
        .map(|result| (result.position, result.url.as_str()))
        .collect();
    hits.sort_unstable_by_key(|(position, _)| *position);
    let (position, url) = *hits.first()?;
    Some(Rank {
        position,
        url: url.to_owned(),
        positions: hits.iter().map(|(position, _)| *position).collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_registrable_domains() {
        assert_eq!(registrable_domain("blog.example.com.tr"), "example.com.tr");
        assert_eq!(registrable_domain("www.bbc.co.uk"), "bbc.co.uk");
        assert_eq!(registrable_domain("example.com."), "example.com");
        assert_eq!(registrable_domain("localhost"), "localhost");
        assert_eq!(registrable_domain("127.0.0.1"), "127.0.0.1");
    }

    #[test]
    fn matches_by_domain_host_and_prefix() {
        let domain = Target::parse("https://WWW.Example.com.tr/x", MatchMode::Domain).unwrap();
        assert_eq!(domain.value, "example.com.tr");
        assert!(domain.matches("https://shop.example.com.tr/a"));
        assert!(!domain.matches("https://example.com/a"));

        let host = Target::parse("www.example.com", MatchMode::Host).unwrap();
        assert!(host.matches("http://example.com/"));
        assert!(!host.matches("https://shop.example.com/"));

        let prefix = Target::parse("example.com/blog", MatchMode::Prefix).unwrap();
        assert!(prefix.matches("https://www.example.com/blog/post"));
        assert!(!prefix.matches("https://example.com/shop"));
    }

    #[test]
    fn keys_keywords_with_turkish_casing() {
        assert_eq!(
            crate::keyword_key(" İstanbul  OTELLERİ ", "tr"),
            "istanbul otelleri"
        );
        assert_eq!(crate::keyword_key("IŞIK", "tr"), "ışık");
        assert_eq!(crate::keyword_key("IKEA Istanbul", "en"), "ikea istanbul");
    }

    #[test]
    fn normalises_internationalised_domains() {
        let target = Target::parse("örnek.com.tr", MatchMode::Domain).unwrap();
        assert_eq!(target.value, "xn--rnek-4qa.com.tr");
        assert!(target.matches("https://www.xn--rnek-4qa.com.tr/"));
        assert!(Target::parse("", MatchMode::Domain).is_err());
    }
}
