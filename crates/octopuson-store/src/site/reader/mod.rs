//! Read-only query execution against one site.

mod decode;

use duckdb::{Connection, params_from_iter};
use octopuson_core::table::TableSink;
use octopuson_query::sql::{CompiledQuery, Page};

use super::{ImportHistoryEntry, SiteStore, SiteTotals, bots, geo, relation, schema, stats};
use crate::catalog::SiteId;
use crate::db;
use crate::error::StoreError;
use crate::lock::SiteLock;
use crate::paths::{DataDir, SitePaths};
use crate::settings::StoreSettings;

/// Read-only handle to one site. Several readers (also in other processes) may be open at
/// once; while an import is running the site reports [`StoreError::SiteLocked`], and while
/// a reader is open imports are refused the same way.
///
/// Readers do not run crash recovery: files of an import interrupted between commit and
/// publish become visible on the next write access.
#[derive(Debug)]
pub struct SiteReader {
    connection: Connection,
    paths: SitePaths,
    // Declared last so the connection closes before the lock is released.
    _lock: SiteLock,
}

impl SiteReader {
    /// Opens `site` for querying; `None` if nothing was imported yet.
    ///
    /// # Errors
    /// [`StoreError::SiteLocked`] during an import, [`StoreError::SchemaTooNew`] for data
    /// written by a newer version, database errors otherwise.
    pub fn open(
        data_dir: &DataDir,
        site: SiteId,
        settings: StoreSettings,
    ) -> Result<Option<Self>, StoreError> {
        let paths = data_dir.site(site);
        if !paths.database().exists() {
            return Ok(None);
        }
        let (mut connection, mut lock) = open_read_only(&paths, settings)?;
        let latest = schema::MIGRATIONS.last().map_or(0, |m| m.version);
        let found = db::schema_version(&connection)?;
        if found > latest {
            return Err(StoreError::SchemaTooNew {
                found,
                supported: latest,
            });
        }
        if found < latest {
            // Upgrading needs write access once; later readers open directly.
            drop((connection, lock));
            drop(SiteStore::open(data_dir, site, settings)?);
            (connection, lock) = open_read_only(&paths, settings)?;
        }
        Ok(Some(Self {
            connection,
            paths,
            _lock: lock,
        }))
    }

    /// Runs `query`, streaming `page` (or all rows) into `sink`; returns the row count.
    ///
    /// # Errors
    /// Database errors (including invalid regular expressions) and sink failures.
    pub fn run(
        &self,
        query: &CompiledQuery,
        page: Option<Page>,
        sink: &mut dyn TableSink,
    ) -> Result<u64, StoreError> {
        let body = query.sql_for(page);
        let sql = if query.source.uses_hits() {
            format!("WITH hits AS ({}) {body}", self.hits_relation()?)
        } else {
            body
        };
        let mut statement = self.connection.prepare(&sql)?;
        let params: Vec<duckdb::types::Value> = query.params.iter().map(decode::to_value).collect();
        let mut rows = statement.query(params_from_iter(params.iter()))?;
        sink.begin(&query.columns).map_err(StoreError::Output)?;
        let mut cells = Vec::with_capacity(query.columns.len());
        let mut count = 0_u64;
        while let Some(row) = rows.next()? {
            cells.clear();
            for index in 0..query.columns.len() {
                cells.push(decode::cell(row.get_ref(index)?));
            }
            sink.row(&cells).map_err(StoreError::Output)?;
            count += 1;
        }
        sink.finish().map_err(StoreError::Output)?;
        Ok(count)
    }

    /// Counts imports, hits and dimensions.
    ///
    /// # Errors
    /// Database errors.
    pub fn totals(&self) -> Result<SiteTotals, StoreError> {
        stats::totals(&self.connection)
    }

    /// Committed imports, newest first.
    ///
    /// # Errors
    /// Database errors.
    pub fn import_history(&self) -> Result<Vec<ImportHistoryEntry>, StoreError> {
        stats::import_history(&self.connection)
    }

    /// Classification rules version of the stored user agents (0 = never classified).
    ///
    /// # Errors
    /// Database errors.
    pub fn bot_catalog_version(&self) -> Result<u32, StoreError> {
        bots::stored_catalog_version(&self.connection)
    }

    /// Data release the stored IP country/AS answers come from (`None`: never resolved).
    ///
    /// # Errors
    /// Database errors.
    pub fn geo_release(&self) -> Result<Option<String>, StoreError> {
        geo::stored_release(&self.connection)
    }

    fn hits_relation(&self) -> Result<String, StoreError> {
        relation::query_hits(&self.paths)
    }
}

/// Takes the shared site lock first so a running import is reported as "busy" (DuckDB
/// itself would let a reader in the same process open a database that is being written).
fn open_read_only(
    paths: &SitePaths,
    settings: StoreSettings,
) -> Result<(Connection, SiteLock), StoreError> {
    let lock = SiteLock::acquire_shared(&paths.lock_file())?;
    let connection = db::open_read_only(&paths.database(), settings)?;
    Ok((connection, lock))
}

#[cfg(test)]
mod tests;
