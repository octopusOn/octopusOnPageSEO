//! Request templates of user-defined providers.
//!
//! A template is text with placeholders in braces: `{keyword}`, `{page}`, `{secret:token}`,
//! `{num:int}`. `{{` and `}}` are literal braces. Values are filled in for the context they
//! end up in, so a keyword can never change the shape of a request:
//!
//! - URLs: values are percent-encoded (everything but RFC 3986 unreserved characters);
//! - headers: values are inserted as they are, and a value with a line break is refused;
//! - JSON bodies: the body is a JSON tree and each string leaf is filled in on its own; a
//!   leaf that is exactly one placeholder takes the value's type (`"{num:int}"` → `10`), and
//!   one whose optional value is missing (`"{location}"` without a location) is left out.

use std::collections::BTreeSet;
use std::fmt::Write as _;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;

use serde_json::Value;

use crate::locale::{
    bing_market, country_name, location_code, uule, yandex_localization, yandex_region,
    yandex_search_base, yandex_search_type,
};
use crate::{SerpError, SerpQuery};

/// Placeholders a template may use, besides `{secret:<name>}`.
pub const PLACEHOLDERS: &[(&str, &str)] = &[
    ("keyword", "the keyword as typed, trimmed"),
    ("page", "result page, counted from `paging.first_page`"),
    ("page0", "result page, counted from 0"),
    (
        "start",
        "offset of the first result, from 0 (0, 10, 20, ...)",
    ),
    (
        "start1",
        "offset of the first result, from 1 (1, 11, 21, ...)",
    ),
    ("num", "results per page (`paging.per_page`)"),
    ("depth", "deepest position wanted"),
    ("country", "ISO country code (tr)"),
    ("gl", "ISO country code (tr)"),
    ("language", "ISO language code (tr)"),
    ("hl", "ISO language code (tr)"),
    ("mkt", "language-COUNTRY market code (tr-TR)"),
    ("device", "desktop or mobile"),
    ("engine", "google, bing or yandex"),
    (
        "location",
        "the location's provider code, else its name (optional)",
    ),
    ("location_name", "the location's name (optional)"),
    (
        "google_canonical",
        "Google canonical location name (optional)",
    ),
    ("uule", "Google uule of the location (optional)"),
    ("lr", "Yandex region id (optional)"),
    ("location_code", "the location's provider code (optional)"),
    ("country_name", "English country name (Turkey)"),
    (
        "google_location",
        "Google canonical location name, else the country name",
    ),
    (
        "google_city",
        "Google canonical name of the chosen city or region (optional)",
    ),
    (
        "google_location_code",
        "the country's Google location code (2792) when no city is chosen (optional)",
    ),
    (
        "yandex_domain",
        "Yandex site of the country (yandex.com.tr)",
    ),
    (
        "yandex_search_type",
        "Yandex Search API search type (SEARCH_TYPE_TR)",
    ),
    (
        "yandex_l10n",
        "Yandex Search API interface language (LOCALIZATION_TR)",
    ),
];

/// What one request is for.
#[derive(Debug, Clone, Copy)]
pub struct TemplateContext<'a> {
    /// The query.
    pub query: &'a SerpQuery,
    /// Result page, from 1.
    pub page: u16,
    /// Results per page.
    pub per_page: u16,
    /// Number the provider gives its first page (usually 1, sometimes 0).
    pub first_page: u32,
}

impl TemplateContext<'_> {
    fn value(&self, name: &str) -> Option<String> {
        let query = self.query;
        let location = query.location.as_ref();
        let offset = u32::from(self.page.saturating_sub(1)) * u32::from(self.per_page);
        match name {
            "keyword" => Some(query.keyword.trim().to_owned()),
            "page" => Some((self.first_page + u32::from(self.page.saturating_sub(1))).to_string()),
            "page0" => Some(self.page.saturating_sub(1).to_string()),
            "start" => Some(offset.to_string()),
            "start1" => Some((offset + 1).to_string()),
            "num" => Some(self.per_page.to_string()),
            "depth" => Some(query.depth.to_string()),
            "country" | "gl" => Some(query.country.clone()),
            "language" | "hl" => Some(query.language.clone()),
            "mkt" => Some(bing_market(&query.language, &query.country)),
            "device" => Some(query.device.as_str().to_owned()),
            "engine" => Some(query.engine.as_str().to_owned()),
            "location" => location.and_then(|location| {
                location
                    .provider_code
                    .clone()
                    .or_else(|| location.name.clone())
            }),
            "location_name" => location.and_then(|location| location.name.clone()),
            "google_canonical" => location.and_then(|location| location.google_canonical.clone()),
            "uule" => location
                .and_then(|location| location.google_canonical.as_deref())
                .and_then(uule),
            "location_code" => location.and_then(|location| location.provider_code.clone()),
            "country_name" => Some(
                country_name(&query.country)
                    .map_or_else(|| query.country.to_ascii_uppercase(), str::to_owned),
            ),
            "google_location" => location
                .and_then(|location| location.google_canonical.clone())
                .or_else(|| country_name(&query.country).map(str::to_owned)),
            "google_city" => location.and_then(|location| location.google_canonical.clone()),
            "google_location_code" => {
                if location.is_some_and(|location| location.google_canonical.is_some()) {
                    None
                } else {
                    location_code(&query.country).map(|code| code.to_string())
                }
            }
            "yandex_domain" => Some(
                yandex_search_base(&query.country)
                    .trim_start_matches("https://")
                    .trim_end_matches("/search/")
                    .to_owned(),
            ),
            "yandex_search_type" => Some(yandex_search_type(&query.country).to_owned()),
            "yandex_l10n" => Some(yandex_localization(&query.language).to_owned()),
            "lr" => location
                .and_then(|location| location.yandex_lr)
                .or_else(|| yandex_region(query))
                .map(|region| region.to_string()),
            _ => None,
        }
    }
}

/// One piece of a parsed template.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Piece {
    Text(String),
    Placeholder {
        name: String,
        secret: bool,
        int: bool,
        base64: bool,
    },
}

fn parse(template: &str) -> Result<Vec<Piece>, SerpError> {
    let mut pieces = Vec::new();
    let mut text = String::new();
    let mut chars = template.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '{' if chars.peek() == Some(&'{') => {
                chars.next();
                text.push('{');
            }
            '}' if chars.peek() == Some(&'}') => {
                chars.next();
                text.push('}');
            }
            '{' => {
                let mut inner = String::new();
                loop {
                    match chars.next() {
                        Some('}') => break,
                        Some(ch) => inner.push(ch),
                        None => {
                            return Err(SerpError::Template(format!(
                                "unclosed `{{` in `{template}`"
                            )));
                        }
                    }
                }
                if !text.is_empty() {
                    pieces.push(Piece::Text(std::mem::take(&mut text)));
                }
                pieces.push(placeholder(inner.trim())?);
            }
            '}' => {
                return Err(SerpError::Template(format!(
                    "unmatched `}}` in `{template}`; write `}}}}` for a literal brace"
                )));
            }
            other => text.push(other),
        }
    }
    if !text.is_empty() {
        pieces.push(Piece::Text(text));
    }
    Ok(pieces)
}

fn placeholder(inner: &str) -> Result<Piece, SerpError> {
    let (secret, rest) = match inner.strip_prefix("secret:") {
        Some(rest) => (true, rest),
        None => (false, inner),
    };
    let (name, modifier) = match rest.split_once(':') {
        Some((name, modifier)) => (name.trim(), Some(modifier.trim())),
        None => (rest.trim(), None),
    };
    let (int, base64) = match modifier {
        None => (false, false),
        Some("int") => (true, false),
        Some("base64") => (false, true),
        Some(other) => {
            return Err(SerpError::Template(format!(
                "unknown modifier `:{other}` (use `:int` or `:base64`)"
            )));
        }
    };
    if secret {
        if !is_secret_name(name) {
            return Err(SerpError::Template(format!(
                "invalid secret name `{name}` (use a-z, 0-9 and _)"
            )));
        }
    } else if !PLACEHOLDERS.iter().any(|(known, _)| *known == name) {
        return Err(SerpError::Template(format!(
            "unknown placeholder `{{{name}}}`"
        )));
    }
    Ok(Piece::Placeholder {
        name: name.to_owned(),
        secret,
        int,
        base64,
    })
}

/// Whether a secret name is valid: 1–40 of `a-z`, `0-9` and `_`.
pub fn is_secret_name(name: &str) -> bool {
    (1..=40).contains(&name.len())
        && name
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}

/// Placeholders and secrets a template uses.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Usage {
    /// Placeholder names.
    pub placeholders: BTreeSet<String>,
    /// Secret names.
    pub secrets: BTreeSet<String>,
}

/// Checks a template's syntax and reports what it uses.
///
/// # Errors
/// [`SerpError::Template`] for unknown placeholders, invalid secret names and unbalanced
/// braces.
pub fn usage(template: &str) -> Result<Usage, SerpError> {
    let mut usage = Usage::default();
    for piece in parse(template)? {
        if let Piece::Placeholder { name, secret, .. } = piece {
            if secret {
                usage.secrets.insert(name);
            } else {
                usage.placeholders.insert(name);
            }
        }
    }
    Ok(usage)
}

/// Reports what a JSON body template uses.
///
/// # Errors
/// As [`usage`], for any string in the body.
pub fn body_usage(body: &Value) -> Result<Usage, SerpError> {
    let mut total = Usage::default();
    match body {
        Value::String(text) => {
            let found = usage(text)?;
            total.placeholders.extend(found.placeholders);
            total.secrets.extend(found.secrets);
        }
        Value::Array(items) => {
            for item in items {
                let found = body_usage(item)?;
                total.placeholders.extend(found.placeholders);
                total.secrets.extend(found.secrets);
            }
        }
        Value::Object(members) => {
            for (key, value) in members {
                for found in [usage(key)?, body_usage(value)?] {
                    total.placeholders.extend(found.placeholders);
                    total.secrets.extend(found.secrets);
                }
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
    Ok(total)
}

/// Fills templates in for one request.
pub struct Renderer<'a> {
    context: TemplateContext<'a>,
    secrets: &'a dyn Fn(&str) -> Option<String>,
    used_secrets: Vec<String>,
}

impl<'a> Renderer<'a> {
    /// A renderer reading secret values from `secrets`.
    pub fn new(context: TemplateContext<'a>, secrets: &'a dyn Fn(&str) -> Option<String>) -> Self {
        Self {
            context,
            secrets,
            used_secrets: Vec::new(),
        }
    }

    /// Secret values filled in so far, for redaction.
    pub fn used_secrets(&self) -> &[String] {
        &self.used_secrets
    }

    fn resolve(
        &mut self,
        name: &str,
        secret: bool,
        base64: bool,
    ) -> Result<Option<String>, SerpError> {
        let value = if secret {
            let value = (self.secrets)(name)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| SerpError::Template(format!("secret `{name}` is not set")))?;
            self.used_secrets.push(value.clone());
            Some(value)
        } else {
            self.context.value(name)
        };
        Ok(value.map(|value| {
            if base64 {
                let encoded = STANDARD.encode(value);
                if secret {
                    self.used_secrets.push(encoded.clone());
                }
                encoded
            } else {
                value
            }
        }))
    }

    /// Fills in a URL template, percent-encoding the values.
    ///
    /// # Errors
    /// [`SerpError::Template`] for syntax errors and missing secrets.
    pub fn url(&mut self, template: &str) -> Result<String, SerpError> {
        let mut out = String::new();
        for piece in parse(template)? {
            match piece {
                Piece::Text(text) => out.push_str(&text),
                Piece::Placeholder {
                    name,
                    secret,
                    base64,
                    ..
                } => {
                    if let Some(value) = self.resolve(&name, secret, base64)? {
                        out.push_str(&percent_encode(&value));
                    }
                }
            }
        }
        Ok(out)
    }

    /// Fills in a header value.
    ///
    /// # Errors
    /// [`SerpError::Template`] for syntax errors, missing secrets and values with control
    /// characters.
    pub fn header(&mut self, template: &str) -> Result<String, SerpError> {
        let value = self.text(template)?;
        if value
            .chars()
            .any(|ch| ch == '\r' || ch == '\n' || ch == '\0')
        {
            return Err(SerpError::Template(
                "a header value may not contain line breaks".to_owned(),
            ));
        }
        Ok(value)
    }

    /// Fills in plain text without encoding.
    ///
    /// # Errors
    /// [`SerpError::Template`] for syntax errors and missing secrets.
    pub fn text(&mut self, template: &str) -> Result<String, SerpError> {
        let mut out = String::new();
        for piece in parse(template)? {
            match piece {
                Piece::Text(text) => out.push_str(&text),
                Piece::Placeholder {
                    name,
                    secret,
                    base64,
                    ..
                } => {
                    if let Some(value) = self.resolve(&name, secret, base64)? {
                        out.push_str(&value);
                    }
                }
            }
        }
        Ok(out)
    }

    /// Fills in a value that may be left out: `None` when the template is a single optional
    /// placeholder without a value.
    ///
    /// # Errors
    /// [`SerpError::Template`] for syntax errors and missing secrets.
    pub fn optional(&mut self, template: &str) -> Result<Option<String>, SerpError> {
        match self.json_leaf(&Value::String(template.to_owned()))? {
            Some(Value::String(text)) => Ok(Some(text)),
            Some(other) => Ok(Some(other.to_string())),
            None => Ok(None),
        }
    }

    /// Fills in a JSON body template leaf by leaf.
    ///
    /// # Errors
    /// [`SerpError::Template`] for syntax errors, missing secrets and `:int` values that are
    /// not integers.
    pub fn json(&mut self, template: &Value) -> Result<Value, SerpError> {
        Ok(self.json_leaf(template)?.unwrap_or(Value::Null))
    }

    fn json_leaf(&mut self, template: &Value) -> Result<Option<Value>, SerpError> {
        match template {
            Value::String(text) => {
                let pieces = parse(text)?;
                if let [
                    Piece::Placeholder {
                        name,
                        secret,
                        int,
                        base64,
                    },
                ] = pieces.as_slice()
                {
                    let Some(value) = self.resolve(name, *secret, *base64)? else {
                        return Ok(None);
                    };
                    if *int {
                        let number: i64 = value.parse().map_err(|_| {
                            SerpError::Template(format!(
                                "`{{{name}:int}}` is `{value}`, not an integer"
                            ))
                        })?;
                        return Ok(Some(Value::from(number)));
                    }
                    return Ok(Some(Value::String(value)));
                }
                Ok(Some(Value::String(self.text(text)?)))
            }
            Value::Array(items) => {
                let mut out = Vec::with_capacity(items.len());
                for item in items {
                    out.push(self.json_leaf(item)?.unwrap_or(Value::Null));
                }
                Ok(Some(Value::Array(out)))
            }
            Value::Object(members) => {
                let mut out = serde_json::Map::new();
                for (key, value) in members {
                    if let Some(value) = self.json_leaf(value)? {
                        out.insert(self.text(key)?, value);
                    }
                }
                Ok(Some(Value::Object(out)))
            }
            other => Ok(Some(other.clone())),
        }
    }
}

/// Replaces secret values (raw and percent-encoded) with `***`.
pub fn redact(text: &str, secrets: &[String]) -> String {
    let mut out = text.to_owned();
    for secret in secrets.iter().filter(|secret| secret.len() >= 4) {
        out = out.replace(secret.as_str(), "***");
        let encoded = percent_encode(secret);
        if encoded != *secret {
            out = out.replace(&encoded, "***");
        }
    }
    out
}

/// Percent-encodes everything but RFC 3986 unreserved characters.
pub fn percent_encode(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            out.push(char::from(byte));
        } else {
            let _ = write!(out, "%{byte:02X}");
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::{Engine, Location};

    fn query() -> SerpQuery {
        let mut query = SerpQuery::new(Engine::Google, " kahve & çay ").with_depth(20);
        query.location = Some(Location {
            name: Some("Istanbul".to_owned()),
            ..Location::default()
        });
        query
    }

    fn secrets(name: &str) -> Option<String> {
        (name == "token").then(|| "s3cr3t/+key".to_owned())
    }

    #[test]
    fn encodes_url_values() {
        let query = query();
        let context = TemplateContext {
            query: &query,
            page: 2,
            per_page: 10,
            first_page: 1,
        };
        let mut renderer = Renderer::new(context, &secrets);
        let url = renderer
            .url("https://api.example/s?q={keyword}&p={page}&s={start}&k={secret:token}&x={{y}}")
            .unwrap();
        assert_eq!(
            url,
            "https://api.example/s?q=kahve%20%26%20%C3%A7ay&p=2&s=10&k=s3cr3t%2F%2Bkey&x={y}"
        );
        assert_eq!(
            redact(&url, renderer.used_secrets()),
            url.replace("s3cr3t%2F%2Bkey", "***")
        );
    }

    #[test]
    fn fills_json_leaves_with_types_and_drops_missing_optionals() {
        let mut query = query();
        query.location = None;
        let context = TemplateContext {
            query: &query,
            page: 1,
            per_page: 10,
            first_page: 1,
        };
        let mut renderer = Renderer::new(context, &secrets);
        let body = renderer
            .json(&json!({
                "q": "{keyword}",
                "num": "{num:int}",
                "loc": "{location}",
                "label": "kw: {keyword} ({hl})",
                "list": ["{gl}", "{location}"]
            }))
            .unwrap();
        assert_eq!(
            body,
            json!({"q": "kahve & çay", "num": 10, "label": "kw: kahve & çay (tr)", "list": ["tr", null]})
        );
    }

    #[test]
    fn rejects_bad_templates_and_header_injection() {
        assert!(usage("{nope}").is_err());
        assert!(usage("{keyword").is_err());
        assert!(usage("a } b").is_err());
        assert!(usage("{keyword:float}").is_err());
        assert!(usage("{secret:creds:base64}").is_ok());
        assert!(usage("{secret:Bad-Name}").is_err());
        let found = usage("{keyword}{secret:token}{{x}}").unwrap();
        assert!(found.placeholders.contains("keyword") && found.secrets.contains("token"));

        let mut query = query();
        query.keyword = "a\r\nX-Evil: 1".to_owned();
        let context = TemplateContext {
            query: &query,
            page: 1,
            per_page: 10,
            first_page: 1,
        };
        let mut renderer = Renderer::new(context, &secrets);
        assert!(renderer.header("{keyword}").is_err());
        assert!(renderer.url("{secret:missing}").is_err());
    }

    proptest::proptest! {
        #[test]
        fn keywords_cannot_change_the_shape_of_a_url(keyword in "\\PC{1,40}") {
            let query = SerpQuery::new(Engine::Google, keyword.clone());
            let context = TemplateContext { query: &query, page: 1, per_page: 10, first_page: 1 };
            let mut renderer = Renderer::new(context, &secrets);
            let url = renderer.url("https://api.example/s?q={keyword}&p={page}").unwrap();
            let parsed = url::Url::parse(&url).unwrap();
            proptest::prop_assert_eq!(parsed.host_str(), Some("api.example"));
            proptest::prop_assert_eq!(parsed.path(), "/s");
            let pairs: Vec<(String, String)> = parsed.query_pairs().into_owned().collect();
            proptest::prop_assert_eq!(pairs.len(), 2);
            proptest::prop_assert_eq!(pairs[0].1.as_str(), keyword.trim());
        }
    }
}
