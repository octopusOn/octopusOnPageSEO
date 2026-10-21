//! Keyword Explorer answers read from data shaped like DataForSEO's documented examples.

use serde_json::{Value, json};

use crate::keywords::*;
use crate::serp::{organic_from_result, organic_task};
use crate::{DfsError, Market};

fn result(fixture: &str) -> Vec<Value> {
    let answer: Value = serde_json::from_str(fixture).unwrap();
    answer["tasks"][0]["result"].as_array().unwrap().clone()
}

fn market() -> Market {
    Market::new("tr", "tr")
}

#[test]
fn keyword_numbers_are_read_with_months_oldest_first() {
    let list = list_from_result(&result(include_str!(
        "../tests/fixtures/keyword_overview.json"
    )));
    assert_eq!(list.total, 2);
    assert_eq!(list.seed, None);
    let first = &list.keywords[0];
    assert_eq!(first.keyword, "spor ayakkabı");
    assert_eq!(first.volume, Some(2900));
    assert_eq!(first.difficulty, Some(62));
    assert_eq!(first.cpc, Some(0.82));
    assert_eq!(first.competition_level.as_deref(), Some("MEDIUM"));
    assert_eq!(first.intent.as_deref(), Some("commercial"));
    assert_eq!(first.other_intents, ["transactional"]);
    let months: Vec<(u32, u32)> = first.monthly.iter().map(|m| (m.year, m.month)).collect();
    assert_eq!(months, [(2026, 7), (2026, 8), (2026, 9)]);
    assert_eq!(first.trend.yearly, Some(-4));
    assert_eq!(
        first.serp_features,
        ["people_also_ask", "images", "ai_overview"]
    );
    assert_eq!(first.results, Some(4_200_000));
    assert_eq!(first.top_referring_domains, Some(96));
}

#[test]
fn suggestions_carry_the_seed_and_related_keywords_their_nested_data() {
    let suggestions = list_from_result(&result(include_str!(
        "../tests/fixtures/keyword_suggestions.json"
    )));
    assert_eq!(suggestions.total, 5210);
    assert_eq!(suggestions.seed.as_ref().unwrap().volume, Some(90_500));
    assert_eq!(
        suggestions.keywords[0].keyword,
        "ayakkabı numarası nasıl ölçülür"
    );

    let related = list_from_result(&result(include_str!(
        "../tests/fixtures/related_keywords.json"
    )));
    assert_eq!(related.total, 72);
    let names: Vec<&str> = related
        .keywords
        .iter()
        .map(|k| k.keyword.as_str())
        .collect();
    assert_eq!(names, ["spor ayakkabı", "bot"]);
    assert_eq!(related.keywords[1].volume, Some(14_800));
}

#[test]
fn keywords_are_cleaned_before_asking() {
    let clean = clean_keywords(
        &[
            "  Spor   Ayakkabı ".to_owned(),
            "spor ayakkabı".to_owned(),
            String::new(),
            "Bot".to_owned(),
        ],
        10,
    )
    .unwrap();
    assert_eq!(clean, ["spor ayakkabı", "bot"]);
    assert!(matches!(
        clean_keywords(&[" ".to_owned()], 10),
        Err(DfsError::Config(_))
    ));
    assert!(matches!(
        clean_keywords(&["x".repeat(81)], 10),
        Err(DfsError::Config(_))
    ));
    let many: Vec<String> = (0..800).map(|n| format!("kelime {n}")).collect();
    let task = overview_task(&many, &market()).unwrap();
    assert_eq!(task["keywords"].as_array().unwrap().len(), MAX_OVERVIEW);
    assert_eq!(task["include_serp_info"], true);
}

#[test]
fn idea_tasks_ask_most_searched_first() {
    let ideas = ideas_task(&["Ayakkabı".to_owned()], &market(), 500).unwrap();
    assert_eq!(ideas["keywords"], json!(["ayakkabı"]));
    assert_eq!(ideas["limit"], 500);
    assert_eq!(
        ideas["order_by"],
        json!(["keyword_info.search_volume,desc"])
    );
    let related = related_task("ayakkabı", &market(), 9, 100).unwrap();
    assert_eq!(related["depth"], 4);
    assert_eq!(related["include_seed_keyword"], true);
    assert_eq!(
        related["order_by"],
        json!(["keyword_data.keyword_info.search_volume,desc"])
    );
    let suggestions = suggestions_task("ayakkabı", &market(), 100).unwrap();
    assert_eq!(suggestions["keyword"], "ayakkabı");
    assert!(matches!(
        suggestions_task(" ", &market(), 100),
        Err(DfsError::Config(_))
    ));
}

#[test]
fn a_result_page_splits_organic_results_from_other_elements() {
    let page = organic_from_result(&result(include_str!("../tests/fixtures/serp_organic.json")));
    assert_eq!(page.keyword, "spor ayakkabı");
    assert_eq!(page.results, Some(4_200_000));
    assert_eq!(
        page.features,
        ["ai_overview", "organic", "people_also_ask", "images"]
    );
    let domains: Vec<&str> = page
        .organic
        .iter()
        .filter_map(|r| r.domain.as_deref())
        .collect();
    assert_eq!(domains, ["www.example.com", "www.rival.com"]);
    assert_eq!(page.organic[0].absolute, Some(2));
    let other: Vec<&str> = page.other.iter().map(|r| r.kind.as_str()).collect();
    assert_eq!(other, ["ai_overview", "people_also_ask"]);
    assert!(
        page.check_url
            .unwrap()
            .starts_with("https://www.google.com.tr/")
    );

    let task = organic_task("  spor  ayakkabı ", &market(), 500, true).unwrap();
    assert_eq!(task["keyword"], "spor ayakkabı");
    assert_eq!(task["depth"], 100);
    assert_eq!(task["device"], "mobile");
}
