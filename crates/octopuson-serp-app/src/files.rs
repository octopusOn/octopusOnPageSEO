//! Small file helpers.

use std::path::Path;

use crate::SerpAppError;

/// Writes `bytes` to `path` through a temporary file, so readers never see half a file.
pub(crate) fn write_atomically(path: &Path, bytes: &[u8]) -> Result<(), SerpAppError> {
    Ok(octopuson_secrets::write_atomically(path, bytes)?)
}
