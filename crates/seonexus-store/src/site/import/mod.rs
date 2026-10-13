//! Transactional, de-duplicating import of hit batches.

mod dedupe;
mod flush;
mod history;
mod types;

pub use types::{ImportOptions, ImportOutcome, ImportRecord, ImportedFile, SkippedSample};

use std::time::{Duration, Instant};

use duckdb::Connection;
use seonexus_core::Timestamp;

use super::bots::BotIpVerifier;
use super::files;
use crate::batch::{HitBatch, STAGING_TABLE_SQL};
use crate::error::StoreError;
use crate::paths::SitePaths;

/// An open import. Everything it writes becomes visible atomically on
/// [`commit`](Self::commit); dropping the session without committing rolls back the
/// database changes and deletes staged Parquet files (this is how cancellation works).
pub struct ImportSession<'a> {
    connection: &'a Connection,
    paths: &'a SitePaths,
    verifier: &'a dyn BotIpVerifier,
    options: ImportOptions,
    import_id: i64,
    started_at: Timestamp,
    staged_rows: usize,
    flushes: u32,
    inserted_rows: u64,
    duplicate_rows: u64,
    append_time: Duration,
    open: bool,
}

impl<'a> ImportSession<'a> {
    pub(super) fn begin(
        connection: &'a Connection,
        paths: &'a SitePaths,
        verifier: &'a dyn BotIpVerifier,
        options: ImportOptions,
    ) -> Result<Self, StoreError> {
        connection.execute_batch("BEGIN TRANSACTION;")?;
        let mut session = Self {
            connection,
            paths,
            verifier,
            options,
            import_id: 0,
            started_at: Timestamp::now(),
            staged_rows: 0,
            flushes: 0,
            inserted_rows: 0,
            duplicate_rows: 0,
            append_time: Duration::ZERO,
            open: true,
        };
        session.import_id =
            connection.query_row("SELECT nextval('import_id_seq')", [], |row| row.get(0))?;
        files::discard(paths, session.import_id)?;
        let staging = paths.staging(session.import_id);
        std::fs::create_dir_all(&staging).map_err(StoreError::io(&staging))?;
        connection.execute_batch(STAGING_TABLE_SQL)?;
        connection.execute_batch(dedupe::MATCHED_TABLE_SQL)?;
        Ok(session)
    }

    /// Identifier this import will be committed under.
    pub fn import_id(&self) -> i64 {
        self.import_id
    }

    /// Stages a batch, flushing to Parquet when enough rows have accumulated.
    ///
    /// # Errors
    /// Database or filesystem errors; the session should then be dropped.
    pub fn append(&mut self, batch: HitBatch) -> Result<(), StoreError> {
        if batch.is_empty() {
            return Ok(());
        }
        self.staged_rows += batch.len();
        let started = Instant::now();
        {
            let mut appender =
                self.connection
                    .appender_to_catalog_and_db("staging_hits", "temp", "main")?;
            appender.append_record_batch(batch.batch)?;
            appender.flush()?;
        }
        self.append_time += started.elapsed();
        if self.staged_rows >= self.options.flush_rows {
            self.flush()?;
        }
        Ok(())
    }

    /// Writes remaining rows, records the import and makes it visible.
    ///
    /// # Errors
    /// Database or filesystem errors. If publishing files fails after the commit, the next
    /// [`SiteStore::open`](super::SiteStore::open) completes it.
    pub fn commit(mut self, record: &ImportRecord<'_>) -> Result<ImportOutcome, StoreError> {
        self.flush()?;
        tracing::debug!(
            append_ms = self.append_time.as_millis(),
            flushes = self.flushes,
            "import staged"
        );
        let outcome = ImportOutcome {
            import_id: self.import_id,
            inserted_rows: self.inserted_rows,
            duplicate_rows: self.duplicate_rows,
        };
        history::record_import(self.connection, record, &outcome, self.started_at)?;
        self.connection
            .execute_batch("DROP TABLE staging_hits; DROP TABLE dedupe_matched; COMMIT;")?;
        self.open = false;
        files::publish(self.paths, self.import_id)?;
        Ok(outcome)
    }

    fn flush(&mut self) -> Result<(), StoreError> {
        if self.staged_rows == 0 {
            return Ok(());
        }
        let result = flush::flush(
            self.connection,
            self.paths,
            self.import_id,
            self.flushes,
            self.verifier,
        )?;
        self.flushes += 1;
        self.staged_rows = 0;
        self.inserted_rows += result.inserted;
        self.duplicate_rows += result.duplicates;
        Ok(())
    }
}

impl std::fmt::Debug for ImportSession<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ImportSession")
            .field("import_id", &self.import_id)
            .field("staged_rows", &self.staged_rows)
            .field("flushes", &self.flushes)
            .finish_non_exhaustive()
    }
}

impl Drop for ImportSession<'_> {
    fn drop(&mut self) {
        if !self.open {
            return;
        }
        if let Err(error) = self.connection.execute_batch("ROLLBACK;") {
            tracing::error!(%error, import_id = self.import_id, "rollback failed");
        }
        if let Err(error) = files::discard(self.paths, self.import_id) {
            tracing::error!(%error, import_id = self.import_id, "removing staged files failed");
        }
    }
}
