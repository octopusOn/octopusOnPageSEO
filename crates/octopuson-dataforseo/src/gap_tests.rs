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

#[test]
fn every_keyword_falls_in_semrush_s_groups() {
    let groups = |own: Option<u32>, theirs: &[Option<u32>]| gap_groups(own, theirs);
    assert_eq!(groups(Some(3), &[Some(1), Some(2)]), ["shared", "weak"]);
    assert_eq!(groups(Some(1), &[Some(4), Some(2)]), ["shared", "strong"]);
    assert_eq!(groups(Some(2), &[Some(1), Some(5)]), ["shared"]);
    assert_eq!(groups(Some(2), &[Some(5), None]), ["strong"]);
    assert_eq!(groups(Some(2), &[None, None]), ["unique"]);
    assert_eq!(groups(None, &[Some(1), Some(9)]), ["missing"]);
    assert_eq!(groups(None, &[Some(1), None]), ["untapped"]);
    assert_eq!(groups(None, &[None, None]).len(), 0);
}

#[test]
fn the_matrix_joins_each_domain_s_keywords() {
    let own = result(include_str!("../tests/fixtures/ranked_keywords.json"));
    // The competitor ranks third for the site's first keyword and has one of its own.
    let mut theirs: Value =
        serde_json::from_str(include_str!("../tests/fixtures/ranked_keywords.json")).unwrap();
    let items = theirs["tasks"][0]["result"][0]["items"]
        .as_array_mut()
        .unwrap();
    items.truncate(1);
    items[0]["ranked_serp_element"]["serp_item"]["rank_group"] = serde_json::json!(3);
    let mut extra = items[0].clone();
    extra["keyword_data"]["keyword"] = serde_json::json!("rakip özel kelime");
    items.push(extra);
    let theirs = theirs["tasks"][0]["result"].as_array().unwrap().clone();
    let gap = matrix(
        "www.example.com",
        &["rival.com".to_owned()],
        &[own, theirs],
        100,
    );
    assert_eq!(gap.own, "example.com");
    assert_eq!(gap.competitors, ["rival.com"]);
    assert_eq!(gap.keywords_per_domain, [3, 2]);
    let find = |keyword: &str| {
        gap.keywords
            .iter()
            .find(|row| row.keyword == keyword)
            .unwrap()
    };
    let shared = find("örnek ayakkabı");
    assert_eq!(shared.groups, ["shared", "strong"]);
    assert_eq!(shared.rankings[1].as_ref().unwrap().position, Some(3));
    assert_eq!(find("rakip özel kelime").groups, ["missing"]);
    assert_eq!(gap.counts["unique"], 2);
    assert_eq!(gap.counts["missing"], 1);
}

#[test]
fn a_page_gap_asks_the_competitor_s_page_without_the_site_s() {
    let task = page_gap_task(
        "https://www.rival.com/kosu",
        "https://www.example.com/kosu",
        &Market::new("tr", "tr"),
        100,
    )
    .unwrap();
    assert_eq!(task["pages"]["1"], "https://www.rival.com/kosu");
    assert_eq!(
        task["exclude_pages"],
        serde_json::json!(["https://www.example.com/kosu"])
    );
    assert!(
        page_gap_task(
            "rival.com/kosu",
            "https://www.example.com/",
            &Market::new("tr", "tr"),
            100
        )
        .is_err()
    );
    let rows = page_gap_from_result(&result(include_str!(
        "../tests/fixtures/page_intersection.json"
    )));
    assert_eq!(rows[0].keyword, "kadın koşu ayakkabısı");
    assert_eq!(rows[0].position, Some(3));
    assert!((rows[1].traffic - 38.0).abs() < 1e-9);
}
