//! List queries written into tasks.

use serde_json::json;

use crate::query::*;

#[allow(clippy::needless_pass_by_value)] // takes `json!` literals
fn query(filters: serde_json::Value, order: serde_json::Value, offset: u32) -> ListQuery {
    serde_json::from_value(json!({ "filters": filters, "order": order, "offset": offset })).unwrap()
}

#[test]
fn an_empty_query_leaves_the_task_as_it_was() {
    let mut task = json!({ "target": "ornek.com", "limit": 100, "order_by": ["ranked_serp_element.serp_item.etv,desc"] });
    let before = task.clone();
    apply(&mut task, &ListQuery::default(), RANKED_KEYWORD_FIELDS).unwrap();
    assert_eq!(task, before);
}

#[test]
fn filters_order_and_offset_are_written_the_same_whatever_their_order() {
    let mut task = json!({ "target": "ornek.com", "limit": 100 });
    let asked = query(
        json!([
            { "field": "volume", "op": "ge", "value": 100 },
            { "field": "keyword", "op": "contains", "value": " ayakkabı " },
            { "field": "intent", "op": "in", "value": ["commercial", "transactional"] },
            { "field": "features", "op": "has", "value": "ai_overview" },
            { "field": "new", "op": "eq", "value": true }
        ]),
        json!([{ "field": "volume", "direction": "desc" }]),
        200,
    );
    apply(&mut task, &asked, RANKED_KEYWORD_FIELDS).unwrap();
    assert_eq!(task["offset"], 200);
    assert_eq!(
        task["order_by"],
        json!(["keyword_data.keyword_info.search_volume,desc"])
    );
    let filters = task["filters"].as_array().unwrap();
    assert_eq!(filters.len(), 9, "five conditions joined by four `and`s");
    assert!(filters.contains(&json!(["keyword_data.keyword", "ilike", "%ayakkabı%"])));
    assert!(filters.contains(&json!([
        "keyword_data.serp_info.serp_item_types",
        "has",
        "ai_overview"
    ])));
    assert_eq!(filters[1], "and");

    let mut shuffled = json!({ "target": "ornek.com", "limit": 100 });
    let mut reversed = asked.clone();
    reversed.filters.reverse();
    apply(&mut shuffled, &reversed, RANKED_KEYWORD_FIELDS).unwrap();
    assert_eq!(shuffled, task, "the same task, so the same cache key");
}

#[test]
fn the_task_s_own_filters_stay_and_one_filter_is_written_alone() {
    let mut task = json!({ "target": "ornek.com", "filters": ["dofollow", "=", true] });
    apply(
        &mut task,
        &query(
            json!([{ "field": "spam", "op": "le", "value": 30 }]),
            json!([]),
            0,
        ),
        BACKLINK_FIELDS,
    )
    .unwrap();
    assert_eq!(
        task["filters"],
        json!([
            ["dofollow", "=", true],
            "and",
            ["backlink_spam_score", "<=", 30]
        ])
    );
    let mut alone = json!({ "target": "ornek.com" });
    apply(
        &mut alone,
        &query(
            json!([{ "field": "rank", "op": "gt", "value": 20 }]),
            json!([]),
            0,
        ),
        REFERRING_DOMAIN_FIELDS,
    )
    .unwrap();
    assert_eq!(alone["filters"], json!(["rank", ">", 20]));
}

#[test]
fn bad_queries_are_refused() {
    let mut task = json!({});
    for (filters, order) in [
        (
            json!([{ "field": "weather", "op": "eq", "value": 1 }]),
            json!([]),
        ),
        (
            json!([{ "field": "volume", "op": "contains", "value": "1" }]),
            json!([]),
        ),
        (
            json!([{ "field": "volume", "op": "ge", "value": "many" }]),
            json!([]),
        ),
        (
            json!([{ "field": "keyword", "op": "contains", "value": "  " }]),
            json!([]),
        ),
        (
            json!([{ "field": "intent", "op": "in", "value": [] }]),
            json!([]),
        ),
        (
            json!([]),
            json!([{ "field": "features", "direction": "asc" }]),
        ),
    ] {
        assert!(apply(&mut task, &query(filters, order, 0), KEYWORD_FIELDS).is_err());
    }
    let many: Vec<serde_json::Value> = (0..9)
        .map(|n| json!({ "field": "volume", "op": "ge", "value": n }))
        .collect();
    assert!(apply(&mut task, &query(json!(many), json!([]), 0), KEYWORD_FIELDS).is_err());
}
