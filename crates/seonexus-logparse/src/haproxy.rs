//! HAProxy HTTP logs (`option httplog`), with or without a syslog prefix.
//!
//! ```text
//! [syslog prefix] client:port [accept_date] frontend backend/server TR/Tw/Tc/Tr/Ta status bytes
//!   req_cookie res_cookie flags actconn/feconn/beconn/srv_conn/retries srv_queue/backend_queue
//!   [{captured request headers}] [{captured response headers}] "request"
//! ```
//!
//! Captured request headers have no names; a value starting with `http://` or `https://` is
//! taken as the referer, a bare host name as the host and anything else as the user agent
//! (configure `capture request header User-Agent`, `Host` and `Referer` to fill them). The
//! accept date has no zone and is taken as UTC.

use std::borrow::Cow;

use crate::error::{Field, ParseError, ParseErrorKind};
use crate::format::LogFormat;
use crate::record::LogRecord;
use crate::structured::parse_ip_maybe_port;
use crate::time;
use crate::tokens::{split_tokens, unwrap};
use crate::values;

/// HAProxy HTTP log format.
#[derive(Debug, Clone, Copy, Default)]
pub struct HaproxyFormat;

impl LogFormat for HaproxyFormat {
    #[allow(
        clippy::unnecessary_literal_bound,
        reason = "signature is fixed by the LogFormat trait"
    )]
    fn id(&self) -> &str {
        "haproxy"
    }

    #[allow(
        clippy::unnecessary_literal_bound,
        reason = "signature is fixed by the LogFormat trait"
    )]
    fn display_name(&self) -> &str {
        "HAProxy (HTTP log)"
    }

    fn specificity(&self) -> usize {
        8
    }

    fn parse<'a>(&self, line: &'a str) -> Result<LogRecord<'a>, ParseError> {
        let layout = || ParseError::new(ParseErrorKind::LayoutMismatch, 0);
        let invalid = |field| ParseError::new(ParseErrorKind::InvalidValue(field), 0);
        if line.trim().is_empty() {
            return Err(ParseError::new(ParseErrorKind::EmptyLine, 0));
        }
        let tokens = split_tokens(line.trim_end()).ok_or_else(layout)?;
        // The client address is the first `ip:port` followed by a bracketed accept date.
        let start = tokens
            .windows(2)
            .position(|pair| {
                pair[0].contains(':')
                    && pair[1].starts_with('[')
                    && parse_ip_maybe_port(pair[0]).is_some()
                    && time::parse_haproxy(unwrap(pair[1])).is_some()
            })
            .ok_or_else(layout)?;
        let field = |offset: usize| tokens.get(start + offset).copied().ok_or_else(layout);

        let client_ip = parse_ip_maybe_port(field(0)?).ok_or(invalid(Field::ClientIp))?;
        let timestamp = time::parse_haproxy(unwrap(field(1)?)).ok_or(invalid(Field::Timestamp))?;
        let duration_micros = field(4)?
            .rsplit('/')
            .next()
            .and_then(values::parse_u64)
            .map(|ms| ms * 1_000);
        let status = values::parse_status(field(5)?).ok_or(invalid(Field::Status))?;
        let bytes = values::parse_optional_u64(field(6)?).map_err(|()| invalid(Field::Bytes))?;

        let rest = tokens.get(start + 12..).ok_or_else(layout)?;
        let request_token =
            rest.iter()
                .rev()
                .find(|t| t.starts_with('"'))
                .ok_or(ParseError::new(
                    ParseErrorKind::MissingValue(Field::Request),
                    0,
                ))?;
        let request =
            values::parse_request_line(unwrap(request_token)).ok_or(invalid(Field::Request))?;

        let mut user_agent = None;
        let mut referer = None;
        let mut host = None;
        if let Some(headers) = rest.iter().find(|t| t.starts_with('{')) {
            for value in unwrap(headers).split('|').map(str::trim) {
                if value.is_empty() {
                    continue;
                }
                if value.starts_with("http://") || value.starts_with("https://") {
                    referer.get_or_insert(Cow::Borrowed(value));
                } else if is_host_name(value) {
                    host.get_or_insert(Cow::Borrowed(value));
                } else {
                    user_agent.get_or_insert(Cow::Borrowed(value));
                }
            }
        }
        Ok(LogRecord {
            timestamp,
            client_ip,
            method: Cow::Borrowed(request.method),
            target: Cow::Borrowed(request.target),
            protocol: Cow::Borrowed(request.protocol),
            status,
            bytes,
            referer,
            user_agent,
            duration_micros,
            host,
            router: None,
            upstream: None,
            request_seq: None,
        })
    }
}

/// `example.com` or `example.com:8080`, but not `curl/8.0` or `Mozilla/5.0 (…)`.
fn is_host_name(value: &str) -> bool {
    value.contains('.')
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b':'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_syslog_lines_with_captured_headers() {
        let line = r#"Oct  7 18:39:53 lb1 haproxy[14389]: 66.249.66.1:33317 [07/Oct/2026:18:39:53.655] https-in~ web/srv1 10/0/30/69/109 200 2750 - - ---- 1/1/1/1/0 0/0 {www.example.com|Mozilla/5.0 (compatible; Googlebot/2.1)|https://www.google.com/} {} "GET /index.html?x=1 HTTP/1.1""#;
        let record = HaproxyFormat.parse(line).unwrap();
        assert_eq!(record.client_ip.to_string(), "66.249.66.1");
        assert_eq!(record.timestamp.to_string(), "2026-10-07T18:39:53.655000Z");
        assert_eq!(record.status, 200);
        assert_eq!(record.bytes, Some(2750));
        assert_eq!(record.duration_micros, Some(109_000));
        assert_eq!(record.target, "/index.html?x=1");
        assert_eq!(record.host.as_deref(), Some("www.example.com"));
        assert_eq!(
            record.user_agent.as_deref(),
            Some("Mozilla/5.0 (compatible; Googlebot/2.1)")
        );
        assert_eq!(record.referer.as_deref(), Some("https://www.google.com/"));
    }

    #[test]
    fn parses_lines_without_prefix_or_headers() {
        let line = r#"10.0.1.2:33317 [07/Oct/2026:18:39:53.655] http-in static/srv1 10/0/30/-1/109 503 212 - - SC-- 1/1/1/1/0 0/0 "GET / HTTP/1.1""#;
        let record = HaproxyFormat.parse(line).unwrap();
        assert_eq!(record.status, 503);
        assert_eq!(record.user_agent, None);
    }

    #[test]
    fn rejects_tcp_logs_and_other_formats() {
        let tcp = "10.0.1.2:33317 [07/Oct/2026:18:39:53.655] fe be/srv 0/0/5 212 -- 1/1/1/1/0 0/0";
        assert!(HaproxyFormat.parse(tcp).is_err());
        let combined =
            r#"66.249.66.1 - - [07/Oct/2026:18:39:53 +0000] "GET / HTTP/1.1" 200 5 "-" "x""#;
        assert!(HaproxyFormat.parse(combined).is_err());
    }
}
