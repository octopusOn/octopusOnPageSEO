use std::path::PathBuf;

use seonexus_report::ExportFormat;
use serde::Serialize;

use super::{QueryError, QueryOutcome, QueryRequest, QuerySpec, run_query};
use crate::table_file::{TableFile, TableFileError};
use crate::workspace::Workspace;

/// Input of [`export_query`].
#[derive(Debug, Clone)]
pub struct ExportRequest {
    /// Site name.
    pub site: String,
    /// What to run.
    pub spec: QuerySpec,
    /// Destination file; replaced if it exists.
    pub path: PathBuf,
    /// File format; derived from the extension of `path` when `None`.
    pub format: Option<ExportFormat>,
}

/// Result of [`export_query`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ExportReport {
    /// Written file.
    pub path: String,
    /// File format (extension).
    pub format: &'static str,
    /// Query metadata and row count.
    #[serde(flatten)]
    pub outcome: QueryOutcome,
}

/// Runs a query and writes every result row to a CSV, JSON or XLSX file.
///
/// The file is written under a temporary name and renamed when complete, so a failed or
/// oversized export never leaves a truncated file behind.
///
/// # Errors
/// See [`QueryError`].
pub fn export_query(
    workspace: &Workspace,
    request: &ExportRequest,
) -> Result<ExportReport, QueryError> {
    let mut file = TableFile::create(&request.path, request.format)?;
    let query = QueryRequest {
        site: request.site.clone(),
        spec: request.spec.clone(),
        page: None,
    };
    let outcome = run_query(workspace, &query, file.sink()).map_err(|error| match error {
        QueryError::Store(seonexus_store::StoreError::Output(source))
        | QueryError::Output(source) => QueryError::File(TableFileError::Write {
            path: request.path.clone(),
            source,
        }),
        other => other,
    })?;
    let format = file.format();
    file.commit()?;
    Ok(ExportReport {
        path: request.path.display().to_string(),
        format: format.extension(),
        outcome,
    })
}
