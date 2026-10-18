//! Bounded, read-only reconstruction of HTTP redirects already recorded by a crawl.

use std::collections::HashSet;

use duckdb::params;
use octopuson_crawl::follows_location;
use serde::Serialize;

use crate::{CrawlReader, CrawlStoreError};

/// Maximum URL rows returned in one redirect trace, including its terminal URL.
pub const MAX_REDIRECT_TRACE_ROWS: usize = 100;

/// One observed URL in a redirect chain, or its unrecorded destination.
#[derive(Debug, Clone, Serialize)]
pub struct RedirectHop {
    /// Absolute URL from the saved crawl, not fetched again.
    pub url: String,
    /// HTTP response code, if recorded.
    pub status: Option<u16>,
    /// Resolved Location target, if recorded.
    pub location: Option<String>,
    /// Stored network/request error, if any.
    pub error: Option<String>,
    /// The crawling agent was blocked by robots.txt.
    pub blocked: bool,
    /// False when the destination has no URL record in this crawl.
    pub recorded: bool,
}

/// Why a stored redirect trace ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RedirectOutcome {
    /// A recorded, non-redirect response below 400 ended the trace.
    Complete,
    /// The last response was HTTP 4xx or 5xx.
    HttpError,
    /// A recorded request failed.
    RequestError,
    /// The destination was blocked by robots.txt.
    Blocked,
    /// The destination has no recorded response or failure.
    Unchecked,
    /// An automatic Location redirect has no usable saved target.
    MissingLocation,
    /// The last URL repeats an earlier URL in this trace.
    Loop,
    /// More rows are needed to establish how the trace ends.
    Limit,
}

/// Ordered observed redirect steps with an explicit terminal state.
#[derive(Debug, Clone, Serialize)]
pub struct RedirectTrace {
    /// URL rows, including a repeated or unchecked terminal URL when present.
    pub hops: Vec<RedirectHop>,
    /// Observed terminal state; unchecked and limited traces are inconclusive.
    pub outcome: RedirectOutcome,
}

impl CrawlReader {
    /// Reconstructs HTTP Location redirects using saved records only.
    ///
    /// Returns at most [`MAX_REDIRECT_TRACE_ROWS`] rows. JavaScript navigation and
    /// meta refresh do not count as HTTP redirect hops.
    ///
    /// # Errors
    /// Database errors and an empty starting URL.
    pub fn redirect_trace(&self, url: &str) -> Result<RedirectTrace, CrawlStoreError> {
        if url.is_empty() {
            return Err(CrawlStoreError::InvalidSelection(
                "empty redirect URL".into(),
            ));
        }
        let mut statement = self.connection.prepare(
            "SELECT status,redirect_url,error,blocked_by_robots FROM urls WHERE url=? LIMIT 1",
        )?;
        let mut hops = Vec::new();
        let mut seen = HashSet::new();
        let mut current = url.to_owned();
        for _ in 0..MAX_REDIRECT_TRACE_ROWS {
            let row = statement.query_row(params![&current], |row| {
                Ok(RedirectHop {
                    url: current.clone(),
                    status: row.get(0)?,
                    location: row.get(1)?,
                    error: row.get(2)?,
                    blocked: row.get(3)?,
                    recorded: true,
                })
            });
            let hop = match row {
                Ok(hop) => hop,
                Err(duckdb::Error::QueryReturnedNoRows) => RedirectHop {
                    url: current.clone(),
                    status: None,
                    location: None,
                    error: None,
                    blocked: false,
                    recorded: false,
                },
                Err(error) => return Err(error.into()),
            };
            let outcome = if !seen.insert(current.clone()) {
                Some(RedirectOutcome::Loop)
            } else if hop.blocked {
                Some(RedirectOutcome::Blocked)
            } else if hop.error.is_some() {
                Some(RedirectOutcome::RequestError)
            } else {
                match hop.status {
                    None => Some(RedirectOutcome::Unchecked),
                    Some(400..) => Some(RedirectOutcome::HttpError),
                    Some(status) if follows_location(status) => {
                        match hop.location.as_ref().filter(|value| !value.is_empty()) {
                            Some(next) => {
                                current.clone_from(next);
                                None
                            }
                            None => Some(RedirectOutcome::MissingLocation),
                        }
                    }
                    Some(_) => Some(RedirectOutcome::Complete),
                }
            };
            hops.push(hop);
            if let Some(outcome) = outcome {
                return Ok(RedirectTrace { hops, outcome });
            }
        }
        Ok(RedirectTrace {
            hops,
            outcome: RedirectOutcome::Limit,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use duckdb::Connection;

    type Row<'a> = (&'a str, Option<u16>, Option<&'a str>, Option<&'a str>, bool);

    fn reader(rows: &[Row<'_>]) -> CrawlReader {
        let connection = Connection::open_in_memory().unwrap();
        connection.execute_batch("CREATE TABLE urls(url VARCHAR, status USMALLINT, redirect_url VARCHAR, error VARCHAR, blocked_by_robots BOOLEAN)").unwrap();
        for &(url, status, target, error, blocked) in rows {
            connection
                .execute(
                    "INSERT INTO urls VALUES (?,?,?,?,?)",
                    params![url, status, target, error, blocked],
                )
                .unwrap();
        }
        CrawlReader::from_connection(connection)
    }

    #[test]
    fn follows_recorded_steps_and_reports_terminal_responses_and_failures() {
        let reader = reader(&[
            ("start", Some(301), Some("middle"), None, false),
            ("middle", Some(302), Some("end"), None, false),
            ("end", Some(404), None, None, false),
            ("ok", Some(200), None, None, false),
            ("network", None, None, Some("connect: refused"), false),
            ("robots", None, None, None, true),
            ("no-location", Some(307), None, None, false),
        ]);
        let trace = reader.redirect_trace("start").unwrap();
        assert_eq!(trace.outcome, RedirectOutcome::HttpError);
        assert_eq!(
            trace
                .hops
                .iter()
                .map(|hop| hop.url.as_str())
                .collect::<Vec<_>>(),
            ["start", "middle", "end"]
        );
        assert_eq!(
            trace
                .hops
                .iter()
                .map(|hop| hop.status.unwrap())
                .collect::<Vec<_>>(),
            [301, 302, 404]
        );
        for (url, outcome) in [
            ("ok", RedirectOutcome::Complete),
            ("network", RedirectOutcome::RequestError),
            ("robots", RedirectOutcome::Blocked),
            ("no-location", RedirectOutcome::MissingLocation),
        ] {
            assert_eq!(reader.redirect_trace(url).unwrap().outcome, outcome);
        }
    }

    #[test]
    fn includes_the_closing_url_for_cycles_and_tails_entering_them() {
        let reader = reader(&[
            ("tail", Some(301), Some("a"), None, false),
            ("a", Some(302), Some("b"), None, false),
            ("b", Some(308), Some("a"), None, false),
            ("self", Some(301), Some("self"), None, false),
        ]);
        let trace = reader.redirect_trace("tail").unwrap();
        assert_eq!(trace.outcome, RedirectOutcome::Loop);
        assert_eq!(
            trace
                .hops
                .iter()
                .map(|hop| hop.url.as_str())
                .collect::<Vec<_>>(),
            ["tail", "a", "b", "a"]
        );
        assert_eq!(reader.redirect_trace("self").unwrap().hops.len(), 2);
    }

    #[test]
    fn preserves_unknown_targets_and_uses_literal_url_parameters() {
        let reader = reader(&[(
            "start",
            Some(301),
            Some("https://outside.example/?quote='"),
            None,
            false,
        )]);
        let trace = reader.redirect_trace("start").unwrap();
        assert_eq!(trace.outcome, RedirectOutcome::Unchecked);
        assert!(!trace.hops[1].recorded);
        assert_eq!(trace.hops[1].url, "https://outside.example/?quote='");
        assert_eq!(reader.redirect_trace("' OR true --").unwrap().hops.len(), 1);
        assert!(reader.redirect_trace("").is_err());
    }

    #[test]
    fn bounds_long_traces_without_claiming_the_unseen_outcome() {
        let reader = reader(&[]);
        for i in 0..=MAX_REDIRECT_TRACE_ROWS {
            let target = (i < MAX_REDIRECT_TRACE_ROWS).then(|| (i + 1).to_string());
            reader
                .connection
                .execute(
                    "INSERT INTO urls VALUES (?,?,?,?,?)",
                    params![
                        i.to_string(),
                        if target.is_some() { 301 } else { 200 },
                        target,
                        None::<String>,
                        false
                    ],
                )
                .unwrap();
        }
        let limited = reader.redirect_trace("0").unwrap();
        assert_eq!(limited.outcome, RedirectOutcome::Limit);
        assert_eq!(limited.hops.len(), MAX_REDIRECT_TRACE_ROWS);
        assert_eq!(
            limited.hops.last().unwrap().location.as_deref(),
            Some("100")
        );
        let complete = reader.redirect_trace("1").unwrap();
        assert_eq!(complete.outcome, RedirectOutcome::Complete);
        assert_eq!(complete.hops.len(), MAX_REDIRECT_TRACE_ROWS);
    }

    #[test]
    fn non_navigation_3xx_ends_a_trace_even_with_a_legacy_saved_target() {
        for status in [300, 304, 305, 306, 399] {
            for target in [None, Some("missing"), Some("source")] {
                let reader = reader(&[
                    ("start", Some(301), Some("source"), None, false),
                    ("source", Some(status), target, None, false),
                    ("missing", Some(404), None, None, false),
                ]);
                let trace = reader.redirect_trace("start").unwrap();
                assert_eq!(trace.outcome, RedirectOutcome::Complete, "HTTP {status}");
                assert_eq!(trace.hops.len(), 2);
                assert_eq!(trace.hops[1].status, Some(status));
                assert_eq!(trace.hops[1].location.as_deref(), target);
            }
        }
    }
}
