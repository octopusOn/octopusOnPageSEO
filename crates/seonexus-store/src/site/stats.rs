//! Read-only summaries of a site shared by [`SiteStore`](super::SiteStore) and
//! [`SiteReader`](super::SiteReader).

use duckdb::{Connection, Row};
use seonexus_core::Timestamp;

use crate::error::StoreError;

/// Aggregate counters of a site.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SiteTotals {
    /// Committed imports.
    pub imports: u64,
    /// Stored hits.
    pub hits: u64,
    /// Distinct raw URLs.
    pub urls: u64,
    /// Distinct user agents.
    pub user_agents: u64,
    /// Earliest stored request.
    pub first_request: Option<Timestamp>,
    /// Latest stored request.
    pub last_request: Option<Timestamp>,
}

/// One committed import as recorded in the site history.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportHistoryEntry {
    /// Identifier.
    pub id: i64,
    /// Source file path as given by the user.
    pub path: String,
    /// Source file size in bytes.
    pub size_bytes: u64,
    /// Serialized format choice used for parsing.
    pub format_spec: String,
    /// When the import started.
    pub started_at: Timestamp,
    /// When the import committed.
    pub finished_at: Timestamp,
    /// Lines read.
    pub total_lines: u64,
    /// Lines parsed.
    pub parsed_lines: u64,
    /// Hits stored.
    pub inserted_rows: u64,
    /// Hits dropped as duplicates of earlier imports.
    pub duplicate_rows: u64,
    /// Earliest request in the file.
    pub first_request: Option<Timestamp>,
    /// Latest request in the file.
    pub last_request: Option<Timestamp>,
}

pub(super) fn totals(connection: &Connection) -> Result<SiteTotals, StoreError> {
    Ok(connection.query_row(
        "SELECT (SELECT count(*) FROM imports),
                (SELECT coalesce(sum(hits), 0) FROM r_hour)::UBIGINT,
                (SELECT count(*) FROM urls),
                (SELECT count(*) FROM user_agents),
                (SELECT epoch_us(min(first_ts)) FROM r_day_url),
                (SELECT epoch_us(max(last_ts)) FROM r_day_url)",
        [],
        |row| {
            Ok(SiteTotals {
                imports: row.get(0)?,
                hits: row.get(1)?,
                urls: row.get(2)?,
                user_agents: row.get(3)?,
                first_request: optional_timestamp(row, 4)?,
                last_request: optional_timestamp(row, 5)?,
            })
        },
    )?)
}

/// Committed imports, newest first.
pub(super) fn import_history(
    connection: &Connection,
) -> Result<Vec<ImportHistoryEntry>, StoreError> {
    let mut statement = connection.prepare(
        "SELECT i.id, coalesce(f.path, ''), coalesce(f.size_bytes, 0), i.format_spec,
                epoch_us(i.started_at), epoch_us(i.finished_at), i.total_lines, i.parsed_lines,
                i.inserted_rows, i.duplicate_rows, epoch_us(i.first_ts), epoch_us(i.last_ts)
         FROM imports i
         LEFT JOIN imported_files f ON f.import_id = i.id
         ORDER BY i.id DESC",
    )?;
    let rows = statement.query_map([], |row| {
        Ok(ImportHistoryEntry {
            id: row.get(0)?,
            path: row.get(1)?,
            size_bytes: row.get(2)?,
            format_spec: row.get(3)?,
            started_at: Timestamp::from_unix_micros(row.get(4)?),
            finished_at: Timestamp::from_unix_micros(row.get(5)?),
            total_lines: row.get(6)?,
            parsed_lines: row.get(7)?,
            inserted_rows: row.get(8)?,
            duplicate_rows: row.get(9)?,
            first_request: optional_timestamp(row, 10)?,
            last_request: optional_timestamp(row, 11)?,
        })
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

fn optional_timestamp(row: &Row<'_>, index: usize) -> duckdb::Result<Option<Timestamp>> {
    Ok(row
        .get::<_, Option<i64>>(index)?
        .map(Timestamp::from_unix_micros))
}
