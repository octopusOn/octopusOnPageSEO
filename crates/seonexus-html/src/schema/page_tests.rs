#![allow(clippy::unwrap_used)]
use super::*;

fn json(input: &str) -> SchemaReport {
    validate(input, true, "https://example.com/page").unwrap()
}
fn no_penalty(report: &SchemaReport) {
    assert!(
        report
            .items
            .iter()
            .flat_map(|i| &i.issues)
            .all(|i| i.severity != "error")
    );
}

#[test]
fn webpage_has_no_invented_required_or_recommended_checklist() {
    let report = json(r#"{"@context":"https://schema.org","@type":"WebPage"}"#);
    assert_eq!(report.items.len(), 1);
    assert_eq!(report.items[0].status, "checked");
    assert_eq!(report.items[0].issues.len(), 0);
}

#[test]
fn webpage_accepts_relative_urls_language_values_references_and_optional_absence() {
    for language in [
        r#""tr-TR""#,
        r#"{"@type":"Language","name":"Türkçe"}"#,
        r#"{"@id":"https://example.com/language/tr"}"#,
    ] {
        let report = json(&format!(
            r##"{{"@context":"https://schema.org","@type":"WebPage","url":["/page",{{"@id":"#page"}}],"name":{{"@value":"Sayfa","@language":"tr"}},"description":null,"datePublished":[],"inLanguage":{language},"breadcrumb":"Ana sayfa > Sayfa","isPartOf":{{"@id":"https://example.com/#site"}}}}"##
        ));
        assert_eq!(report.items[0].status, "checked");
        assert_eq!(report.items[0].issues.len(), 0);
    }
}

#[test]
fn webpage_supplied_bad_shapes_keep_values_and_paths_without_a_penalty() {
    let report = json(
        r#"{"@context":"https://schema.org","@type":"WebPage","url":42,"name":" ","inLanguage":false,"dateModified":12}"#,
    );
    no_penalty(&report);
    assert_eq!(report.items[0].status, "warning");
    assert_eq!(report.items[0].issues.len(), 4);
    assert!(
        report.items[0]
            .issues
            .iter()
            .any(|i| i.path == "/url" && i.current.as_deref() == Some("42"))
    );
    assert!(
        !report.items[0]
            .issues
            .iter()
            .any(|i| i.path == "/description")
    );
}

#[test]
fn faq_single_question_and_html_answer_are_checked_with_no_component_noise() {
    let report = json(
        r#"{"@context":"https://schema.org","@type":["WebPage","FAQPage"],"mainEntity":{"@type":"Question","name":{"@value":"Nasıl?","@language":"tr"},"acceptedAnswer":{"@type":"Answer","text":"<p>Yanıt <strong>burada</strong>.</p>"}}}"#,
    );
    assert_eq!(report.items.len(), 1);
    assert_eq!(report.items[0].profile, Some("FAQPage (Schema.org)"));
    assert_eq!(report.items[0].status, "checked");
    assert_eq!(report.items[0].issues.len(), 0);
}

#[test]
fn faq_suggested_and_multiple_answers_are_allowed_without_google_eligibility_rules() {
    let report = json(
        r#"{"@context":"https://schema.org","@type":"FAQPage","mainEntity":[{"@type":"Question","name":"Nasıl?","suggestedAnswer":[{"@type":"Answer","text":"Birinci yanıt."},{"@type":"Answer","text":"İkinci yanıt."}]}]}"#,
    );
    assert_eq!(report.items.len(), 1);
    assert_eq!(report.items[0].status, "checked");
    assert_eq!(report.items[0].issues.len(), 0);
}

#[test]
fn faq_missing_or_empty_questions_produce_one_actionable_advisory() {
    for fragment in [
        "",
        r#", "mainEntity":[]"#,
        r#", "mainEntity":42"#,
        r#", "mainEntity":null"#,
    ] {
        let report = json(&format!(
            r#"{{"@context":"https://schema.org","@type":"FAQPage"{fragment}}}"#
        ));
        no_penalty(&report);
        assert_eq!(report.items[0].issues.len(), 1, "{fragment}");
        assert_eq!(report.items[0].issues[0].code, "faq-questions");
    }
}

#[test]
fn faq_blank_text_and_empty_html_are_explained_once_in_the_parent() {
    let report = json(
        r#"{"@context":"https://schema.org","@type":"FAQPage","mainEntity":[{"@type":"Question","name":" ","acceptedAnswer":{"@type":"Answer","text":"<p> </p>"}}]}"#,
    );
    no_penalty(&report);
    assert_eq!(report.items.len(), 1);
    assert_eq!(report.items[0].issues.len(), 2);
    assert!(
        report.items[0]
            .issues
            .iter()
            .any(|i| i.path == "/mainEntity/0/acceptedAnswer/text" && i.code == "faq-answer-text")
    );
    assert_eq!(report.items[0].status, "warning");
}

#[test]
fn faq_and_breadcrumb_graph_components_work_across_blocks() {
    let report = validate(r##"<script type="application/ld+json">{"@context":"https://schema.org","@graph":[{"@type":"WebPage","url":"/page","breadcrumb":{"@id":"#crumbs"},"hasPart":{"@id":"#faq"}},{"@type":"FAQPage","@id":"#faq","mainEntity":[{"@id":"#q"}]},{"@type":"BreadcrumbList","@id":"#crumbs","itemListElement":[{"@id":"#first"},{"@type":"ListItem","position":2,"name":"Page"}]}]}</script><script type="application/ld+json">{"@context":"https://schema.org","@graph":[{"@id":"#first","@type":"ListItem","position":1,"name":"Home","item":"/"},{"@id":"#q","@type":"Question","name":"How?","acceptedAnswer":{"@id":"#answer"}},{"@id":"#answer","@type":"Answer","text":"Like this."}]}</script>"##, false, "https://example.com/page").unwrap();
    assert_eq!(report.items.len(), 3);
    assert!(report.items.iter().all(|i| i.status == "checked"));
}

#[test]
fn faq_unresolved_conflicting_or_advanced_references_remain_unknown_not_missing() {
    for definition in [
        "",
        r##",{"@id":"#q","@type":"Question","name":"One"},{"@id":"#q","@type":"Question","name":"Two"}"##,
        r##",{"@id":"#q","@type":"Question","name":{"@set":["How?"]}}"##,
        r##",{"@context":"https://example.com/context","@id":"#q","@type":"Question","name":"How?"}"##,
    ] {
        let report = json(&format!(
            r##"{{"@context":"https://schema.org","@graph":[{{"@type":"FAQPage","mainEntity":{{"@id":"#q"}}}}{definition}]}}"##
        ));
        no_penalty(&report);
        assert_eq!(report.items[0].status, "unchecked");
        assert!(report.items[0].issues.iter().all(|i| i.severity == "info"));
    }
}

#[test]
fn faq_unresolved_answers_are_not_reported_as_empty_text() {
    let report = json(
        r##"{"@context":"https://schema.org","@type":"FAQPage","mainEntity":{"@type":"Question","name":"How?","acceptedAnswer":{"@id":"#missing"}}}"##,
    );
    no_penalty(&report);
    assert_eq!(report.items.len(), 1);
    assert_eq!(report.items[0].status, "unchecked");
    assert_eq!(report.items[0].issues[0].code, "unresolved-reference");
}

#[test]
fn unrelated_components_and_unknown_types_are_not_silently_declared_checked() {
    let report = json(
        r#"{"@context":"https://schema.org","@graph":[{"@type":"WebPage"},{"@type":"Question","name":"Unrelated?"},{"@type":"Answer","text":"Unrelated."},{"@type":"ListItem","name":"Unrelated"},{"@type":"Service"}]}"#,
    );
    assert_eq!(report.items.len(), 5);
    assert_eq!(
        report
            .items
            .iter()
            .filter(|i| i.status == "unchecked")
            .count(),
        4
    );
}

#[test]
fn page_type_cannot_hide_existing_required_product_failures() {
    let report =
        json(r#"{"@context":"https://schema.org","@type":["Product","WebPage","FAQPage"]}"#);
    assert_eq!(report.items[0].profile, Some("Product snippet"));
    assert!(report.items[0].issues.iter().any(|i| i.severity == "error"));
}

#[test]
fn empty_or_inert_answer_markup_does_not_hide_an_empty_answer() {
    for answer in [
        "<p></p>",
        "<script>ignored()</script>",
        "<style>body{}</style>",
        "<template>ignored</template>",
    ] {
        let report = json(&format!(
            r#"{{"@context":"https://schema.org","@type":"FAQPage","mainEntity":{{"@type":"Question","name":"How?","acceptedAnswer":{{"@type":"Answer","text":{}}}}}}}"#,
            serde_json::to_string(answer).unwrap()
        ));
        no_penalty(&report);
        assert!(
            report.items[0]
                .issues
                .iter()
                .any(|i| i.code == "faq-answer-text")
        );
    }
}

#[test]
fn jsonld_null_values_do_not_add_redundant_question_or_answer_warnings() {
    let report = json(
        r#"{"@context":"https://schema.org","@type":"FAQPage","mainEntity":[null,{"@type":"Question","name":[null,"How?"],"acceptedAnswer":[null,{"@type":"Answer","text":[null,"Like this."]}]}]}"#,
    );
    assert_eq!(report.items.len(), 1);
    assert_eq!(report.items[0].status, "checked");
}

#[test]
fn parent_breadcrumb_errors_survive_component_suppression() {
    let report = json(
        r#"{"@context":"https://schema.org","@type":"BreadcrumbList","itemListElement":[{"@type":"ListItem","position":0,"name":"Home"}]}"#,
    );
    assert_eq!(report.items.len(), 1);
    assert_eq!(report.items[0].status, "error");
    assert!(
        report.items[0]
            .issues
            .iter()
            .any(|i| i.path == "/itemListElement/0/position")
    );
}
