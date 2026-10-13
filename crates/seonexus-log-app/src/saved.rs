//! Saved queries: reusable, named queries shared by every site, plus a JSON exchange format so
//! teams can pass them around.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use seonexus_store::{Catalog, SavedQuery, StoreError};
use serde::{Deserialize, Serialize};

use crate::catalog::NameError;
use crate::catalog::names::normalize_name;
use crate::query::{QueryError, QuerySpec, check_query};
use crate::workspace::Workspace;

/// Version of the exchange file format.
const FILE_VERSION: u32 = 1;

/// Errors of the saved-query use-cases.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum SavedQueryError {
    /// The name is empty, too long or contains control characters.
    #[error(transparent)]
    InvalidName(#[from] NameError),
    /// The query text is empty.
    #[error("the query is empty")]
    EmptyQuery,
    /// The query text is not valid.
    #[error(transparent)]
    Query(#[from] QueryError),
    /// Storage failure, duplicate or unknown name.
    #[error(transparent)]
    Store(#[from] StoreError),
    /// An exchange file could not be read or written.
    #[error("cannot access `{path}`")]
    File {
        /// File involved.
        path: PathBuf,
        /// Underlying error.
        #[source]
        source: io::Error,
    },
    /// An exchange file is not in the expected format.
    #[error("`{path}` is not a saved-query file: {reason}")]
    InvalidFile {
        /// File involved.
        path: PathBuf,
        /// What is wrong.
        reason: String,
    },
}

/// A saved query as shown to users.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SavedQueryInfo {
    /// Unique name.
    pub name: String,
    /// Free-text description.
    pub description: String,
    /// Query text in canonical form.
    pub query: String,
    /// Creation time (RFC 3339).
    pub created_at: String,
    /// Last change (RFC 3339).
    pub updated_at: String,
}

impl From<SavedQuery> for SavedQueryInfo {
    fn from(saved: SavedQuery) -> Self {
        Self {
            name: saved.name,
            description: saved.description,
            query: saved.query,
            created_at: saved.created_at.to_string(),
            updated_at: saved.updated_at.to_string(),
        }
    }
}

/// Input of [`save_query`].
#[derive(Debug, Clone)]
pub struct SaveQueryRequest {
    /// Name, unique across the data directory.
    pub name: String,
    /// Optional description.
    pub description: String,
    /// Query text; checked and stored in canonical form.
    pub query: String,
    /// Replace an existing query of the same name.
    pub overwrite: bool,
}

/// All saved queries ordered by name.
///
/// # Errors
/// See [`SavedQueryError`].
pub fn list_saved_queries(workspace: &Workspace) -> Result<Vec<SavedQueryInfo>, SavedQueryError> {
    let catalog = Catalog::open(workspace.data_dir())?;
    Ok(catalog
        .saved_queries()?
        .into_iter()
        .map(SavedQueryInfo::from)
        .collect())
}

/// One saved query.
///
/// # Errors
/// See [`SavedQueryError`]; unknown names are store errors.
pub fn saved_query(workspace: &Workspace, name: &str) -> Result<SavedQueryInfo, SavedQueryError> {
    let catalog = Catalog::open(workspace.data_dir())?;
    Ok(catalog.saved_query(name.trim())?.into())
}

/// Checks and saves a query under a name.
///
/// # Errors
/// See [`SavedQueryError`]; a taken name without `overwrite` is a store error.
pub fn save_query(
    workspace: &Workspace,
    request: &SaveQueryRequest,
) -> Result<SavedQueryInfo, SavedQueryError> {
    let name = normalize_name(&request.name)?;
    if request.query.trim().is_empty() {
        return Err(SavedQueryError::EmptyQuery);
    }
    let checked = check_query(&QuerySpec::text(request.query.trim()))?;
    let catalog = Catalog::open(workspace.data_dir())?;
    let saved = catalog.save_query(
        name,
        request.description.trim(),
        &checked.query,
        request.overwrite,
    )?;
    Ok(saved.into())
}

/// Deletes a saved query.
///
/// # Errors
/// See [`SavedQueryError`].
pub fn remove_saved_query(workspace: &Workspace, name: &str) -> Result<(), SavedQueryError> {
    Catalog::open(workspace.data_dir())?.remove_saved_query(name.trim())?;
    Ok(())
}

/// The exchange file.
#[derive(Debug, Serialize, Deserialize)]
struct SavedQueryFile {
    version: u32,
    queries: Vec<SavedQueryEntry>,
}

#[derive(Debug, Serialize, Deserialize)]
struct SavedQueryEntry {
    name: String,
    #[serde(default)]
    description: String,
    query: String,
}

/// Writes every saved query to a JSON file; returns how many were written.
///
/// # Errors
/// See [`SavedQueryError`].
pub fn export_saved_queries(workspace: &Workspace, path: &Path) -> Result<usize, SavedQueryError> {
    let queries = list_saved_queries(workspace)?;
    let file = SavedQueryFile {
        version: FILE_VERSION,
        queries: queries
            .into_iter()
            .map(|q| SavedQueryEntry {
                name: q.name,
                description: q.description,
                query: q.query,
            })
            .collect(),
    };
    let count = file.queries.len();
    let json =
        serde_json::to_string_pretty(&file).map_err(|error| SavedQueryError::InvalidFile {
            path: path.to_owned(),
            reason: error.to_string(),
        })?;
    fs::write(path, json + "\n").map_err(|source| SavedQueryError::File {
        path: path.to_owned(),
        source,
    })?;
    Ok(count)
}

/// A query of an imported file that was not saved.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SkippedQuery {
    /// Name in the file.
    pub name: String,
    /// Why it was skipped.
    pub reason: String,
}

/// Result of [`import_saved_queries`].
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct SavedImportReport {
    /// Newly saved names.
    pub added: Vec<String>,
    /// Replaced names (with `overwrite`).
    pub replaced: Vec<String>,
    /// Queries that were not saved.
    pub skipped: Vec<SkippedQuery>,
}

/// Saves the queries of a JSON file written by [`export_saved_queries`]. Existing names are
/// skipped unless `overwrite` is set; invalid queries are skipped and reported.
///
/// # Errors
/// [`SavedQueryError::File`] or [`SavedQueryError::InvalidFile`] when the file cannot be used
/// at all; storage errors.
pub fn import_saved_queries(
    workspace: &Workspace,
    path: &Path,
    overwrite: bool,
) -> Result<SavedImportReport, SavedQueryError> {
    let text = fs::read_to_string(path).map_err(|source| SavedQueryError::File {
        path: path.to_owned(),
        source,
    })?;
    let file: SavedQueryFile =
        serde_json::from_str(&text).map_err(|error| SavedQueryError::InvalidFile {
            path: path.to_owned(),
            reason: error.to_string(),
        })?;
    if file.version > FILE_VERSION {
        return Err(SavedQueryError::InvalidFile {
            path: path.to_owned(),
            reason: format!(
                "version {} is newer than supported ({FILE_VERSION})",
                file.version
            ),
        });
    }
    let existing: Vec<String> = list_saved_queries(workspace)?
        .into_iter()
        .map(|q| q.name)
        .collect();
    let mut report = SavedImportReport::default();
    for entry in file.queries {
        let request = SaveQueryRequest {
            name: entry.name.clone(),
            description: entry.description,
            query: entry.query,
            overwrite,
        };
        let was_there = existing.iter().any(|name| name == entry.name.trim());
        match save_query(workspace, &request) {
            Ok(saved) if was_there => report.replaced.push(saved.name),
            Ok(saved) => report.added.push(saved.name),
            Err(error) => report.skipped.push(SkippedQuery {
                name: entry.name,
                reason: error.to_string(),
            }),
        }
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use seonexus_core::table::TableBuffer;

    use super::*;
    use crate::query::{QueryRequest, run_query};
    use crate::test_support::Fixture;

    fn request(name: &str, query: &str) -> SaveQueryRequest {
        SaveQueryRequest {
            name: name.into(),
            description: "  weekly  ".into(),
            query: query.into(),
            overwrite: false,
        }
    }

    #[test]
    fn saves_checked_canonical_queries() {
        let fixture = Fixture::new();
        let saved = save_query(
            &fixture.workspace,
            &request(" 404s ", "status:404   group:url"),
        )
        .unwrap();
        assert_eq!(
            (
                saved.name.as_str(),
                saved.description.as_str(),
                saved.query.as_str()
            ),
            ("404s", "weekly", "status:404 group:url")
        );
        assert!(matches!(
            save_query(&fixture.workspace, &request("bad", "grop:url")),
            Err(SavedQueryError::Query(QueryError::Parse { .. }))
        ));
        assert!(matches!(
            save_query(&fixture.workspace, &request("empty", "  ")),
            Err(SavedQueryError::EmptyQuery)
        ));
        assert!(matches!(
            save_query(&fixture.workspace, &request("404s", "status:404")),
            Err(SavedQueryError::Store(StoreError::DuplicateSavedQuery(_)))
        ));
        assert_eq!(list_saved_queries(&fixture.workspace).unwrap().len(), 1);
        remove_saved_query(&fixture.workspace, "404s").unwrap();
        assert_eq!(list_saved_queries(&fixture.workspace).unwrap(), Vec::new());
    }

    #[test]
    fn saved_queries_run_with_refinements() {
        let fixture = Fixture::imported();
        save_query(
            &fixture.workspace,
            &request("by status", "group:status sort:status:asc"),
        )
        .unwrap();
        let mut table = TableBuffer::default();
        let spec = QuerySpec::Saved {
            name: "by status".into(),
            refine: Some("status:404".into()),
        };
        let outcome = run_query(
            &fixture.workspace,
            &QueryRequest {
                site: "acme.com".into(),
                spec,
                page: None,
            },
            &mut table,
        )
        .unwrap();
        assert_eq!(outcome.query, "status:404 group:status sort:status:asc");
        assert_eq!(table.rows.len(), 1);
        let missing = QuerySpec::Saved {
            name: "nope".into(),
            refine: None,
        };
        let error = run_query(
            &fixture.workspace,
            &QueryRequest {
                site: "acme.com".into(),
                spec: missing,
                page: None,
            },
            &mut table,
        );
        assert!(matches!(
            error,
            Err(QueryError::Store(StoreError::SavedQueryNotFound(_)))
        ));
    }

    #[test]
    fn exchange_files_round_trip() {
        let source = Fixture::new();
        save_query(&source.workspace, &request("a", "status:404")).unwrap();
        save_query(&source.workspace, &request("b", "bot:googlebot* group:day")).unwrap();
        let path = source.path("queries.json");
        assert_eq!(export_saved_queries(&source.workspace, &path).unwrap(), 2);

        let target = Fixture::new();
        save_query(&target.workspace, &request("a", "status:500")).unwrap();
        let report = import_saved_queries(&target.workspace, &path, false).unwrap();
        assert_eq!(report.added, ["b"]);
        assert_eq!(report.skipped.len(), 1, "existing names are kept");
        let report = import_saved_queries(&target.workspace, &path, true).unwrap();
        assert_eq!(report.replaced, ["a", "b"]);
        assert_eq!(
            saved_query(&target.workspace, "a").unwrap().query,
            "status:404"
        );

        let broken = target.write("broken.json", "{\"queries\": 3}");
        assert!(matches!(
            import_saved_queries(&target.workspace, &broken, false),
            Err(SavedQueryError::InvalidFile { .. })
        ));
    }
}
