//! Disjoint response groups shared by overview counts and filtered tables/exports.

use serde::{Deserialize, Serialize};

/// Stored response outcome. HTTP status takes precedence over diagnostic flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ResponseGroup {
    /// A 2xx HTTP response was recorded.
    Success,
    /// A 3xx HTTP response was recorded.
    Redirects,
    /// A 4xx HTTP response was recorded.
    ClientError,
    /// A 5xx HTTP response was recorded.
    ServerError,
    /// A recorded status outside 2xx–5xx.
    OtherHttp,
    /// No HTTP status, and access was blocked by robots.txt.
    RobotsBlocked,
    /// No HTTP status or robots block, and a request error was recorded.
    RequestError,
    /// No HTTP status, robots block or recorded request error.
    NoResponse,
}

impl ResponseGroup {
    pub(crate) const ALL: [Self; 8] = [
        Self::Success,
        Self::Redirects,
        Self::ClientError,
        Self::ServerError,
        Self::OtherHttp,
        Self::RobotsBlocked,
        Self::RequestError,
        Self::NoResponse,
    ];

    // Column names come only from the two built-in callers, never user input.
    pub(crate) fn predicate(self, status: &str) -> String {
        match self {
            Self::Success => format!("{status} BETWEEN 200 AND 299"),
            Self::Redirects => format!("{status} BETWEEN 300 AND 399"),
            Self::ClientError => format!("{status} BETWEEN 400 AND 499"),
            Self::ServerError => format!("{status} BETWEEN 500 AND 599"),
            Self::OtherHttp => format!("{status} IS NOT NULL AND {status} NOT BETWEEN 200 AND 599"),
            Self::RobotsBlocked => format!("{status} IS NULL AND blocked_by_robots"),
            Self::RequestError => {
                format!("{status} IS NULL AND NOT blocked_by_robots AND error IS NOT NULL")
            }
            Self::NoResponse => {
                format!("{status} IS NULL AND NOT blocked_by_robots AND error IS NULL")
            }
        }
    }
}

/// One response group and its exact URL count, including zero-count groups.
#[derive(Debug, Clone, Serialize)]
pub struct ResponseCount {
    /// Response outcome.
    pub group: ResponseGroup,
    /// Number of stored URLs in this group.
    pub count: u64,
}
