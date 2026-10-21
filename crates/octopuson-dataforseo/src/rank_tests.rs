//! Rank checks read from data shaped like DataForSEO's documented examples.

use octopuson_serp::{Device, Engine};
use serde_json::Value;

use crate::Market;
use crate::rank::*;

fn result() -> Vec<Value> {
    let answer: Value =
        serde_json::from_str(include_str!("../tests/fixtures/serp_rank.json")).unwrap();
    answer["tasks"][0]["result"].as_array().unwrap().clone()
}

fn query(market: &Market, engine: Engine) -> RankQuery<'_> {
    RankQuery {
        keyword: "  spor   ayakkabı ",
        engine,
        market,
        location: None,
        device: Device::Desktop,
        depth: 20,
        ai_overview: true,
    }
}

#[test]
fn checks_are_priced_per_page_of_ten_results() {
    assert_eq!(
        endpoint(Engine::Google),
        "serp/google/organic/live/advanced"
    );
    assert_eq!(
        endpoint(Engine::Yandex),
        "serp/yandex/organic/live/advanced"
    );
    assert_eq!(depth(5), 10);
    assert_eq!(depth(15), 20);
    assert_eq!(depth(250), 100);
    assert!((estimate(Engine::Google, 20, true) - 0.006).abs() < 1e-12);
    assert!((estimate(Engine::Google, 100, false) - 0.02).abs() < 1e-12);
    // Only Google loads AI Overviews later.
    assert!((estimate(Engine::Bing, 20, true) - 0.004).abs() < 1e-12);
}

#[test]
fn locations_stay_in_the_market_country() {
    let market = Market::new("tr", "tr");
    assert_eq!(
        location(" Istanbul, Istanbul ,Turkey ", &market).unwrap(),
        "Istanbul,Istanbul,Turkey"
    );
    assert!(location("Berlin,Germany", &market).is_err());
    assert!(location("Turkey", &market).is_err());
    assert!(location("Ankara,Turkey", &Market::new("zz", "tr")).is_err());
}

#[test]
fn a_task_asks_one_live_page_with_the_late_ai_overview_on_google_only() {
    let market = Market::new("tr", "tr");
    let google = task(&query(&market, Engine::Google)).unwrap();
    assert_eq!(google["keyword"], "spor ayakkabı");
    assert_eq!(google["location_name"], "Turkey");
    assert_eq!(google["language_code"], "tr");
    assert_eq!(google["depth"], 20);
    assert_eq!(google["device"], "desktop");
    assert_eq!(google["load_async_ai_overview"], true);

    let mut city = query(&market, Engine::Yandex);
    city.location = Some("Istanbul,Istanbul,Turkey");
    city.device = Device::Mobile;
    let yandex = task(&city).unwrap();
    assert_eq!(yandex["location_name"], "Istanbul,Istanbul,Turkey");
    assert_eq!(yandex["device"], "mobile");
    assert!(yandex.get("load_async_ai_overview").is_none());

    let mut empty = query(&market, Engine::Google);
    empty.keyword = "   ";
    assert!(task(&empty).is_err());
}

#[test]
fn a_check_finds_the_sites_pages_snippet_and_ai_citation() {
    let check = rank_from_result(&result(), "example.com");
    assert_eq!(check.position, Some(2));
    assert_eq!(
        check.url.as_deref(),
        Some("https://www.example.com/spor-ayakkabi")
    );
    // A subdomain is the site too: two pages rank.
    assert_eq!(
        check.pages,
        vec![
            (2, "https://www.example.com/spor-ayakkabi".to_owned()),
            (4, "https://shop.example.com/kategori/spor".to_owned())
        ]
    );
    assert!(check.snippet);
    assert!(check.ai_overview);
    assert!(check.ai_cited, "cited through blog.example.com");
    assert_eq!(check.total, Some(4_200_000));
    assert!(check.features.contains(&"local_pack".to_owned()));
    let domains: Vec<&str> = check
        .results
        .iter()
        .map(|result| result.domain.as_str())
        .collect();
    assert_eq!(
        domains,
        [
            "rival.com",
            "example.com",
            "other.com.tr",
            "shop.example.com",
            "notexample.com"
        ]
    );

    let rival = rank_from_result(&result(), "https://www.rival.com/");
    assert_eq!(rival.position, Some(1));
    assert!(rival.ai_cited);
    assert!(!rival.snippet);

    let absent = rank_from_result(&result(), "example.org");
    assert_eq!(absent.position, None);
    assert_eq!(absent.pages, [] as [(u16, String); 0]);
    assert!(!absent.ai_cited);
    assert_eq!(rank_from_result(&[], "example.com"), RankCheck::default());
}
