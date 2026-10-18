use std::io;
use std::path::PathBuf;

/// Errors of the storage layer.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum StoreError {
    /// DuckDB reported an error.
    #[error("database error")]
    Database(#[from] duckdb::Error),
    /// Building a columnar batch failed (indicates a bug).
    #[error("internal error while building a hit batch")]
    Arrow(#[from] duckdb::arrow::error::ArrowError),
    /// A file or directory could not be accessed.
    #[error("cannot access `{path}`")]
    Io {
        /// Path involved.
        path: PathBuf,
        /// Underlying error.
        #[source]
        source: io::Error,
    },
    /// No platform data directory exists and none was configured.
    #[error("cannot determine a data directory; pass --data-dir or set OCTOPUSSEO_DATA_DIR")]
    NoDataDir,
    /// Paths handed to DuckDB must be valid UTF-8.
    #[error("path `{0}` is not valid UTF-8")]
    NonUtf8Path(PathBuf),
    /// Another process or import holds the site lock.
    #[error("site is busy: another import or application instance is using `{0}`")]
    SiteLocked(PathBuf),
    /// The database was written by a newer version of the application.
    #[error(
        "database schema version {found} is newer than this application supports ({supported})"
    )]
    SchemaTooNew {
        /// Version found on disk.
        found: i64,
        /// Latest version known to this build.
        supported: i64,
    },
    /// A client with this name already exists.
    #[error("a client named `{0}` already exists")]
    DuplicateClient(String),
    /// A site with this name already exists.
    #[error("a site named `{0}` already exists")]
    DuplicateSite(String),
    /// A client still owns sites and cannot be removed.
    #[error("client `{0}` still has sites; remove them first")]
    ClientHasSites(String),
    /// A saved query with this name already exists.
    #[error("a saved query named `{0}` already exists")]
    DuplicateSavedQuery(String),
    /// No saved query has this name.
    #[error("saved query `{0}` not found")]
    SavedQueryNotFound(String),
    /// No client has this name.
    #[error("client `{0}` not found")]
    ClientNotFound(String),
    /// No site has this name.
    #[error("site `{0}` not found")]
    SiteNotFound(String),
    /// No import has this id.
    #[error("import {0} not found")]
    ImportNotFound(i64),
    /// Writing query results to the output sink failed.
    #[error("writing query results failed")]
    Output(#[source] io::Error),
}

impl StoreError {
    pub(crate) fn io(path: impl Into<PathBuf>) -> impl FnOnce(io::Error) -> Self {
        let path = path.into();
        move |source| Self::Io { path, source }
    }
}
