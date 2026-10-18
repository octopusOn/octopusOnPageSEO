//! Shared pieces of the self-describing formats (W3C extended, CSV/TSV with a header and
//! JSON): recognising field names and assembling a [`LogRecord`] from named values.
//!
//! Field names are compared case-insensitively with everything but letters and digits removed,
//! so `http_user_agent`, `User-Agent`, `cs(User-Agent)` and `userAgent` all read as
//! `useragent`. When several fields claim the same role (`remote_ip` and `client_ip`), the
//! one with the stronger (lower) priority wins.

use std::borrow::Cow;

use crate::error::{Field, ParseError, ParseErrorKind};
use crate::record::LogRecord;
use crate::time;
use crate::values;

/// Unit of a duration field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DurationUnit {
    Seconds,
    Millis,
    Micros,
    Nanos,
    /// `0.012s` or `0.012` are seconds, `12ms` and plain integers milliseconds.
    Auto,
}

/// What a named field contributes to a record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Role {
    /// A full timestamp.
    Timestamp,
    /// A `YYYY-MM-DD` date, combined with [`Role::Time`].
    Date,
    /// A time of day, or a full timestamp when there is no date field.
    Time,
    ClientIp,
    ForwardedFor,
    Method,
    /// Path with query, or an absolute URL.
    Target,
    /// Path without query, combined with [`Role::Query`].
    Path,
    Query,
    /// `GET /path HTTP/1.1`.
    RequestLine,
    Status,
    Bytes,
    UserAgent,
    Referer,
    Host,
    Protocol,
    Duration(DurationUnit),
}

/// Lower-case letters and digits of `name`.
pub(crate) fn normalize_name(name: &str) -> String {
    name.chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

/// Role and priority (lower wins) of a field name.
pub(crate) fn role_of(name: &str) -> Option<(Role, u8)> {
    use DurationUnit::{Auto, Micros, Millis, Nanos, Seconds};
    use Role::{
        Bytes, ClientIp, Date, Duration, ForwardedFor, Host, Method, Path, Protocol, Query,
        Referer, RequestLine, Status, Target, Time, Timestamp, UserAgent,
    };
    Some(match normalize_name(name).as_str() {
        "timestamp" | "datetime" | "timeiso8601" | "startutc" | "edgestarttimestamp" => {
            (Timestamp, 0)
        }
        "ts" | "msec" | "reqtimesec" | "eventtime" | "logtime" | "starttime" | "timelocal"
        | "startlocal" | "requeststarttime" => (Timestamp, 1),
        "date" => (Date, 0),
        "time" => (Time, 0),
        "clientip" | "realip" | "trueclientip" | "cfconnectingip" => (ClientIp, 0),
        "remoteaddr" | "remoteip" | "cip" | "cliip" | "clienthost" | "clientaddress"
        | "sourceip" | "srcip" => (ClientIp, 1),
        "ip" | "client" | "remotehost" | "clientaddr" => (ClientIp, 2),
        "xforwardedfor" | "httpxforwardedfor" | "forwardedfor" | "csxforwardedfor" => {
            (ForwardedFor, 0)
        }
        "method"
        | "requestmethod"
        | "csmethod"
        | "httpmethod"
        | "reqmethod"
        | "clientrequestmethod"
        | "verb" => (Method, 0),
        "requesturi" | "clientrequesturi" | "requesturl" | "csuri" | "requestpath" => (Target, 0),
        "url" | "uri" | "target" | "fullurl" => (Target, 1),
        "path" | "reqpath" | "csuristem" | "urlpath" | "pathname" => (Path, 0),
        "query" | "querystring" | "csuriquery" | "querystr" | "args" | "search" => (Query, 0),
        "request" | "requestline" | "firstline" => (RequestLine, 0),
        "status" | "statuscode" | "scstatus" | "httpstatus" | "responsestatus"
        | "edgeresponsestatus" | "downstreamstatus" | "elbstatuscode" | "responsecode"
        | "httpstatuscode" => (Status, 0),
        "bytes"
        | "bodybytessent"
        | "scbytes"
        | "responsebytes"
        | "edgeresponsebytes"
        | "downstreamcontentsize"
        | "sentbytes"
        | "responsesize"
        | "responsebodysize"
        | "rspcontentlen" => (Bytes, 0),
        "size" | "bytessent" | "totalbytes" | "contentlength" => (Bytes, 1),
        "useragent"
        | "httpuseragent"
        | "csuseragent"
        | "clientrequestuseragent"
        | "requestuseragent"
        | "ua"
        | "agent" => (UserAgent, 0),
        "referer"
        | "referrer"
        | "httpreferer"
        | "httpreferrer"
        | "csreferer"
        | "csreferrer"
        | "clientrequestreferer"
        | "requestreferer" => (Referer, 0),
        "xhostheader" | "clientrequesthost" | "requesthost" | "reqhost" | "httphost" => (Host, 0),
        "host" | "hostname" | "vhost" | "servername" | "cshost" | "domain" | "domainname" => {
            (Host, 1)
        }
        "protocol"
        | "serverprotocol"
        | "csversion"
        | "csprotocolversion"
        | "clientrequestprotocol"
        | "requestprotocol"
        | "httpversion"
        | "proto" => (Protocol, 0),
        "requesttime" | "durationseconds" | "durations" | "elapsedseconds" => {
            (Duration(Seconds), 0)
        }
        "durationms"
        | "responsetimems"
        | "originresponsedurationms"
        | "turnaroundtimemsec"
        | "elapsedms"
        | "edgetimetofirstbytems"
        | "requestdurationms" => (Duration(Millis), 0),
        "durationus" | "durationmicros" | "responsetimeus" | "timetakenus" => (Duration(Micros), 0),
        "durationns" | "durationnanos" => (Duration(Nanos), 0),
        "duration" | "responsetime" | "timetaken" | "latency" | "elapsed" => (Duration(Auto), 1),
        _ => return None,
    })
}

/// Named values of one line, keyed by role.
#[derive(Debug, Default)]
pub(crate) struct Parts<'a> {
    values: Vec<(Role, u8, Cow<'a, str>)>,
}

impl<'a> Parts<'a> {
    /// Records `value` for `role` unless it is empty/`-` or a stronger field already set it.
    pub(crate) fn set(&mut self, role: Role, priority: u8, value: Cow<'a, str>) {
        if values::non_dash(value.trim()).is_none() {
            return;
        }
        let same_kind = |known: &Role| match (known, &role) {
            (Role::Duration(_), Role::Duration(_)) => true,
            (known, role) => known == role,
        };
        match self
            .values
            .iter_mut()
            .find(|(known, _, _)| same_kind(known))
        {
            Some(slot) if priority < slot.1 => *slot = (role, priority, value),
            Some(_) => {}
            None => self.values.push((role, priority, value)),
        }
    }

    fn take(&mut self, role: Role) -> Option<Cow<'a, str>> {
        let index = self
            .values
            .iter()
            .position(|(known, _, _)| *known == role)?;
        Some(self.values.swap_remove(index).2)
    }

    fn take_duration(&mut self) -> Option<(DurationUnit, Cow<'a, str>)> {
        let index = self
            .values
            .iter()
            .position(|(known, _, _)| matches!(known, Role::Duration(_)))?;
        match self.values.swap_remove(index) {
            (Role::Duration(unit), _, value) => Some((unit, value)),
            _ => None,
        }
    }

    /// Whether the roles a record needs are present (used to accept CSV headers).
    pub(crate) fn has_required(roles: &[Role]) -> bool {
        let has = |wanted: &[Role]| roles.iter().any(|r| wanted.contains(r));
        has(&[Role::Timestamp, Role::Date, Role::Time])
            && has(&[Role::Target, Role::Path, Role::RequestLine])
            && has(&[Role::Status])
            && has(&[Role::ClientIp, Role::ForwardedFor])
    }

    /// Builds the record.
    ///
    /// # Errors
    /// A missing or invalid timestamp, client IP, request target or status; unreadable
    /// byte counts and durations are left unknown.
    pub(crate) fn into_record(mut self) -> Result<LogRecord<'a>, ParseError> {
        let missing = |field| ParseError::new(ParseErrorKind::MissingValue(field), 0);
        let invalid = |field| ParseError::new(ParseErrorKind::InvalidValue(field), 0);

        let timestamp = match (
            self.take(Role::Date),
            self.take(Role::Time),
            self.take(Role::Timestamp),
        ) {
            (_, _, Some(full)) => time::parse_flexible(&full),
            (Some(date), Some(time), None) => time::parse_date_time_utc(date.trim(), time.trim())
                .or_else(|| time::parse_flexible(&format!("{} {}", date.trim(), time.trim()))),
            (Some(single), None, None) | (None, Some(single), None) => {
                time::parse_flexible(&single)
            }
            (None, None, None) => return Err(missing(Field::Timestamp)),
        }
        .ok_or(invalid(Field::Timestamp))?;

        let forwarded = self
            .take(Role::ForwardedFor)
            .and_then(|list| values::parse_forwarded_for(&list));
        let client_ip = if let Some(ip) = forwarded {
            ip
        } else {
            let raw = self.take(Role::ClientIp).ok_or(missing(Field::ClientIp))?;
            parse_ip_maybe_port(raw.trim()).ok_or(invalid(Field::ClientIp))?
        };

        let mut method = self.take(Role::Method).unwrap_or_default();
        let mut protocol = self.take(Role::Protocol).unwrap_or_default();
        let mut host = self.take(Role::Host);
        let request_line = self.take(Role::RequestLine);
        let target = match (self.take(Role::Target), self.take(Role::Path), request_line) {
            (Some(target), _, _) => target,
            (None, Some(path), _) => match self.take(Role::Query) {
                Some(query) => {
                    let query = query.trim_start_matches('?');
                    Cow::Owned(format!("{path}?{query}"))
                }
                None => path,
            },
            (None, None, Some(line)) => {
                let parsed = values::parse_request_line(&line).ok_or(invalid(Field::Request))?;
                if method.is_empty() {
                    method = Cow::Owned(parsed.method.to_owned());
                }
                if protocol.is_empty() {
                    protocol = Cow::Owned(parsed.protocol.to_owned());
                }
                Cow::Owned(parsed.target.to_owned())
            }
            (None, None, None) => return Err(missing(Field::Request)),
        };
        let target = absolute_to_target(target, &mut host);

        let status_text = self.take(Role::Status).ok_or(missing(Field::Status))?;
        let status = values::parse_status(status_text.trim()).ok_or(invalid(Field::Status))?;
        // Optional measurements are unknown rather than fatal when a value cannot be read
        // (e.g. a float a JSON encoder wrote as `1e-7`).
        let bytes = self
            .take(Role::Bytes)
            .and_then(|text| values::parse_optional_u64(text.trim()).ok().flatten());
        let duration_micros = self
            .take_duration()
            .and_then(|(unit, text)| parse_duration(text.trim(), unit));

        Ok(LogRecord {
            timestamp,
            client_ip,
            method,
            target,
            protocol,
            status,
            bytes,
            referer: self.take(Role::Referer),
            user_agent: self.take(Role::UserAgent),
            duration_micros,
            host,
            router: None,
            upstream: None,
            request_seq: None,
        })
    }
}

/// An IP address, also written as `ip:port` or `[v6]:port`.
pub(crate) fn parse_ip_maybe_port(value: &str) -> Option<std::net::IpAddr> {
    if let Some(ip) = values::parse_ip(value) {
        return Some(ip);
    }
    if let Ok(socket) = value.parse::<std::net::SocketAddr>() {
        return Some(socket.ip());
    }
    // IPv4 with port, or AWS's unbracketed `2001:db8::1:443`.
    value
        .rsplit_once(':')
        .filter(|(_, port)| values::parse_u64(port).is_some())
        .and_then(|(ip, _)| values::parse_ip(ip))
}

/// Replaces an absolute URL by its target and fills `host` from it when unknown.
pub(crate) fn absolute_to_target<'a>(
    target: Cow<'a, str>,
    host: &mut Option<Cow<'a, str>>,
) -> Cow<'a, str> {
    let (url_host, relative) = values::split_absolute_url(&target);
    let Some(url_host) = url_host else {
        return target;
    };
    if host.is_none() {
        *host = Some(Cow::Owned(url_host.to_owned()));
    }
    Cow::Owned(values::with_root(relative).into_owned())
}

/// Converts a duration in `unit` to microseconds.
pub(crate) fn parse_duration(value: &str, unit: DurationUnit) -> Option<u64> {
    match unit {
        DurationUnit::Seconds => values::parse_scaled_decimal(value.trim_end_matches('s'), 6),
        DurationUnit::Millis => values::parse_scaled_decimal(value, 3),
        DurationUnit::Micros => values::parse_scaled_decimal(value, 0),
        DurationUnit::Nanos => values::parse_scaled_decimal(value, 0).map(|nanos| nanos / 1_000),
        DurationUnit::Auto => {
            if let Some(ms) = value.strip_suffix("ms") {
                values::parse_scaled_decimal(ms, 3)
            } else if let Some(seconds) = value.strip_suffix('s') {
                values::parse_scaled_decimal(seconds, 6)
            } else if value.contains('.') {
                values::parse_scaled_decimal(value, 6)
            } else {
                values::parse_scaled_decimal(value, 3)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_recognised_in_any_spelling() {
        assert_eq!(
            role_of("http_user_agent").map(|r| r.0),
            Some(Role::UserAgent)
        );
        assert_eq!(
            role_of("cs(User-Agent)").map(|r| r.0),
            Some(Role::UserAgent)
        );
        assert_eq!(role_of("userAgent").map(|r| r.0), Some(Role::UserAgent));
        assert_eq!(role_of("ClientRequestURI").map(|r| r.0), Some(Role::Target));
        assert_eq!(
            role_of("request_time").map(|r| r.0),
            Some(Role::Duration(DurationUnit::Seconds))
        );
        assert_eq!(role_of("server_ip"), None);
    }

    #[test]
    fn stronger_fields_win_and_empty_values_are_ignored() {
        let mut parts = Parts::default();
        parts.set(Role::ClientIp, 1, "10.0.0.1".into());
        parts.set(Role::ClientIp, 0, "203.0.113.9".into());
        parts.set(Role::ClientIp, 2, "10.0.0.2".into());
        parts.set(Role::Timestamp, 0, "2026-10-07T18:39:53Z".into());
        parts.set(Role::Path, 0, "/a".into());
        parts.set(Role::Query, 0, "x=1".into());
        parts.set(Role::Status, 0, "200".into());
        parts.set(Role::Bytes, 0, "-".into());
        parts.set(Role::UserAgent, 0, "-".into());
        let record = parts.into_record().unwrap();
        assert_eq!(record.client_ip.to_string(), "203.0.113.9");
        assert_eq!(record.target, "/a?x=1");
        assert_eq!(record.bytes, None);
        assert_eq!(record.user_agent, None);
    }

    #[test]
    fn absolute_urls_request_lines_and_durations() {
        let mut parts = Parts::default();
        parts.set(Role::Timestamp, 0, "1791398393".into());
        parts.set(Role::ClientIp, 0, "[2001:db8::1]:443".into());
        parts.set(Role::Target, 0, "https://www.example.com/p?q=1".into());
        parts.set(Role::Status, 0, "301".into());
        parts.set(Role::Duration(DurationUnit::Auto), 1, "0.012s".into());
        let record = parts.into_record().unwrap();
        assert_eq!(record.client_ip.to_string(), "2001:db8::1");
        assert_eq!(record.target, "/p?q=1");
        assert_eq!(record.host.as_deref(), Some("www.example.com"));
        assert_eq!(record.duration_micros, Some(12_000));

        let mut parts = Parts::default();
        parts.set(Role::Date, 0, "2026-10-07".into());
        parts.set(Role::Time, 0, "18:39:53".into());
        parts.set(Role::ForwardedFor, 0, "198.51.100.7, 10.0.0.1".into());
        parts.set(Role::ClientIp, 0, "10.0.0.1".into());
        parts.set(Role::RequestLine, 0, "GET /x HTTP/1.1".into());
        parts.set(Role::Status, 0, "404".into());
        let record = parts.into_record().unwrap();
        assert_eq!(record.client_ip.to_string(), "198.51.100.7");
        assert_eq!(
            (
                record.method.as_ref(),
                record.target.as_ref(),
                record.protocol.as_ref()
            ),
            ("GET", "/x", "HTTP/1.1")
        );
        assert_eq!(record.timestamp.to_string(), "2026-10-07T18:39:53Z");
    }

    #[test]
    fn duration_units() {
        assert_eq!(parse_duration("15", DurationUnit::Auto), Some(15_000));
        assert_eq!(parse_duration("0.5", DurationUnit::Auto), Some(500_000));
        assert_eq!(parse_duration("12ms", DurationUnit::Auto), Some(12_000));
        assert_eq!(
            parse_duration("1.25", DurationUnit::Seconds),
            Some(1_250_000)
        );
        assert_eq!(parse_duration("7", DurationUnit::Millis), Some(7_000));
        assert_eq!(parse_duration("5123456", DurationUnit::Nanos), Some(5_123));
    }

    #[test]
    fn missing_fields_are_reported() {
        let mut parts = Parts::default();
        parts.set(Role::Timestamp, 0, "2026-10-07T18:39:53Z".into());
        parts.set(Role::ClientIp, 0, "10.0.0.1".into());
        parts.set(Role::Status, 0, "200".into());
        assert_eq!(
            parts.into_record().unwrap_err().kind,
            ParseErrorKind::MissingValue(Field::Request)
        );
    }
}
