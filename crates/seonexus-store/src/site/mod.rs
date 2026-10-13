//! Per-site storage: the site database plus its Parquet hit files.

mod bots;
mod files;
mod geo;
mod import;
mod maintenance;
mod reader;
mod relation;
mod schema;
mod stats;

pub use bots::{
    BotIpPair, BotIpVerdict, BotIpVerifier, BotTag, NoBotVerification, UserAgentClassifier,
    VerifyScope,
};
pub use geo::{GeoRefresh, IpGeo, IpGeoLookup};
pub use import::{
    ImportOptions, ImportOutcome, ImportRecord, ImportSession, ImportedFile, SkippedSample,
};
pub use maintenance::{BotVerifyOutcome, ReclassifyOutcome};
pub use reader::SiteReader;
pub use stats::{ImportHistoryEntry, SiteTotals};

use duckdb::{Connection, params};

use crate::catalog::SiteId;
use crate::db;
use crate::error::StoreError;
use crate::fingerprint::FileFingerprint;
use crate::lock::SiteLock;
use crate::paths::{DataDir, SitePaths};
use crate::settings::StoreSettings;
use crate::sql::path_literal;

/// Exclusive handle to one site's data. Opening it acquires the site lock and finishes or
/// rolls back imports interrupted by a crash.
#[derive(Debug)]
pub struct SiteStore {
    connection: Connection,
    paths: SitePaths,
    _lock: SiteLock,
}

impl SiteStore {
    /// Opens the store of `site` inside `data_dir`, creating it on first use.
    ///
    /// # Errors
    /// [`StoreError::SiteLocked`] if another process uses the site; database and
    /// filesystem errors otherwise.
    pub fn open(
        data_dir: &DataDir,
        site: SiteId,
        settings: StoreSettings,
    ) -> Result<Self, StoreError> {
        let paths = data_dir.site(site);
        std::fs::create_dir_all(paths.dir()).map_err(StoreError::io(paths.dir()))?;
        let lock = SiteLock::acquire(&paths.lock_file())?;
        let connection = db::open_migrated(&paths.database(), schema::MIGRATIONS, settings)?;
        files::recover(&connection, &paths)?;
        Ok(Self {
            connection,
            paths,
            _lock: lock,
        })
    }

    /// Whether the site has a database yet (i.e. something was imported or opened before).
    pub fn exists(data_dir: &DataDir, site: SiteId) -> bool {
        data_dir.site(site).database().exists()
    }

    /// Flushes the database and lists the site's files for an archive as
    /// `(path inside the archive, file)`: `site.duckdb` and every published hit file. Keep the
    /// store open while copying them; it holds the site lock.
    ///
    /// # Errors
    /// Database or filesystem errors.
    pub fn archive_files(&self) -> Result<Vec<(String, std::path::PathBuf)>, StoreError> {
        self.connection.execute_batch("CHECKPOINT;")?;
        let mut files = vec![("site.duckdb".to_owned(), self.paths.database())];
        let hits = self.paths.hits();
        if let Ok(entries) = std::fs::read_dir(&hits) {
            let mut months: Vec<_> = entries.filter_map(Result::ok).map(|e| e.path()).collect();
            months.sort();
            for month in months {
                let Some(month_name) = month
                    .file_name()
                    .and_then(|n| n.to_str())
                    .map(str::to_owned)
                else {
                    continue;
                };
                if !month_name.starts_with("month=") {
                    continue;
                }
                for file in files::parquet_files(&month)? {
                    if let Some(name) = file.file_name().and_then(|n| n.to_str()) {
                        files.push((format!("hits/{month_name}/{name}"), file.clone()));
                    }
                }
            }
        }
        Ok(files)
    }

    /// Whether a file with this fingerprint was already imported.
    ///
    /// # Errors
    /// Database errors.
    pub fn is_imported(&self, fingerprint: &FileFingerprint) -> Result<bool, StoreError> {
        Ok(self.connection.query_row(
            "SELECT count(*) > 0 FROM imported_files WHERE fingerprint = ?",
            params![fingerprint.to_hex()],
            |row| row.get(0),
        )?)
    }

    /// Starts an import. Only one session can be open at a time. `verifier` decides verdicts
    /// of `(ip, bot)` pairs seen for the first time.
    ///
    /// # Errors
    /// Database or filesystem errors.
    pub fn begin_import<'a>(
        &'a mut self,
        options: ImportOptions,
        verifier: &'a dyn BotIpVerifier,
    ) -> Result<ImportSession<'a>, StoreError> {
        ImportSession::begin(&self.connection, &self.paths, verifier, options)
    }

    /// Counts imports, hits and dimensions.
    ///
    /// # Errors
    /// Database errors.
    pub fn totals(&self) -> Result<SiteTotals, StoreError> {
        stats::totals(&self.connection)
    }

    /// Deletes one import: its rollups and history rows, then its hit files. Rows that later
    /// imports skipped as duplicates of this import are deleted with it; importing the file
    /// again restores them.
    ///
    /// # Errors
    /// [`StoreError::ImportNotFound`], database or filesystem errors.
    pub fn delete_import(&mut self, import_id: i64) -> Result<(), StoreError> {
        let exists: bool = self.connection.query_row(
            "SELECT count(*) > 0 FROM imports WHERE id = ?",
            params![import_id],
            |row| row.get(0),
        )?;
        if !exists {
            return Err(StoreError::ImportNotFound(import_id));
        }
        maintenance::in_transaction(&self.connection, |connection| {
            for table in [
                "r_day_url",
                "r_hour",
                "imported_files",
                "import_skips",
                "import_samples",
            ] {
                connection.execute(
                    &format!("DELETE FROM {table} WHERE import_id = ?"),
                    params![import_id],
                )?;
            }
            connection.execute("DELETE FROM imports WHERE id = ?", params![import_id])?;
            Ok(())
        })?;
        files::remove_orphans(&self.connection, &self.paths)?;
        Ok(())
    }

    /// Looks up country and autonomous system of client IPs not resolved yet (all of them when
    /// `lookup` carries a different data release than the stored answers).
    ///
    /// # Errors
    /// Database or filesystem errors.
    pub fn refresh_geo(&mut self, lookup: &dyn IpGeoLookup) -> Result<GeoRefresh, StoreError> {
        geo::refresh(&self.connection, &self.paths, lookup)
    }

    /// Records the site's own host (e.g. `acme.com`), which queries use to recognise
    /// internal referers; `None` forgets it.
    ///
    /// # Errors
    /// Database errors.
    pub fn set_site_host(&self, host: Option<&str>) -> Result<(), StoreError> {
        match host {
            Some(host) => self.connection.execute(
                "INSERT OR REPLACE INTO site_meta VALUES ('site_host', ?)",
                params![host.to_ascii_lowercase()],
            )?,
            None => self
                .connection
                .execute("DELETE FROM site_meta WHERE key = 'site_host'", [])?,
        };
        Ok(())
    }

    /// Deletes every file of the site (database and hits). The site lock is held until the
    /// data is gone, so no reader or import can observe a half-deleted site.
    ///
    /// # Errors
    /// Database or filesystem errors.
    pub fn destroy(self) -> Result<(), StoreError> {
        let Self {
            connection,
            paths,
            _lock: lock,
        } = self;
        connection
            .close()
            .map_err(|(_, error)| StoreError::Database(error))?;
        let database = paths.database();
        let mut wal = database.clone().into_os_string();
        wal.push(".wal");
        for file in [database, wal.into()] {
            remove_if_exists(&file, |path| std::fs::remove_file(path))?;
        }
        remove_if_exists(&paths.hits(), |path| std::fs::remove_dir_all(path))?;
        // The lock file can only be deleted once released (Windows refuses open files).
        drop(lock);
        remove_if_exists(paths.dir(), |path| std::fs::remove_dir_all(path))
    }

    /// Counts rows in the published Parquet files (slow; for verification and tests).
    ///
    /// # Errors
    /// Database or filesystem errors.
    pub fn count_stored_hits(&self) -> Result<u64, StoreError> {
        if !files::has_published_hits(&self.paths)? {
            return Ok(0);
        }
        Ok(self.connection.query_row(
            &format!(
                "SELECT count(*) FROM read_parquet({}, hive_partitioning = true)",
                path_literal(&files::published_glob(&self.paths))?
            ),
            [],
            |row| row.get(0),
        )?)
    }
}

fn remove_if_exists(
    path: &std::path::Path,
    remove: fn(&std::path::Path) -> std::io::Result<()>,
) -> Result<(), StoreError> {
    match remove(path) {
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => Err(StoreError::Io {
            path: path.to_owned(),
            source: error,
        }),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests;
