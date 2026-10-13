use std::fs::{File, TryLockError};
use std::path::Path;

use crate::error::StoreError;

/// Lock on a site directory, released on drop.
///
/// Writers hold it exclusively, readers shared. DuckDB alone would let a reader in the same
/// process open a database that is being written; the lock closes that gap, also covers the
/// Parquet files and gives users a clear "site is busy" message instead of a database error.
#[derive(Debug)]
pub(crate) struct SiteLock {
    _file: File,
}

impl SiteLock {
    /// Exclusive lock for writers.
    pub(crate) fn acquire(path: &Path) -> Result<Self, StoreError> {
        Self::lock(path, File::try_lock)
    }

    /// Shared lock for readers; fails while a writer holds the site.
    pub(crate) fn acquire_shared(path: &Path) -> Result<Self, StoreError> {
        Self::lock(path, File::try_lock_shared)
    }

    fn lock(
        path: &Path,
        try_lock: fn(&File) -> Result<(), TryLockError>,
    ) -> Result<Self, StoreError> {
        let file = File::options()
            .create(true)
            .truncate(false)
            .write(true)
            .open(path)
            .map_err(StoreError::io(path))?;
        match try_lock(&file) {
            Ok(()) => Ok(Self { _file: file }),
            Err(TryLockError::WouldBlock) => Err(StoreError::SiteLocked(path.to_owned())),
            Err(TryLockError::Error(source)) => Err(StoreError::Io {
                path: path.to_owned(),
                source,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn second_lock_is_refused_until_release() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".lock");
        let first = SiteLock::acquire(&path).unwrap();
        assert!(matches!(
            SiteLock::acquire(&path),
            Err(StoreError::SiteLocked(_))
        ));
        drop(first);
        SiteLock::acquire(&path).unwrap();
    }

    #[test]
    fn shared_locks_coexist_but_exclude_writers() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".lock");
        let first = SiteLock::acquire_shared(&path).unwrap();
        let second = SiteLock::acquire_shared(&path).unwrap();
        assert!(matches!(
            SiteLock::acquire(&path),
            Err(StoreError::SiteLocked(_))
        ));
        drop((first, second));
        let writer = SiteLock::acquire(&path).unwrap();
        assert!(matches!(
            SiteLock::acquire_shared(&path),
            Err(StoreError::SiteLocked(_))
        ));
        drop(writer);
    }
}
