//! Crawl-budget overview: how search-engine crawlers spend their requests.

use serde::Serialize;

use crate::share::{Share, ratio};

/// Aggregate counts the overview is derived from (fetched by the application).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct OverviewInput {
    /// All hits.
    pub total_hits: u64,
    /// Hits per bot category; browsers under `None`.
    pub categories: Vec<(Option<String>, u64)>,
    /// Hits per bot and verdict.
    pub bots: Vec<(String, String, u64)>,
    /// Verified search-engine hits per status class (`2xx`, …).
    pub search_status_classes: Vec<(String, u64)>,
    /// Verified search-engine hits.
    pub search_hits: u64,
    /// Distinct URLs requested by verified search engines.
    pub search_unique_urls: u64,
    /// Days with verified search-engine hits.
    pub search_days: u64,
    /// Sum over those days of the distinct URLs crawled that day.
    pub search_daily_unique_urls: u64,
    /// Verified search-engine hits answered with 4xx or 5xx.
    pub search_errors: u64,
    /// Mean response size of verified search-engine hits (`None` without byte counts).
    pub search_avg_bytes: Option<f64>,
    /// Mean response time of verified search-engine hits (`None` if the format lacks it).
    pub search_avg_ms: Option<f64>,
    /// Verified search-engine hits on URLs with a query string.
    pub search_parameter_hits: u64,
    /// Verified search-engine hits on static assets (scripts, styles, images, fonts).
    pub search_asset_hits: u64,
}

/// Hits of one bot split by verdict.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BotHits {
    /// Bot id.
    pub bot: String,
    /// Verified hits.
    pub verified: u64,
    /// Spoofed hits.
    pub spoofed: u64,
    /// Unverified hits.
    pub unverified: u64,
}

impl BotHits {
    /// All hits.
    pub fn total(&self) -> u64 {
        self.verified + self.spoofed + self.unverified
    }
}

/// Crawl indicators of verified search-engine bots.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SearchCrawl {
    /// Hits.
    pub hits: u64,
    /// Distinct URLs.
    pub unique_urls: u64,
    /// Average hits per active day.
    pub hits_per_day: f64,
    /// Average distinct URLs crawled per active day.
    pub unique_urls_per_day: f64,
    /// Hits answered with 4xx or 5xx.
    pub errors: u64,
    /// Mean response size in bytes.
    pub avg_bytes: Option<f64>,
    /// Mean response time in milliseconds.
    pub avg_ms: Option<f64>,
    /// Hits per status class.
    pub status_classes: Vec<Share>,
    /// Share of hits answered with anything but 2xx (redirects, errors).
    pub non_success_ratio: f64,
    /// Share of hits on parameterized URLs.
    pub parameter_ratio: f64,
    /// Share of hits on static assets.
    pub asset_ratio: f64,
}

/// The overview.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CrawlOverview {
    /// All hits.
    pub total_hits: u64,
    /// Hits per category (`browser` for non-bots).
    pub categories: Vec<Share>,
    /// Bots by total hits.
    pub bots: Vec<BotHits>,
    /// Hits claiming to be a bot that failed verification.
    pub spoofed_hits: u64,
    /// Search-engine crawl indicators.
    pub search: SearchCrawl,
}

/// Derives the overview from aggregate counts.
pub fn crawl_overview(input: &OverviewInput) -> CrawlOverview {
    let mut categories: Vec<Share> = input
        .categories
        .iter()
        .map(|(name, hits)| {
            Share::of(
                name.clone().unwrap_or_else(|| "browser".to_owned()),
                *hits,
                input.total_hits,
            )
        })
        .collect();
    categories.sort_by_key(|share| std::cmp::Reverse(share.value));

    let mut bots: Vec<BotHits> = Vec::new();
    for (bot, verdict, hits) in &input.bots {
        let index = if let Some(index) = bots.iter().position(|b| &b.bot == bot) {
            index
        } else {
            bots.push(BotHits {
                bot: bot.clone(),
                verified: 0,
                spoofed: 0,
                unverified: 0,
            });
            bots.len() - 1
        };
        let line = &mut bots[index];
        match verdict.as_str() {
            "verified" => line.verified += hits,
            "spoofed" => line.spoofed += hits,
            _ => line.unverified += hits,
        }
    }
    bots.sort_by(|a, b| b.total().cmp(&a.total()).then_with(|| a.bot.cmp(&b.bot)));
    let spoofed_hits = bots.iter().map(|b| b.spoofed).sum();

    let search_hits = input.search_hits;
    let success: u64 = input
        .search_status_classes
        .iter()
        .filter(|(class, _)| class == "2xx")
        .map(|(_, hits)| hits)
        .sum();
    #[allow(
        clippy::cast_precision_loss,
        reason = "average for display; day counts are small"
    )]
    let per_day = |total: u64| {
        if input.search_days == 0 {
            0.0
        } else {
            total as f64 / input.search_days as f64
        }
    };
    CrawlOverview {
        total_hits: input.total_hits,
        categories,
        bots,
        spoofed_hits,
        search: SearchCrawl {
            hits: search_hits,
            unique_urls: input.search_unique_urls,
            hits_per_day: per_day(search_hits),
            unique_urls_per_day: per_day(input.search_daily_unique_urls),
            errors: input.search_errors,
            avg_bytes: input.search_avg_bytes,
            avg_ms: input.search_avg_ms,
            status_classes: input
                .search_status_classes
                .iter()
                .map(|(class, hits)| Share::of(class.clone(), *hits, search_hits))
                .collect(),
            non_success_ratio: ratio(search_hits.saturating_sub(success), search_hits),
            parameter_ratio: ratio(input.search_parameter_hits, search_hits),
            asset_ratio: ratio(input.search_asset_hits, search_hits),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derives_shares_and_bot_lines() {
        let input = OverviewInput {
            total_hits: 100,
            categories: vec![(None, 40), (Some("search".into()), 60)],
            bots: vec![
                ("googlebot-smartphone".into(), "verified".into(), 50),
                ("googlebot-smartphone".into(), "spoofed".into(), 5),
                ("bingbot".into(), "unverified".into(), 5),
            ],
            search_status_classes: vec![("2xx".into(), 40), ("3xx".into(), 10)],
            search_hits: 50,
            search_unique_urls: 20,
            search_days: 10,
            search_daily_unique_urls: 25,
            search_errors: 3,
            search_avg_bytes: Some(1500.0),
            search_avg_ms: None,
            search_parameter_hits: 5,
            search_asset_hits: 10,
        };
        let overview = crawl_overview(&input);
        assert_eq!(overview.categories[0].name, "search");
        assert_eq!(overview.categories[1].name, "browser");
        assert_eq!(overview.bots[0].total(), 55);
        assert_eq!(overview.spoofed_hits, 5);
        assert!((overview.search.hits_per_day - 5.0).abs() < 1e-9);
        assert!((overview.search.unique_urls_per_day - 2.5).abs() < 1e-9);
        assert_eq!(overview.search.errors, 3);
        assert!((overview.search.non_success_ratio - 0.2).abs() < 1e-9);
        assert!((overview.search.parameter_ratio - 0.1).abs() < 1e-9);
        assert!((overview.search.asset_ratio - 0.2).abs() < 1e-9);
        let json = serde_json::to_value(&overview).unwrap();
        assert_eq!(json["search"]["unique_urls"], 20);
    }

    #[test]
    fn empty_input_has_no_divisions_by_zero() {
        let overview = crawl_overview(&OverviewInput::default());
        assert!(overview.search.non_success_ratio.abs() < f64::EPSILON);
        assert!(overview.search.hits_per_day.abs() < f64::EPSILON);
    }
}
