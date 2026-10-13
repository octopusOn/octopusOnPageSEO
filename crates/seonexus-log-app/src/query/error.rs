use std::io;

use seonexus_query::{ParseError, ValidationError};
use seonexus_store::StoreError;

use crate::table_file::TableFileError;

/// Errors of the query use-cases.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum QueryError {
    /// The query text is not valid query-language syntax.
    #[error("invalid query: {source}")]
    Parse {
        /// Text that failed to parse (the span of `source` points into it).
        text: String,
        /// Positioned syntax error.
        #[source]
        source: ParseError,
    },
    /// No built-in template has this id.
    #[error("unknown template `{id}`; available: {available}")]
    UnknownTemplate {
        /// Requested id.
        id: String,
        /// Comma-separated valid ids.
        available: String,
    },
    /// A saved query was given where no data directory is available to look it up.
    #[error("saved query `{0}` can only be run against a data directory")]
    SavedQueryUnavailable(String),
    /// The query is syntactically valid but not meaningful.
    #[error(transparent)]
    Invalid(#[from] ValidationError),
    /// Storage failure, unknown site or busy site.
    #[error(transparent)]
    Store(#[from] StoreError),
    /// The result file could not be written.
    #[error(transparent)]
    File(#[from] TableFileError),
    /// Writing the result to the caller's sink failed.
    #[error("writing query results failed")]
    Output(#[source] io::Error),
}
