//! Keyword and link gaps merged from intersection answers shaped like DataForSEO's examples.

use serde_json::{Value, json};

use crate::gap::*;
use crate::{DfsError, Market};

fn result(fixture: &str) -> Vec<Value> {
    let answer: Value = serde_json::from_str(fixture).unwrap();
    answer["tasks"][0]["result"].as_array().unwrap().clone()
}

#[test]
fn missing_keywords_merge_competitors_most_shared_first() {
    let answers = vec![
        (
            "rival.com".to_owned(),
            result(include_str!(
                "../tests/fixtures/intersection_missing_rival.json"
            )),
        ),
        (
            "other.com.tr".to_owned(),
            result(include_str!(
                "../tests/fixtures/intersection_missing_other.json"
            )),
        ),
    ];
    let gap = missing("www.example.com", &answers);
    assert_eq!(gap.own, "example.com");
    assert_eq!(gap.competitors, ["rival.com", "other.com.tr"]);
    let names: Vec<&str> = gap.keywords.iter().map(|k| k.keyword.as_str()).collect();
    assert_eq!(names, ["koşu ayakkabısı", "trail ayakkabı"]);
    let first = &gap.keywords[0];
    assert_eq!(first.own, None);
    // Competitors best ranked first.
    let ranked: Vec<(&str, Option<u32>)> = first
        .competitors
        .iter()
        .map(|r| (r.domain.as_str(), r.position))
        .collect();
    assert_eq!(ranked, [("other.com.tr", Some(1)), ("rival.com", Some(3))]);
    assert_eq!(first.volume, Some(14_800));
    assert_eq!(first.difficulty, Some(51));
}

#[test]
fn shared_keywords_mark_where_a_competitor_ranks_higher() {
    let answers = vec![(
        "rival.com".to_owned(),
        result(include_str!(
            "../tests/fixtures/intersection_shared_rival.json"
        )),
    )];
    let gap = shared("example.com", &answers);
    let spor = gap
        .keywords
        .iter()
        .find(|k| k.keyword == "spor ayakkabı")
        .unwrap();
    assert_eq!(spor.own.as_ref().unwrap().position, Some(7));
    assert_eq!(spor.competitors[0].position, Some(2));
    assert!(spor.weak);
    let beyaz = gap
        .keywords
        .iter()
        .find(|k| k.keyword == "beyaz ayakkabı")
        .unwrap();
    assert!(!beyaz.weak);
}

#[test]
fn gap_tasks_and_competitors_are_cleaned() {
    let competitors = competitors(
        "example.com",
        &[
            "https://www.rival.com/".to_owned(),
            "rival.com".to_owned(),
            "example.com".to_owned(),
            "a.com".to_owned(),
            "b.com".to_owned(),
            "c.com".to_owned(),
            "d.com".to_owned(),
            "e.com".to_owned(),
        ],
    )
    .unwrap();
    assert_eq!(
        competitors,
        ["rival.com", "a.com", "b.com", "c.com", "d.com"]
    );
    assert!(matches!(
        super::gap::competitors("example.com", &["example.com".to_owned()]),
        Err(DfsError::Config(_))
    ));

    let task = keyword_intersection_task(
        "rival.com",
        "www.example.com",
        &Market::new("tr", "tr"),
        false,
        100,
    )
    .unwrap();
    assert_eq!(task["target1"], "rival.com");
    assert_eq!(task["target2"], "example.com");
    assert_eq!(task["intersections"], false);
    assert_eq!(
        task["order_by"],
        json!(["keyword_data.keyword_info.search_volume,desc"])
    );

    let links = link_gap_task(
        "example.com",
        &["rival.com".to_owned(), "other.com.tr".to_owned()],
        100,
    )
    .unwrap();
    assert_eq!(
        links["targets"],
        json!({ "1": "rival.com", "2": "other.com.tr" })
    );
    assert_eq!(links["exclude_targets"], json!(["example.com"]));
    assert_eq!(links["rank_scale"], "one_hundred");
}

#[test]
fn link_gaps_count_backlinks_per_competitor() {
    let rows = link_gap_from_result(
        &["rival.com".to_owned(), "other.com.tr".to_owned()],
        &result(include_str!(
            "../tests/fixtures/backlinks_domain_intersection.json"
        )),
    );
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].domain, "haber.com.tr");
    assert_eq!(rows[0].rank, Some(72));
    assert_eq!(rows[0].backlinks["rival.com"], 12);
    assert_eq!(rows[0].backlinks["other.com.tr"], 3);
}
