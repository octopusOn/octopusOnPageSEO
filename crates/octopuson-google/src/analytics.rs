//! Google Analytics 4 (read only): the user's properties, and organic search sessions, engaged
//! sessions, key events and revenue by landing page or by day.

use octopuson_core::http::{HttpRequest, HttpTransport};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::{GoogleError, post, send};

/// Admin API base.
pub const ADMIN: &str = "https://analyticsadmin.googleapis.com/v1beta";

/// Data API base.
pub const DATA: &str = "https://analyticsdata.googleapis.com/v1beta";

/// Rows a report request returns at most.
pub const ROWS: u32 = 10_000;

/// Pages of account summaries read at most.
const SUMMARY_PAGES: usize = 20;

/// The channel group of organic search traffic.
const ORGANIC: &str = "Organic Search";

/// A GA4 property.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Property {
    /// `properties/123456`.
    pub id: String,
    /// Its display name.
    pub name: String,
    /// The account's display name.
    pub account: String,
}

/// `properties/123` from `123` or `properties/123`; `None` when it is not a property id.
pub fn property_id(raw: &str) -> Option<String> {
    let raw = raw.trim();
    let id = raw.strip_prefix("properties/").unwrap_or(raw);
    (!id.is_empty() && id.len() <= 20 && id.bytes().all(|b| b.is_ascii_digit()))
        .then(|| format!("properties/{id}"))
}

/// The properties the user can read, by account then name.
///
/// # Errors
/// [`GoogleError`] when the Admin API refuses or cannot be reached.
pub fn properties(
    transport: &dyn HttpTransport,
    token: &str,
) -> Result<Vec<Property>, GoogleError> {
    let mut properties = Vec::new();
    let mut page_token = String::new();
    for _ in 0..SUMMARY_PAGES {
        let mut url = url::Url::parse(&format!("{ADMIN}/accountSummaries"))
            .map_err(|error| GoogleError::Invalid(error.to_string()))?;
        url.query_pairs_mut().append_pair("pageSize", "200");
        if !page_token.is_empty() {
            url.query_pairs_mut().append_pair("pageToken", &page_token);
        }
        let value = send(transport, token, HttpRequest::get(url.as_str()))?;
        for account in value
            .get("accountSummaries")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let account_name = account
                .get("displayName")
                .and_then(Value::as_str)
                .unwrap_or_default();
            for property in account
                .get("propertySummaries")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                if let Some(id) = property.get("property").and_then(Value::as_str) {
                    properties.push(Property {
                        id: id.to_owned(),
                        name: property
                            .get("displayName")
                            .and_then(Value::as_str)
                            .unwrap_or(id)
                            .to_owned(),
                        account: account_name.to_owned(),
                    });
                }
            }
        }
        value
            .get("nextPageToken")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .clone_into(&mut page_token);
        if page_token.is_empty() {
            break;
        }
    }
    properties.sort_by(|left, right| {
        (left.account.as_str(), left.name.as_str())
            .cmp(&(right.account.as_str(), right.name.as_str()))
    });
    properties.dedup_by(|left, right| left.id == right.id);
    Ok(properties)
}

/// What an organic report is split by.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Breakdown {
    /// The landing page's path (`/shoes/`), most sessions first.
    LandingPage,
    /// The day, `YYYY-MM-DD`, oldest first.
    Date,
}

impl Breakdown {
    const fn dimension(self) -> &'static str {
        match self {
            Self::LandingPage => "landingPage",
            Self::Date => "date",
        }
    }
}

/// One row of an organic report.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrganicRow {
    /// The landing page's path or the day.
    pub key: String,
    /// Sessions.
    pub sessions: f64,
    /// Engaged sessions.
    pub engaged_sessions: f64,
    /// Key events (conversions).
    pub key_events: f64,
    /// Total revenue in the property's currency.
    pub revenue: f64,
}

/// An organic report.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Organic {
    /// Its rows.
    pub rows: Vec<OrganicRow>,
    /// Google sampled or thresholded the data, or more rows exist than were read.
    pub limited: bool,
    /// The property's currency (`TRY`), when reported.
    pub currency: Option<String>,
}

const METRICS: [&str; 4] = ["sessions", "engagedSessions", "keyEvents", "totalRevenue"];

/// Organic search traffic of a property between two days (`YYYY-MM-DD`), up to `max_rows`.
///
/// # Errors
/// [`GoogleError::Invalid`] for a property id that is not one; [`GoogleError`] when the Data
/// API refuses or cannot be reached.
pub fn organic(
    transport: &dyn HttpTransport,
    token: &str,
    property: &str,
    (start, end): (&str, &str),
    breakdown: Breakdown,
    max_rows: u32,
) -> Result<Organic, GoogleError> {
    let property = property_id(property)
        .ok_or_else(|| GoogleError::Invalid(format!("`{property}` is not a GA4 property id")))?;
    let url = format!("{DATA}/{property}:runReport");
    let order = match breakdown {
        Breakdown::LandingPage => json!([{ "metric": { "metricName": "sessions" }, "desc": true }]),
        Breakdown::Date => json!([{ "dimension": { "dimensionName": "date" } }]),
    };
    let mut report = Organic {
        rows: Vec::new(),
        limited: false,
        currency: None,
    };
    loop {
        let read = u32::try_from(report.rows.len()).unwrap_or(u32::MAX);
        let wanted = max_rows.saturating_sub(read).min(ROWS);
        if wanted == 0 {
            // More rows than asked for.
            report.limited = true;
            break;
        }
        let body = json!({
            "dateRanges": [{ "startDate": start, "endDate": end }],
            "dimensions": [{ "name": breakdown.dimension() }],
            "metrics": METRICS.iter().map(|name| json!({ "name": name })).collect::<Vec<_>>(),
            "dimensionFilter": { "filter": {
                "fieldName": "sessionDefaultChannelGroup",
                "stringFilter": { "matchType": "EXACT", "value": ORGANIC }
            }},
            "orderBys": order,
            "offset": read.to_string(),
            "limit": wanted.to_string(),
        });
        let value = send(transport, token, post(&url, &body))?;
        let metadata = value.get("metadata").unwrap_or(&Value::Null);
        report.limited |= metadata
            .get("subjectToThresholding")
            .and_then(Value::as_bool)
            == Some(true)
            || metadata
                .get("samplingMetadatas")
                .and_then(Value::as_array)
                .is_some_and(|samples| !samples.is_empty());
        if report.currency.is_none() {
            report.currency = metadata
                .get("currencyCode")
                .and_then(Value::as_str)
                .map(str::to_owned);
        }
        let page: Vec<OrganicRow> = value
            .get("rows")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|row| row_of(row, breakdown))
            .collect();
        let received = u32::try_from(page.len()).unwrap_or(u32::MAX);
        report.rows.extend(page);
        let total = value
            .get("rowCount")
            .and_then(Value::as_u64)
            .unwrap_or_default();
        if received < wanted || report.rows.len() as u64 >= total {
            break;
        }
    }
    Ok(report)
}

fn row_of(row: &Value, breakdown: Breakdown) -> Option<OrganicRow> {
    let key = row.pointer("/dimensionValues/0/value")?.as_str()?;
    let key = match breakdown {
        // `20261021` → `2026-10-21`.
        Breakdown::Date if key.len() == 8 && key.bytes().all(|b| b.is_ascii_digit()) => {
            format!("{}-{}-{}", &key[..4], &key[4..6], &key[6..])
        }
        Breakdown::Date => return None,
        Breakdown::LandingPage => key.to_owned(),
    };
    let metric = |index: usize| {
        row.pointer(&format!("/metricValues/{index}/value"))
            .and_then(Value::as_str)
            .and_then(|value| value.parse::<f64>().ok())
            .filter(|value| value.is_finite() && *value >= 0.0)
            .unwrap_or_default()
    };
    Some(OrganicRow {
        key,
        sessions: metric(0),
        engaged_sessions: metric(1),
        key_events: metric(2),
        revenue: metric(3),
    })
}
