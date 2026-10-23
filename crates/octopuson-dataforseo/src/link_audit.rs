//! The toxic-link audit: each linking site judged from one of its links by plain rules, so the
//! user sees why a site looks risky and decides what to keep, watch or disavow.
//!
//! The rules: DataForSEO's spam score; several sites in one network (the same /24 block of IPv4
//! addresses, or the first four groups of IPv6), a sign of a link network; top-level domains
//! mostly used for spam; a site linking from every page (site-wide); a page linking to very
//! many sites (a link farm); and anchors of gambling, adult or pill spam. Links that pass no
//! authority (nofollow) weigh half.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::backlinks::BacklinkRow;

/// Sites in one network from which they look like a link network.
const NETWORK_SITES: usize = 3;

/// Links from one site above which it links from every page.
const SITEWIDE_LINKS: u64 = 50;

/// Links to other sites on a page above which it is a link farm.
const FARM_LINKS: u64 = 150;

/// Top-level domains mostly used for spam.
const RISKY_TLDS: &[&str] = &[
    "xyz", "top", "icu", "buzz", "tk", "ml", "ga", "cf", "gq", "pw", "loan", "win", "bid", "click",
    "work", "rest", "cyou", "sbs", "cfd", "monster", "quest",
];

/// Words of spam anchors (lowercase, whole words).
const SPAM_WORDS: &[&str] = &[
    "casino", "bahis", "bet", "betting", "slot", "slots", "poker", "jackpot", "kumar", "porn",
    "porno", "xxx", "sex", "escort", "viagra", "cialis", "pharmacy", "replica",
];

/// How risky a linking site looks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Risk {
    /// Nothing points to harm.
    Low,
    /// Worth a look.
    Medium,
    /// Likely spam.
    High,
}

/// A linking site and why it looks risky.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LinkRisk {
    /// The linking site.
    pub domain: String,
    /// One of its links.
    pub url_from: String,
    /// The page it links to.
    pub url_to: Option<String>,
    /// Its anchor.
    pub anchor: Option<String>,
    /// Whether it passes authority.
    pub dofollow: bool,
    /// DataForSEO's spam score, 0–100.
    pub spam_score: Option<u32>,
    /// Authority of the site, 0–100.
    pub domain_rank: Option<u32>,
    /// The network it is in (`1.2.3` of an IPv4 address).
    pub network: Option<String>,
    /// Its top-level domain.
    pub tld: Option<String>,
    /// Links from it to the target.
    pub links: Option<u64>,
    /// Country of the site.
    pub country: Option<String>,
    /// First seen.
    pub first_seen: Option<String>,
    /// The rules it breaks: `spam`, `spam_some`, `network`, `tld`, `sitewide`, `link_farm`,
    /// `spam_anchor`.
    pub flags: Vec<String>,
    /// 0–100.
    pub score: u32,
    /// How risky it looks.
    pub risk: Risk,
}

/// The network of an address: its /24 block (IPv4) or first four groups (IPv6).
pub fn network_of(ip: &str) -> Option<String> {
    let ip = ip.trim();
    if ip.contains(':') {
        let groups: Vec<&str> = ip.split(':').take(4).collect();
        return (groups.len() == 4).then(|| groups.join(":"));
    }
    let octets: Vec<&str> = ip.split('.').collect();
    (octets.len() == 4 && octets.iter().all(|octet| octet.parse::<u8>().is_ok()))
        .then(|| octets[..3].join("."))
}

fn spam_anchor(anchor: &str) -> bool {
    // Turkish capital İ lowercases to i with a dot above unless told.
    anchor
        .replace('İ', "i")
        .to_lowercase()
        .split(|character: char| !character.is_alphanumeric())
        .any(|word| SPAM_WORDS.contains(&word))
}

/// The rules a linking site breaks and the score they add up to, before nofollow halves it.
fn rules(
    row: &BacklinkRow,
    network: Option<&str>,
    tld: Option<&str>,
    sites_in: &BTreeMap<String, BTreeSet<String>>,
) -> (Vec<String>, u32) {
    let mut flags: Vec<String> = Vec::new();
    let mut score = 0_u32;
    match row.spam_score {
        Some(spam) if spam >= 60 => {
            flags.push("spam".to_owned());
            score += 55;
        }
        Some(spam) if spam >= 30 => {
            flags.push("spam_some".to_owned());
            score += 25;
        }
        _ => {}
    }
    if network
        .and_then(|network| sites_in.get(network))
        .is_some_and(|sites| sites.len() >= NETWORK_SITES)
    {
        flags.push("network".to_owned());
        score += 20;
    }
    if tld.is_some_and(|tld| RISKY_TLDS.contains(&tld)) {
        flags.push("tld".to_owned());
        score += 15;
    }
    if row
        .links_from_domain
        .is_some_and(|links| links >= SITEWIDE_LINKS)
    {
        flags.push("sitewide".to_owned());
        score += 10;
    }
    if row
        .page_external_links
        .is_some_and(|links| links >= FARM_LINKS)
    {
        flags.push("link_farm".to_owned());
        score += 20;
    }
    if row.anchor.as_deref().is_some_and(spam_anchor) {
        flags.push("spam_anchor".to_owned());
        score += 35;
    }
    (flags, score)
}

/// Judges each linking site (one link each, as a one-per-domain list gives them), the riskiest
/// first.
pub fn assess(rows: &[BacklinkRow]) -> Vec<LinkRisk> {
    let mut sites_in: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for row in rows {
        if let (Some(network), Some(domain)) =
            (row.ip.as_deref().and_then(network_of), &row.domain_from)
        {
            sites_in.entry(network).or_default().insert(domain.clone());
        }
    }
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut risks: Vec<LinkRisk> = Vec::new();
    for row in rows {
        let Some(domain) = row.domain_from.clone() else {
            continue;
        };
        if !seen.insert(domain.clone()) {
            continue;
        }
        let network = row.ip.as_deref().and_then(network_of);
        let tld = row
            .tld
            .clone()
            .or_else(|| domain.rsplit('.').next().map(str::to_owned))
            .map(|tld| tld.to_ascii_lowercase());
        let (flags, mut score) = rules(row, network.as_deref(), tld.as_deref(), &sites_in);
        if !row.dofollow {
            score /= 2;
        }
        let score = score.min(100);
        let risk = if score >= 50 {
            Risk::High
        } else if score >= 25 {
            Risk::Medium
        } else {
            Risk::Low
        };
        risks.push(LinkRisk {
            domain,
            url_from: row.url_from.clone(),
            url_to: row.url_to.clone(),
            anchor: row.anchor.clone(),
            dofollow: row.dofollow,
            spam_score: row.spam_score,
            domain_rank: row.domain_rank,
            network,
            tld,
            links: row.links_from_domain,
            country: row.country.clone(),
            first_seen: row.first_seen.clone(),
            flags,
            score,
            risk,
        });
    }
    risks.sort_by(|left, right| {
        right
            .score
            .cmp(&left.score)
            .then_with(|| left.domain.cmp(&right.domain))
    });
    risks
}
