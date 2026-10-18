//! Import history rows written when an import commits.

use duckdb::{Connection, params};
use octopuson_core::Timestamp;

use super::types::{ImportOutcome, ImportRecord};
use crate::error::StoreError;

pub(super) fn record_import(
    connection: &Connection,
    record: &ImportRecord<'_>,
    outcome: &ImportOutcome,
    started_at: Timestamp,
) -> Result<(), StoreError> {
    let micros = |ts: Option<Timestamp>| ts.map(Timestamp::as_unix_micros);
    connection.execute(
        "INSERT INTO imports VALUES (
             ?, ?, make_timestamp(?), make_timestamp(?), ?, ?, ?, ?,
             make_timestamp(?), make_timestamp(?))",
        params![
            outcome.import_id,
            record.format_spec,
            started_at.as_unix_micros(),
            Timestamp::now().as_unix_micros(),
            record.total_lines,
            record.parsed_lines,
            outcome.inserted_rows,
            outcome.duplicate_rows,
            micros(record.first_request),
            micros(record.last_request),
        ],
    )?;
    connection.execute(
        "INSERT INTO imported_files VALUES (?, ?, ?, ?)",
        params![
            record.file.fingerprint.to_hex(),
            outcome.import_id,
            record.file.path,
            record.file.fingerprint.size(),
        ],
    )?;
    for (reason, lines) in record.skipped {
        connection.execute(
            "INSERT INTO import_skips VALUES (?, ?, ?)",
            params![outcome.import_id, reason, lines],
        )?;
    }
    for sample in record.samples {
        connection.execute(
            "INSERT INTO import_samples VALUES (?, ?, ?, ?, ?)",
            params![
                outcome.import_id,
                sample.stream,
                sample.line_number,
                sample.reason,
                sample.excerpt
            ],
        )?;
    }
    Ok(())
}
