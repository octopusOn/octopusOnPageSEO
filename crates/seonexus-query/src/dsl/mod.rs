//! The text query language.
//!
//! ```text
//! status:5xx group:url metric:hits,last_seen sort:hits:desc limit:500
//! (ua:*Googlebot* OR ua:*bingbot*) -status:200 date:last30d group:directory
//! url:~"^/blog/" response_ms:>1000 group:url metric:hits,p95_ms
//! ```
//!
//! Juxtaposed conditions are combined with AND, `OR` binds looser than AND, `-`/`NOT` negates and
//! parentheses group. See ADR 0004 for the full reference.

mod error;
mod lexer;
mod parser;
mod printer;
mod value;

pub use error::{ParseError, ParseErrorKind};
pub use printer::to_dsl;

use crate::ast::Query;

/// Longest accepted query text, in bytes.
pub const MAX_QUERY_LEN: usize = 16 * 1024;

/// Parses query text. The result is syntactically valid; call
/// [`validate`](crate::validate) for semantic checks such as sort columns.
///
/// # Errors
/// A [`ParseError`] pointing at the offending part of `text`.
pub fn parse(text: &str) -> Result<Query, ParseError> {
    if text.len() > MAX_QUERY_LEN {
        return Err(ParseError::new(
            ParseErrorKind::TooLong,
            MAX_QUERY_LEN..text.len(),
        ));
    }
    let tokens = lexer::tokenize(text)?;
    parser::parse_tokens(tokens, text.len())
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod round_trip;
