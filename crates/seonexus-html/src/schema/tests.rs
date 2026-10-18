#![allow(clippy::unwrap_used)]
use super::*;

fn json(input: &str) -> SchemaReport {
    validate(input, true, "https://example.com/page").unwrap()
}
fn errors(report: &SchemaReport) -> usize {
    report
        .items
        .iter()
        .flat_map(|i| &i.issues)
        .filter(|i| i.severity == "error")
        .count()
}

#[test]
fn last_breadcrumb_may_omit_item_and_thing_name_can_supply_name() {
    let report = json(
        r#"{"@context":"https://schema.org","@type":"BreadcrumbList","itemListElement":[{"@type":"ListItem","position":1,"item":{"@id":"https://example.com/","name":"Home"}},{"@type":"ListItem","position":2,"name":"Page"}]}"#,
    );
    assert_eq!(errors(&report), 0);
    assert_eq!(report.items[0].status, "checked");
}
#[test]
fn breadcrumb_errors_keep_exact_object_paths_and_current_values() {
    let report = json(
        r#"{"@context":"https://schema.org","@type":"BreadcrumbList","itemListElement":[{"position":0,"name":"Home"}]}"#,
    );
    assert!(
        report.items[0]
            .issues
            .iter()
            .any(|i| i.path == "/itemListElement/0/position" && i.current.as_deref() == Some("0"))
    );
    assert!(report.items[0].issues.iter().any(|i| i.code == "min-items"));
}
#[test]
fn article_and_organization_fields_are_recommendations_not_required() {
    for kind in ["Article", "BlogPosting", "NewsArticle", "Organization"] {
        let report = json(&format!(
            r#"{{"@context":"https://schema.org","@type":"{kind}"}}"#
        ));
        assert_eq!(errors(&report), 0, "{kind}");
        assert_eq!(report.items[0].status, "warning");
    }
}
#[test]
fn product_does_not_require_offers_when_review_branch_exists() {
    let report = json(
        r#"{"@context":"https://schema.org","@type":"Product","name":"Test","review":{"@type":"Review"}}"#,
    );
    assert_eq!(errors(&report), 0);
    assert_eq!(report.items[0].status, "unchecked");
}
#[test]
fn free_offer_and_price_specification_are_supported_without_requiring_currency() {
    let report = json(
        r#"{"@context":"https://schema.org","@type":"Product","name":"Free","image":"https://example.com/i.png","offers":{"@type":"Offer","priceSpecification":{"price":"0"}}}"#,
    );
    assert_eq!(errors(&report), 0);
    assert!(
        report.items[0]
            .issues
            .iter()
            .any(|i| i.path == "/offers/priceCurrency" && i.severity == "warning")
    );
}
#[test]
fn graph_references_work_across_script_blocks_and_nested_entities_are_discovered() {
    let report = validate(r##"<script type="application/ld+json">{"@context":"https://schema.org","@graph":[{"@type":"Product","@id":"#product","name":"Test","offers":{"@id":"#offer"}}]}</script><script type="application/ld+json">{"@context":"https://schema.org","@type":"Offer","@id":"#offer","price":12,"priceCurrency":"TRY"}</script>"##, false, "https://example.com/page").unwrap();
    assert_eq!(report.jsonld_blocks, 2);
    assert_eq!(errors(&report), 0);
    assert!(report.items.iter().any(|i| i.types == ["Offer"]));
    assert!(
        !report.items[0]
            .issues
            .iter()
            .any(|i| i.code == "unresolved-reference")
    );
}
#[test]
fn unresolved_or_conflicting_ids_are_unverified_not_missing_property_errors() {
    for input in [
        r##"{"@context":"https://schema.org","@type":"Product","name":"Test","offers":{"@id":"#missing"}}"##,
        r##"{"@context":"https://schema.org","@graph":[{"@type":"Product","name":"Test","offers":{"@id":"#offer"}},{"@id":"#offer","@type":"Offer","price":1},{"@id":"#offer","@type":"Offer","price":2}]}"##,
    ] {
        let report = json(input);
        assert_eq!(errors(&report), 0);
        assert_eq!(report.items[0].status, "unchecked");
    }
}
#[test]
fn remote_context_aliases_and_unknown_types_do_not_get_invented_errors() {
    for input in [
        r#"{"@context":"https://example.com/context","@type":"Product"}"#,
        r#"{"@context":{"@vocab":"https://schema.org/","n":"name"},"@type":"Product","n":"Example"}"#,
        r#"{"@context":"https://schema.org","@type":"Service"}"#,
    ] {
        let report = json(input);
        assert_eq!(errors(&report), 0);
        assert_eq!(report.items[0].status, "unchecked");
    }
}
#[test]
fn malformed_json_and_scalar_roots_are_not_validated_as_schemas() {
    for input in [r#"{"@type":"Product",}"#, "true", "null"] {
        let report = json(input);
        assert_eq!(errors(&report), 1);
        assert_eq!(report.items[0].issues[0].code, "json-syntax");
    }
}
#[test]
fn templates_are_ignored_and_microdata_is_explicitly_unverified() {
    let report = validate(r#"<template><script type="application/ld+json">broken</script></template><div itemscope itemtype="https://schema.org/Product"></div>"#, false, "https://example.com").unwrap();
    assert_eq!(report.jsonld_blocks, 0);
    assert_eq!(errors(&report), 0);
    assert_eq!(report.items[0].format, "microdata");
    assert_eq!(report.items[0].status, "unchecked");
}
#[test]
fn no_markup_does_not_become_a_required_schema_error_and_input_is_bounded() {
    let report = validate("<html><p>hello</p></html>", false, "https://example.com").unwrap();
    assert_eq!(report.items.len(), 0);
    assert!(validate(&" ".repeat(INPUT_LIMIT + 1), false, "https://example.com").is_err());
}

#[test]
fn advanced_jsonld_and_foreign_nested_contexts_remain_unverified() {
    for input in [
        r#"{"@context":"https://schema.org","@type":"Product","name":{"@set":["Test"]}}"#,
        r#"{"@context":"https://schema.org","@type":"Product","name":"Test","offers":{"@context":"https://example.com/context","@type":"Offer"}}"#,
    ] {
        let report = json(input);
        assert_eq!(errors(&report), 0);
        assert_eq!(report.items[0].status, "unchecked");
    }
}

#[test]
fn referenced_price_specifications_do_not_invent_missing_prices() {
    for input in [
        r##"{"@context":"https://schema.org","@graph":[{"@type":"Product","name":"Test","offers":{"@type":"Offer","priceSpecification":{"@id":"#price"}}},{"@id":"#price","@type":"PriceSpecification","price":0,"priceCurrency":"TRY"}]}"##,
        r##"{"@context":"https://schema.org","@type":"Product","name":"Test","offers":{"@type":"Offer","priceSpecification":{"@id":"#unresolved"}}}"##,
    ] {
        let report = json(input);
        assert_eq!(errors(&report), 0);
    }
}
