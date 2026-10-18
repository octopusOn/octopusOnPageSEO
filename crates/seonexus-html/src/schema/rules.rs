use serde_json::Value;

use super::{Context, GUIDELINES, SchemaItem, issue, types};

const ROOT: &str = "https://developers.google.com/search/docs/appearance/structured-data/";
const ARTICLE: &str =
    "https://developers.google.com/search/docs/appearance/structured-data/article";
const BREADCRUMB: &str =
    "https://developers.google.com/search/docs/appearance/structured-data/breadcrumb";
const PRODUCT: &str =
    "https://developers.google.com/search/docs/appearance/structured-data/product-snippet";
const ORGANIZATION: &str =
    "https://developers.google.com/search/docs/appearance/structured-data/organization";
const BUSINESS: &str =
    "https://developers.google.com/search/docs/appearance/structured-data/local-business";

pub(super) fn check(item: &mut SchemaItem, value: &Value, context: &Context<'_>) {
    let resolved = context.resolve(value);
    let Some(value) = resolved.as_ref() else {
        item.issues.push(issue(
            "info",
            "unresolved-reference",
            &item.path,
            value.get("@id"),
            "manual",
            None,
            GUIDELINES,
        ));
        return;
    };
    // Select the most specific implemented profile. Supporting nodes retain their types,
    // but are not mistaken for independent rich-result features.
    if item.types.iter().any(|t| t == "BreadcrumbList") {
        item.profile = Some("Breadcrumb");
        breadcrumb(item, value, context);
    } else if item.types.iter().any(|t| t == "Product") {
        item.profile = Some("Product snippet");
        product(item, value, context);
    } else if item.types.iter().any(|t| t == "LocalBusiness") {
        item.profile = Some("LocalBusiness");
        text(item, value, "name", true, BUSINESS);
        let address = value.get("address").and_then(|v| context.resolve(v));
        if address.as_ref().is_some_and(|v| {
            v.is_object()
                && v.as_object()
                    .is_some_and(|v| v.keys().any(|k| !k.starts_with('@')))
        }) {
            // No invented mandatory street/zip/country fields: Google requires address.
        } else if value.get("address").is_some_and(Value::is_object) && address.is_none() {
            unverified(item, "address", value.get("address"), BUSINESS);
        } else {
            item.issues.push(issue(
                "error",
                "required",
                &path(item, "address"),
                value.get("address"),
                "postal-address",
                Some(r#"{"@type":"PostalAddress","streetAddress":"…","addressLocality":"…"}"#),
                BUSINESS,
            ));
        }
        text(item, value, "url", false, BUSINESS);
        text(item, value, "telephone", false, BUSINESS);
    } else if item
        .types
        .iter()
        .any(|t| matches!(t.as_str(), "Article" | "NewsArticle" | "BlogPosting"))
    {
        item.profile = Some("Article");
        // Google currently specifies no required Article properties.
        for field in ["headline", "datePublished", "dateModified"] {
            text(item, value, field, false, ARTICLE);
        }
        recommended(item, value, "image", ARTICLE);
        recommended(item, value, "author", ARTICLE);
    } else if item.types.iter().any(|t| t == "Organization") {
        item.profile = Some("Organization");
        // Organization also has recommended fields, not a fabricated required checklist.
        for field in ["name", "url"] {
            text(item, value, field, false, ORGANIZATION);
        }
        recommended(item, value, "logo", ORGANIZATION);
    } else if item.types.iter().any(|t| t == "FAQPage") {
        item.profile = Some("FAQPage (Schema.org)");
        super::pages::web_page(item, value, context);
        super::pages::faq(item, value, context);
    } else if item.types.iter().any(|t| t == "WebPage") {
        item.profile = Some("WebPage (Schema.org)");
        super::pages::web_page(item, value, context);
    } else {
        item.issues.push(issue(
            "info",
            "unsupported-profile",
            &item.path,
            value.get("@type"),
            "manual",
            None,
            ROOT,
        ));
    }
}

fn path(item: &SchemaItem, field: &str) -> String {
    format!("{}/{field}", item.path)
}
fn present(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::String(v) => !v.trim().is_empty(),
        Value::Array(v) => v.iter().any(present),
        Value::Object(v) => !v.is_empty(),
        _ => true,
    }
}
fn nonempty_text(value: &Value) -> bool {
    match value {
        Value::String(v) => !v.trim().is_empty(),
        Value::Array(v) => v.iter().any(nonempty_text),
        Value::Object(v) => v.get("@value").is_some_and(nonempty_text),
        _ => false,
    }
}
fn text(item: &mut SchemaItem, value: &Value, field: &str, required: bool, doc: &'static str) {
    if !value.get(field).is_some_and(nonempty_text) {
        item.issues.push(issue(
            if required { "error" } else { "warning" },
            if required { "required" } else { "recommended" },
            &path(item, field),
            value.get(field),
            "text",
            Some(r#""…""#),
            doc,
        ));
    }
}
fn recommended(item: &mut SchemaItem, value: &Value, field: &str, doc: &'static str) {
    if !value.get(field).is_some_and(present) {
        item.issues.push(issue(
            "warning",
            "recommended",
            &path(item, field),
            value.get(field),
            "value",
            None,
            doc,
        ));
    }
}
fn unverified(item: &mut SchemaItem, field: &str, value: Option<&Value>, doc: &'static str) {
    item.issues.push(issue(
        "info",
        "unresolved-reference",
        &path(item, field),
        value,
        "manual",
        None,
        doc,
    ));
}
fn integer(value: &Value) -> Option<u64> {
    value
        .as_u64()
        .or_else(|| value.as_str()?.parse().ok())
        .filter(|n| *n > 0)
}

fn breadcrumb(item: &mut SchemaItem, value: &Value, context: &Context<'_>) {
    let Some(values) = value.get("itemListElement").and_then(Value::as_array) else {
        if value.get("itemListElement").is_some_and(Value::is_object) {
            unverified(
                item,
                "itemListElement",
                value.get("itemListElement"),
                BREADCRUMB,
            );
        } else {
            item.issues.push(issue(
                "error",
                "required",
                &path(item, "itemListElement"),
                value.get("itemListElement"),
                "breadcrumb-list",
                None,
                BREADCRUMB,
            ));
        }
        return;
    };
    if values.len() < 2 {
        item.issues.push(issue(
            "error",
            "min-items",
            &path(item, "itemListElement"),
            value.get("itemListElement"),
            "breadcrumb-list",
            None,
            BREADCRUMB,
        ));
    }
    let resolved: Vec<_> = values.iter().map(|v| context.resolve(v)).collect();
    let last = resolved
        .iter()
        .flatten()
        .filter_map(|v| v.get("position").and_then(integer))
        .max();
    for (i, node) in resolved.iter().enumerate() {
        let prefix = format!("itemListElement/{i}");
        let Some(node) = node.as_ref() else {
            unverified(item, &prefix, values.get(i), BREADCRUMB);
            continue;
        };
        context.cover(&values[i]);
        let position = node.get("position").and_then(integer);
        if position.is_none() {
            item.issues.push(issue(
                "error",
                "required",
                &path(item, &format!("{prefix}/position")),
                node.get("position"),
                "positive-integer",
                Some("1"),
                BREADCRUMB,
            ));
        }
        let target = node.get("item");
        let target_node = target.and_then(|v| context.resolve(v));
        if !node.get("name").is_some_and(nonempty_text)
            && !target_node
                .as_ref()
                .and_then(|v| v.get("name"))
                .is_some_and(nonempty_text)
        {
            if target.is_some_and(Value::is_object) && target_node.is_none() {
                unverified(item, &format!("{prefix}/name"), target, BREADCRUMB);
            } else {
                item.issues.push(issue(
                    "error",
                    "required",
                    &path(item, &format!("{prefix}/name")),
                    node.get("name"),
                    "text",
                    Some(r#""Hizmetler""#),
                    BREADCRUMB,
                ));
            }
        }
        // A last breadcrumb may omit item. Do not require the document URL to be repeated.
        if position.is_some()
            && position != last
            && !target.is_some_and(|v| {
                v.as_str().is_some_and(|s| !s.trim().is_empty())
                    || v.get("@id")
                        .and_then(Value::as_str)
                        .is_some_and(|s| !s.trim().is_empty())
            })
        {
            item.issues.push(issue(
                "error",
                "required",
                &path(item, &format!("{prefix}/item")),
                target,
                "url",
                Some(r#""https://example.com/hizmetler""#),
                BREADCRUMB,
            ));
        }
    }
}

fn product(item: &mut SchemaItem, value: &Value, context: &Context<'_>) {
    text(item, value, "name", true, PRODUCT);
    if !["offers", "review", "aggregateRating"].iter().any(|field| {
        value.get(field).is_some_and(|v| {
            v.is_object() && present(v)
                || v.as_array().is_some_and(|v| v.iter().any(Value::is_object))
        })
    }) {
        item.issues.push(issue(
            "error",
            "product-alternative",
            &item.path,
            None,
            "product-alternative",
            None,
            PRODUCT,
        ));
    }
    if let Some(offers) = value.get("offers") {
        let offers: Vec<_> = match offers {
            Value::Array(v) => v.iter().collect(),
            v => vec![v],
        };
        for (i, raw) in offers.into_iter().enumerate() {
            let field = if value.get("offers").is_some_and(Value::is_array) {
                format!("offers/{i}")
            } else {
                "offers".into()
            };
            if let Some(offer) = context.resolve(raw) {
                offer_checks(item, &offer, &field, context);
            } else {
                unverified(item, &field, Some(raw), PRODUCT);
            }
        }
    }
    if value.get("review").is_some_and(present) || value.get("aggregateRating").is_some_and(present)
    {
        item.issues.push(issue(
            "info",
            "review-scope",
            &item.path,
            None,
            "manual",
            None,
            PRODUCT,
        ));
    }
    recommended(item, value, "image", PRODUCT);
}

fn offer_checks(item: &mut SchemaItem, offer: &Value, prefix: &str, context: &Context<'_>) {
    let kinds = types(offer);
    let aggregate = kinds.iter().any(|t| t == "AggregateOffer");
    if !aggregate && !kinds.iter().any(|t| t == "Offer") {
        unverified(item, prefix, Some(offer), PRODUCT);
        return;
    }
    let specification = offer
        .get("priceSpecification")
        .and_then(|v| context.resolve(v));
    if !aggregate
        && offer.get("price").is_none()
        && offer.get("priceSpecification").is_some()
        && specification.is_none()
    {
        unverified(
            item,
            &format!("{prefix}/priceSpecification"),
            offer.get("priceSpecification"),
            PRODUCT,
        );
        return;
    }
    let (field, price) = if aggregate {
        ("lowPrice", offer.get("lowPrice"))
    } else if offer.get("price").is_some() {
        ("price", offer.get("price"))
    } else {
        (
            "priceSpecification/price",
            specification.as_ref().and_then(|v| v.get("price")),
        )
    };
    if !price.is_some_and(|v| {
        v.as_f64()
            .or_else(|| v.as_str()?.parse::<f64>().ok())
            .is_some_and(|n| n.is_finite() && n >= 0.0)
    }) {
        item.issues.push(issue(
            "error",
            "required",
            &path(item, &format!("{prefix}/{field}")),
            price,
            "price",
            Some("0"),
            PRODUCT,
        ));
    }
    let currency = offer
        .get("priceCurrency")
        .or_else(|| specification.as_ref().and_then(|v| v.get("priceCurrency")));
    if !currency.is_some_and(nonempty_text) {
        item.issues.push(issue(
            if aggregate { "error" } else { "warning" },
            if aggregate { "required" } else { "recommended" },
            &path(item, &format!("{prefix}/priceCurrency")),
            currency,
            "currency",
            Some(r#""TRY""#),
            PRODUCT,
        ));
    }
}
