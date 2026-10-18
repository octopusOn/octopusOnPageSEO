//! Strength of a site: link authority, popularity, age and technical health.

use crate::breakdown::{Breakdown, Builder, round};
use crate::signals::SiteSignals;

/// Version of the site model.
pub const SITE_MODEL: &str = "site-1";

/// The site score. `tranco` says whether Tranco ranks are available at all (the dataset is
/// optional); without it the component is left out rather than counted as missing.
#[allow(clippy::cast_precision_loss)] // counts and ranks
pub fn site_score(site: &SiteSignals, tranco: bool) -> Breakdown {
    let mut builder = Builder::new();
    let pagerank = site.open_page_rank.map(|score| score / 10.0);
    builder
        .add(
            "pagerank",
            30.0,
            site.open_page_rank,
            pagerank,
            if pagerank.is_some() {
                "site.pagerank"
            } else {
                "site.pagerank.unknown"
            },
        )
        .with(
            "open_page_rank",
            site.open_page_rank
                .map_or(serde_json::Value::Null, |score| round(score).into()),
        );
    let referring = site
        .referring_domains
        .map(|count| (1.0 + count as f64).log10() / 4.0);
    builder
        .add(
            "referring_domains",
            25.0,
            site.referring_domains.map(|count| count as f64),
            referring,
            if referring.is_some() {
                "site.referring_domains"
            } else {
                "site.referring_domains.unknown"
            },
        )
        .with("referring_domains", site.referring_domains);
    if tranco {
        // A site outside the top million scores 0 rather than "unknown".
        let popularity = site
            .tranco_rank
            .map_or(0.0, |rank| 1.0 - f64::from(rank.max(1)).log10() / 6.0);
        builder
            .add(
                "popularity",
                20.0,
                site.tranco_rank.map(f64::from),
                Some(popularity),
                "site.popularity",
            )
            .with("tranco_rank", site.tranco_rank);
    }
    let age = site
        .age_years
        .map(|years| (1.0 + years.max(0.0)).ln() / 16f64.ln());
    builder
        .add(
            "age",
            15.0,
            site.age_years,
            age,
            if age.is_some() {
                "site.age"
            } else {
                "site.age.unknown"
            },
        )
        .with("years", site.age_years.map(round));
    let technical = 0.4 * f64::from(u8::from(site.https))
        + 0.2 * f64::from(u8::from(site.hsts))
        + 0.4 * site.technical.unwrap_or(50.0) / 100.0;
    builder
        .add(
            "technical",
            10.0,
            site.technical,
            Some(technical),
            "site.technical",
        )
        .with("https", site.https)
        .with("hsts", site.hsts);
    builder.finish(SITE_MODEL)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn site() -> SiteSignals {
        SiteSignals {
            domain: "example.com".to_owned(),
            open_page_rank: Some(8.0),
            referring_domains: Some(100_000),
            tranco_rank: Some(1_000),
            age_years: Some(15.0),
            https: true,
            hsts: true,
            technical: Some(100.0),
        }
    }

    #[test]
    fn strong_sites_score_high_and_unknowns_lower_confidence() {
        let strong = site_score(&site(), true);
        assert!(strong.score > 80.0, "{strong:?}");
        assert!((strong.confidence - 1.0).abs() < 1e-9);
        let unknown = site_score(
            &SiteSignals {
                open_page_rank: None,
                referring_domains: None,
                ..site()
            },
            false,
        );
        assert!(unknown.confidence < 0.5);
        assert!(unknown.component("popularity").is_none());
        let weak = site_score(
            &SiteSignals {
                open_page_rank: Some(1.0),
                referring_domains: Some(3),
                tranco_rank: None,
                age_years: Some(0.5),
                https: false,
                hsts: false,
                technical: Some(40.0),
                ..site()
            },
            true,
        );
        assert!(weak.score < 25.0, "{weak:?}");
    }
}
