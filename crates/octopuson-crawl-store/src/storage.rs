//! Filesystem-only size snapshots: never connect to a database, including running archives.

use std::{fs, io, path::Path};

use serde::{Deserialize, Serialize};

use crate::{CrawlLibrary, CrawlStoreError};

/// The logical file size of one saved crawl at measurement time.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrawlStorage {
    /// Saved crawl identifier.
    pub id: String,
    /// All regular files, including metadata, WAL and nested spill files.
    ///
    /// Unknown if any entry is unreadable, disappears, is a symbolic link or is not a
    /// regular file/directory. This is not physical allocated space or a deletion estimate.
    pub bytes: Option<u64>,
}

impl CrawlLibrary {
    /// Measures the files of every readable saved crawl, without changing its metadata.
    ///
    /// Individual failures produce an unknown size, never a misleading zero or partial
    /// total. Symbolic links are not followed. Active crawls may change during measurement.
    ///
    /// # Errors
    /// The crawl library cannot be listed.
    pub fn storage(&self) -> Result<Vec<CrawlStorage>, CrawlStoreError> {
        Ok(self
            .list()?
            .into_iter()
            .map(|meta| CrawlStorage {
                bytes: self
                    .dir(&meta.id)
                    .ok()
                    .and_then(|dir| file_bytes(&dir).ok()),
                id: meta.id,
            })
            .collect())
    }
}

fn file_bytes(root: &Path) -> io::Result<u64> {
    let mut pending = vec![root.to_owned()];
    let mut total = 0_u64;
    while let Some(path) = pending.pop() {
        let meta = fs::symlink_metadata(&path)?;
        if meta.is_dir() {
            for entry in fs::read_dir(&path)? {
                pending.push(entry?.path());
            }
        } else if meta.is_file() {
            total = total
                .checked_add(meta.len())
                .ok_or_else(|| io::Error::other("crawl size exceeds u64"))?;
        } else {
            return Err(io::Error::other("crawl contains a non-regular entry"));
        }
    }
    Ok(total)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::{CrawlMeta, CrawlStatus};
    use octopuson_crawl::{CrawlConfig, CrawlMode, CrawlProgress};

    fn fixture(library: &CrawlLibrary, id: &str) -> Vec<u8> {
        let dir = library.path().join(id);
        fs::create_dir_all(&dir).unwrap();
        let meta = CrawlMeta {
            id: id.into(),
            name: id.into(),
            start_urls: vec![],
            mode: CrawlMode::Spider,
            created_at_ms: 0,
            finished_at_ms: None,
            status: CrawlStatus::Running,
            progress: CrawlProgress::default(),
            limit_reached: false,
            error: None,
            config: CrawlConfig::default(),
        };
        let bytes = serde_json::to_vec(&meta).unwrap();
        fs::write(dir.join("crawl.json"), &bytes).unwrap();
        bytes
    }

    #[test]
    fn includes_wal_and_nested_spill_without_opening_invalid_or_running_databases() {
        let root = tempfile::tempdir().unwrap();
        let library = CrawlLibrary::new(root.path());
        let before = fixture(&library, "running");
        let dir = library.path().join("running");
        fs::write(dir.join("crawl.duckdb"), b"not a DuckDB file").unwrap();
        fs::write(dir.join("crawl.duckdb.wal"), b"journal").unwrap();
        fs::create_dir_all(dir.join("crawl.duckdb.tmp/nested")).unwrap();
        fs::write(dir.join("crawl.duckdb.tmp/nested/spill.tmp"), [0; 37]).unwrap();
        fs::write(root.path().join("unrelated-export.csv"), [0; 100]).unwrap();
        let expected =
            u64::try_from(before.len() + b"not a DuckDB file".len() + b"journal".len() + 37)
                .unwrap();
        assert_eq!(
            library.storage().unwrap(),
            vec![CrawlStorage {
                id: "running".into(),
                bytes: Some(expected)
            }]
        );
        assert_eq!(fs::read(dir.join("crawl.json")).unwrap(), before);
        assert_eq!(
            fs::read(dir.join("crawl.duckdb")).unwrap(),
            b"not a DuckDB file"
        );
        assert_eq!(fs::read(dir.join("crawl.duckdb.wal")).unwrap(), b"journal");
        fs::remove_file(dir.join("crawl.duckdb.tmp/nested/spill.tmp")).unwrap();
        assert_eq!(library.storage().unwrap()[0].bytes, Some(expected - 37));
    }

    #[test]
    fn preserves_unknown_sizes_for_invalid_metadata_ids_and_reports_library_errors() {
        let root = tempfile::tempdir().unwrap();
        let library = CrawlLibrary::new(root.path());
        assert_eq!(library.storage().unwrap(), vec![]);
        let bytes = fixture(&library, "valid");
        let path = library.path().join("valid/crawl.json");
        let mut meta: CrawlMeta = serde_json::from_slice(&bytes).unwrap();
        meta.id = "../escape".into();
        fs::write(path, serde_json::to_vec(&meta).unwrap()).unwrap();
        assert_eq!(
            library.storage().unwrap(),
            vec![CrawlStorage {
                id: meta.id,
                bytes: None
            }]
        );
        fs::remove_dir_all(library.path()).unwrap();
        fs::write(library.path(), b"not a directory").unwrap();
        assert!(library.storage().is_err());
    }

    #[cfg(unix)]
    #[test]
    fn does_not_follow_directory_or_file_symlinks_or_publish_partial_sizes() {
        use std::os::unix::fs::symlink;
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let library = CrawlLibrary::new(root.path());
        fixture(&library, "with-link");
        let safe = fixture(&library, "safe");
        fs::write(outside.path().join("secret"), [0; 100]).unwrap();
        symlink(outside.path(), library.path().join("with-link/spill")).unwrap();
        let snapshot = library.storage().unwrap();
        assert_eq!(
            snapshot.iter().find(|s| s.id == "with-link").unwrap().bytes,
            None
        );
        assert_eq!(
            snapshot.iter().find(|s| s.id == "safe").unwrap().bytes,
            Some(u64::try_from(safe.len()).unwrap())
        );
        fs::remove_file(library.path().join("with-link/spill")).unwrap();
        symlink(
            outside.path().join("secret"),
            library.path().join("with-link/crawl.duckdb"),
        )
        .unwrap();
        assert_eq!(
            library
                .storage()
                .unwrap()
                .iter()
                .find(|s| s.id == "with-link")
                .unwrap()
                .bytes,
            None
        );
        let linked = fixture(&library, "linked");
        fs::write(outside.path().join("crawl.json"), linked).unwrap();
        fs::remove_dir_all(library.path().join("linked")).unwrap();
        symlink(outside.path(), library.path().join("linked")).unwrap();
        assert_eq!(
            library
                .storage()
                .unwrap()
                .iter()
                .find(|s| s.id == "linked")
                .unwrap()
                .bytes,
            None
        );
    }
}
