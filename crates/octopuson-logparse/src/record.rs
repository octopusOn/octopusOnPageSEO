use std::borrow::Cow;
use std::net::IpAddr;

use octopuson_core::Timestamp;

/// One successfully parsed access-log line.
///
/// String fields borrow from the input line whenever possible (`Cow::Borrowed`) so the hot
/// parsing path does not allocate. Values are kept as logged (no percent-decoding); URL
/// normalization is a separate, configurable step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogRecord<'a> {
    /// Request time in UTC.
    pub timestamp: Timestamp,
    /// Client address. When the format logs `X-Forwarded-For`, its first valid address wins.
    pub client_ip: IpAddr,
    /// HTTP method, empty if the format does not log it.
    pub method: Cow<'a, str>,
    /// Request target (path and query) exactly as logged.
    pub target: Cow<'a, str>,
    /// Protocol such as `HTTP/1.1`; empty for HTTP/0.9 or when not logged.
    pub protocol: Cow<'a, str>,
    /// HTTP status code.
    pub status: u16,
    /// Response size in bytes, `None` when logged as `-`.
    pub bytes: Option<u64>,
    /// `Referer` header, `None` when absent or `-`.
    pub referer: Option<Cow<'a, str>>,
    /// `User-Agent` header, `None` when absent or `-`.
    pub user_agent: Option<Cow<'a, str>>,
    /// Total request duration in microseconds.
    pub duration_micros: Option<u64>,
    /// Virtual host / requested host name.
    pub host: Option<Cow<'a, str>>,
    /// Reverse-proxy router name (e.g. Traefik `RouterName`).
    pub router: Option<Cow<'a, str>>,
    /// Upstream/backend URL the request was forwarded to.
    pub upstream: Option<Cow<'a, str>>,
    /// Proxy-assigned request counter (e.g. Traefik `RequestCount`).
    pub request_seq: Option<u64>,
}
