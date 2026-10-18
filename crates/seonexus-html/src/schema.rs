//! Explicit, offline structured-data checks. This is not Google's Rich Results Test.
//!
//! Only ordinary schema.org JSON-LD contexts are interpreted. Remote contexts, aliases,
//! Microdata and RDFa remain unverified rather than producing false eligibility claims.

#[cfg(test)]
mod page_tests;
mod pages;
mod rules;
#[cfg(test)]
mod tests;

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};

use scraper::{ElementRef, Html, Selector};
use serde::Serialize;
use serde_json::Value;
use url::Url;

/// Rule snapshot, independently versioned from crawl scores.
pub const RULE_VERSION: &str = "2026-10-18.2";
/// Maximum input accepted by the explicit validator (4 MiB).
pub const INPUT_LIMIT: usize = 4 * 1024 * 1024;
const NODE_LIMIT: usize = 2000;
const GUIDELINES: &str =
    "https://developers.google.com/search/docs/appearance/structured-data/sd-policies";

/// One actionable check, including its original JSON path and observed value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SchemaIssue {
    /// `error`, `warning` (recommended), or `info` (unverified).
    pub severity: &'static str,
    /// Stable message identifier.
    pub code: &'static str,
    /// JSON pointer within the numbered block.
    pub path: String,
    /// Observed JSON, bounded to 400 characters; absent means missing.
    pub current: Option<String>,
    /// Short expected-value identifier, localized by the UI.
    pub expected: &'static str,
    /// Illustrative value, never automatically applied.
    pub example: Option<&'static str>,
    /// Official documentation for this check.
    pub documentation: &'static str,
}

/// A typed JSON-LD object or a detected, unverified non-JSON-LD block.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SchemaItem {
    /// One-based script/block number within its format.
    pub block: usize,
    /// `jsonld`, `microdata`, or `rdfa`.
    pub format: &'static str,
    /// Exact JSON pointer of this object.
    pub path: String,
    /// Declared types; unknown types are retained.
    pub types: Vec<String>,
    /// Implemented local field profile, if any; not necessarily a Google feature.
    pub profile: Option<&'static str>,
    /// `error`, `warning`, `checked`, or `unchecked`; never a Google eligibility verdict.
    pub status: &'static str,
    /// Required, recommended, and unverified checks.
    pub issues: Vec<SchemaIssue>,
}

/// Bounded local validation result; absence and optional advice are not errors.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SchemaReport {
    /// Version and review date of the rules.
    pub rule_version: &'static str,
    /// Number of JSON-LD script blocks (one for pasted JSON-LD).
    pub jsonld_blocks: usize,
    /// Checks were bounded and some objects could not be inspected.
    pub limited: bool,
    /// Individual objects and unsupported-format observations.
    pub items: Vec<SchemaItem>,
}

/// Validates HTML or a pasted JSON-LD document without any network requests.
///
/// # Errors
/// The input exceeds the explicit input limit.
pub fn validate(input: &str, jsonld: bool, page_url: &str) -> Result<SchemaReport, String> {
    validate_impl(input, jsonld, page_url, None)
}

/// Validates a crawl document while reusing already extracted non-JSON-LD observations.
/// # Errors
/// The source exceeds the validator's input limit.
pub fn validate_page(
    input: &str,
    page_url: &str,
    facts: &crate::PageFacts,
) -> Result<SchemaReport, String> {
    validate_impl(input, false, page_url, Some(&facts.structured_data))
}

fn validate_impl(
    input: &str,
    jsonld: bool,
    page_url: &str,
    extracted: Option<&[crate::StructuredData]>,
) -> Result<SchemaReport, String> {
    if input.len() > INPUT_LIMIT {
        return Err("Schema input exceeds 4 MiB".into());
    }
    let mut report = SchemaReport {
        rule_version: RULE_VERSION,
        jsonld_blocks: 0,
        limited: false,
        items: Vec::new(),
    };
    let mut documents = Vec::new();
    if jsonld {
        parse_block(input, &mut documents, &mut report);
    } else {
        let document = Html::parse_document(input);
        // Constant, tested selector. An impossible parse failure leaves the report unverified.
        let Ok(selector) = Selector::parse("script[type]") else {
            report.limited = true;
            return Ok(report);
        };
        for script in document.select(&selector).filter(|element| {
            !element
                .ancestors()
                .filter_map(ElementRef::wrap)
                .any(|p| p.value().name() == "template")
        }) {
            if script
                .value()
                .attr("type")
                .is_some_and(|kind| kind.trim().eq_ignore_ascii_case("application/ld+json"))
            {
                parse_block(
                    &script.text().collect::<String>(),
                    &mut documents,
                    &mut report,
                );
            }
        }
        let parsed;
        let blocks: &[crate::StructuredData] = if let Some(blocks) = extracted {
            blocks
        } else if let Ok(base) = Url::parse(page_url) {
            parsed = crate::parse_page(input, &base, &[]);
            &parsed.structured_data
        } else {
            &[]
        };
        for block in blocks {
            if block.format != crate::StructuredDataFormat::JsonLd {
                let mut item = empty_item(0, block.format.as_str(), "", block.types.clone());
                item.issues.push(issue(
                    "info",
                    "unsupported-format",
                    "",
                    None,
                    "manual",
                    None,
                    GUIDELINES,
                ));
                report.items.push(item);
            }
        }
    }
    let mut nodes = Vec::new();
    for (block, document) in &documents {
        collect_nodes(document, *block, "", false, &mut nodes, &mut report.limited);
    }
    let index = index_nodes(&nodes, page_url);
    let context = Context {
        index,
        page_url,
        covered: RefCell::default(),
    };
    inspect_nodes(&nodes, &context, &mut report);
    Ok(report)
}

fn inspect_nodes(nodes: &[Node<'_>], context: &Context<'_>, report: &mut SchemaReport) {
    // Parents check supporting entities in their actual role. Do not turn each
    // ListItem, Question and Answer into a second unsupported rich-result feature.
    // A separate, unconsumed supporting node must still remain unverified.
    for node in nodes
        .iter()
        .filter(|n| !support_only(n.value))
        .chain(nodes.iter().filter(|n| support_only(n.value)))
    {
        let types = types(node.value);
        if types.is_empty() {
            continue;
        }
        if node.ordinary
            && !advanced(node.value)
            && support_only(node.value)
            && context.is_covered(node.value)
        {
            continue;
        }
        let mut item = empty_item(node.block, "jsonld", &node.path, types);
        if node.ordinary && !advanced(node.value) {
            rules::check(&mut item, node.value, context);
        } else {
            item.issues.push(issue(
                "info",
                "unsupported-context",
                &node.path,
                node.value.get("@context"),
                "manual",
                None,
                GUIDELINES,
            ));
        }
        item.status = if item.issues.iter().any(|i| i.severity == "error") {
            "error"
        } else if item.issues.iter().any(|i| i.severity == "info") || item.profile.is_none() {
            "unchecked"
        } else if item.issues.iter().any(|i| i.severity == "warning") {
            "warning"
        } else {
            "checked"
        };
        report.items.push(item);
    }
}

fn parse_block(input: &str, documents: &mut Vec<(usize, Value)>, report: &mut SchemaReport) {
    report.jsonld_blocks += 1;
    if report.jsonld_blocks > NODE_LIMIT {
        report.limited = true;
        return;
    }
    match serde_json::from_str::<Value>(input.trim()) {
        Ok(value @ (Value::Object(_) | Value::Array(_))) => {
            documents.push((report.jsonld_blocks, value));
        }
        parsed => {
            let mut item = empty_item(report.jsonld_blocks, "jsonld", "", Vec::new());
            item.status = "error";
            let mut finding = issue(
                "error",
                "json-syntax",
                "",
                None,
                "json-object",
                None,
                GUIDELINES,
            );
            finding.current = Some(match parsed {
                Err(error) => error.to_string(),
                Ok(value) => value.to_string(),
            });
            item.issues.push(finding);
            report.items.push(item);
        }
    }
}

struct Node<'a> {
    block: usize,
    path: String,
    ordinary: bool,
    value: &'a Value,
}

fn index_nodes<'a>(nodes: &[Node<'a>], page_url: &str) -> HashMap<String, Vec<&'a Value>> {
    let mut index: HashMap<String, Vec<&Value>> = HashMap::new();
    for node in nodes {
        if node.ordinary
            && !advanced(node.value)
            && let Some(id) = node.value.get("@id").and_then(Value::as_str)
        {
            index
                .entry(identifier(id, page_url))
                .or_default()
                .push(node.value);
        }
    }
    index
}

fn advanced(value: &Value) -> bool {
    match value {
        Value::Object(object) => object.iter().any(|(key, value)| {
            matches!(key.as_str(), "@set" | "@list" | "@reverse" | "@nest") || advanced(value)
        }),
        Value::Array(values) => values.iter().any(advanced),
        _ => false,
    }
}

fn ordinary_context(value: &Value) -> bool {
    match value {
        Value::String(v) => matches!(
            v.trim_end_matches('/'),
            "https://schema.org" | "http://schema.org"
        ),
        Value::Object(v) => v.len() == 1 && v.get("@vocab").is_some_and(ordinary_context),
        _ => false,
    }
}

fn collect_nodes<'a>(
    value: &'a Value,
    block: usize,
    path: &str,
    inherited: bool,
    nodes: &mut Vec<Node<'a>>,
    limited: &mut bool,
) {
    if nodes.len() >= NODE_LIMIT {
        *limited = true;
        return;
    }
    match value {
        Value::Object(object) => {
            let ordinary = object.get("@context").map_or(inherited, ordinary_context);
            nodes.push(Node {
                block,
                path: path.into(),
                ordinary,
                value,
            });
            for (key, child) in object {
                if !key.starts_with('@') || matches!(key.as_str(), "@graph" | "@included") {
                    collect_nodes(
                        child,
                        block,
                        &format!("{path}/{}", key.replace('~', "~0").replace('/', "~1")),
                        ordinary,
                        nodes,
                        limited,
                    );
                }
            }
        }
        Value::Array(values) => {
            for (i, child) in values.iter().enumerate() {
                collect_nodes(
                    child,
                    block,
                    &format!("{path}/{i}"),
                    inherited,
                    nodes,
                    limited,
                );
            }
        }
        _ => {}
    }
}

fn types(value: &Value) -> Vec<String> {
    let values = value
        .get("@type")
        .map(|v| match v {
            Value::Array(v) => v.iter().collect(),
            v => vec![v],
        })
        .unwrap_or_default();
    values
        .into_iter()
        .filter_map(Value::as_str)
        .map(|kind| {
            kind.strip_prefix("https://schema.org/")
                .or_else(|| kind.strip_prefix("http://schema.org/"))
                .unwrap_or(kind)
                .to_owned()
        })
        .collect()
}

fn empty_item(block: usize, format: &'static str, path: &str, types: Vec<String>) -> SchemaItem {
    SchemaItem {
        block,
        format,
        path: path.into(),
        types,
        profile: None,
        status: "unchecked",
        issues: Vec::new(),
    }
}

fn issue(
    severity: &'static str,
    code: &'static str,
    path: &str,
    current: Option<&Value>,
    expected: &'static str,
    example: Option<&'static str>,
    documentation: &'static str,
) -> SchemaIssue {
    SchemaIssue {
        severity,
        code,
        path: path.into(),
        current: current.map(|v| v.to_string().chars().take(400).collect()),
        expected,
        example,
        documentation,
    }
}

fn identifier(raw: &str, base: &str) -> String {
    Url::parse(base)
        .ok()
        .and_then(|base| base.join(raw).ok())
        .map_or_else(|| raw.into(), |url| url.to_string())
}

struct Context<'a> {
    index: HashMap<String, Vec<&'a Value>>,
    page_url: &'a str,
    // Resolved values also cover @id definitions in other blocks. Exact serialized
    // values avoid treating a similarly typed but unrelated object as checked.
    covered: RefCell<HashSet<String>>,
}

impl Context<'_> {
    fn cover(&self, raw: &Value) {
        if !advanced(raw)
            && let Some(value) = self.resolve(raw)
            && let Ok(key) = serde_json::to_string(&value)
        {
            self.covered.borrow_mut().insert(key);
        }
    }
    fn is_covered(&self, raw: &Value) -> bool {
        self.resolve(raw)
            .and_then(|v| serde_json::to_string(&v).ok())
            .is_some_and(|key| self.covered.borrow().contains(&key))
    }
    fn resolve(&self, value: &Value) -> Option<Value> {
        if advanced(value) || value.get("@context").is_some_and(|v| !ordinary_context(v)) {
            return None;
        }
        if let Value::Array(values) = value {
            return values.first().and_then(|v| self.resolve(v));
        }
        let Some(id) = value.get("@id").and_then(Value::as_str) else {
            return Some(value.clone());
        };
        let mut object = serde_json::Map::new();
        if let Some(parts) = self.index.get(&identifier(id, self.page_url)) {
            for part in parts {
                for (key, v) in part.as_object()? {
                    if object.get(key).is_some_and(|previous| previous != v) && key != "@context" {
                        return None;
                    }
                    object.insert(key.clone(), v.clone());
                }
            }
        }
        for (key, v) in value.as_object()? {
            object.insert(key.clone(), v.clone());
        }
        if advanced(&Value::Object(object.clone()))
            || object
                .keys()
                .all(|k| matches!(k.as_str(), "@id" | "@context"))
        {
            None
        } else {
            Some(Value::Object(object))
        }
    }
}

fn support_only(value: &Value) -> bool {
    let types = types(value);
    !types.is_empty()
        && types
            .iter()
            .all(|t| matches!(t.as_str(), "ListItem" | "Question" | "Answer"))
}
