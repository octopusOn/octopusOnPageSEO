use std::fs::{File, TryLockError};
use std::path::Path;

use crate::KwStoreError;

/// Exclusive lock on a data directory, released on drop.
///
/// DuckDB refuses a second writer too, but with a database error; the lock gives a clear
/// "in use" message and also covers the raw response files.
#[derive(Debug)]
pub(crate) struct StoreLock {
    _file: File,
}

impl StoreLock {
    pub(crate) fn acquire(path: &Path) -> Result<Self, KwStoreError> {
        let file = File::options()
            .create(true)
            .truncate(false)
            .write(true)
            .open(path)
            .map_err(KwStoreError::io(path))?;
        match file.try_lock() {
            Ok(()) => Ok(Self { _file: file }),
            Err(TryLockError::WouldBlock) => {
                Err(KwStoreError::Busy(path.parent().unwrap_or(path).to_owned()))
            }
            Err(TryLockError::Error(source)) => Err(KwStoreError::Io {
                path: path.to_owned(),
                source,
            }),
        }
    }
}
