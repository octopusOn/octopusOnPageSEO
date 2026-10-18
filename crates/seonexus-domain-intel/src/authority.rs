//! Link authority: Open PageRank (free, from the Common Crawl link graph) and DataForSEO
//! backlink summaries (paid, optional).

use std::collections::BTreeMap;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use seonexus_core::http::{HttpRequest, HttpTransport};
use serde_json::{Value, json};

use crate::IntelError;

/// Open PageRank's bulk endpoint (Keywords Everywhere).
pub const OPEN_PAGERANK_URL: &str = "https://openpagerank.keywordseverywhere.com/v1/domains/bulk";

/// Most domains per Open PageRank request.
pub const OPEN_PAGERANK_BATCH: usize = 100;

/// Open PageRank of a domain.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PageRank {
    /// Score from 0 to 10.
    pub score: f64,
    /// Global rank.
    pub rank: Option<u64>,
    /// Referring domains in the link graph.
    pub referring_domains: Option<u64>,
}

fn number(value: &Value) -> Option<f64> {
    value
        .as_f64()
        .or_else(|| value.as_str().and_then(|text| text.trim().parse().ok()))
}

fn whole(value: &Value) -> Option<u64> {
    value
        .as_u64()
        .or_else(|| value.as_str().and_then(|text| text.trim().parse().ok()))
}

/// Open PageRank of up to [`OPEN_PAGERANK_BATCH`] domains per request; domains the graph does
/// not know are left out.
///
/// # Errors
/// [`IntelError::Auth`] for a rejected key, [`IntelError::RateLimited`], network and parse
/// errors.
pub fn open_pagerank(
    transport: &dyn HttpTransport,
    key: &str,
    domains: &[String],
) -> Result<BTreeMap<String, PageRank>, IntelError> {
    let mut ranks = BTreeMap::new();
    for batch in domains.chunks(OPEN_PAGERANK_BATCH) {
        let body = json!({"domains": batch, "include_history": false});
        let request = HttpRequest::post(OPEN_PAGERANK_URL, body.to_string().into_bytes())
            .header("Authorization", format!("Bearer {key}"))
            .header("Content-Type", "application/json");
        let response = transport
            .send(&request)
            .map_err(|error| IntelError::Network(error.to_string()))?;
        match response.status {
            200 => {}
            401 | 403 => {
                return Err(IntelError::Auth(
                    "Open PageRank rejected the API key".to_owned(),
                ));
            }
            429 => return Err(IntelError::RateLimited),
            status => {
                return Err(IntelError::Network(format!(
                    "Open PageRank answered HTTP {status}"
                )));
            }
        }
        let value: Value = serde_json::from_slice(&response.body)
            .map_err(|error| IntelError::Parse(format!("Open PageRank: {error}")))?;
        for result in value
            .get("results")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let found = result.get("found").and_then(Value::as_bool).unwrap_or(true);
            let (Some(domain), Some(score)) = (
                result.get("domain").and_then(Value::as_str),
                result.get("open_page_rank").and_then(number),
            ) else {
                continue;
            };
            if found {
                ranks.insert(
                    domain.to_ascii_lowercase(),
                    PageRank {
                        score: score.clamp(0.0, 10.0),
                        rank: result.get("rank").and_then(whole),
                        referring_domains: result.get("referring_domains").and_then(whole),
                    },
                );
            }
        }
    }
    Ok(ranks)
}

/// DataForSEO backlink summary of a domain.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BacklinkSummary {
    /// Backlinks.
    pub backlinks: u64,
    /// Referring domains.
    pub referring_domains: u64,
    /// DataForSEO's domain rank (0–1000).
    pub rank: Option<u64>,
}

/// DataForSEO's backlink summary endpoint.
pub const BACKLINKS_URL: &str = "https://api.dataforseo.com/v3/backlinks/summary/live";

/// Backlink summary of a domain and the US dollars DataForSEO charged.
///
/// # Errors
/// [`IntelError::Auth`], [`IntelError::Quota`], network and parse errors.
pub fn backlink_summary(
    transport: &dyn HttpTransport,
    credentials: &str,
    domain: &str,
) -> Result<(BacklinkSummary, Option<f64>), IntelError> {
    let body = json!([{"target": domain, "include_subdomains": true}]);
    let request = HttpRequest::post(BACKLINKS_URL, body.to_string().into_bytes())
        .header(
            "Authorization",
            format!("Basic {}", STANDARD.encode(credentials)),
        )
        .header("Content-Type", "application/json");
    let response = transport
        .send(&request)
        .map_err(|error| IntelError::Network(error.to_string()))?;
    if matches!(response.status, 401 | 403) {
        return Err(IntelError::Auth(
            "DataForSEO rejected the credentials".to_owned(),
        ));
    }
    if !response.is_success() {
        return Err(IntelError::Network(format!(
            "DataForSEO answered HTTP {}",
            response.status
        )));
    }
    let value: Value = serde_json::from_slice(&response.body)
        .map_err(|error| IntelError::Parse(format!("DataForSEO: {error}")))?;
    let status = value.pointer("/tasks/0/status_code").and_then(whole);
    match status {
        Some(20_000) => {}
        Some(40_100..=40_104) => {
            return Err(IntelError::Auth(
                "DataForSEO rejected the credentials".to_owned(),
            ));
        }
        Some(40_200 | 40_210) => {
            return Err(IntelError::Quota(
                "the DataForSEO balance is exhausted".to_owned(),
            ));
        }
        other => {
            return Err(IntelError::Parse(format!(
                "DataForSEO status {}",
                other.map_or_else(|| "missing".to_owned(), |code| code.to_string())
            )));
        }
    }
    let result = value
        .pointer("/tasks/0/result/0")
        .ok_or_else(|| IntelError::NotFound(domain.to_owned()))?;
    Ok((
        BacklinkSummary {
            backlinks: result.get("backlinks").and_then(whole).unwrap_or(0),
            referring_domains: result.get("referring_domains").and_then(whole).unwrap_or(0),
            rank: result.get("rank").and_then(whole),
        },
        value.get("cost").and_then(Value::as_f64),
    ))
}

#[cfg(test)]
mod tests {
    use seonexus_core::http::testing::FakeTransport;

    use super::*;

    #[test]
    fn reads_open_pagerank_results() {
        let transport = FakeTransport::new().reply(
            200,
            json!({"as_of": "2026-10-01", "count": 2, "results": [
                {"domain": "wikipedia.org", "found": true, "open_page_rank": 9.2, "rank": 4, "referring_domains": 1_200_000},
                {"domain": "nope.example", "found": false, "open_page_rank": null}
            ]})
            .to_string(),
        );
        let ranks = open_pagerank(
            &transport,
            "k3y",
            &["wikipedia.org".to_owned(), "nope.example".to_owned()],
        )
        .unwrap();
        assert_eq!(ranks.len(), 1);
        assert_eq!(ranks["wikipedia.org"].referring_domains, Some(1_200_000));
        let request = &transport.requests()[0];
        assert!(
            request
                .headers
                .contains(&("Authorization".to_owned(), "Bearer k3y".to_owned()))
        );
        let unauthorised = FakeTransport::new().reply(401, "");
        assert!(matches!(
            open_pagerank(&unauthorised, "x", &["a.com".to_owned()]),
            Err(IntelError::Auth(_))
        ));
    }

    #[test]
    fn reads_backlink_summaries() {
        let transport = FakeTransport::new().reply(
            200,
            json!({"cost": 0.02, "tasks": [{"status_code": 20000, "result": [
                {"target": "ornek.com.tr", "rank": 312, "backlinks": 5400, "referring_domains": 210}
            ]}]})
            .to_string(),
        );
        let (summary, cost) = backlink_summary(&transport, "me:pw", "ornek.com.tr").unwrap();
        assert_eq!(summary.referring_domains, 210);
        assert_eq!(cost, Some(0.02));
        let broke =
            FakeTransport::new().reply(200, json!({"tasks": [{"status_code": 40210}]}).to_string());
        assert!(matches!(
            backlink_summary(&broke, "me:pw", "x.com"),
            Err(IntelError::Quota(_))
        ));
    }
}
