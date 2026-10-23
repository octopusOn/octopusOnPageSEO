//! Market reports read from answers shaped like DataForSEO's documented examples.

use serde_json::Value;

use crate::Market;
use crate::market::*;

fn result(fixture: &str) -> Vec<Value> {
    let answer: Value = serde_json::from_str(fixture).unwrap();
    answer["tasks"][0]["result"].as_array().unwrap().clone()
}

fn market() -> Market {
    Market::new("tr", "tr")
}

#[test]
fn traffic_history_reads_each_domain_oldest_month_first() {
    let task = traffic_history_task(
        &[
            "https://www.example.com/".to_owned(),
            "rival.com".to_owned(),
            "example.com".to_owned(),
        ],
        &market(),
        "2019-01-01",
    )
    .unwrap();
    assert_eq!(
        task["targets"],
        serde_json::json!(["example.com", "rival.com"])
    );
    assert_eq!(task["date_from"], "2020-10-01");
    assert!(traffic_history_task(&[], &market(), "2025-01-01").is_err());
    let history = traffic_history_from_result(&result(include_str!(
        "../tests/fixtures/historical_bulk_traffic_estimation.json"
    )));
    assert_eq!(history[1].target, "rival.com");
    assert_eq!(
        history[0]
            .months
            .iter()
            .map(|month| month.month)
            .collect::<Vec<_>>(),
        [7, 8, 9]
    );
    assert!((traffic_history_estimate(2) - (0.12 + 0.0012 * 2.0)).abs() < 1e-9);
}

#[test]
fn categories_and_their_players() {
    let categories = categories_from_result(&result(include_str!(
        "../tests/fixtures/categories_for_domain.json"
    )));
    assert_eq!(categories[0].code, 10_885);
    assert_eq!(categories[0].path, [10_021, 10_178, 10_885]);
    assert_eq!(categories[1].keywords, 920);
    let task = players_task(
        &[10_885, 1, 2, 3, 4, 5, 6],
        &market(),
        "2026-03-01",
        "2026-09-01",
    )
    .unwrap();
    assert_eq!(task["category_codes"].as_array().unwrap().len(), 5);
    assert!(players_task(&[], &market(), "2026-03-01", "2026-09-01").is_err());
    let players = players_from_result(&result(include_str!(
        "../tests/fixtures/domain_metrics_by_categories.json"
    )));
    assert_eq!(players[0].domain, "rival.com");
    assert_eq!(players[1].traffic_change, Some(-120.0));
    let names = category_names_from_result(&result(include_str!(
        "../tests/fixtures/dataforseo_labs_categories.json"
    )));
    assert_eq!(names[2].name, "Athletic Shoes");
    assert_eq!(names[2].parent, Some(10_178));
    assert_eq!(names[0].parent, None);
}
