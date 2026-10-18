//! Schema.org page/FAQ shape checks. These are advisory, not retired Google FAQ
//! eligibility requirements, and never create required-property score deductions.
use serde_json::Value;

use super::{Context, SchemaItem, issue, types};

const PAGE: &str = "https://schema.org/WebPage";
const FAQ: &str = "https://schema.org/FAQPage";
const QUESTION: &str = "https://schema.org/Question";
const ANSWER: &str = "https://schema.org/Answer";

fn entries(value: &Value) -> Vec<(String, &Value)> {
    match value {
        Value::Array(values) => values
            .iter()
            .enumerate()
            .filter(|(_, v)| !v.is_null())
            .map(|(i, v)| (format!("/{i}"), v))
            .collect(),
        value => vec![(String::new(), value)],
    }
}

fn text(value: &Value) -> bool {
    match value {
        Value::String(text) => !text.trim().is_empty(),
        Value::Object(object) => object.get("@value").is_some_and(self::text),
        _ => false,
    }
}

fn content_text(value: &Value) -> bool {
    match value {
        Value::String(value) => {
            let fragment = scraper::Html::parse_fragment(value);
            fragment.root_element().descendants().any(|node| {
                matches!(node.value(), scraper::Node::Text(text) if !text.trim().is_empty())
                    && !node
                        .ancestors()
                        .filter_map(scraper::ElementRef::wrap)
                        .any(|parent| {
                            matches!(parent.value().name(), "script" | "style" | "template")
                        })
            })
        }
        Value::Object(object) => object.get("@value").is_some_and(content_text),
        _ => false,
    }
}

fn content(value: &Value) -> bool {
    let parts = entries(value);
    !parts.is_empty() && parts.iter().all(|(_, value)| content_text(value))
}

fn warning(
    item: &mut SchemaItem,
    field: &str,
    value: Option<&Value>,
    code: &'static str,
    expected: &'static str,
    example: Option<&'static str>,
    doc: &'static str,
) {
    item.issues.push(issue(
        "warning",
        code,
        &format!("{}/{field}", item.path),
        value,
        expected,
        example,
        doc,
    ));
}

fn unresolved(item: &mut SchemaItem, field: &str, raw: &Value) {
    item.issues.push(issue(
        "info",
        "unresolved-reference",
        &format!("{}/{field}", item.path),
        Some(raw),
        "manual",
        None,
        FAQ,
    ));
}

pub(super) fn web_page(item: &mut SchemaItem, value: &Value, context: &Context<'_>) {
    // Schema.org does not define a mandatory WebPage checklist. Inspect only
    // supplied values, accepting JSON-LD value objects and multiple values.
    for field in ["name", "description", "datePublished", "dateModified"] {
        supplied(item, value, field, text, "text", Some(r#""…""#));
    }
    supplied(
        item,
        value,
        "url",
        |v| {
            let raw = v
                .as_str()
                .or_else(|| v.get("@id").and_then(Value::as_str))
                .or_else(|| v.get("@value").and_then(Value::as_str));
            raw.is_some_and(|s| {
                !s.trim().is_empty()
                    && url::Url::parse(context.page_url).is_ok_and(|base| base.join(s).is_ok())
            })
        },
        "url",
        Some(r#""https://example.com/page""#),
    );
    // inLanguage accepts Text OR Language, not just a hard-coded language list.
    supplied(
        item,
        value,
        "inLanguage",
        |v| {
            text(v)
                || v.is_object()
                    && (v
                        .get("@id")
                        .and_then(Value::as_str)
                        .is_some_and(|s| !s.trim().is_empty())
                        || types(v).iter().any(|t| t == "Language"))
        },
        "language",
        Some(r#""tr-TR""#),
    );
    for field in ["isPartOf", "hasPart", "mainEntity"] {
        if field == "mainEntity" && item.types.iter().any(|t| t == "FAQPage") {
            continue;
        }
        supplied(
            item,
            value,
            field,
            |v| v.is_object() || text(v),
            "entity",
            None,
        );
    }
    // Breadcrumb can also be plain text. Do not demand an inline BreadcrumbList
    // when a valid @id reference or textual breadcrumb is supplied.
    supplied(
        item,
        value,
        "breadcrumb",
        |v| v.is_object() || text(v),
        "breadcrumb-value",
        None,
    );
}

fn supplied(
    item: &mut SchemaItem,
    value: &Value,
    field: &str,
    valid: impl Fn(&Value) -> bool,
    expected: &'static str,
    example: Option<&'static str>,
) {
    let Some(raw) = value.get(field).filter(|v| !v.is_null()) else {
        return;
    };
    // Empty optional arrays are simply absent in JSON-LD.
    for (suffix, part) in entries(raw) {
        if !part.is_null() && !valid(part) {
            warning(
                item,
                &format!("{field}{suffix}"),
                Some(part),
                "property-value",
                expected,
                example,
                PAGE,
            );
        }
    }
}

pub(super) fn faq(item: &mut SchemaItem, value: &Value, context: &Context<'_>) {
    let Some(main) = value.get("mainEntity").filter(|v| !v.is_null()) else {
        warning(
            item,
            "mainEntity",
            value.get("mainEntity"),
            "faq-questions",
            "questions",
            Some(
                r#"{"@type":"Question","name":"Soru?","acceptedAnswer":{"@type":"Answer","text":"Yanıt."}}"#,
            ),
            FAQ,
        );
        return;
    };
    let questions = entries(main);
    if questions.is_empty() {
        warning(
            item,
            "mainEntity",
            Some(main),
            "faq-questions",
            "questions",
            None,
            FAQ,
        );
    }
    for (suffix, raw) in questions {
        let field = format!("mainEntity{suffix}");
        let Some(question) = context.resolve(raw) else {
            unresolved(item, &field, raw);
            continue;
        };
        if !question.is_object() || !types(&question).iter().any(|t| t == "Question") {
            warning(
                item,
                &field,
                Some(raw),
                "faq-questions",
                "questions",
                None,
                FAQ,
            );
            continue;
        }
        context.cover(raw);
        if !question.get("name").is_some_and(content) {
            warning(
                item,
                &format!("{field}/name"),
                question.get("name"),
                "faq-question-text",
                "text",
                Some(r#""Hizmet nasıl çalışır?""#),
                QUESTION,
            );
        }
        answers(item, &question, &field, context);
    }
}

fn answers(item: &mut SchemaItem, question: &Value, field: &str, context: &Context<'_>) {
    // Schema.org also allows suggestedAnswer. Do not resurrect Google's
    // retired single accepted-answer / government-or-health restrictions.
    let answer_field = if question.get("acceptedAnswer").is_none_or(Value::is_null)
        && question
            .get("suggestedAnswer")
            .is_some_and(|v| !v.is_null())
    {
        "suggestedAnswer"
    } else {
        "acceptedAnswer"
    };
    let answer_path = format!("{field}/{answer_field}");
    let Some(answers) = question.get(answer_field).filter(|v| !v.is_null()) else {
        warning(
            item,
            &answer_path,
            None,
            "faq-answer",
            "answer",
            None,
            ANSWER,
        );
        return;
    };
    let answers_to_check = entries(answers);
    if answers_to_check.is_empty() {
        warning(
            item,
            &answer_path,
            Some(answers),
            "faq-answer",
            "answer",
            None,
            ANSWER,
        );
    }
    for (suffix, raw) in answers_to_check {
        let field = format!("{answer_path}{suffix}");
        let Some(answer) = context.resolve(raw) else {
            unresolved(item, &field, raw);
            continue;
        };
        if types(&answer).iter().any(|t| t == "ItemList") {
            // A permitted Schema.org alternative whose detailed rules are
            // outside this profile. Keep it unknown, never missing/invalid.
            unresolved(item, &field, raw);
            continue;
        }
        if !answer.is_object() || !types(&answer).iter().any(|t| t == "Answer") {
            warning(
                item,
                &field,
                Some(raw),
                "faq-answer",
                "answer",
                None,
                ANSWER,
            );
            continue;
        }
        context.cover(raw);
        if !answer.get("text").is_some_and(content) {
            warning(
                item,
                &format!("{field}/text"),
                answer.get("text"),
                "faq-answer-text",
                "text",
                Some(r#""Yanıtı sayfadaki içerikle aynı olacak şekilde yazın.""#),
                ANSWER,
            );
        }
    }
}
