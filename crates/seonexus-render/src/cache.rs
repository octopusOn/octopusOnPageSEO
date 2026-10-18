//! A bounded, crawl-local cache. Page storage remains isolated in fresh browser contexts.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde_json::Value;
use time::{OffsetDateTime, format_description::well_known::Rfc2822};
use url::Url;

pub(crate) const ENTRY_BYTES: usize = 2 * 1024 * 1024;
const CACHE_BYTES: usize = 64 * 1024 * 1024;
const HEADER_BYTES: usize = 16 * 1024;
const MAX_ENTRIES: usize = 1024;

#[derive(Default)]
pub(crate) struct ResourceCache(Mutex<Entries>);
#[derive(Default)]
struct Entries {
    items: VecDeque<Arc<Resource>>,
    bytes: usize,
}
pub(crate) struct Resource {
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
    expires: Instant,
}
impl Resource {
    fn bytes(&self) -> usize {
        self.url.len()
            + self.body.len()
            + self
                .headers
                .iter()
                .map(|(n, v)| n.len() + v.len())
                .sum::<usize>()
    }
}

impl ResourceCache {
    pub(crate) fn get(&self, url: &str) -> Option<Arc<Resource>> {
        let mut entries = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        let index = entries.items.iter().position(|item| item.url == url)?;
        let item = entries.items.remove(index)?;
        if item.expires <= Instant::now() {
            entries.bytes -= item.bytes();
            return None;
        }
        entries.items.push_back(item.clone());
        Some(item)
    }

    pub(crate) fn insert(&self, resource: Resource) {
        if resource.body.len() > ENTRY_BYTES || resource.expires <= Instant::now() {
            return;
        }
        let mut entries = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(index) = entries
            .items
            .iter()
            .position(|item| item.url == resource.url)
            && let Some(old) = entries.items.remove(index)
        {
            entries.bytes -= old.bytes();
        }
        entries.bytes += resource.bytes();
        entries.items.push_back(Arc::new(resource));
        while entries.bytes > CACHE_BYTES || entries.items.len() > MAX_ENTRIES {
            if let Some(old) = entries.items.pop_front() {
                entries.bytes -= old.bytes();
            }
        }
    }
}

/// Only same-origin GET scripts and CSS can be reused across isolated page contexts.
pub(crate) fn reusable_request(params: &Value, page: &str) -> bool {
    let request = &params["request"];
    if !matches!(
        params["resourceType"].as_str(),
        Some("Script" | "Stylesheet")
    ) || request["method"].as_str() != Some("GET")
    {
        return false;
    }
    let (Ok(resource), Ok(page)) = (
        Url::parse(request["url"].as_str().unwrap_or_default()),
        Url::parse(page),
    ) else {
        return false;
    };
    if !matches!(resource.scheme(), "http" | "https")
        || resource.origin() != page.origin()
        || !resource.username().is_empty()
        || resource.password().is_some()
    {
        return false;
    }
    request["headers"].as_object().is_some_and(|headers| {
        !headers.keys().any(|name| {
            matches!(
                name.to_ascii_lowercase().as_str(),
                "authorization" | "cookie" | "range" | "cache-control" | "pragma"
            )
        })
    })
}

/// Response metadata retained only until its bounded body can be read from Chrome.
pub(crate) struct Response {
    url: String,
    headers: Vec<(String, String)>,
    expires: Instant,
}
impl Response {
    /// Fetch's response-stage interception runs before Chrome's body/MIME buffering.
    pub(crate) fn intercepted(params: &Value) -> Option<Self> {
        let mut headers = serde_json::Map::new();
        for header in params["responseHeaders"].as_array()? {
            if headers
                .insert(
                    header["name"].as_str()?.to_owned(),
                    Value::String(header["value"].as_str()?.to_owned()),
                )
                .is_some()
            {
                return None;
            }
        }
        Self::new(
            &serde_json::json!({"status":params["responseStatusCode"],"url":params["request"]["url"],"headers":headers}),
        )
    }

    pub(crate) fn new(value: &Value) -> Option<Self> {
        if value["status"].as_u64()? != 200 {
            return None;
        }
        let headers: Vec<_> = value["headers"]
            .as_object()?
            .iter()
            .map(|(name, value)| Some((name.to_ascii_lowercase(), value.as_str()?.to_owned())))
            .collect::<Option<_>>()?;
        if headers
            .iter()
            .map(|(n, v)| n.len() + v.len())
            .sum::<usize>()
            > HEADER_BYTES
            || headers.iter().any(|(name, _)| name == "set-cookie")
        {
            return None;
        }
        let mut names = std::collections::HashSet::new();
        if headers.iter().any(|(name, _)| !names.insert(name)) {
            return None;
        }
        let header = |name| {
            headers
                .iter()
                .find(|(n, _)| n == name)
                .map(|(_, v)| v.as_str())
        };
        if header("vary").is_some_and(|value| {
            value
                .split(',')
                .any(|v| !v.trim().eq_ignore_ascii_case("accept-encoding"))
        }) {
            return None;
        }
        let content_type = header("content-type")?.to_ascii_lowercase();
        if !matches!(
            content_type.split(';').next()?.trim(),
            "text/css" | "application/javascript" | "text/javascript" | "application/x-javascript"
        ) || content_type.split(';').skip(1).any(|p| {
            let Some((name, value)) = p.trim().split_once('=') else {
                return true;
            };
            name.trim() != "charset"
                || !matches!(
                    value.trim().trim_matches('"'),
                    "utf-8" | "utf8" | "us-ascii"
                )
        }) {
            return None;
        }
        let expires = fresh_until(header("cache-control")?, header("date")?, header("age"))?;
        let headers = headers
            .into_iter()
            .filter(|(n, _)| {
                !matches!(
                    n.as_str(),
                    "content-encoding"
                        | "content-length"
                        | "transfer-encoding"
                        | "connection"
                        | "keep-alive"
                )
            })
            .collect();
        Some(Self {
            url: value["url"].as_str()?.to_owned(),
            headers,
            expires,
        })
    }

    pub(crate) fn with_body(self, body: Vec<u8>) -> Option<Resource> {
        // CDP returns textual bodies as UTF-8. Unsupported encodings are never cached.
        let text = std::str::from_utf8(&body).ok()?;
        if body.len() > ENTRY_BYTES || text.trim_start_matches('\u{feff}').starts_with("@charset") {
            return None;
        }
        Some(Resource {
            url: self.url,
            headers: self.headers,
            body,
            expires: self.expires,
        })
    }
}

fn fresh_until(control: &str, date: &str, age: Option<&str>) -> Option<Instant> {
    let control = control.to_ascii_lowercase();
    let directives: Vec<_> = control.split(',').map(str::trim).collect();
    if !directives.contains(&"public")
        || !directives.contains(&"immutable")
        || directives.iter().any(|d| {
            matches!(
                d.split('=').next().unwrap_or_default().trim(),
                "no-store" | "private" | "no-cache"
            )
        })
    {
        return None;
    }
    let ages: Vec<_> = directives
        .iter()
        .filter_map(|d| {
            d.split_once('=')
                .filter(|(name, _)| name.trim() == "max-age")
                .map(|(_, v)| v.trim().trim_matches('"').parse::<u64>())
        })
        .collect();
    if ages.len() != 1 {
        return None;
    }
    let max_age = ages[0].as_ref().ok()?;
    // Account for both intermediary Age and an already old Date, never extending freshness.
    let age = age.map(str::parse::<u64>).transpose().ok()?.unwrap_or(0);
    let date = OffsetDateTime::parse(date, &Rfc2822).ok()?.unix_timestamp();
    let now = SystemTime::now().duration_since(UNIX_EPOCH).ok()?.as_secs();
    let apparent = now.saturating_sub(u64::try_from(date).ok()?);
    let remaining = max_age.checked_sub(age.max(apparent))?;
    if remaining == 0 {
        return None;
    }
    Instant::now().checked_add(Duration::from_secs(remaining))
}

#[cfg(test)]
mod tests;
