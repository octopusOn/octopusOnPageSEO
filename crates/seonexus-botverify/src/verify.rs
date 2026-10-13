//! Deciding whether a hit claiming to be a bot really comes from the bot's operator.

use std::collections::HashMap;
use std::fmt;
use std::net::IpAddr;
use std::sync::Arc;

use serde::Serialize;

use crate::catalog::BotDef;
use crate::cidr::IpRangeSet;
use crate::ranges::RangeSnapshot;

/// Outcome of verifying an `(ip, bot)` pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Verdict {
    /// The IP belongs to the operator.
    Verified,
    /// The operator publishes verification data and the IP failed it.
    Spoofed,
    /// Could not be decided (no verification means, data missing, DNS skipped or failed).
    Unverified,
}

impl Verdict {
    /// Stable machine name.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Verified => "verified",
            Self::Spoofed => "spoofed",
            Self::Unverified => "unverified",
        }
    }

    /// Looks a verdict up by name.
    pub fn from_name(name: &str) -> Option<Self> {
        [Self::Verified, Self::Spoofed, Self::Unverified]
            .into_iter()
            .find(|v| v.name() == name)
    }
}

impl fmt::Display for Verdict {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// How a verdict was reached.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Method {
    /// Matched against the operator's published IP list.
    IpList,
    /// Forward-confirmed reverse DNS.
    Dns,
    /// The operator publishes no verification data.
    NotVerifiable,
    /// Only DNS could decide, and DNS checks were disabled.
    DnsSkipped,
    /// DNS lookups failed (network error, timeout).
    DnsFailed,
    /// The operator's IP lists are not available locally.
    DataMissing,
}

impl Method {
    /// Stable machine name.
    pub const fn name(self) -> &'static str {
        match self {
            Self::IpList => "ip-list",
            Self::Dns => "dns",
            Self::NotVerifiable => "not-verifiable",
            Self::DnsSkipped => "dns-skipped",
            Self::DnsFailed => "dns-failed",
            Self::DataMissing => "data-missing",
        }
    }

    /// Looks a method up by name.
    pub fn from_name(name: &str) -> Option<Self> {
        [
            Self::IpList,
            Self::Dns,
            Self::NotVerifiable,
            Self::DnsSkipped,
            Self::DnsFailed,
            Self::DataMissing,
        ]
        .into_iter()
        .find(|m| m.name() == name)
    }

    /// Whether checking again later could change the verdict.
    pub const fn is_retryable(self) -> bool {
        matches!(self, Self::DnsSkipped | Self::DnsFailed | Self::DataMissing)
    }
}

/// A verdict and how it was reached.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub struct Verification {
    /// Outcome.
    pub verdict: Verdict,
    /// Evidence used.
    pub method: Method,
}

impl Verification {
    const fn new(verdict: Verdict, method: Method) -> Self {
        Self { verdict, method }
    }
}

/// A DNS lookup failure.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum DnsError {
    /// The name or address has no record (authoritative answer).
    #[error("no DNS record")]
    NotFound,
    /// The lookup could not be completed.
    #[error("DNS lookup failed: {0}")]
    Failed(String),
}

/// Port for DNS lookups, implemented by a network adapter.
pub trait DnsResolver: Send + Sync {
    /// Host names of `ip` (PTR records).
    ///
    /// # Errors
    /// [`DnsError::NotFound`] when the address has no PTR record.
    fn reverse(&self, ip: IpAddr) -> Result<Vec<String>, DnsError>;

    /// Addresses of `host` (A and AAAA records).
    ///
    /// # Errors
    /// [`DnsError::NotFound`] when the name does not exist.
    fn forward(&self, host: &str) -> Result<Vec<IpAddr>, DnsError>;
}

/// Verifies `(ip, bot)` pairs with IP lists and, optionally, DNS.
///
/// Published lists only contain ranges in use *now*, so an address outside them is not proof
/// of spoofing when the operator also supports DNS verification (older logs contain retired
/// ranges): such addresses are decided by DNS, or stay unverified without it.
pub struct Verifier {
    ranges: HashMap<String, IpRangeSet>,
    dns: Option<Arc<dyn DnsResolver>>,
}

impl fmt::Debug for Verifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Verifier")
            .field("lists", &self.ranges.len())
            .field("dns", &self.dns.is_some())
            .finish()
    }
}

impl Verifier {
    /// Uses the IP lists of `snapshot`; DNS is used only when a resolver is given.
    pub fn new(snapshot: &RangeSnapshot, dns: Option<Arc<dyn DnsResolver>>) -> Self {
        Self {
            ranges: snapshot.range_sets(),
            dns,
        }
    }

    /// Whether DNS checks are enabled.
    pub fn uses_dns(&self) -> bool {
        self.dns.is_some()
    }

    /// Verifies one pair. May block on DNS when enabled.
    pub fn verify(&self, ip: IpAddr, bot: &BotDef) -> Verification {
        if !bot.is_verifiable() {
            return Verification::new(Verdict::Unverified, Method::NotVerifiable);
        }
        let lists: Vec<&IpRangeSet> = bot
            .ip_lists
            .iter()
            .filter_map(|id| self.ranges.get(*id))
            .filter(|set| !set.is_empty())
            .collect();
        if lists.iter().any(|set| set.contains(ip)) {
            return Verification::new(Verdict::Verified, Method::IpList);
        }
        if !bot.dns_suffixes.is_empty() {
            return match &self.dns {
                Some(dns) => verify_dns(dns.as_ref(), ip, bot.dns_suffixes),
                None => Verification::new(Verdict::Unverified, Method::DnsSkipped),
            };
        }
        if lists.is_empty() {
            Verification::new(Verdict::Unverified, Method::DataMissing)
        } else {
            Verification::new(Verdict::Spoofed, Method::IpList)
        }
    }
}

/// Forward-confirmed reverse DNS: a PTR name under an accepted suffix that resolves back to
/// the same address.
fn verify_dns(dns: &dyn DnsResolver, ip: IpAddr, suffixes: &[&str]) -> Verification {
    let spoofed = Verification::new(Verdict::Spoofed, Method::Dns);
    let failed = Verification::new(Verdict::Unverified, Method::DnsFailed);
    let names = match dns.reverse(ip) {
        Ok(names) => names,
        Err(DnsError::NotFound) => return spoofed,
        Err(DnsError::Failed(_)) => return failed,
    };
    let Some(host) = names.iter().find(|name| has_suffix(name, suffixes)) else {
        return spoofed;
    };
    match dns.forward(host.trim_end_matches('.')) {
        Ok(addresses) if addresses.iter().any(|a| same_ip(*a, ip)) => {
            Verification::new(Verdict::Verified, Method::Dns)
        }
        Ok(_) | Err(DnsError::NotFound) => spoofed,
        Err(DnsError::Failed(_)) => failed,
    }
}

fn has_suffix(host: &str, suffixes: &[&str]) -> bool {
    let host = host.trim_end_matches('.').to_ascii_lowercase();
    suffixes.iter().any(|suffix| {
        host.strip_suffix(suffix)
            .is_some_and(|rest| rest.is_empty() || rest.ends_with('.'))
    })
}

fn same_ip(a: IpAddr, b: IpAddr) -> bool {
    let canonical = |ip: IpAddr| match ip {
        IpAddr::V6(v6) => v6.to_ipv4_mapped().map_or(ip, IpAddr::V4),
        IpAddr::V4(_) => ip,
    };
    canonical(a) == canonical(b)
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use crate::catalog::find;
    use crate::ranges::{SOURCES, source};

    struct FakeDns {
        reverse: HashMap<IpAddr, Result<Vec<String>, DnsError>>,
        forward: HashMap<String, Result<Vec<IpAddr>, DnsError>>,
    }

    impl DnsResolver for FakeDns {
        fn reverse(&self, ip: IpAddr) -> Result<Vec<String>, DnsError> {
            self.reverse
                .get(&ip)
                .cloned()
                .unwrap_or(Err(DnsError::NotFound))
        }

        fn forward(&self, host: &str) -> Result<Vec<IpAddr>, DnsError> {
            self.forward
                .get(host)
                .cloned()
                .unwrap_or(Err(DnsError::NotFound))
        }
    }

    fn ip(text: &str) -> IpAddr {
        text.parse().unwrap()
    }

    fn snapshot() -> RangeSnapshot {
        let mut snapshot = RangeSnapshot::default();
        let google = SOURCES
            .iter()
            .find(|s| s.id == source::GOOGLE_COMMON)
            .unwrap();
        snapshot.insert(google, 1, &["66.249.64.0/19".parse().unwrap()]);
        snapshot
    }

    fn fake_dns() -> FakeDns {
        FakeDns {
            reverse: HashMap::from([
                (
                    ip("95.108.213.1"),
                    Ok(vec!["spider-95-108-213-1.yandex.com.".into()]),
                ),
                (ip("95.108.213.2"), Ok(vec!["evil.example.com".into()])),
                (ip("95.108.213.3"), Ok(vec!["fake.yandex.com".into()])),
                (ip("95.108.213.4"), Err(DnsError::Failed("timeout".into()))),
                (
                    ip("1.2.3.4"),
                    Ok(vec!["crawl-1-2-3-4.googlebot.com".into()]),
                ),
            ]),
            forward: HashMap::from([
                (
                    "spider-95-108-213-1.yandex.com".into(),
                    Ok(vec![ip("95.108.213.1")]),
                ),
                ("fake.yandex.com".into(), Ok(vec![ip("10.0.0.1")])),
                (
                    "crawl-1-2-3-4.googlebot.com".into(),
                    Ok(vec![ip("1.2.3.4")]),
                ),
            ]),
        }
    }

    fn verdict(verifier: &Verifier, address: &str, bot: &str) -> (Verdict, Method) {
        let result = verifier.verify(ip(address), find(bot).unwrap());
        (result.verdict, result.method)
    }

    #[test]
    fn ip_lists_verify_and_expose_spoofing() {
        let mut snapshot = snapshot();
        let gptbot = SOURCES
            .iter()
            .find(|s| s.id == source::OPENAI_GPTBOT)
            .unwrap();
        snapshot.insert(gptbot, 1, &["20.171.206.0/24".parse().unwrap()]);
        let verifier = Verifier::new(&snapshot, None);
        let bot = "googlebot-smartphone";
        assert_eq!(
            verdict(&verifier, "66.249.66.1", bot),
            (Verdict::Verified, Method::IpList)
        );
        assert_eq!(
            verdict(&verifier, "::ffff:66.249.66.1", bot).0,
            Verdict::Verified
        );
        // Google also verifies by DNS, so leaving its current ranges is not proof.
        assert_eq!(
            verdict(&verifier, "185.220.101.5", bot),
            (Verdict::Unverified, Method::DnsSkipped)
        );
        // OpenAI verifies by list only.
        assert_eq!(
            verdict(&verifier, "20.171.206.9", "gptbot"),
            (Verdict::Verified, Method::IpList)
        );
        assert_eq!(
            verdict(&verifier, "3.15.62.1", "gptbot"),
            (Verdict::Spoofed, Method::IpList)
        );
        // No list loaded for Anthropic and no DNS: cannot decide.
        assert_eq!(
            verdict(&verifier, "160.79.104.10", "claudebot"),
            (Verdict::Unverified, Method::DataMissing)
        );
        assert_eq!(
            verdict(&verifier, "95.108.213.1", "yandexbot"),
            (Verdict::Unverified, Method::DnsSkipped)
        );
        assert_eq!(
            verdict(&verifier, "1.1.1.1", "bytespider"),
            (Verdict::Unverified, Method::NotVerifiable)
        );
    }

    #[test]
    fn dns_requires_suffix_and_forward_confirmation() {
        let verifier = Verifier::new(&snapshot(), Some(Arc::new(fake_dns())));
        assert!(verifier.uses_dns());
        let bot = "yandexbot";
        assert_eq!(
            verdict(&verifier, "95.108.213.1", bot),
            (Verdict::Verified, Method::Dns)
        );
        assert_eq!(
            verdict(&verifier, "95.108.213.2", bot),
            (Verdict::Spoofed, Method::Dns)
        );
        assert_eq!(
            verdict(&verifier, "95.108.213.3", bot),
            (Verdict::Spoofed, Method::Dns)
        );
        assert_eq!(
            verdict(&verifier, "95.108.213.4", bot),
            (Verdict::Unverified, Method::DnsFailed)
        );
        assert_eq!(
            verdict(&verifier, "95.108.213.9", bot),
            (Verdict::Spoofed, Method::Dns)
        );
        // Outside Google's current lists but confirmed by DNS (e.g. a retired range).
        assert_eq!(
            verdict(&verifier, "1.2.3.4", "googlebot-desktop"),
            (Verdict::Verified, Method::Dns)
        );
    }

    #[test]
    fn suffixes_match_whole_labels() {
        let suffixes = &["google.com"];
        assert!(has_suffix("crawl.google.com.", suffixes));
        assert!(has_suffix("GOOGLE.COM", suffixes));
        assert!(!has_suffix("notgoogle.com", suffixes));
        assert!(!has_suffix("google.com.evil.net", suffixes));
    }

    #[test]
    fn names_round_trip() {
        for verdict in [Verdict::Verified, Verdict::Spoofed, Verdict::Unverified] {
            assert_eq!(Verdict::from_name(verdict.name()), Some(verdict));
        }
        assert_eq!(Method::from_name("dns-skipped"), Some(Method::DnsSkipped));
        assert!(Method::DnsSkipped.is_retryable() && !Method::IpList.is_retryable());
    }
}
