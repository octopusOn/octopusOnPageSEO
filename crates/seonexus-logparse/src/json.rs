//! Traefik JSON access logs (`accessLog.format = "json"`).

use std::borrow::Cow;
use std::net::{IpAddr, SocketAddr};

use serde::Deserialize;
use serde::de::IgnoredAny;

use crate::error::{Field, ParseError, ParseErrorKind};
use crate::format::LogFormat;
use crate::record::LogRecord;
use crate::{time, values};

/// Traefik's JSON access-log format.
///
/// Note: Traefik drops request headers by default. To analyse bots, enable
/// `accessLog.fields.headers.names.User-Agent=keep` (and `Referer=keep`).
#[derive(Debug, Clone, Copy, Default)]
pub struct TraefikJsonFormat;

impl TraefikJsonFormat {
    const ID: &'static str = "traefik-json";
}

/// Fields of interest; everything else is ignored by serde.
#[derive(Deserialize)]
struct Line<'a> {
    #[serde(rename = "ClientHost", borrow)]
    client_host: Option<Cow<'a, str>>,
    #[serde(rename = "ClientAddr", borrow)]
    client_addr: Option<Cow<'a, str>>,
    #[serde(rename = "StartUTC", borrow)]
    start_utc: Option<Cow<'a, str>>,
    #[serde(rename = "StartLocal", borrow)]
    start_local: Option<Cow<'a, str>>,
    #[serde(rename = "RequestMethod", borrow)]
    method: Option<Cow<'a, str>>,
    #[serde(rename = "RequestPath", borrow)]
    path: Option<Cow<'a, str>>,
    #[serde(rename = "RequestProtocol", borrow)]
    protocol: Option<Cow<'a, str>>,
    #[serde(rename = "RequestHost", borrow)]
    host: Option<Cow<'a, str>>,
    #[serde(rename = "DownstreamStatus")]
    status: Option<u16>,
    #[serde(rename = "DownstreamContentSize")]
    bytes: Option<u64>,
    /// Nanoseconds.
    #[serde(rename = "Duration")]
    duration_nanos: Option<u64>,
    #[serde(rename = "RequestCount")]
    request_count: Option<u64>,
    #[serde(rename = "RouterName", borrow)]
    router: Option<Cow<'a, str>>,
    #[serde(rename = "ServiceURL", borrow)]
    service_url: Option<ServiceUrl<'a>>,
    #[serde(rename = "request_User-Agent", borrow)]
    user_agent: Option<Cow<'a, str>>,
    #[serde(rename = "request_Referer", borrow)]
    referer: Option<Cow<'a, str>>,
}

/// Traefik versions serialise `ServiceURL` either as a string or as a URL object.
#[derive(Deserialize)]
#[serde(untagged)]
enum ServiceUrl<'a> {
    Text(#[serde(borrow)] Cow<'a, str>),
    Other(IgnoredAny),
}

impl LogFormat for TraefikJsonFormat {
    fn id(&self) -> &str {
        Self::ID
    }

    #[allow(
        clippy::unnecessary_literal_bound,
        reason = "signature is fixed by the LogFormat trait"
    )]
    fn display_name(&self) -> &str {
        "Traefik (JSON)"
    }

    fn specificity(&self) -> usize {
        usize::MAX
    }

    fn parse<'a>(&self, line: &'a str) -> Result<LogRecord<'a>, ParseError> {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return Err(ParseError::new(ParseErrorKind::EmptyLine, 0));
        }
        if !trimmed.starts_with('{') {
            return Err(ParseError::new(ParseErrorKind::InvalidJson, 0));
        }
        let parsed: Line<'a> = serde_json::from_str(trimmed).map_err(|error| {
            ParseError::new(
                ParseErrorKind::InvalidJson,
                error.column().saturating_sub(1),
            )
        })?;
        into_record(parsed)
    }
}

fn into_record(line: Line<'_>) -> Result<LogRecord<'_>, ParseError> {
    let missing = |field| ParseError::new(ParseErrorKind::MissingValue(field), 0);
    let invalid = |field| ParseError::new(ParseErrorKind::InvalidValue(field), 0);

    let timestamp_text = line
        .start_utc
        .or(line.start_local)
        .ok_or(missing(Field::Timestamp))?;
    let timestamp = time::parse_iso8601(&timestamp_text).ok_or(invalid(Field::Timestamp))?;
    let client_ip = client_ip(line.client_host.as_deref(), line.client_addr.as_deref())?;
    let status = line
        .status
        .filter(|s| (100..=999).contains(s))
        .ok_or(invalid(Field::Status))?;

    Ok(LogRecord {
        timestamp,
        client_ip,
        method: line.method.unwrap_or_default(),
        target: line.path.ok_or(missing(Field::Request))?,
        protocol: line.protocol.unwrap_or_default(),
        status,
        bytes: line.bytes,
        referer: non_dash(line.referer),
        user_agent: non_dash(line.user_agent),
        duration_micros: line.duration_nanos.map(|nanos| nanos / 1_000),
        host: non_dash(line.host),
        router: non_dash(line.router),
        upstream: match line.service_url {
            Some(ServiceUrl::Text(url)) => non_dash(Some(url)),
            Some(ServiceUrl::Other(_)) | None => None,
        },
        request_seq: line.request_count,
    })
}

fn client_ip(host: Option<&str>, addr: Option<&str>) -> Result<IpAddr, ParseError> {
    host.and_then(values::parse_ip)
        .or_else(|| addr.and_then(|a| a.parse::<SocketAddr>().ok().map(|s| s.ip())))
        .or_else(|| addr.and_then(values::parse_ip))
        .ok_or(ParseError::new(
            ParseErrorKind::InvalidValue(Field::ClientIp),
            0,
        ))
}

fn non_dash(value: Option<Cow<'_, str>>) -> Option<Cow<'_, str>> {
    value.filter(|v| values::non_dash(v).is_some())
}

#[cfg(test)]
mod tests {
    use super::*;

    const LINE: &str = r#"{"ClientAddr":"57.141.20.59:41220","ClientHost":"57.141.20.59","DownstreamContentSize":535,"DownstreamStatus":200,"Duration":5123456,"RequestCount":12771189,"RequestHost":"example.com","RequestMethod":"GET","RequestPath":"/_next/image?url=%2Fflags%2Fgb.svg\u0026w=828","RequestProtocol":"HTTP/2.0","RouterName":"web@docker","ServiceURL":"http://172.18.0.6:3000","StartUTC":"2026-10-07T18:39:53.207245731Z","request_User-Agent":"meta-externalagent/1.1","level":"info"}"#;

    #[test]
    fn parses_full_line() {
        let record = TraefikJsonFormat.parse(LINE).unwrap();
        assert_eq!(record.client_ip.to_string(), "57.141.20.59");
        assert_eq!(record.status, 200);
        assert_eq!(record.target, "/_next/image?url=%2Fflags%2Fgb.svg&w=828");
        assert!(
            matches!(record.target, Cow::Owned(_)),
            "escaped JSON must be decoded"
        );
        assert_eq!(record.duration_micros, Some(5_123));
        assert_eq!(record.request_seq, Some(12_771_189));
        assert_eq!(record.upstream.as_deref(), Some("http://172.18.0.6:3000"));
        assert_eq!(record.timestamp.to_string(), "2026-10-07T18:39:53.207245Z");
        assert!(record.referer.is_none());
    }

    #[test]
    fn falls_back_to_client_addr_and_object_service_url() {
        let line = r#"{"ClientAddr":"[2001:db8::1]:443","StartUTC":"2026-10-07T18:39:53Z","RequestPath":"/","DownstreamStatus":301,"ServiceURL":{"Scheme":"http","Host":"10.0.0.2"}}"#;
        let record = TraefikJsonFormat.parse(line).unwrap();
        assert_eq!(record.client_ip.to_string(), "2001:db8::1");
        assert!(record.upstream.is_none());
    }

    #[test]
    fn rejects_non_json_quickly() {
        let error = TraefikJsonFormat.parse("1.2.3.4 - - [...]").unwrap_err();
        assert_eq!(error.kind, ParseErrorKind::InvalidJson);
    }
}
