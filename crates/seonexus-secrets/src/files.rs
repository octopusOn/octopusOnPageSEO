//! Small file helpers.

use std::fs;
use std::io::Write;
use std::path::Path;

use crate::SecretError;

/// Writes `bytes` to `path` through a temporary file, so readers never see half a file.
///
/// # Errors
/// [`SecretError::Io`] when the file cannot be written.
pub fn write_atomically(path: &Path, bytes: &[u8]) -> Result<(), SecretError> {
    write_with_mode(path, bytes, false)
}

/// Like [`write_atomically`], readable by the owner only on Unix.
///
/// # Errors
/// [`SecretError::Io`] when the file cannot be written.
pub fn write_private(path: &Path, bytes: &[u8]) -> Result<(), SecretError> {
    write_with_mode(path, bytes, true)
}

fn write_with_mode(path: &Path, bytes: &[u8], private: bool) -> Result<(), SecretError> {
    let io = |source| SecretError::Io {
        path: path.to_owned(),
        source,
    };
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(io)?;
    }
    let temporary = path.with_extension("tmp");
    let mut options = fs::File::options();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    if private {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    #[cfg(not(unix))]
    let _ = private;
    let mut file = options.open(&temporary).map_err(io)?;
    file.write_all(bytes).map_err(io)?;
    file.sync_all().map_err(io)?;
    drop(file);
    fs::rename(&temporary, path).map_err(io)
}
