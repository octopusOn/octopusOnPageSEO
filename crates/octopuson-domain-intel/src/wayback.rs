//! The Internet Archive's first capture of a site, a lower bound of its age.

use octopuson_core::http::{HttpRequest, HttpTransport};
use serde_json::Value;

use crate::IntelError;
use crate::dates::Day;

/// CDX query of the earliest capture of a domain's home page. An exact URL match sorts by
/// time, so the first row is the oldest capture; a domain match would sort by URL key.
pub fn query_url(domain: &str) -> String {
    format!("https://web.archive.org/cdx/search/cdx?url={domain}/&limit=1&fl=timestamp&output=json")
}

/// Reads a CDX JSON answer: a header row, then `[["timestamp"], ["19970101000000"]]`.
///
/// # Errors
/// [`IntelError::Parse`] for JSON that is not a CDX table.
pub fn parse(json: &str) -> Result<Option<Day>, IntelError> {
    if json.trim().is_empty() {
        return Ok(None);
    }
    let rows: Vec<Vec<Value>> = serde_json::from_str(json)
        .map_err(|error| IntelError::Parse(format!("Wayback CDX: {error}")))?;
    Ok(rows
        .get(1)
        .and_then(|row| row.first())
        .and_then(Value::as_str)
        .and_then(|timestamp| {
            Day::new(
                timestamp.get(..4)?.parse().ok()?,
                timestamp.get(4..6)?.parse().ok()?,
                timestamp.get(6..8)?.parse().ok()?,
            )
        }))
}

/// The date of the earliest capture of a domain's home page, if the archive has one.
///
/// # Errors
/// [`IntelError::Network`], [`IntelError::RateLimited`] or [`IntelError::Parse`].
pub fn first_capture(
    transport: &dyn HttpTransport,
    domain: &str,
) -> Result<Option<Day>, IntelError> {
    let response = transport
        .send(&HttpRequest::get(query_url(domain)))
        .map_err(|error| IntelError::Network(error.to_string()))?;
    match response.status {
        200 => parse(&response.text()),
        429 => Err(IntelError::RateLimited),
        status => Err(IntelError::Network(format!(
            "the Wayback Machine answered HTTP {status}"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use octopuson_core::http::testing::FakeTransport;

    use super::*;

    #[test]
    fn reads_the_first_capture() {
        assert_eq!(
            parse(r#"[["timestamp"],["19980111234512"]]"#)
                .unwrap()
                .map(|day| day.to_string())
                .as_deref(),
            Some("1998-01-11")
        );
        assert_eq!(parse("[]").unwrap(), None);
        assert_eq!(parse("").unwrap(), None);
        assert!(parse("<html>").is_err());
        let transport = FakeTransport::new()
            .reply(200, r#"[["timestamp"],["20050101000000"]]"#)
            .reply(429, "");
        assert!(first_capture(&transport, "ornek.com.tr").unwrap().is_some());
        assert!(
            transport.requests()[0]
                .url
                .contains("url=ornek.com.tr/&limit=1")
        );
        assert!(matches!(
            first_capture(&transport, "x.com"),
            Err(IntelError::RateLimited)
        ));
    }
}
