//! AI visibility answers read from data shaped like DataForSEO's documented examples.

use serde_json::{Value, json};

use crate::Market;
use crate::ai::*;

fn result(value: Value) -> Vec<Value> {
    vec![value]
}

#[test]
fn llm_tasks_ask_with_web_search_in_the_market() {
    assert_eq!(
        llm_endpoint("claude"),
        Some("ai_optimization/claude/llm_responses/live")
    );
    assert_eq!(llm_endpoint("bard"), None);
    assert_eq!(default_model("gemini"), "gemini-2.5-flash");
    let task = llm_task(" En iyi spor ayakkabı? ", "gpt-5", Some("tr")).unwrap();
    assert_eq!(task["user_prompt"], "En iyi spor ayakkabı?");
    assert_eq!(task["model_name"], "gpt-5");
    assert_eq!(task["web_search"], true);
    assert_eq!(task["web_search_country_iso_code"], "TR");
    assert!(llm_task("  ", "gpt-5", None).is_err());
    assert!(llm_task(&"x".repeat(501), "gpt-5", None).is_err());
    assert!((llm_estimate("chatgpt") - 0.0306).abs() < 1e-9);
    assert!((mentions_estimate(100) - 0.2).abs() < 1e-9);
}

#[test]
fn llm_answers_join_sections_and_annotations() {
    let answer = llm_from_result(&result(json!({
        "model_name": "gpt-5-2025-08-07",
        "money_spent": 0.0123,
        "items": [
            { "type": "reasoning", "sections": [{ "type": "summary_text", "text": "thinking" }] },
            { "type": "message", "sections": [
                { "type": "text", "text": "Örnek Mağaza öne çıkıyor. ", "annotations": [
                    { "title": "ornekmagaza.com", "url": "https://ornekmagaza.com/spor" }
                ]},
                { "type": "text", "text": "Rakip de var.", "annotations": null }
            ]}
        ]
    })));
    assert_eq!(answer.text, "Örnek Mağaza öne çıkıyor. Rakip de var.");
    assert_eq!(
        answer.citations,
        [(
            "https://ornekmagaza.com/spor".to_owned(),
            Some("ornekmagaza.com".to_owned())
        )]
    );
    assert_eq!(answer.model.as_deref(), Some("gpt-5-2025-08-07"));
    assert_eq!(
        money_spent(&result(json!({ "money_spent": 0.0123 }))),
        Some(0.0123)
    );
}

#[test]
fn ai_mode_reads_the_overview_and_nested_references() {
    let market = Market::new("tr", "tr");
    let task = ai_mode_task("en iyi  spor ayakkabı", &market).unwrap();
    assert_eq!(task["keyword"], "en iyi spor ayakkabı");
    assert_eq!(task["location_code"], 2792);
    let answer = ai_mode_from_result(&result(json!({
        "type": "ai_mode",
        "items": [{
            "type": "ai_overview",
            "markdown": "Örnek Mağaza ve Rakip Mağaza...",
            "references": [{ "source": "Rakip", "domain": "rakip-magaza.com", "url": "https://rakip-magaza.com/a", "title": "Rakip" }],
            "items": [{ "type": "ai_overview_element", "references": [
                { "source": "Örnek", "domain": "ornekmagaza.com", "url": "https://ornekmagaza.com/b", "title": "Örnek" }
            ]}]
        }]
    })));
    assert_eq!(answer.text, "Örnek Mağaza ve Rakip Mağaza...");
    assert_eq!(answer.citations.len(), 2);
    assert_eq!(answer.citations[1].0, "https://ornekmagaza.com/b");
}

#[test]
fn mentions_read_metrics_and_questions() {
    let market = Market::new("tr", "tr");
    let task = mentions_metrics_task("https://www.ornekmagaza.com/", &market).unwrap();
    assert_eq!(task["target"][0]["domain"], "ornekmagaza.com");
    assert_eq!(task["target"][0]["include_subdomains"], true);
    let search = mentions_search_task("ornekmagaza.com", &market, 5000).unwrap();
    assert_eq!(search["limit"], 1000);
    assert_eq!(search["order_by"][0], "ai_search_volume,desc");

    let metrics = metrics_from_result(
        "ornekmagaza.com",
        &result(json!({
            "total_count": 0,
            "items": [],
            "aggregated_metrics": {
                "total": { "mentions": 42, "ai_search_volume": 18500 },
                "platform": [{ "key": "google", "mentions": 40, "ai_search_volume": 18000 }, { "key": "chat_gpt", "mentions": 2, "ai_search_volume": 500 }],
                "sources_domain": [{ "key": "rakip-magaza.com", "mentions": 12, "ai_search_volume": 4000 }],
                "brand_entities_title": []
            }
        })),
    );
    assert_eq!((metrics.mentions, metrics.ai_search_volume), (42, 18_500));
    assert_eq!(metrics.platforms[1].key, "chat_gpt");
    assert_eq!(metrics.sources[0].mentions, 12);

    let rows = mentions_from_result(&result(json!({
        "total_count": 1,
        "items": [{
            "platform": "google",
            "model_name": "google_ai_overview",
            "question": "en iyi spor ayakkabı markası",
            "answer": "x".repeat(900),
            "sources": [{ "domain": "www.ornekmagaza.com", "url": "https://www.ornekmagaza.com/", "title": "Örnek", "rank": 1 }],
            "ai_search_volume": 1300,
            "last_response_at": "2026-10-01 10:00:00 +00:00"
        }]
    })));
    assert_eq!(rows[0].question, "en iyi spor ayakkabı markası");
    assert_eq!(rows[0].answer.chars().count(), 600);
    assert_eq!(rows[0].sources[0].domain, "ornekmagaza.com");
    assert_eq!(rows[0].ai_search_volume, Some(1300));
}
