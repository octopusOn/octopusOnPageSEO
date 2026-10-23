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
    // DataForSEO's newer names say Türkiye.
    assert!(location("Ankara,Ankara,Türkiye", &market).is_ok());
    assert!(location("Berlin,Germany", &market).is_err());
    assert!(location("Turkey", &market).is_err());
    assert!(location("Ankara,Turkey", &Market::new("zz", "tr")).is_err());
}

#[test]
fn a_task_asks_one_live_page_with_the_late_ai_overview_on_google_only() {
    let market = Market::new("tr", "tr");
    let google = task(&query(&market, Engine::Google)).unwrap();
    assert_eq!(google["keyword"], "spor ayakkabı");
    assert_eq!(google["location_code"], 2792);
    assert_eq!(google.get("location_name"), None);
    assert_eq!(google["language_code"], "tr");
    assert_eq!(google["depth"], 20);
    assert_eq!(google["device"], "desktop");
    assert_eq!(google["load_async_ai_overview"], true);

    let mut city = query(&market, Engine::Yandex);
    city.location = Some("Istanbul,Istanbul,Turkey");
    city.device = Device::Mobile;
    let yandex = task(&city).unwrap();
    assert_eq!(yandex["location_name"], "Istanbul,Istanbul,Turkey");
    assert_eq!(yandex.get("location_code"), None, "one location only");
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
    // The snippet and the AI Overview citing blog.example.com show the site; the questions do not.
    assert_eq!(check.owned, ["ai_overview", "featured_snippet"]);
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
    assert_eq!(absent.owned.len(), 0);
    assert_eq!(absent.position, None);
    assert_eq!(absent.pages, [] as [(u16, String); 0]);
    assert!(!absent.ai_cited);
    assert_eq!(rank_from_result(&[], "example.com"), RankCheck::default());
}

#[test]
fn the_standard_queue_takes_tagged_tasks_for_less() {
    assert_eq!(queue_post(Engine::Google), "serp/google/organic/task_post");
    assert_eq!(queue_post(Engine::Bing), "serp/bing/organic/task_post");
    assert_eq!(
        queue_get(Engine::Yandex, "10291530-1535-0066-0000-61a3a5a1dc8b"),
        "serp/yandex/organic/task_get/advanced/10291530-1535-0066-0000-61a3a5a1dc8b"
    );
    // Two pages and the late AI Overview: 3 × $0.0006 instead of 3 × $0.002.
    assert!((queue_estimate(Engine::Google, 20, true) - 0.001_8).abs() < 1e-12);
    assert!((queue_estimate(Engine::Bing, 20, true) - 0.001_2).abs() < 1e-12);
    assert!(queue_estimate(Engine::Google, 20, true) < estimate(Engine::Google, 20, true) * 0.31);

    let market = Market::new("tr", "tr");
    let queued = queued_task(&query(&market, Engine::Google), "k12|t3").unwrap();
    assert_eq!(queued["priority"], 1);
    assert_eq!(queued["tag"], "k12|t3");
    assert_eq!(queued["keyword"], "spor ayakkabı");
    assert_eq!(queued["depth"], 20);
}

#[test]
fn posted_tasks_give_their_ids_or_why_they_were_refused() {
    let tasks: Vec<Value> = serde_json::from_str(
        r#"[
          {"id": "10291530-1535-0066-0000-61a3a5a1dc8b", "status_code": 20100, "status_message": "Task Created.", "cost": 0.0012, "data": {"tag": "k1|t1"}},
          {"id": "10291530-1535-0066-0000-61a3a5a1dc8c", "status_code": 40501, "status_message": "Invalid Field: 'depth'.", "cost": 0, "data": {"tag": "k2|t1"}},
          {"id": "../../etc", "status_code": 20100, "status_message": "Task Created.", "cost": 0.0006, "data": {"tag": "k3|t1"}}
        ]"#,
    )
    .unwrap();
    let posted = posted_from_tasks(&tasks);
    assert_eq!(posted.len(), 3);
    assert_eq!(posted[0].tag, "k1|t1");
    assert_eq!(
        posted[0].id.as_deref(),
        Some("10291530-1535-0066-0000-61a3a5a1dc8b")
    );
    assert_eq!(posted[0].error, None);
    assert!((posted[0].cost_usd - 0.0012).abs() < 1e-12);
    assert_eq!(posted[1].id, None);
    assert_eq!(
        posted[1].error.as_deref(),
        Some("40501: Invalid Field: 'depth'.")
    );
    // An id that is not one cannot go into a path.
    assert_eq!(posted[2].id, None);
    assert!(posted[2].error.is_some());
}

#[test]
fn places_may_be_postal_codes_or_coordinates() {
    let market = Market::new("tr", "tr");
    assert_eq!(
        location("34000,Istanbul,Turkey", &market).unwrap(),
        "34000,Istanbul,Turkey"
    );
    assert!(is_coordinate(" 41.0082, 28.9784 "));
    assert!(!is_coordinate("Istanbul,Istanbul,Turkey"));
    assert_eq!(
        location("41.00823456789,28.9784", &market).unwrap(),
        "41.0082346,28.9784,200"
    );
    assert_eq!(
        location("41.0082,28.9784,5000", &market).unwrap(),
        "41.0082,28.9784,5000"
    );
    assert!(location("141.0,28.9", &market).is_err());
    assert!(
        location("41.0,28.9,50", &market).is_err(),
        "radius below 199"
    );

    let mut point = query(&market, Engine::Google);
    point.location = Some("41.0082,28.9784");
    let task = task(&point).unwrap();
    assert_eq!(task["location_coordinate"], "41.0082,28.9784,200");
    assert_eq!(task.get("location_code"), None);
    assert_eq!(task.get("location_name"), None);
}
