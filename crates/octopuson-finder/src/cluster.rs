//! Directory and URL path pattern cluster diagnostics for indexed URLs.
//!
//! Groups verified indexed URLs by top-level directory segment (e.g. `/urunler`, `/blog`,
//! `/kampanya`) to pinpoint where indexed 404s, Soft-404s, and thin-content 200 OK pages
//! concentrate, and emits section-level SEO remediation hints.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use url::Url;

use crate::model::{FindingCategory, VerifiedUrl};

/// Aggregated health and error metrics for a top-level URL directory / path prefix.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PathCluster {
    /// Directory prefix (e.g. `/urunler`, `/blog`, or `/` for root).
    pub prefix: String,
    /// Total indexed URLs discovered under this prefix.
    pub total_urls: u32,
    /// Total broken / thin / error URLs under this prefix.
    pub issue_urls: u32,
    /// Hard 404 / 410 count under this prefix.
    pub count_404: u32,
    /// Soft-404 + Thin-200 count under this prefix.
    pub count_soft_thin: u32,
    /// Other 4xx + 5xx count under this prefix.
    pub count_4xx_5xx: u32,
    /// Average visible word count across pages in this prefix.
    pub avg_word_count: u32,
    /// Average HTTP response time in milliseconds.
    pub avg_response_ms: u32,
    /// Automated diagnostic recommendation key (`redirect_section_301`, `thin_content_audit`, `server_5xx_check`, `healthy_section`).
    pub action_hint: String,
}

#[derive(Default)]
struct ClusterAccum {
    total_urls: u32,
    issue_urls: u32,
    count_404: u32,
    count_soft_thin: u32,
    count_4xx_5xx: u32,
    word_sum: u64,
    time_sum: u64,
}

/// Computes directory-level path clusters sorted by `issue_urls` descending, then `total_urls` descending.
pub fn build_path_clusters(rows: &[VerifiedUrl]) -> Vec<PathCluster> {
    let mut map: BTreeMap<String, ClusterAccum> = BTreeMap::new();

    for row in rows {
        let prefix = extract_directory_prefix(&row.url);
        let entry = map.entry(prefix).or_default();
        entry.total_urls = entry.total_urls.saturating_add(1);
        if row.category.is_issue() {
            entry.issue_urls = entry.issue_urls.saturating_add(1);
        }
        match row.category {
            FindingCategory::Error404 => {
                entry.count_404 = entry.count_404.saturating_add(1);
            }
            FindingCategory::Soft404 | FindingCategory::Thin200 => {
                entry.count_soft_thin = entry.count_soft_thin.saturating_add(1);
            }
            FindingCategory::Error4xx
            | FindingCategory::Error5xx
            | FindingCategory::NetworkError => {
                entry.count_4xx_5xx = entry.count_4xx_5xx.saturating_add(1);
            }
            FindingCategory::Healthy200 => {}
        }
        entry.word_sum = entry.word_sum.saturating_add(u64::from(row.word_count));
        entry.time_sum = entry
            .time_sum
            .saturating_add(u64::from(row.response_time_ms));
    }

    let mut clusters: Vec<PathCluster> = map
        .into_iter()
        .map(|(prefix, acc)| {
            let denom = u64::from(acc.total_urls.max(1));
            let avg_word_count = u32::try_from(acc.word_sum / denom).unwrap_or(0);
            let avg_response_ms = u32::try_from(acc.time_sum / denom).unwrap_or(0);
            let action_hint = classify_cluster_hint(&acc).to_owned();
            PathCluster {
                prefix,
                total_urls: acc.total_urls,
                issue_urls: acc.issue_urls,
                count_404: acc.count_404,
                count_soft_thin: acc.count_soft_thin,
                count_4xx_5xx: acc.count_4xx_5xx,
                avg_word_count,
                avg_response_ms,
                action_hint,
            }
        })
        .collect();

    clusters.sort_by(|a, b| {
        b.issue_urls
            .cmp(&a.issue_urls)
            .then_with(|| b.total_urls.cmp(&a.total_urls))
            .then_with(|| a.prefix.cmp(&b.prefix))
    });
    clusters.truncate(20);
    clusters
}

fn extract_directory_prefix(raw_url: &str) -> String {
    let Ok(parsed) = Url::parse(raw_url) else {
        return "/".to_owned();
    };
    let mut segments = parsed.path().split('/').filter(|s| !s.is_empty());
    let Some(first) = segments.next() else {
        return "/".to_owned();
    };
    // If there is a second segment, `first` is a directory; otherwise if `first` looks like a top-level section, group by `/<first>`
    format!("/{}", first.to_ascii_lowercase())
}

const fn classify_cluster_hint(acc: &ClusterAccum) -> &'static str {
    if acc.issue_urls == 0 {
        return "healthy_section";
    }
    if acc.count_404 > 0
        && acc.count_404 >= acc.count_soft_thin
        && acc.count_404 >= acc.count_4xx_5xx
    {
        return "redirect_section_301";
    }
    if acc.count_soft_thin > 0 && acc.count_soft_thin >= acc.count_4xx_5xx {
        return "thin_content_audit";
    }
    "server_5xx_check"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn groups_urls_by_top_level_directory_and_computes_action_hints() {
        let rows = vec![
            VerifiedUrl {
                url: "https://acme.com/urunler/eski-1".to_owned(),
                engines: "google,yandex".to_owned(),
                serp_title: "Eski 1".to_owned(),
                serp_snippet: String::new(),
                found_by_query: "site:acme.com".to_owned(),
                initial_status: 404,
                final_status: 404,
                redirect_hops: 0,
                final_url: "https://acme.com/urunler/eski-1".to_owned(),
                response_time_ms: 40,
                content_type: "text/html".to_owned(),
                live_title: "404".to_owned(),
                h1: "404".to_owned(),
                word_count: 10,
                canonical: String::new(),
                noindex: false,
                category: FindingCategory::Error404,
                reason: "http_404".to_owned(),
                suggested_target: "https://acme.com/urunler/".to_owned(),
                error_message: String::new(),
            },
            VerifiedUrl {
                url: "https://acme.com/blog/ince-yazi".to_owned(),
                engines: "yandex".to_owned(),
                serp_title: "İnce Yazı".to_owned(),
                serp_snippet: String::new(),
                found_by_query: "site:acme.com".to_owned(),
                initial_status: 200,
                final_status: 200,
                redirect_hops: 0,
                final_url: "https://acme.com/blog/ince-yazi".to_owned(),
                response_time_ms: 80,
                content_type: "text/html".to_owned(),
                live_title: "İnce Yazı".to_owned(),
                h1: "İnce Yazı".to_owned(),
                word_count: 18,
                canonical: String::new(),
                noindex: false,
                category: FindingCategory::Thin200,
                reason: "thin_content_200".to_owned(),
                suggested_target: "https://acme.com/blog/".to_owned(),
                error_message: String::new(),
            },
        ];

        let clusters = build_path_clusters(&rows);
        assert_eq!(clusters.len(), 2);
        let urunler = clusters.iter().find(|c| c.prefix == "/urunler").unwrap();
        assert_eq!(urunler.count_404, 1);
        assert_eq!(urunler.action_hint, "redirect_section_301");

        let blog = clusters.iter().find(|c| c.prefix == "/blog").unwrap();
        assert_eq!(blog.count_soft_thin, 1);
        assert_eq!(blog.action_hint, "thin_content_audit");
    }
}
