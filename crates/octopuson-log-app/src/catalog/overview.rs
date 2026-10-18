use octopuson_store::{Catalog, ImportHistoryEntry, SiteTotals};
use serde::Serialize;

use super::{CatalogError, SiteInfo, site_info};
use crate::site_access::open_reader;
use crate::workspace::Workspace;

/// A site with its stored data volume and import history.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SiteOverview {
    /// The site.
    #[serde(flatten)]
    pub site: SiteInfo,
    /// Committed imports.
    pub imports: u64,
    /// Stored hits.
    pub hits: u64,
    /// Distinct URLs.
    pub urls: u64,
    /// Distinct user agents.
    pub user_agents: u64,
    /// Earliest stored request (RFC 3339).
    pub first_request: Option<String>,
    /// Latest stored request (RFC 3339).
    pub last_request: Option<String>,
    /// Imports, newest first.
    pub history: Vec<ImportSummary>,
}

/// One committed import.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ImportSummary {
    /// Identifier.
    pub id: i64,
    /// Source file.
    pub path: String,
    /// Source file size in bytes.
    pub size_bytes: u64,
    /// Format specification used.
    pub format: String,
    /// Commit time (RFC 3339).
    pub finished_at: String,
    /// Import duration in milliseconds.
    pub elapsed_ms: u64,
    /// Lines read.
    pub total_lines: u64,
    /// Lines parsed.
    pub parsed_lines: u64,
    /// Hits stored.
    pub inserted_rows: u64,
    /// Hits dropped as duplicates.
    pub duplicate_rows: u64,
    /// Earliest request in the file (RFC 3339).
    pub first_request: Option<String>,
    /// Latest request in the file (RFC 3339).
    pub last_request: Option<String>,
}

/// Describes a site: format, stored volume and import history.
///
/// # Errors
/// See [`CatalogError`]; a busy site (import running) is reported as a store error.
pub fn site_overview(workspace: &Workspace, name: &str) -> Result<SiteOverview, CatalogError> {
    let site = Catalog::open(workspace.data_dir())?.site_by_name(name.trim())?;
    let (totals, history) = match open_reader(workspace, site.id)? {
        Some(reader) => (reader.totals()?, reader.import_history()?),
        None => (SiteTotals::default(), Vec::new()),
    };
    Ok(SiteOverview {
        site: site_info(site),
        imports: totals.imports,
        hits: totals.hits,
        urls: totals.urls,
        user_agents: totals.user_agents,
        first_request: totals.first_request.map(|t| t.to_string()),
        last_request: totals.last_request.map(|t| t.to_string()),
        history: history.into_iter().map(import_summary).collect(),
    })
}

fn import_summary(entry: ImportHistoryEntry) -> ImportSummary {
    let elapsed_us = entry.finished_at.as_unix_micros() - entry.started_at.as_unix_micros();
    ImportSummary {
        id: entry.id,
        path: entry.path,
        size_bytes: entry.size_bytes,
        format: entry.format_spec,
        finished_at: entry.finished_at.to_string(),
        elapsed_ms: u64::try_from(elapsed_us / 1_000).unwrap_or_default(),
        total_lines: entry.total_lines,
        parsed_lines: entry.parsed_lines,
        inserted_rows: entry.inserted_rows,
        duplicate_rows: entry.duplicate_rows,
        first_request: entry.first_request.map(|t| t.to_string()),
        last_request: entry.last_request.map(|t| t.to_string()),
    }
}
