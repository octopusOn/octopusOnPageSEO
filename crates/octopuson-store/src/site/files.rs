//! Parquet file management: listing, publishing staged imports and crash recovery.
//!
//! An import writes into `hits/.staging-<id>/flush-<k>/month=YYYY-MM/*.parquet`. After the
//! database transaction commits, files are renamed into `hits/month=YYYY-MM/`. Renames are
//! idempotent, so recovery after a crash simply repeats publishing for committed imports and
//! deletes staging directories of uncommitted ones.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use duckdb::{Connection, params};

use crate::error::StoreError;
use crate::paths::SitePaths;

/// Parquet files directly inside `dir`, sorted; a missing directory yields none.
pub(super) fn parquet_files(dir: &Path) -> Result<Vec<PathBuf>, StoreError> {
    let mut files = Vec::new();
    for entry in read_dir_if_exists(dir)? {
        let path = entry.map_err(StoreError::io(dir))?.path();
        if path.extension().is_some_and(|ext| ext == "parquet") {
            files.push(path);
        }
    }
    files.sort();
    Ok(files)
}

/// Glob matching every published hit file.
pub(super) fn published_glob(paths: &SitePaths) -> PathBuf {
    paths.hits().join("month=*").join("*.parquet")
}

/// Whether at least one hit file is published (DuckDB rejects a glob matching nothing).
pub(super) fn has_published_hits(paths: &SitePaths) -> Result<bool, StoreError> {
    let hits = paths.hits();
    for entry in read_dir_if_exists(&hits)? {
        let entry = entry.map_err(StoreError::io(&hits))?;
        let is_month = entry.file_name().to_string_lossy().starts_with("month=");
        if is_month && !parquet_files(&entry.path())?.is_empty() {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Moves the staged files of a committed import into the live month directories.
pub(super) fn publish(paths: &SitePaths, import_id: i64) -> Result<(), StoreError> {
    let staging = paths.staging(import_id);
    for flush_dir in read_dir_if_exists(&staging)? {
        let flush_dir = flush_dir.map_err(StoreError::io(&staging))?.path();
        for month_dir in read_dir_if_exists(&flush_dir)? {
            let month_dir = month_dir.map_err(StoreError::io(&flush_dir))?.path();
            let Some(name) = month_dir.file_name() else {
                continue;
            };
            let target = paths.hits().join(name);
            fs::create_dir_all(&target).map_err(StoreError::io(&target))?;
            for file in parquet_files(&month_dir)? {
                if let Some(file_name) = file.file_name() {
                    let destination = target.join(file_name);
                    fs::rename(&file, &destination).map_err(StoreError::io(&destination))?;
                }
            }
        }
    }
    discard(paths, import_id)
}

/// Deletes the staging directory of an import, if any.
pub(super) fn discard(paths: &SitePaths, import_id: i64) -> Result<(), StoreError> {
    let staging = paths.staging(import_id);
    match fs::remove_dir_all(&staging) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => Err(StoreError::Io {
            path: staging,
            source: error,
        }),
        _ => Ok(()),
    }
}

/// Finishes or rolls back imports interrupted by a crash.
pub(super) fn recover(connection: &Connection, paths: &SitePaths) -> Result<(), StoreError> {
    let hits = paths.hits();
    for entry in read_dir_if_exists(&hits)? {
        let entry = entry.map_err(StoreError::io(&hits))?;
        let Some(import_id) = entry
            .file_name()
            .to_str()
            .and_then(SitePaths::staging_import_id)
        else {
            continue;
        };
        let committed: bool = connection.query_row(
            "SELECT count(*) > 0 FROM imports WHERE id = ?",
            params![import_id],
            |row| row.get(0),
        )?;
        if committed {
            tracing::warn!(import_id, "publishing files of an interrupted import");
            publish(paths, import_id)?;
        } else {
            tracing::warn!(import_id, "removing files of an unfinished import");
            discard(paths, import_id)?;
        }
    }
    let orphans = remove_orphans(connection, paths)?;
    if orphans > 0 {
        tracing::warn!(orphans, "removed files of deleted imports");
    }
    Ok(())
}

/// Import id of a published hit file named `import_{id}_f{n}_{i}.parquet`.
fn published_import_id(file_name: &str) -> Option<i64> {
    file_name
        .strip_prefix("import_")?
        .split('_')
        .next()?
        .parse()
        .ok()
}

/// Deletes published hit files whose import no longer exists: the second step of deleting an
/// import, repeated on every write access so a deletion interrupted by a crash completes.
/// Returns the number of files removed.
pub(super) fn remove_orphans(
    connection: &Connection,
    paths: &SitePaths,
) -> Result<u64, StoreError> {
    let mut statement = connection.prepare("SELECT id FROM imports")?;
    let known: std::collections::HashSet<i64> = statement
        .query_map([], |row| row.get(0))?
        .collect::<Result<_, _>>()?;
    let hits = paths.hits();
    let mut removed = 0;
    for entry in read_dir_if_exists(&hits)? {
        let month = entry.map_err(StoreError::io(&hits))?.path();
        let is_month = month
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with("month="));
        if !is_month {
            continue;
        }
        let files = parquet_files(&month)?;
        let mut left = files.len();
        for file in files {
            let orphan = file
                .file_name()
                .and_then(|name| name.to_str())
                .and_then(published_import_id)
                .is_some_and(|id| !known.contains(&id));
            if orphan {
                fs::remove_file(&file).map_err(StoreError::io(&file))?;
                removed += 1;
                left -= 1;
            }
        }
        if left == 0 {
            // Best effort: the directory may hold other files.
            let _ = fs::remove_dir(&month);
        }
    }
    Ok(removed)
}

fn read_dir_if_exists(
    dir: &Path,
) -> Result<impl Iterator<Item = io::Result<fs::DirEntry>>, StoreError> {
    match fs::read_dir(dir) {
        Ok(entries) => Ok(Some(entries).into_iter().flatten()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None.into_iter().flatten()),
        Err(source) => Err(StoreError::Io {
            path: dir.to_owned(),
            source,
        }),
    }
}
