//! The port through which the engine fetches URLs.

use std::time::Duration;

/// Whether this crawler treats the status as an automatic `Location` navigation.
/// Other 3xx responses retain their HTTP family without creating a navigation edge.
pub const fn follows_location(status: u16) -> bool {
    matches!(status, 301 | 302 | 303 | 307 | 308)
}

/// Fetches one URL without following redirects. Implementations send the configured user
/// agent and give up after the configured timeout.
pub trait Fetcher: Send + Sync {
    /// Sends a GET request for `request.url`.
    ///
    /// # Errors
    /// The request got no HTTP response (DNS, connection, TLS, timeout).
    fn fetch(&self, request: &FetchRequest<'_>) -> Result<FetchResponse, FetchError>;
}

/// One request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FetchRequest<'a> {
    /// Absolute http(s) URL.
    pub url: &'a str,
    /// Body bytes to read at most; 0 reads only the status and headers.
    pub body_limit: u64,
}

/// A response, whatever its status.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FetchResponse {
    /// HTTP status code.
    pub status: u16,
    /// Headers in order, names lowercase.
    pub headers: Vec<(String, String)>,
    /// Decompressed body, up to the request's limit.
    pub body: Vec<u8>,
    /// The body was longer than the limit.
    pub truncated: bool,
    /// `HTTP/1.1`, `HTTP/2`, ...
    pub http_version: String,
    /// Time until the response headers arrived.
    pub response_time: Duration,
}

impl FetchResponse {
    /// A response with `status`, headers and body (for fetchers and tests).
    pub fn new(status: u16, headers: &[(&str, &str)], body: impl Into<Vec<u8>>) -> Self {
        Self {
            status,
            headers: headers
                .iter()
                .map(|(name, value)| (name.to_ascii_lowercase(), (*value).to_owned()))
                .collect(),
            body: body.into(),
            truncated: false,
            http_version: "HTTP/1.1".to_owned(),
            response_time: Duration::ZERO,
        }
    }

    /// The first value of header `name` (lowercase).
    pub fn header(&self, name: &str) -> Option<&str> {
        self.header_values(name).next()
    }

    /// Every value of header `name` (lowercase).
    pub fn header_values<'a>(&'a self, name: &str) -> impl Iterator<Item = &'a str> + use<'a> {
        let name = name.to_owned();
        self.headers
            .iter()
            .filter(move |(n, _)| *n == name)
            .map(|(_, value)| value.as_str())
    }
}

/// Why a request got no response.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum FetchError {
    /// No response within the timeout.
    #[error("timeout")]
    Timeout,
    /// The host name does not resolve.
    #[error("DNS lookup failed")]
    Dns,
    /// The connection was refused or reset.
    #[error("connection failed: {0}")]
    Connection(String),
    /// The TLS handshake failed (for example an invalid certificate).
    #[error("TLS error: {0}")]
    Tls(String),
    /// Anything else.
    #[error("{0}")]
    Other(String),
}

impl FetchError {
    /// Short code stored with the URL.
    pub fn code(&self) -> &'static str {
        match self {
            Self::Timeout => "timeout",
            Self::Dns => "dns",
            Self::Connection(_) => "connection",
            Self::Tls(_) => "tls",
            Self::Other(_) => "error",
        }
    }
}
