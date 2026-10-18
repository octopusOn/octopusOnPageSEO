//! Clusters against a site: which page targets each cluster, where pages compete, and which
//! clusters the site has no page for.

use std::collections::{BTreeMap, HashMap};

use seonexus_serp::registrable_domain;
use serde::{Deserialize, Serialize};

use crate::cluster::Cluster;
use crate::serp::{Level, result_key};

/// A page of the site that is seen for a keyword (Search Console row or a result page).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SitePage {
    /// Index of the keyword in the clustered list.
    pub keyword: usize,
    /// The page.
    pub url: String,
    /// Impressions (Search Console), if known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub impressions: Option<f64>,
    /// Average or observed position.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub position: Option<f64>,
}

/// A cluster against the site.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClusterMap {
    /// The page that serves the cluster best (most impressions, then best position).
    pub target: Option<String>,
    /// Other pages of the site that compete for the cluster's keywords.
    pub competing: Vec<String>,
    /// The site has no page for any of its keywords.
    pub gap: bool,
    /// Sites that rank for its keywords most often, with the number of keywords.
    pub competitors: Vec<(String, u32)>,
}

/// Share of the cluster's impressions (or keywords) a second page needs to count as
/// competing.
const COMPETING_SHARE: f64 = 0.1;

/// Maps a cluster: `pages` are the site's pages per keyword, `results` the result pages of
/// the keywords (indexed like the clustered list), `site` the site's registrable domain.
#[allow(clippy::cast_precision_loss)] // counts
pub fn map_cluster(
    cluster: &Cluster,
    pages: &[SitePage],
    results: &[Option<&[String]>],
    site: &str,
) -> ClusterMap {
    let members: Vec<usize> = cluster.indices().collect();
    // Impressions (or one per keyword) and best position of each page of the site.
    let mut seen: HashMap<String, (String, f64, f64)> = HashMap::new();
    for page in pages.iter().filter(|page| members.contains(&page.keyword)) {
        let Some(key) = result_key(&page.url, Level::Page) else {
            continue;
        };
        let entry = seen
            .entry(key)
            .or_insert_with(|| (page.url.clone(), 0.0, f64::MAX));
        entry.1 += page.impressions.unwrap_or(1.0);
        entry.2 = entry.2.min(page.position.unwrap_or(f64::MAX));
    }
    let mut ranked: Vec<(String, f64, f64)> = seen.into_values().collect();
    ranked.sort_by(|a, b| {
        b.1.total_cmp(&a.1)
            .then_with(|| a.2.total_cmp(&b.2))
            .then_with(|| a.0.cmp(&b.0))
    });
    let total: f64 = ranked.iter().map(|(_, weight, _)| weight).sum();
    let target = ranked.first().map(|(url, _, _)| url.clone());
    let competing = ranked
        .iter()
        .skip(1)
        .filter(|(_, weight, _)| total > 0.0 && weight / total >= COMPETING_SHARE)
        .map(|(url, _, _)| url.clone())
        .collect();

    let mut domains: BTreeMap<String, u32> = BTreeMap::new();
    for index in &members {
        let Some(Some(addresses)) = results.get(*index) else {
            continue;
        };
        let mut own: Vec<String> = addresses
            .iter()
            .take(10)
            .filter_map(|address| result_key(address, Level::Site))
            .filter(|domain| domain != site)
            .collect();
        own.sort_unstable();
        own.dedup();
        for domain in own {
            *domains.entry(domain).or_default() += 1;
        }
    }
    let mut competitors: Vec<(String, u32)> = domains.into_iter().collect();
    competitors.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    competitors.truncate(5);
    ClusterMap {
        gap: target.is_none(),
        target,
        competing,
        competitors,
    }
}

/// The registrable domain of a site as typed (`https://www.örnek.com.tr/` → `xn--rnek-4qa.com.tr`).
pub fn site_domain(site: &str) -> Option<String> {
    let site = site.trim();
    let address = if site.contains("://") {
        site.to_owned()
    } else {
        format!("https://{site}")
    };
    let url = url::Url::parse(&address).ok()?;
    Some(registrable_domain(
        url.host_str()?.trim_start_matches("www."),
    ))
}
