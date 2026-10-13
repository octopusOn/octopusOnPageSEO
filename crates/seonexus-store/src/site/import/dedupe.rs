//! Count-aware de-duplication against previously committed imports.
//!
//! Identical lines can legitimately occur (two requests in the same second), so a line is a
//! duplicate only as many times as earlier imports already contain it. Only hours that
//! earlier imports touched are examined, which keeps re-reading Parquet cheap for the common
//! case of non-overlapping, chronologically appended logs.

use duckdb::{Connection, params};

use super::super::files::parquet_files;
use crate::error::StoreError;
use crate::paths::SitePaths;
use crate::sql::path_list_literal;

/// Per-session table tracking how many existing copies of a line were already matched, so
/// duplicates spanning several flushes are counted correctly.
pub(super) const MATCHED_TABLE_SQL: &str =
    "CREATE TEMP TABLE dedupe_matched (line_hash UBIGINT NOT NULL, n BIGINT NOT NULL);";

/// Removes staged rows already stored by earlier imports; returns how many were removed.
pub(super) fn remove_duplicates(
    connection: &Connection,
    paths: &SitePaths,
    import_id: i64,
) -> Result<u64, StoreError> {
    connection.execute(
        "CREATE OR REPLACE TEMP TABLE flush_overlap AS
         SELECT DISTINCT r.hour
         FROM (SELECT DISTINCT date_trunc('hour', ts) AS hour FROM staging_hits) h
         JOIN r_hour r ON r.hour = h.hour AND r.import_id <> ?",
        params![import_id],
    )?;
    let files = overlapping_files(connection, paths)?;
    let removed = if files.is_empty() {
        0
    } else {
        delete_matches(
            connection,
            &path_list_literal(files.iter().map(AsRef::as_ref))?,
        )?
    };
    connection.execute_batch("DROP TABLE flush_overlap;")?;
    Ok(removed)
}

fn overlapping_files(
    connection: &Connection,
    paths: &SitePaths,
) -> Result<Vec<std::path::PathBuf>, StoreError> {
    let mut statement = connection
        .prepare("SELECT DISTINCT strftime(hour, '%Y-%m') FROM flush_overlap ORDER BY 1")?;
    let months = statement
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    let mut files = Vec::new();
    for month in months {
        files.extend(parquet_files(&paths.month(&month))?);
    }
    Ok(files)
}

fn delete_matches(connection: &Connection, file_list: &str) -> Result<u64, StoreError> {
    connection.execute_batch(&format!(
        "CREATE OR REPLACE TEMP TABLE existing_counts AS
         SELECT line_hash, count(*) AS n
         FROM read_parquet({file_list})
         WHERE ts >= (SELECT min(hour) FROM flush_overlap)
           AND ts < (SELECT max(hour) FROM flush_overlap) + INTERVAL 1 HOUR
           AND date_trunc('hour', ts) IN (SELECT hour FROM flush_overlap)
         GROUP BY line_hash;

         CREATE OR REPLACE TEMP TABLE flush_duplicates AS
         SELECT rid, line_hash FROM (
             SELECT s.rowid AS rid,
                    s.line_hash,
                    row_number() OVER (PARTITION BY s.line_hash ORDER BY s.rowid) AS rn,
                    e.n - coalesce(m.n, 0) AS remaining
             FROM staging_hits s
             JOIN existing_counts e USING (line_hash)
             LEFT JOIN (SELECT line_hash, sum(n) AS n FROM dedupe_matched GROUP BY line_hash) m
                 USING (line_hash)
         )
         WHERE rn <= remaining;

         INSERT INTO dedupe_matched
         SELECT line_hash, count(*) FROM flush_duplicates GROUP BY line_hash;"
    ))?;
    let removed = connection.execute(
        "DELETE FROM staging_hits WHERE rowid IN (SELECT rid FROM flush_duplicates)",
        [],
    )?;
    connection.execute_batch("DROP TABLE existing_counts; DROP TABLE flush_duplicates;")?;
    Ok(removed as u64)
}
