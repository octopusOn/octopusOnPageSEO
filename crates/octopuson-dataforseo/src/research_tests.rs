//! Keyword research reports read from answers shaped like DataForSEO's documented examples.

use serde_json::{Value, json};

use crate::research::*;
use crate::{DfsError, Market};

fn result(fixture: &str) -> Vec<Value> {
    let answer: Value = serde_json::from_str(fixture).unwrap();
    answer["tasks"][0]["result"].as_array().unwrap().clone()
}

fn market() -> Market {
    Market::new("tr", "tr")
}

#[test]
fn search_history_joins_the_monthly_snapshots() {
    assert_eq!(
        keyword_history_task(&[" Spor  Ayakkabı ".to_owned()], &market()).unwrap(),
        json!({ "location_code": 2792, "language_code": "tr", "keywords": ["spor ayakkabı"] })
    );
    let history = keyword_history_from_result(&result(include_str!(
        "../tests/fixtures/historical_keyword_data.json"
    )));
    assert_eq!(history.len(), 1);
    let months: Vec<(u32, u32, u64)> = history[0]
        .months
        .iter()
        .map(|month| (month.year, month.month, month.volume))
        .collect();
    // July 2026 is in two snapshots: the later one's figure stays; the oldest months come first.
    assert_eq!(
        months,
        [
            (2021, 7, 21_000),
            (2021, 8, 22_000),
            (2026, 6, 35_000),
            (2026, 7, 40_000),
            (2026, 8, 39_000),
            (2026, 9, 41_000)
        ]
    );
    assert!((keyword_history_estimate(1) - 0.012_12).abs() < 1e-9);
}

#[test]
fn serp_history_reads_each_page_oldest_first() {
    let task = serp_history_task("Spor ayakkabı", &market(), "2026-09-01").unwrap();
    assert_eq!(task["keyword"], "spor ayakkabı");
    assert_eq!(task["date_from"], "2026-09-01");
    let fixture = result(include_str!("../tests/fixtures/historical_serps.json"));
    let pages = serp_history_from_result(&fixture);
    assert_eq!(
        pages
            .iter()
            .map(|page| page.date.as_str())
            .collect::<Vec<_>>(),
        ["2026-09-03", "2026-10-04"]
    );
    let latest = &pages[1];
    assert_eq!(latest.results[0].rank, 1);
    assert_eq!(
        latest.results[0].url,
        "https://www.ornekmagaza.com/spor-ayakkabi"
    );
    assert_eq!(latest.features, ["people_also_ask", "ai_overview"]);
    assert_eq!(
        latest_results(&fixture).unwrap(),
        [
            "https://www.ornekmagaza.com/spor-ayakkabi",
            "https://www.rakip-magaza.com/spor",
            "https://www.digerdukkan.com.tr/a"
        ]
    );
    assert_eq!(latest_results(&[]), None);
    // A price per page and none per task: two pages a month at most.
    assert!((serp_history_estimate(12) - 0.000_12 * 24.0).abs() < 1e-12);
}

#[test]
fn a_page_s_keywords_and_reach() {
    let task = page_keywords_task("https://www.example.com/ayakkabi", &market(), 100).unwrap();
    assert_eq!(task["target"], "https://www.example.com/ayakkabi");
    assert!(matches!(
        page_keywords_task("example.com/ayakkabi", &market(), 100),
        Err(DfsError::Config(_))
    ));
    let reach = page_reach(&result(include_str!(
        "../tests/fixtures/ranked_keywords.json"
    )));
    assert_eq!(reach.keywords, 3052);
    assert!((reach.traffic - 4256.6).abs() < 1e-9);
    assert_eq!(page_reach(&[]), PageReach::default());
}

#[test]
fn pages_are_weighed_at_once() {
    let targets = [
        "https://www.ornekmagaza.com/spor-ayakkabi".to_owned(),
        "https://www.rakip-magaza.com/".to_owned(),
        "rakip-magaza.com".to_owned(),
    ];
    assert_eq!(
        bulk_referring_domains_task(&targets).unwrap()["targets"],
        json!([
            "https://www.ornekmagaza.com/spor-ayakkabi",
            "rakip-magaza.com"
        ])
    );
    assert_eq!(
        bulk_page_traffic_task(&targets, &market()).unwrap()["targets"],
        json!([
            "https://www.ornekmagaza.com/spor-ayakkabi",
            "https://www.rakip-magaza.com/",
            "rakip-magaza.com"
        ])
    );
    assert!(bulk_referring_domains_task(&[]).is_err());
    let rows = bulk_referring_domains_from_result(&result(include_str!(
        "../tests/fixtures/backlinks_bulk_referring_domains.json"
    )));
    assert_eq!(rows[0].referring_domains, 48);
    assert_eq!(rows[1].referring_main_domains, 2900);
}
