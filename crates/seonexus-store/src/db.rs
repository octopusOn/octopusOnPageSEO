//! Opening DuckDB databases with application-wide settings and versioned migrations.

use std::path::Path;
use std::time::Duration;

use duckdb::{AccessMode, Config, Connection, params};

use crate::error::StoreError;
use crate::settings::StoreSettings;

/// One forward-only schema change.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Migration {
    pub(crate) version: i64,
    pub(crate) sql: &'static str,
}

/// Opens (creating if needed) a database file.
///
/// Extension auto-install/auto-load is disabled: Parquet is compiled in, and the
/// application must never download code at runtime.
pub(crate) fn open(path: &Path, settings: StoreSettings) -> Result<Connection, StoreError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(StoreError::io(parent))?;
    }
    with_lock_retry(|| Ok(Connection::open_with_flags(path, config(settings)?)?))
}

/// Opens an existing database without write access; several readers may share a file.
pub(crate) fn open_read_only(
    path: &Path,
    settings: StoreSettings,
) -> Result<Connection, StoreError> {
    with_lock_retry(|| {
        let config = config(settings)?.access_mode(AccessMode::ReadOnly)?;
        Ok(Connection::open_with_flags(path, config)?)
    })
}

/// Attempts before a file lock held by another process is reported.
const LOCK_ATTEMPTS: u32 = 30;
const LOCK_RETRY_DELAY: Duration = Duration::from_millis(100);

/// Another process (the CLI next to the desktop app) may briefly hold the database file; short
/// operations are waited for instead of failing immediately.
fn with_lock_retry(
    mut open: impl FnMut() -> Result<Connection, StoreError>,
) -> Result<Connection, StoreError> {
    let mut attempt = 1;
    loop {
        match open() {
            Err(StoreError::Database(error))
                if attempt < LOCK_ATTEMPTS && error.to_string().contains("Could not set lock") =>
            {
                tracing::debug!(
                    attempt,
                    "database file is locked by another process; retrying"
                );
                std::thread::sleep(LOCK_RETRY_DELAY);
                attempt += 1;
            }
            other => return other,
        }
    }
}

fn config(settings: StoreSettings) -> Result<Config, StoreError> {
    Ok(Config::default()
        .enable_autoload_extension(false)?
        .with("autoinstall_known_extensions", "false")?
        .max_memory(&format!("{}MiB", settings.memory_limit_mb.max(64)))?)
}

/// Latest applied migration version (0 for an empty database).
pub(crate) fn schema_version(connection: &Connection) -> Result<i64, StoreError> {
    Ok(connection.query_row(
        "SELECT coalesce(max(version), 0) FROM schema_migrations",
        [],
        |row| row.get(0),
    )?)
}

/// Opens `path` and applies pending `migrations` (sorted by version).
pub(crate) fn open_migrated(
    path: &Path,
    migrations: &[Migration],
    settings: StoreSettings,
) -> Result<Connection, StoreError> {
    let mut connection = open(path, settings)?;
    migrate(&mut connection, migrations)?;
    Ok(connection)
}

fn migrate(connection: &mut Connection, migrations: &[Migration]) -> Result<(), StoreError> {
    connection.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
             version BIGINT PRIMARY KEY,
             applied_at TIMESTAMP NOT NULL DEFAULT current_timestamp
         );",
    )?;
    let current = schema_version(connection)?;
    let supported = migrations.last().map_or(0, |m| m.version);
    if current > supported {
        return Err(StoreError::SchemaTooNew {
            found: current,
            supported,
        });
    }
    for migration in migrations.iter().filter(|m| m.version > current) {
        let transaction = connection.transaction()?;
        transaction.execute_batch(migration.sql)?;
        transaction.execute(
            "INSERT INTO schema_migrations (version) VALUES (?)",
            params![migration.version],
        )?;
        transaction.commit()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const V1: Migration = Migration {
        version: 1,
        sql: "CREATE TABLE t (x INTEGER);",
    };
    const V2: Migration = Migration {
        version: 2,
        sql: "ALTER TABLE t ADD COLUMN y INTEGER;",
    };

    #[test]
    fn applies_pending_migrations_once() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("db.duckdb");
        drop(open_migrated(&path, &[V1], StoreSettings::default()).unwrap());
        let connection = open_migrated(&path, &[V1, V2], StoreSettings::default()).unwrap();
        connection
            .execute("INSERT INTO t VALUES (1, 2)", [])
            .unwrap();
        drop(connection);
        drop(open_migrated(&path, &[V1, V2], StoreSettings::default()).unwrap());
    }

    #[test]
    fn refuses_newer_schema() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("db.duckdb");
        drop(open_migrated(&path, &[V1, V2], StoreSettings::default()).unwrap());
        let error = open_migrated(&path, &[V1], StoreSettings::default()).unwrap_err();
        assert!(matches!(
            error,
            StoreError::SchemaTooNew {
                found: 2,
                supported: 1
            }
        ));
    }
}
