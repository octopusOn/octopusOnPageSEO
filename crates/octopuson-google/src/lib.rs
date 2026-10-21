//! Google's read-only data APIs for the GEO-SEO Platform, with the user's OAuth access token
//! (`octopuson-oauth` signs in and refreshes it):
//!
//! - [`search_console`]: the user's properties and search analytics (clicks, impressions,
//!   CTR and average position) by date, query, page, country or device, 25,000 rows a request;
//! - [`analytics`]: the user's Google Analytics 4 properties and organic search sessions,
//!   engaged sessions, key events and revenue by landing page or by day.
//!
//! Requests go only to Google's API hosts; the token is never part of an error.

pub mod analytics;
pub mod search_console;

use octopuson_core::http::{HttpRequest, HttpResponse, HttpTransport};
use serde_json::Value;

/// Hosts requests may go to.
const HOSTS: &[&str] = &[
    "www.googleapis.com",
    "searchconsole.googleapis.com",
    "analyticsdata.googleapis.com",
    "analyticsadmin.googleapis.com",
];

/// Why Google did not answer.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum GoogleError {
    /// The access token was refused: sign in again.
    #[error("Google refused the sign-in: {0}")]
    Auth(String),
    /// No permission for the property, or the API is not enabled for the OAuth client.
    #[error("Google refused access: {0}")]
    Forbidden(String),
    /// Too many requests.
    #[error("Google's quota is used up for now")]
    RateLimited {
        /// Seconds to wait, when known.
        retry_after: Option<u64>,
    },
    /// Google or the network failed.
    #[error("Google is unavailable: {0}")]
    Unavailable(String),
    /// The request was refused or the answer is unreadable.
    #[error("Google: {0}")]
    Invalid(String),
}

/// Google's message in an error answer (`{"error": {"message": …}}`).
fn message_of(response: &HttpResponse) -> String {
    serde_json::from_slice::<Value>(&response.body)
        .ok()
        .and_then(|value| {
            let error = value.get("error")?;
            error
                .get("message")
                .or_else(|| error.get("status"))
                .and_then(Value::as_str)
                .map(str::to_owned)
        })
        .unwrap_or_else(|| format!("HTTP {}", response.status))
}

/// Sends a request with the access token and reads the JSON answer.
pub(crate) fn send(
    transport: &dyn HttpTransport,
    token: &str,
    request: HttpRequest,
) -> Result<Value, GoogleError> {
    let host = url::Url::parse(&request.url)
        .ok()
        .and_then(|url| url.host_str().map(str::to_owned));
    if !host.as_deref().is_some_and(|host| HOSTS.contains(&host)) {
        return Err(GoogleError::Invalid(format!(
            "not a Google API address: {}",
            request.url
        )));
    }
    let request = request.header("Authorization", format!("Bearer {token}"));
    let response = transport
        .send(&request)
        .map_err(|error| GoogleError::Unavailable(error.to_string().replace(token, "…")))?;
    if !response.is_success() {
        let message = message_of(&response).replace(token, "…");
        return Err(match response.status {
            401 => GoogleError::Auth(message),
            403 => GoogleError::Forbidden(message),
            429 => GoogleError::RateLimited {
                retry_after: response.retry_after(),
            },
            500..=599 => GoogleError::Unavailable(message),
            _ => GoogleError::Invalid(message),
        });
    }
    serde_json::from_slice(&response.body)
        .map_err(|error| GoogleError::Invalid(format!("unreadable answer: {error}")))
}

/// A JSON body for a POST.
pub(crate) fn post(url: &str, body: &Value) -> HttpRequest {
    HttpRequest::post(url, body.to_string().into_bytes()).header("Content-Type", "application/json")
}

#[cfg(test)]
mod tests;
