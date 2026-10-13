//! Accumulates template field values into a [`LogRecord`].

use std::borrow::Cow;
use std::net::IpAddr;

use seonexus_core::Timestamp;

use super::spec::FieldSpec;
use crate::error::{Field, ParseError, ParseErrorKind};
use crate::record::LogRecord;
use crate::{time, values};

#[derive(Default)]
pub(crate) struct RecordBuilder<'a> {
    timestamp: Option<Timestamp>,
    client_ip: Option<IpAddr>,
    forwarded_ip: Option<IpAddr>,
    method: Option<&'a str>,
    target: Option<&'a str>,
    path: Option<&'a str>,
    query: Option<&'a str>,
    protocol: Option<&'a str>,
    status: Option<u16>,
    bytes: Option<u64>,
    referer: Option<&'a str>,
    user_agent: Option<&'a str>,
    duration_micros: Option<u64>,
    host: Option<&'a str>,
    router: Option<&'a str>,
    upstream: Option<&'a str>,
    request_seq: Option<u64>,
}

impl<'a> RecordBuilder<'a> {
    /// Interprets `value` according to `spec`. `offset` is the value's byte position.
    pub(crate) fn apply(
        &mut self,
        spec: FieldSpec,
        value: &'a str,
        offset: usize,
    ) -> Result<(), ParseError> {
        let invalid = |field| ParseError::new(ParseErrorKind::InvalidValue(field), offset);
        match spec {
            FieldSpec::Ignore => {}
            FieldSpec::ClientIp => {
                self.client_ip = Some(values::parse_ip(value).ok_or(invalid(Field::ClientIp))?);
            }
            FieldSpec::ForwardedFor => self.forwarded_ip = values::parse_forwarded_for(value),
            FieldSpec::TimeClf => self.set_time(time::parse_clf(value), offset)?,
            FieldSpec::TimeIso8601 => self.set_time(time::parse_iso8601(value), offset)?,
            FieldSpec::TimeEpochSeconds => {
                self.set_time(time::parse_epoch_seconds(value), offset)?;
            }
            FieldSpec::RequestLine => {
                let request = values::parse_request_line(value).ok_or(invalid(Field::Request))?;
                self.method = Some(request.method);
                self.target = Some(request.target);
                self.protocol = Some(request.protocol);
            }
            FieldSpec::Method => self.method = values::non_dash(value),
            FieldSpec::Target => self.target = values::non_dash(value),
            FieldSpec::Path => self.path = values::non_dash(value),
            FieldSpec::Query => self.query = values::non_dash(value),
            FieldSpec::Protocol => self.protocol = values::non_dash(value),
            FieldSpec::Status => {
                self.status = Some(values::parse_status(value).ok_or(invalid(Field::Status))?);
            }
            FieldSpec::Bytes => {
                self.bytes =
                    values::parse_optional_u64(value).map_err(|()| invalid(Field::Bytes))?;
            }
            FieldSpec::Referer => self.referer = values::non_dash(value),
            FieldSpec::UserAgent => self.user_agent = values::non_dash(value),
            FieldSpec::Host => self.host = values::non_dash(value),
            FieldSpec::Router => self.router = values::non_dash(value),
            FieldSpec::Upstream => self.upstream = values::non_dash(value),
            FieldSpec::Duration(unit) => {
                self.duration_micros = values::non_dash(value)
                    .map(|v| values::parse_scaled_decimal(v, unit.scale_digits()))
                    .map(|parsed| parsed.ok_or(invalid(Field::Duration)))
                    .transpose()?;
            }
            FieldSpec::RequestSeq => {
                self.request_seq =
                    values::parse_optional_u64(value).map_err(|()| invalid(Field::RequestSeq))?;
            }
        }
        Ok(())
    }

    fn set_time(&mut self, parsed: Option<Timestamp>, offset: usize) -> Result<(), ParseError> {
        let timestamp = parsed.ok_or(ParseError::new(
            ParseErrorKind::InvalidValue(Field::Timestamp),
            offset,
        ))?;
        self.timestamp = Some(timestamp);
        Ok(())
    }

    /// Validates required fields and produces the record.
    pub(crate) fn finish(self, line_len: usize) -> Result<LogRecord<'a>, ParseError> {
        let missing = |field| ParseError::new(ParseErrorKind::MissingValue(field), line_len);
        let target = match (self.target, self.path) {
            (Some(target), _) => Cow::Borrowed(target),
            (None, Some(path)) => join_path_query(path, self.query),
            (None, None) => return Err(missing(Field::Request)),
        };
        Ok(LogRecord {
            timestamp: self.timestamp.ok_or(missing(Field::Timestamp))?,
            client_ip: self
                .forwarded_ip
                .or(self.client_ip)
                .ok_or(missing(Field::ClientIp))?,
            method: Cow::Borrowed(self.method.unwrap_or_default()),
            target,
            protocol: Cow::Borrowed(self.protocol.unwrap_or_default()),
            status: self.status.ok_or(missing(Field::Status))?,
            bytes: self.bytes,
            referer: self.referer.map(Cow::Borrowed),
            user_agent: self.user_agent.map(Cow::Borrowed),
            duration_micros: self.duration_micros,
            host: self.host.map(Cow::Borrowed),
            router: self.router.map(Cow::Borrowed),
            upstream: self.upstream.map(Cow::Borrowed),
            request_seq: self.request_seq,
        })
    }
}

/// Combines a path and a query that were logged separately (`%U%q`, `$uri?$args`).
fn join_path_query<'a>(path: &'a str, query: Option<&str>) -> Cow<'a, str> {
    match query.map(|q| q.strip_prefix('?').unwrap_or(q)) {
        None | Some("") => Cow::Borrowed(path),
        Some(query) => Cow::Owned(format!("{path}?{query}")),
    }
}
