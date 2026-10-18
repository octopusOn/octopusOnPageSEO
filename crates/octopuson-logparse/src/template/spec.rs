//! Field kinds a template can capture.

use crate::error::Field;

/// Unit of a logged request duration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DurationUnit {
    Seconds,
    Millis,
    Micros,
}

impl DurationUnit {
    /// Decimal places that become integral when converting to microseconds.
    pub(crate) const fn scale_digits(self) -> u32 {
        match self {
            Self::Seconds => 6,
            Self::Millis => 3,
            Self::Micros => 0,
        }
    }
}

/// What a template field position contains and how to interpret it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FieldSpec {
    /// Present in the line but not needed (ident, remote user, unknown variables …).
    Ignore,
    ClientIp,
    ForwardedFor,
    TimeClf,
    TimeIso8601,
    TimeEpochSeconds,
    RequestLine,
    Method,
    Target,
    Path,
    Query,
    Protocol,
    Status,
    Bytes,
    Referer,
    UserAgent,
    Host,
    Duration(DurationUnit),
    RequestSeq,
    Router,
    Upstream,
}

impl FieldSpec {
    /// Stable name used in error messages for built-in presets.
    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::Ignore => "ignored",
            Self::ClientIp => "client_ip",
            Self::ForwardedFor => "forwarded_for",
            Self::TimeClf => "time_clf",
            Self::TimeIso8601 => "time_iso8601",
            Self::TimeEpochSeconds => "time_epoch",
            Self::RequestLine => "request",
            Self::Method => "method",
            Self::Target => "target",
            Self::Path => "path",
            Self::Query => "query",
            Self::Protocol => "protocol",
            Self::Status => "status",
            Self::Bytes => "bytes",
            Self::Referer => "referer",
            Self::UserAgent => "user_agent",
            Self::Host => "host",
            Self::Duration(_) => "duration",
            Self::RequestSeq => "request_seq",
            Self::Router => "router",
            Self::Upstream => "upstream",
        }
    }

    /// Values of this kind never contain whitespace.
    pub(crate) const fn is_single_token(self) -> bool {
        matches!(
            self,
            Self::ClientIp
                | Self::TimeIso8601
                | Self::TimeEpochSeconds
                | Self::Method
                | Self::Target
                | Self::Path
                | Self::Query
                | Self::Protocol
                | Self::Status
                | Self::Bytes
                | Self::Duration(_)
                | Self::RequestSeq
        )
    }

    /// Whether capturing this field provides the record field `required`.
    pub(crate) fn satisfies(self, required: Field) -> bool {
        match required {
            Field::Timestamp => matches!(
                self,
                Self::TimeClf | Self::TimeIso8601 | Self::TimeEpochSeconds
            ),
            Field::ClientIp => matches!(self, Self::ClientIp | Self::ForwardedFor),
            Field::Request => matches!(self, Self::RequestLine | Self::Target | Self::Path),
            Field::Status => self == Self::Status,
            _ => false,
        }
    }
}
