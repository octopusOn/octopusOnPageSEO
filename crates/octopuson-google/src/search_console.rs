//! Google Search Console (read only): the user's properties, and search analytics of web search
//! by date, query, page, country or device.

use octopuson_core::http::{HttpRequest, HttpTransport};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use url::form_urlencoded::byte_serialize;

use crate::{GoogleError, post, send};

/// API base.
pub const BASE: &str = "https://www.googleapis.com/webmasters/v3";

/// Rows a request returns at most.
pub const ROWS: u32 = 25_000;

/// A Search Console property.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Site {
    /// `sc-domain:example.com` (a domain property) or `https://www.example.com/` (a prefix).
    pub site_url: String,
    /// `siteOwner`, `siteFullUser`, `siteRestrictedUser`.
    pub permission: String,
}

/// The properties the user can read, sorted.
///
/// # Errors
/// [`GoogleError`] when Search Console refuses or cannot be reached.
pub fn sites(transport: &dyn HttpTransport, token: &str) -> Result<Vec<Site>, GoogleError> {
    let value = send(transport, token, HttpRequest::get(format!("{BASE}/sites")))?;
    let mut sites: Vec<Site> = value
        .get("siteEntry")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|entry| {
            Some(Site {
                site_url: entry.get("siteUrl")?.as_str()?.to_owned(),
                permission: entry
                    .get("permissionLevel")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
            })
        })
        .filter(|site| site.permission != "siteUnverifiedUser")
        .collect();
    sites.sort_by(|left, right| left.site_url.cmp(&right.site_url));
    Ok(sites)
}

/// The host a property covers (`sc-domain:example.com` → `example.com`).
fn host_of(site_url: &str) -> String {
    let host = match site_url.strip_prefix("sc-domain:") {
        Some(domain) => domain,
        None => site_url
            .split_once("://")
            .map_or(site_url, |(_, rest)| rest)
            .split('/')
            .next()
            .unwrap_or_default(),
    };
    let host = host.to_ascii_lowercase();
    host.strip_prefix("www.").unwrap_or(&host).to_owned()
}

/// The property that best covers `domain`: its domain property, else its https prefix
/// (with or without `www.`), else any property of the same host.
pub fn matching<'a>(sites: &'a [Site], domain: &str) -> Option<&'a Site> {
    let domain = host_of(domain);
    let rank = |site: &Site| {
        if site.site_url == format!("sc-domain:{domain}") {
            0
        } else if site.site_url.starts_with("https://") {
            1
        } else {
            2
        }
    };
    sites
        .iter()
        .filter(|site| host_of(&site.site_url) == domain)
        .min_by_key(|site| rank(site))
}

/// What search analytics are split by.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Dimension {
    /// `YYYY-MM-DD`, in Pacific time.
    Date,
    /// The search query.
    Query,
    /// The page's address.
    Page,
    /// Three-letter country code (`tur`).
    Country,
    /// `DESKTOP`, `MOBILE` or `TABLET`.
    Device,
}

impl Dimension {
    /// The API's name.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Date => "date",
            Self::Query => "query",
            Self::Page => "page",
            Self::Country => "country",
            Self::Device => "device",
        }
    }
}

/// A search analytics question.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Search<'a> {
    /// The property.
    pub site_url: &'a str,
    /// First day, `YYYY-MM-DD`.
    pub start: &'a str,
    /// Last day, `YYYY-MM-DD`.
    pub end: &'a str,
    /// Split by these, in this order.
    pub dimensions: &'a [Dimension],
    /// Rows at most (up to [`ROWS`] a request).
    pub max_rows: u32,
}

/// One row: its keys in the order of the dimensions, and its numbers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Row {
    /// One per dimension.
    pub keys: Vec<String>,
    /// Clicks.
    pub clicks: f64,
    /// Impressions.
    pub impressions: f64,
    /// Click-through rate, 0–1.
    pub ctr: f64,
    /// Average position, from 1.
    pub position: f64,
}

fn rows_of(value: &Value) -> Vec<Row> {
    let number =
        |row: &Value, name: &str| row.get(name).and_then(Value::as_f64).unwrap_or_default();
    value
        .get("rows")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|row| Row {
            keys: row
                .get("keys")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|key| key.as_str().map(str::to_owned))
                .collect(),
            clicks: number(row, "clicks"),
            impressions: number(row, "impressions"),
            ctr: number(row, "ctr"),
            position: number(row, "position"),
        })
        .collect()
}

/// Search analytics of web search, final data, by the clicks Google sorts them by; asks page
/// after page until `max_rows` or the last row.
///
/// # Errors
/// [`GoogleError`] when Search Console refuses or cannot be reached.
pub fn search(
    transport: &dyn HttpTransport,
    token: &str,
    search: &Search<'_>,
) -> Result<Vec<Row>, GoogleError> {
    let site: String = byte_serialize(search.site_url.as_bytes()).collect();
    let url = format!("{BASE}/sites/{site}/searchAnalytics/query");
    let dimensions: Vec<&str> = search.dimensions.iter().map(|d| d.as_str()).collect();
    let mut rows: Vec<Row> = Vec::new();
    loop {
        let wanted = (search
            .max_rows
            .saturating_sub(u32::try_from(rows.len()).unwrap_or(u32::MAX)))
        .min(ROWS);
        if wanted == 0 {
            break;
        }
        let body = json!({
            "startDate": search.start,
            "endDate": search.end,
            "dimensions": dimensions,
            "type": "web",
            "dataState": "final",
            "rowLimit": wanted,
            "startRow": rows.len(),
        });
        let page = rows_of(&send(transport, token, post(&url, &body))?);
        let full = u32::try_from(page.len()).unwrap_or(u32::MAX) >= wanted;
        rows.extend(
            page.into_iter()
                .filter(|row| row.keys.len() == dimensions.len()),
        );
        if !full {
            break;
        }
    }
    Ok(rows)
}
