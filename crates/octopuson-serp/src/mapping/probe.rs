//! One compiled selector: `JSONPath` or JSON Pointer, CSS with a pick, or a regular expression.

use regex::{Regex, RegexBuilder};
use scraper::{ElementRef, Selector};
use serde_json::Value;
use serde_json_path::JsonPath;

use super::Format;
use crate::SerpError;
use crate::text::squash;

/// Compiled size limit of user regular expressions, so a pattern cannot exhaust memory.
const REGEX_SIZE_LIMIT: usize = 1 << 20;

/// What a CSS probe reads from the element it selects.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Pick {
    /// All text inside the element, whitespace collapsed.
    Text,
    /// Only the element's own text nodes.
    OwnText,
    /// The inner HTML.
    Html,
    /// An attribute.
    Attr(String),
}

/// A compiled selector.
#[derive(Debug, Clone)]
pub(crate) enum Probe {
    /// RFC 9535 `JSONPath` (`$.organic[*].link`).
    JsonPath(JsonPath),
    /// RFC 6901 JSON Pointer (`/organic/0/link`); empty for the value itself.
    Pointer(String),
    /// CSS selector, or `None` for the context element itself, and what to read.
    Css(Option<Selector>, Pick),
    /// Regular expression; fields read the `v` group, else group 1, else the match.
    Regex(Regex),
}

impl Probe {
    /// Compiles one selector written for `format`.
    pub(crate) fn compile(format: Format, source: &str) -> Result<Self, SerpError> {
        match format {
            Format::Json => compile_json(source),
            Format::Html => compile_css(source),
            Format::Text => compile_regex(source).map(Self::Regex),
        }
    }

    /// Values a JSON probe selects below `value`.
    pub(crate) fn json<'a>(&self, value: &'a Value) -> Vec<&'a Value> {
        match self {
            Self::JsonPath(path) => path.query(value).all(),
            Self::Pointer(pointer) => value.pointer(pointer).into_iter().collect(),
            Self::Css(..) | Self::Regex(_) => Vec::new(),
        }
    }

    /// Elements a CSS probe selects inside `element` (the element itself for an empty
    /// selector).
    pub(crate) fn elements<'a>(&self, element: ElementRef<'a>) -> Vec<ElementRef<'a>> {
        match self {
            Self::Css(Some(selector), _) => element.select(selector).collect(),
            Self::Css(None, _) => vec![element],
            Self::JsonPath(_) | Self::Pointer(_) | Self::Regex(_) => Vec::new(),
        }
    }

    /// What a CSS probe reads from an element it selected.
    pub(crate) fn read(&self, element: ElementRef<'_>) -> Option<String> {
        let Self::Css(_, pick) = self else {
            return None;
        };
        let value = match pick {
            Pick::Text => squash(&element.text().collect::<String>()),
            Pick::OwnText => squash(
                &element
                    .children()
                    .filter_map(|child| child.value().as_text().map(|text| &**text))
                    .collect::<String>(),
            ),
            Pick::Html => element.inner_html().trim().to_owned(),
            Pick::Attr(name) => element.value().attr(name)?.trim().to_owned(),
        };
        (!value.is_empty()).then_some(value)
    }

    /// The regular expression of a text probe.
    pub(crate) fn regex(&self) -> Option<&Regex> {
        match self {
            Self::Regex(regex) => Some(regex),
            Self::JsonPath(_) | Self::Pointer(_) | Self::Css(..) => None,
        }
    }
}

fn compile_json(source: &str) -> Result<Probe, SerpError> {
    let source = source.trim();
    if source.is_empty() || source.starts_with('/') {
        return Ok(Probe::Pointer(source.to_owned()));
    }
    if source.starts_with('$') {
        return JsonPath::parse(source)
            .map(Probe::JsonPath)
            .map_err(|error| {
                SerpError::InvalidSpec(format!("invalid JSONPath `{source}`: {error}"))
            });
    }
    Err(SerpError::InvalidSpec(format!(
        "`{source}` is neither a JSONPath (`$...`) nor a JSON Pointer (`/...`)"
    )))
}

/// `selector`, `selector @attr`, `selector ::text|::own-text|::html`, `@attr` or `::text`.
fn compile_css(source: &str) -> Result<Probe, SerpError> {
    let source = source.trim();
    let (selector, pick) = split_pick(source)?;
    let selector = if selector.is_empty() {
        None
    } else {
        Some(Selector::parse(selector).map_err(|error| {
            SerpError::InvalidSpec(format!("invalid CSS selector `{selector}`: {error}"))
        })?)
    };
    Ok(Probe::Css(selector, pick))
}

fn split_pick(source: &str) -> Result<(&str, Pick), SerpError> {
    let (selector, pick) = match source.rfind(char::is_whitespace) {
        Some(index) if is_pick(source[index..].trim()) => {
            (source[..index].trim(), source[index..].trim())
        }
        _ if is_pick(source) => ("", source),
        _ => return Ok((source, Pick::Text)),
    };
    let pick = match pick {
        "::text" => Pick::Text,
        "::own-text" => Pick::OwnText,
        "::html" => Pick::Html,
        attr => {
            let name = attr.trim_start_matches('@');
            if name.is_empty()
                || !name
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b':'))
            {
                return Err(SerpError::InvalidSpec(format!(
                    "invalid attribute pick `{attr}`"
                )));
            }
            Pick::Attr(name.to_ascii_lowercase())
        }
    };
    Ok((selector, pick))
}

fn is_pick(token: &str) -> bool {
    token.starts_with('@') || matches!(token, "::text" | "::own-text" | "::html")
}

/// Compiles a user regular expression with a size limit.
pub(crate) fn compile_regex(source: &str) -> Result<Regex, SerpError> {
    RegexBuilder::new(source)
        .size_limit(REGEX_SIZE_LIMIT)
        .dfa_size_limit(REGEX_SIZE_LIMIT)
        .build()
        .map_err(|error| SerpError::InvalidSpec(format!("invalid regular expression: {error}")))
}

/// A JSON value as field text: strings as they are, numbers and booleans printed, arrays of
/// scalars joined with spaces, objects as JSON; `None` for null and empty strings.
pub(crate) fn json_text(value: &Value) -> Option<String> {
    let text = match value {
        Value::Null => return None,
        Value::String(text) => squash(text),
        Value::Bool(flag) => flag.to_string(),
        Value::Number(number) => number.to_string(),
        Value::Array(items) => squash(
            &items
                .iter()
                .filter_map(|item| match item {
                    Value::Array(_) | Value::Object(_) => None,
                    scalar => json_text(scalar),
                })
                .collect::<Vec<_>>()
                .join(" "),
        ),
        Value::Object(_) => value.to_string(),
    };
    (!text.is_empty()).then_some(text)
}

/// The text a field regex extracts from one match.
pub(crate) fn regex_field(regex: &Regex, haystack: &str) -> Option<String> {
    let captures = regex.captures(haystack)?;
    let matched = captures
        .name("v")
        .or_else(|| captures.get(1))
        .or_else(|| captures.get(0))?;
    let text = squash(matched.as_str());
    (!text.is_empty()).then_some(text)
}

#[cfg(test)]
mod tests {
    use scraper::Html;
    use serde_json::json;

    use super::*;

    #[test]
    fn splits_css_picks() {
        let probe = Probe::compile(Format::Html, "a.title @href").unwrap();
        assert!(matches!(probe, Probe::Css(Some(_), Pick::Attr(ref name)) if name == "href"));
        assert!(matches!(
            Probe::compile(Format::Html, "@data-q").unwrap(),
            Probe::Css(None, Pick::Attr(_))
        ));
        assert!(matches!(
            Probe::compile(Format::Html, "div > span ::own-text").unwrap(),
            Probe::Css(Some(_), Pick::OwnText)
        ));
        assert!(matches!(
            Probe::compile(Format::Html, "h3").unwrap(),
            Probe::Css(Some(_), Pick::Text)
        ));
        assert!(Probe::compile(Format::Html, "div[").is_err());
        assert!(Probe::compile(Format::Html, "a @").is_err());
    }

    #[test]
    fn reads_css_values() {
        let document = Html::parse_document(
            r#"<div id=r><a class=t href=" /x "> Hello <b>world</b> </a><p>own <i>not</i> text</p></div>"#,
        );
        let root = document.root_element();
        let href = Probe::compile(Format::Html, "a.t @href").unwrap();
        let element = href.elements(root)[0];
        assert_eq!(href.read(element).as_deref(), Some("/x"));
        let text = Probe::compile(Format::Html, "a.t").unwrap();
        assert_eq!(
            text.read(text.elements(root)[0]).as_deref(),
            Some("Hello world")
        );
        let own = Probe::compile(Format::Html, "p ::own-text").unwrap();
        assert_eq!(own.read(own.elements(root)[0]).as_deref(), Some("own text"));
    }

    #[test]
    fn evaluates_json_paths_and_pointers() {
        let value = json!({"organic": [{"link": "a"}, {"link": "b"}], "n": 3});
        let path = Probe::compile(Format::Json, "$.organic[*].link").unwrap();
        assert_eq!(path.json(&value).len(), 2);
        let pointer = Probe::compile(Format::Json, "/n").unwrap();
        assert_eq!(json_text(pointer.json(&value)[0]).as_deref(), Some("3"));
        assert!(Probe::compile(Format::Json, "organic").is_err());
        assert!(Probe::compile(Format::Json, "$.[").is_err());
    }

    #[test]
    fn converts_json_values_to_text() {
        assert_eq!(
            json_text(&json!(["a", "b", {"c": 1}])).as_deref(),
            Some("a b")
        );
        assert_eq!(json_text(&json!(null)), None);
        assert_eq!(json_text(&json!("  ")), None);
        assert_eq!(json_text(&json!(true)).as_deref(), Some("true"));
    }

    #[test]
    fn reads_regex_groups() {
        let named = compile_regex(r"id=(?P<v>\d+)").unwrap();
        assert_eq!(regex_field(&named, "x id=42 y").as_deref(), Some("42"));
        let numbered = compile_regex(r"<b>(.*?)</b>").unwrap();
        assert_eq!(
            regex_field(&numbered, "<b> bold </b>").as_deref(),
            Some("bold")
        );
        assert!(compile_regex("(").is_err());
    }
}
