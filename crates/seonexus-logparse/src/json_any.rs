//! JSON access logs with arbitrary key names: Caddy, Cloudflare Logpush, Google Cloud load
//! balancers (`httpRequest`), Nginx `log_format … escape=json`, Akamai DataStream, Vercel
//! and similar.
//!
//! Keys are recognised by name at any nesting depth (see [`crate::structured`]); arrays of
//! strings (Caddy's header lists) contribute their first element. Response headers are
//! skipped so that e.g. a `Date` header is not mistaken for the request time.

use std::borrow::Cow;

use serde_json::Value;

use crate::error::{ParseError, ParseErrorKind};
use crate::format::LogFormat;
use crate::record::LogRecord;
use crate::structured::{Parts, normalize_name, role_of};

/// Nesting depth searched for known keys.
const MAX_DEPTH: usize = 4;

/// Objects never holding request data.
const SKIPPED_OBJECTS: [&str; 4] = ["respheaders", "responseheaders", "resource", "labels"];

/// JSON lines with recognisable key names.
#[derive(Debug, Clone, Copy, Default)]
pub struct JsonFormat;

impl LogFormat for JsonFormat {
    #[allow(
        clippy::unnecessary_literal_bound,
        reason = "signature is fixed by the LogFormat trait"
    )]
    fn id(&self) -> &str {
        "json"
    }

    #[allow(
        clippy::unnecessary_literal_bound,
        reason = "signature is fixed by the LogFormat trait"
    )]
    fn display_name(&self) -> &str {
        "JSON (Caddy, Cloudflare, Google Cloud, Nginx JSON, …)"
    }

    fn specificity(&self) -> usize {
        5
    }

    fn parse<'a>(&self, line: &'a str) -> Result<LogRecord<'a>, ParseError> {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return Err(ParseError::new(ParseErrorKind::EmptyLine, 0));
        }
        if !trimmed.starts_with('{') {
            return Err(ParseError::new(ParseErrorKind::InvalidJson, 0));
        }
        let value: Value = serde_json::from_str(trimmed).map_err(|error| {
            ParseError::new(
                ParseErrorKind::InvalidJson,
                error.column().saturating_sub(1),
            )
        })?;
        let Value::Object(object) = value else {
            return Err(ParseError::new(ParseErrorKind::InvalidJson, 0));
        };
        let mut parts = Parts::default();
        for (key, value) in object {
            visit(&key, value, 1, &mut parts);
        }
        parts.into_record()
    }
}

fn visit(key: &str, value: Value, depth: usize, parts: &mut Parts<'_>) {
    match value {
        Value::Object(object) => {
            if depth >= MAX_DEPTH || SKIPPED_OBJECTS.contains(&normalize_name(key).as_str()) {
                return;
            }
            for (child, value) in object {
                visit(&child, value, depth + 1, parts);
            }
        }
        Value::Array(items) => {
            if let Some(Value::String(first)) = items.into_iter().next() {
                set(key, first, parts);
            }
        }
        Value::String(text) => set(key, text, parts),
        Value::Number(number) => set(key, number.to_string(), parts),
        Value::Bool(_) | Value::Null => {}
    }
}

fn set(key: &str, text: String, parts: &mut Parts<'_>) {
    if let Some((role, priority)) = role_of(key) {
        parts.set(role, priority, Cow::Owned(text));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_caddy_access_logs() {
        let line = r#"{"level":"info","ts":1791398393.5241024,"logger":"http.log.access","msg":"handled request","request":{"remote_ip":"10.0.0.1","remote_port":"41342","client_ip":"66.249.66.1","proto":"HTTP/2.0","method":"GET","host":"www.example.com","uri":"/a?x=1","headers":{"User-Agent":["Mozilla/5.0 (compatible; Googlebot/2.1)"],"Referer":["https://www.google.com/"]}},"bytes_read":0,"user_id":"","duration":0.000929675,"size":10900,"status":200,"resp_headers":{"Date":["Wed, 07 Oct 2026 18:39:53 GMT"],"Content-Length":["10900"]}}"#;
        let record = JsonFormat.parse(line).unwrap();
        assert_eq!(record.client_ip.to_string(), "66.249.66.1");
        assert_eq!(record.timestamp.to_string(), "2026-10-07T18:39:53.524102Z");
        assert_eq!(record.target, "/a?x=1");
        assert_eq!(record.host.as_deref(), Some("www.example.com"));
        assert_eq!(record.status, 200);
        assert_eq!(record.bytes, Some(10_900));
        assert_eq!(record.duration_micros, Some(929));
        assert_eq!(record.protocol, "HTTP/2.0");
        assert_eq!(
            record.user_agent.as_deref(),
            Some("Mozilla/5.0 (compatible; Googlebot/2.1)")
        );
        assert_eq!(record.referer.as_deref(), Some("https://www.google.com/"));
    }

    #[test]
    fn parses_cloudflare_logpush() {
        let line = r#"{"ClientIP":"66.249.66.1","ClientRequestHost":"www.example.com","ClientRequestMethod":"GET","ClientRequestURI":"/blog/","ClientRequestProtocol":"HTTP/2","ClientRequestUserAgent":"Googlebot/2.1","ClientRequestReferer":"","EdgeResponseStatus":301,"EdgeResponseBytes":512,"EdgeStartTimestamp":1791398393000000000,"OriginResponseDurationMs":12,"OriginResponseStatus":200}"#;
        let record = JsonFormat.parse(line).unwrap();
        assert_eq!(record.status, 301);
        assert_eq!(record.target, "/blog/");
        assert_eq!(record.timestamp.to_string(), "2026-10-07T18:39:53Z");
        assert_eq!(record.duration_micros, Some(12_000));
        assert_eq!(record.referer, None);
    }

    #[test]
    fn parses_google_cloud_load_balancer_entries() {
        let line = r#"{"httpRequest":{"requestMethod":"GET","requestUrl":"https://www.example.com/p?q=1","requestSize":"120","status":404,"responseSize":"233","userAgent":"Mozilla/5.0","remoteIp":"203.0.113.9","serverIp":"10.0.0.2","referer":"https://chatgpt.com/","latency":"0.012345s","protocol":"HTTP/1.1"},"resource":{"type":"http_load_balancer","labels":{"zone":"global"}},"timestamp":"2026-10-07T18:39:53.123456Z","severity":"WARNING"}"#;
        let record = JsonFormat.parse(line).unwrap();
        assert_eq!(record.client_ip.to_string(), "203.0.113.9");
        assert_eq!(record.target, "/p?q=1");
        assert_eq!(record.host.as_deref(), Some("www.example.com"));
        assert_eq!(record.bytes, Some(233));
        assert_eq!(record.duration_micros, Some(12_345));
        assert_eq!(record.referer.as_deref(), Some("https://chatgpt.com/"));
    }

    #[test]
    fn parses_nginx_json_logs() {
        let line = r#"{"time_iso8601":"2026-10-07T21:39:53+03:00","remote_addr":"10.0.0.1","http_x_forwarded_for":"66.249.66.1","request":"GET /x?y=1 HTTP/1.1","status":"200","body_bytes_sent":"535","request_time":"0.004","http_referer":"-","http_user_agent":"Googlebot/2.1","http_host":"example.com"}"#;
        let record = JsonFormat.parse(line).unwrap();
        assert_eq!(record.client_ip.to_string(), "66.249.66.1");
        assert_eq!(record.timestamp.to_string(), "2026-10-07T18:39:53Z");
        assert_eq!(
            (record.method.as_ref(), record.target.as_ref()),
            ("GET", "/x?y=1")
        );
        assert_eq!(record.duration_micros, Some(4_000));
        assert_eq!(record.referer, None);
    }

    #[test]
    fn rejects_json_without_request_data() {
        let error = JsonFormat
            .parse(r#"{"level":"info","msg":"server started"}"#)
            .unwrap_err();
        assert!(matches!(error.kind, ParseErrorKind::MissingValue(_)));
        assert_eq!(
            JsonFormat.parse("[1,2]").unwrap_err().kind,
            ParseErrorKind::InvalidJson
        );
    }
}
