//! Site Explorer reports read from answers shaped like DataForSEO's documented examples.

use serde_json::{Value, json};

use crate::site::*;
use crate::{DfsError, Market};

fn result(fixture: &str) -> Vec<Value> {
    let answer: Value = serde_json::from_str(fixture).unwrap();
    answer["tasks"][0]["result"].as_array().unwrap().clone()
}

fn market() -> Market {
    Market::new("tr", "tr")
}

#[test]
fn history_is_read_oldest_first() {
    let points = history_from_result(&result(include_str!(
        "../tests/fixtures/historical_rank_overview.json"
    )));
    let months: Vec<(u32, u32)> = points
        .iter()
        .map(|point| (point.year, point.month))
        .collect();
    assert_eq!(months, [(2026, 7), (2026, 8), (2026, 9)]);
    assert_eq!(points[2].organic.keywords, 3000);
    assert!((points[2].organic.traffic - 4200.5).abs() < 1e-9);
    // A month without paid results reads as zeros.
    assert_eq!(points[0].paid.keywords, 0);
}

#[test]
fn history_starts_a_fixed_number_of_months_back() {
    assert_eq!(months_before(2026, 10, 24), "2024-10-01");
    assert_eq!(months_before(2026, 1, 1), "2025-12-01");
    assert_eq!(months_before(2021, 3, 24), "2020-10-01");
    let task = history_task("https://www.example.com/", &market(), "2024-10-01").unwrap();
    assert_eq!(
        task,
        json!({ "target": "example.com", "location_code": 2792, "language_code": "tr", "date_from": "2024-10-01" })
    );
}

#[test]
fn ranked_keywords_carry_position_change_metrics_and_features() {
    let ranked = ranked_keywords_from_result(&result(include_str!(
        "../tests/fixtures/ranked_keywords.json"
    )));
    assert_eq!(ranked.total, 3052);
    assert_eq!(ranked.keywords.len(), 3);
    let first = &ranked.keywords[0];
    assert_eq!(first.keyword, "örnek ayakkabı");
    assert_eq!(first.position, Some(1));
    assert_eq!(first.absolute_position, Some(2));
    assert_eq!(first.previous_position, Some(4));
    assert_eq!(first.change, Change::Up);
    assert_eq!(first.volume, Some(2900));
    assert_eq!(first.difficulty, Some(34));
    assert_eq!(first.intent.as_deref(), Some("transactional"));
    assert_eq!(
        first.url.as_deref(),
        Some("https://www.example.com/ayakkabi")
    );
    assert_eq!(first.serp_features, ["people_also_ask", "ai_overview"]);
    assert!((first.traffic - 880.0).abs() < 1e-9);
    assert_eq!(ranked.keywords[1].change, Change::New);
    assert_eq!(ranked.keywords[1].previous_position, None);
    assert_eq!(ranked.keywords[2].change, Change::Down);
}

#[test]
fn report_tasks_ask_for_the_right_order_and_rows() {
    let keywords = ranked_keywords_task("example.com", &market(), 5_000).unwrap();
    assert_eq!(keywords["limit"], 1_000);
    assert_eq!(keywords["item_types"], json!(["organic"]));
    assert_eq!(
        keywords["order_by"],
        json!(["ranked_serp_element.serp_item.etv,desc"])
    );
    let pages = pages_task("example.com", &market(), 1).unwrap();
    assert_eq!(pages["limit"], 10);
    assert_eq!(pages["order_by"], json!(["metrics.organic.etv,desc"]));
    let competitors = competitors_task("example.com", &market(), 100).unwrap();
    assert_eq!(competitors["exclude_top_domains"], true);
    assert_eq!(
        competitors["order_by"],
        json!(["metrics.organic.count,desc"])
    );
    let subdomains = subdomains_task("example.com", &market(), 100).unwrap();
    assert_eq!(subdomains["target"], "example.com");
    assert!(matches!(
        ranked_keywords_task(" ", &market(), 100),
        Err(DfsError::Config(_))
    ));
}

#[test]
fn pages_competitors_and_subdomains_are_read() {
    let pages = pages_from_result(&result(include_str!(
        "../tests/fixtures/relevant_pages.json"
    )));
    assert_eq!(pages.len(), 2);
    assert_eq!(pages[0].url, "https://www.example.com/ayakkabi");
    assert_eq!(pages[0].organic.keywords, 120);
    assert_eq!(pages[0].organic.pos_1, 12);

    let competitors = competitors_from_result(
        "www.example.com",
        &result(include_str!("../tests/fixtures/competitors_domain.json")),
    );
    // The domain itself is not its own competitor.
    let names: Vec<&str> = competitors.iter().map(|row| row.domain.as_str()).collect();
    assert_eq!(names, ["rival.com", "other.com.tr"]);
    assert_eq!(competitors[0].shared_keywords, 910);
    assert_eq!(competitors[0].keywords, 5100);
    assert!((competitors[0].traffic - 8800.0).abs() < 1e-9);
    assert!((competitors[0].shared_traffic - 1400.0).abs() < 1e-9);
    assert_eq!(competitors[0].average_position, Some(11.4));

    let subdomains =
        subdomains_from_result(&result(include_str!("../tests/fixtures/subdomains.json")));
    assert_eq!(subdomains[1].subdomain, "blog.example.com");
    assert_eq!(subdomains[1].organic.keywords, 150);
}

#[test]
fn several_domains_are_compared_once_each() {
    let task = bulk_traffic_task(
        &[
            "https://www.example.com/".to_owned(),
            "example.com".to_owned(),
            "rival.com".to_owned(),
            " ".to_owned(),
        ],
        &market(),
    )
    .unwrap();
    assert_eq!(task["targets"], json!(["example.com", "rival.com"]));
    assert!(matches!(
        bulk_traffic_task(&[], &market()),
        Err(DfsError::Config(_))
    ));
    let many: Vec<String> = (0..1_100).map(|n| format!("site{n}.com")).collect();
    assert_eq!(
        bulk_traffic_task(&many, &market()).unwrap()["targets"]
            .as_array()
            .unwrap()
            .len(),
        MAX_COMPARED
    );

    let rows = bulk_traffic_from_result(&result(include_str!(
        "../tests/fixtures/bulk_traffic_estimation.json"
    )));
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].organic_keywords, 3052);
    assert!((rows[0].paid_traffic - 120.5).abs() < 1e-9);
    // A domain without paid results reads as zero.
    assert_eq!(rows[1].paid_keywords, 0);
}
