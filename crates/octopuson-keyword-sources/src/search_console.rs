//! Google Search Console (read only), with an OAuth access token: the user's properties and
//! the queries and pages of one, 25,000 rows per request.

use octopuson_core::http::HttpRequest;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use url::form_urlencoded::byte_serialize;

use crate::countries::alpha3;
use crate::{SourceContext, SourceError, parse, redact};

/// A Search Console property.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Property {
    /// `https://www.example.com/` or `sc-domain:example.com`.
    pub site_url: String,
    /// `siteOwner`, `siteFullUser`, `siteRestrictedUser` …
    pub permission: String,
}

/// One query on one page.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QueryRow {
    /// The query.
    pub query: String,
    /// The page.
    pub page: String,
    /// Clicks.
    pub clicks: f64,
    /// Impressions.
    pub impressions: f64,
    /// Average position.
    pub position: f64,
}

/// What a query request asks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueryRequest<'a> {
    /// The property.
    pub site_url: &'a str,
    /// First day, `YYYY-MM-DD`.
    pub start: &'a str,
    /// Last day, `YYYY-MM-DD`.
    pub end: &'a str,
    /// Only searches from this country (alpha-2), if any.
    pub country: Option<&'a str>,
    /// Rows to skip.
    pub start_row: u32,
}

/// Google Search Console.
#[derive(Debug, Clone, Copy, Default)]
pub struct SearchConsole;

impl SearchConsole {
    /// Source id.
    pub const ID: &'static str = "search-console";
    /// API base.
    pub const BASE: &'static str = "https://www.googleapis.com/webmasters/v3";
    /// Rows per request.
    pub const ROWS: u32 = 25_000;

    fn check(
        response: &octopuson_core::http::HttpResponse,
        token: &str,
    ) -> Result<Value, SourceError> {
        if !response.is_success() {
            let hide = |text: &str| redact(text, &[token.to_owned()]);
            return Err(SourceError::from_status(response, &hide));
        }
        parse(response, "Search Console")
    }

    /// The user's properties.
    ///
    /// # Errors
    /// [`SourceError`] when Search Console cannot be asked.
    pub fn properties(
        context: &SourceContext<'_>,
        token: &str,
    ) -> Result<Vec<Property>, SourceError> {
        let request = HttpRequest::get(format!("{}/sites", Self::BASE))
            .header("Authorization", format!("Bearer {token}"));
        let value = Self::check(&context.send(&request)?, token)?;
        let mut properties: Vec<Property> = value
            .get("siteEntry")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|entry| {
                Some(Property {
                    site_url: entry.get("siteUrl")?.as_str()?.to_owned(),
                    permission: entry
                        .get("permissionLevel")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_owned(),
                })
            })
            .filter(|property| property.permission != "siteUnverifiedUser")
            .collect();
        properties.sort_by(|a, b| a.site_url.cmp(&b.site_url));
        Ok(properties)
    }

    /// One page of query and page rows of web search, final data only.
    ///
    /// # Errors
    /// [`SourceError`] when Search Console cannot be asked.
    pub fn query(
        context: &SourceContext<'_>,
        token: &str,
        request: &QueryRequest<'_>,
    ) -> Result<Vec<QueryRow>, SourceError> {
        let mut body = json!({
            "startDate": request.start,
            "endDate": request.end,
            "dimensions": ["query", "page"],
            "type": "web",
            "dataState": "final",
            "rowLimit": Self::ROWS,
            "startRow": request.start_row,
        });
        if let Some(country) = request.country.and_then(alpha3) {
            body["dimensionFilterGroups"] = json!([{
                "filters": [{"dimension": "country", "operator": "equals", "expression": country}]
            }]);
        }
        let site: String = byte_serialize(request.site_url.as_bytes()).collect();
        let http = HttpRequest::post(
            format!("{}/sites/{site}/searchAnalytics/query", Self::BASE),
            body.to_string().into_bytes(),
        )
        .header("Authorization", format!("Bearer {token}"))
        .header("Content-Type", "application/json");
        let value = Self::check(&context.send(&http)?, token)?;
        Ok(value
            .get("rows")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|row| {
                let keys = row.get("keys")?.as_array()?;
                Some(QueryRow {
                    query: keys.first()?.as_str()?.to_owned(),
                    page: keys.get(1)?.as_str()?.to_owned(),
                    clicks: row
                        .get("clicks")
                        .and_then(Value::as_f64)
                        .unwrap_or_default(),
                    impressions: row
                        .get("impressions")
                        .and_then(Value::as_f64)
                        .unwrap_or_default(),
                    position: row
                        .get("position")
                        .and_then(Value::as_f64)
                        .unwrap_or_default(),
                })
            })
            .collect())
    }
}
