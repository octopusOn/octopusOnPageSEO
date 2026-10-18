use std::io;
use std::path::{Path, PathBuf};

/// A crawl could not be stored or read.
#[derive(Debug, thiserror::Error)]
pub enum CrawlStoreError {
    /// DuckDB failed.
    #[error("database error: {0}")]
    Database(#[from] duckdb::Error),
    /// A file or directory could not be accessed.
    #[error("cannot access `{}`: {source}", path.display())]
    Io {
        /// The path involved.
        path: PathBuf,
        /// The underlying error.
        source: io::Error,
    },
    /// `crawl.json` is not valid.
    #[error("invalid crawl metadata in `{}`: {source}", path.display())]
    Meta {
        /// The metadata file.
        path: PathBuf,
        /// The parse error.
        source: serde_json::Error,
    },
    /// No crawl has this identifier.
    #[error("no crawl `{0}`")]
    NotFound(String),
    /// A table selection names an unknown tab or column.
    #[error("invalid table selection: {0}")]
    InvalidSelection(String),
    /// Writing an export failed.
    #[error("cannot write the output: {0}")]
    Output(io::Error),
    /// The platform reports no home directory.
    #[error("cannot determine the data directory")]
    NoDataDir,
}

impl CrawlStoreError {
    pub(crate) fn io(path: &Path) -> impl FnOnce(io::Error) -> Self + '_ {
        move |source| Self::Io {
            path: path.to_owned(),
            source,
        }
    }
}
