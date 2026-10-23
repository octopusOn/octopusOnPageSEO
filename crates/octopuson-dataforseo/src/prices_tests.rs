//! Every endpoint's estimate against DataForSEO's list prices (US dollars), so a price change
//! shows up here before users see a wrong figure.

use octopuson_serp::Engine;

use crate::{ai, backlinks, keywords, labs, local, prices, rank, serp, site};

fn assert_usd(actual: f64, expected: f64, what: &str) {
    assert!(
        (actual - expected).abs() < 1e-9,
        "{what}: {actual} instead of {expected}"
    );
}

#[test]
fn estimates_follow_the_list_prices() {
    // Labs: a task and a row each.
    assert_usd(
        labs::domain_overview_estimate(),
        0.012_12,
        "domain_rank_overview",
    );
    assert_usd(site::estimate(100), 0.024, "ranked_keywords, 100 rows");
    assert_usd(site::estimate(1000), 0.132, "ranked_keywords, 1000 rows");
    assert_usd(
        keywords::overview_estimate(700),
        0.096,
        "keyword_overview, 700 keywords",
    );
    // The ten-times endpoints: historical_rank_overview over 24 months.
    assert_usd(
        site::history_estimate(),
        0.148_8,
        "historical_rank_overview",
    );
    assert_usd(
        prices::labs_history(10),
        0.132,
        "historical_bulk_traffic_estimation, 10 domains",
    );
    assert_usd(
        site::history_estimate_for(73),
        0.207_6,
        "historical_rank_overview since October 2020",
    );
    assert_usd(
        site::estimate(site::COUNTRY_ROWS),
        0.024,
        "domain_rank_overview in every country",
    );
    assert_usd(
        prices::HISTORICAL_SERP * 12.0,
        0.001_44,
        "historical_serps, 12 pages",
    );
    // Backlinks: a request and a row each.
    assert_usd(backlinks::estimate(1), 0.024_036, "backlinks/summary");
    assert_usd(backlinks::estimate(100), 0.027_6, "backlinks, 100 rows");
    assert_usd(
        backlinks::estimate(backlinks::history_months(2026, 10)),
        0.027_384,
        "backlinks/history since 2019",
    );
    // SERP: a live page of ten results, and the AI Overview loaded after it.
    assert_usd(serp::LIVE_PAGE, 0.002, "serp organic live");
    assert_usd(
        rank::estimate(Engine::Google, 30, false),
        0.006,
        "rank check, 30 results",
    );
    assert_usd(
        rank::estimate(Engine::Google, 10, true),
        0.004,
        "rank check with AI Overview",
    );
    assert_usd(
        rank::estimate(Engine::Bing, 10, true),
        0.002,
        "Bing has no AI Overview",
    );
    // The standard queue: $0.0006 a page and for the late AI Overview.
    assert_usd(
        rank::queue_estimate(Engine::Google, 30, false),
        0.001_8,
        "queued rank check, 30 results",
    );
    assert_usd(
        rank::queue_estimate(Engine::Google, 10, true),
        0.001_2,
        "queued rank check with AI Overview",
    );
    // Local: a Maps page per grid point, profiles, listings and reviews.
    assert_usd(local::maps_estimate(25, false), 0.05, "5×5 grid live");
    assert_usd(local::maps_estimate(49, true), 0.029_4, "7×7 grid queued");
    assert_usd(local::business_estimate(), 0.005_4, "business profile");
    assert_usd(local::listings_estimate(50), 0.025, "50 businesses around");
    assert_usd(local::reviews_estimate(100), 0.007_5, "100 reviews");
    // AI optimisation.
    assert_usd(ai::llm_estimate("chatgpt"), 0.030_6, "ChatGPT answer");
    assert_usd(ai::ai_mode_estimate(), 0.004, "AI Mode answer");
    assert_usd(ai::mentions_estimate(100), 0.2, "LLM Mentions, 100 rows");
}
