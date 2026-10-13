use std::fmt;

/// Why a line could not be turned into a [`LogRecord`](crate::LogRecord).
///
/// Errors are `Copy` and allocation-free: imports may skip millions of lines and only
/// aggregate counts per [`ParseErrorKind`] plus a few samples.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, thiserror::Error)]
#[error("{kind} (at byte {offset})")]
pub struct ParseError {
    /// Error category, suitable for aggregation.
    pub kind: ParseErrorKind,
    /// Byte offset in the line where the problem was detected.
    pub offset: usize,
}

impl ParseError {
    pub(crate) const fn new(kind: ParseErrorKind, offset: usize) -> Self {
        Self { kind, offset }
    }
}

/// Category of a [`ParseError`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, thiserror::Error)]
#[non_exhaustive]
pub enum ParseErrorKind {
    /// The line is empty or whitespace only.
    #[error("empty line")]
    EmptyLine,
    /// The line exceeded the configured maximum length and was truncated.
    #[error("line too long")]
    LineTooLong,
    /// Expected separators or quotes were not found where the format requires them.
    #[error("line does not match the format layout")]
    LayoutMismatch,
    /// Unexpected content follows the last field of the format.
    #[error("unexpected trailing data")]
    TrailingData,
    /// A field was present but its value could not be interpreted.
    #[error("invalid {0}")]
    InvalidValue(Field),
    /// A required field is absent.
    #[error("missing {0}")]
    MissingValue(Field),
    /// The line is not valid JSON or not a JSON object.
    #[error("invalid JSON")]
    InvalidJson,
    /// A comment, directive or header line (`#Fields: …`, a CSV header) rather than a request.
    #[error("comment or header line")]
    Comment,
}

/// Record field referenced by [`ParseErrorKind`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[non_exhaustive]
pub enum Field {
    /// Client IP address.
    ClientIp,
    /// Request timestamp.
    Timestamp,
    /// HTTP request line (`METHOD target PROTOCOL`).
    Request,
    /// HTTP status code.
    Status,
    /// Response size in bytes.
    Bytes,
    /// Request duration.
    Duration,
    /// Proxy request sequence number.
    RequestSeq,
}

impl fmt::Display for Field {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::ClientIp => "client IP",
            Self::Timestamp => "timestamp",
            Self::Request => "request line",
            Self::Status => "status code",
            Self::Bytes => "response size",
            Self::Duration => "duration",
            Self::RequestSeq => "request sequence number",
        })
    }
}
