//! Positioned parse errors of the query language.

use std::fmt;
use std::ops::Range;

/// A syntax error with the byte range it refers to.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{kind}")]
pub struct ParseError {
    kind: ParseErrorKind,
    span: Range<usize>,
}

impl ParseError {
    pub(super) fn new(kind: ParseErrorKind, span: Range<usize>) -> Self {
        Self { kind, span }
    }

    /// What went wrong.
    pub fn kind(&self) -> &ParseErrorKind {
        &self.kind
    }

    /// Byte range in the query text.
    pub fn span(&self) -> Range<usize> {
        self.span.clone()
    }
}

/// Kinds of [`ParseError`].
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ParseErrorKind {
    /// A character that cannot start a token.
    UnexpectedChar(char),
    /// A `"` without its closing quote.
    UnterminatedQuote,
    /// `field:` without a value.
    MissingValue,
    /// A word that is not `field:value`, `OR`, `AND` or `NOT`.
    ExpectedCondition(String),
    /// Unknown field or directive name.
    UnknownField(String),
    /// Unknown `group:` name.
    UnknownDimension(String),
    /// Unknown `metric:` name.
    UnknownMetric(String),
    /// A value that does not fit the field.
    InvalidValue {
        /// Field name.
        field: String,
        /// Expected form.
        expected: &'static str,
    },
    /// The operator is not available for this field.
    UnsupportedOperator {
        /// Field name.
        field: String,
        /// Operator as written.
        operator: &'static str,
    },
    /// A comma-separated list where only one value is allowed.
    ListNotAllowed(String),
    /// `(` without `)` or a stray `)`.
    UnbalancedParenthesis,
    /// `()`.
    EmptyParentheses,
    /// `OR`, `AND`, `NOT` or `-` without an operand.
    DanglingOperator(&'static str),
    /// `group:`, `metric:`, `sort:` or `limit:` inside parentheses or after `-`.
    MisplacedDirective(String),
    /// A directive given twice.
    DuplicateDirective(String),
    /// Something follows a value without a separating space.
    MissingSeparator,
    /// Parentheses or negations nested deeper than the parser allows.
    TooDeep,
    /// The query text exceeds [`MAX_QUERY_LEN`](super::MAX_QUERY_LEN).
    TooLong,
}

impl fmt::Display for ParseErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnexpectedChar(c) => write!(f, "unexpected character `{c}`"),
            Self::UnterminatedQuote => f.write_str("missing closing `\"`"),
            Self::MissingValue => f.write_str("missing value after `:`"),
            Self::ExpectedCondition(word) => write!(
                f,
                "expected `field:value` but found `{word}` (to search URLs write url:\"*{word}*\")"
            ),
            Self::UnknownField(name) => write!(
                f,
                "unknown field `{name}`; fields: {}; directives: group, metric, sort, limit",
                names(crate::Field::ALL.iter().map(|f| f.name()))
            ),
            Self::UnknownDimension(name) => write!(
                f,
                "unknown group `{name}`; available: {}",
                names(crate::Dimension::ALL.iter().map(|d| d.name()))
            ),
            Self::UnknownMetric(name) => write!(
                f,
                "unknown metric `{name}`; available: {}",
                names(crate::Metric::ALL.iter().map(|m| m.name()))
            ),
            Self::InvalidValue { field, expected } => {
                write!(f, "invalid value for `{field}`: expected {expected}")
            }
            Self::UnsupportedOperator { field, operator } => {
                write!(f, "`{field}` does not support `{operator}`")
            }
            Self::ListNotAllowed(name) => write!(f, "`{name}` accepts a single value here"),
            Self::UnbalancedParenthesis => f.write_str("unbalanced parenthesis"),
            Self::EmptyParentheses => f.write_str("empty parentheses"),
            Self::DanglingOperator(op) => write!(f, "`{op}` needs a condition"),
            Self::MisplacedDirective(name) => {
                write!(f, "`{name}:` must be used at the top level")
            }
            Self::DuplicateDirective(name) => write!(f, "`{name}:` is given twice"),
            Self::MissingSeparator => f.write_str("expected a space after the value"),
            Self::TooDeep => f.write_str("the query is nested too deeply"),
            Self::TooLong => f.write_str("the query is too long"),
        }
    }
}

fn names<'a>(items: impl Iterator<Item = &'a str>) -> String {
    items.collect::<Vec<_>>().join(", ")
}
