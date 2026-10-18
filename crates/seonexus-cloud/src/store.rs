//! The account token, one per gateway address, in a file every OctopusOn SEO program shares.
//!
//! The file (`account.json` in the common OctopusOn SEO data folder) is readable by its owner only,
//! like the secret files of the programs. One file instead of the system keychain means a
//! sign-in in one program reaches the others at once, with no keychain prompt per program, and
//! on every platform.

use std::path::{Path, PathBuf};

use seonexus_secrets::{FileBackend, SecretBackend};

use crate::{ACCOUNT_DIR_VARIABLE, CloudError};

/// Where the account token lives.
pub struct TokenStore {
    backend: Box<dyn SecretBackend>,
    base: String,
    /// The folder made owner-only before the first write (the shared folder).
    private_dir: Option<PathBuf>,
}

impl std::fmt::Debug for TokenStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TokenStore")
            .field("backend", &self.backend.name())
            .field("base", &self.base)
            .finish_non_exhaustive()
    }
}

/// The common data folder of the OctopusOn SEO programs, or [`ACCOUNT_DIR_VARIABLE`].
///
/// # Errors
/// [`CloudError::Invalid`] when the platform reports no data folder.
pub fn shared_dir() -> Result<PathBuf, CloudError> {
    if let Some(dir) = std::env::var_os(ACCOUNT_DIR_VARIABLE).filter(|dir| !dir.is_empty()) {
        return Ok(PathBuf::from(dir));
    }
    directories::ProjectDirs::from("com", "octopusSEO", "octopusSEO")
        .map(|dirs| dirs.data_dir().to_owned())
        .ok_or_else(|| CloudError::Invalid(format!("no data folder; set {ACCOUNT_DIR_VARIABLE}")))
}

impl TokenStore {
    /// Tokens for the gateway at `base` in `backend`.
    pub fn new(backend: Box<dyn SecretBackend>, base: impl Into<String>) -> Self {
        Self {
            backend,
            base: base.into(),
            private_dir: None,
        }
    }

    /// The account file every OctopusOn SEO program on this computer shares.
    ///
    /// # Errors
    /// [`CloudError::Invalid`] when the platform reports no data folder.
    pub fn shared(base: impl Into<String>) -> Result<Self, CloudError> {
        Ok(Self::in_dir(&shared_dir()?, base))
    }

    /// The account file in `dir` (tests, a program's own folder).
    pub fn in_dir(dir: &Path, base: impl Into<String>) -> Self {
        Self {
            backend: Box::new(FileBackend::new(dir.join("account.json"))),
            base: base.into(),
            private_dir: Some(dir.to_owned()),
        }
    }

    /// The gateway address.
    pub fn base(&self) -> &str {
        &self.base
    }

    fn key(&self) -> String {
        format!("account-token {}", self.base)
    }

    /// The saved token.
    ///
    /// # Errors
    /// [`CloudError::Secrets`] when the store cannot be read.
    pub fn load(&self) -> Result<Option<String>, CloudError> {
        self.backend
            .get(&self.key())
            .map(|token| token.filter(|token| !token.is_empty()))
            .map_err(|error| CloudError::Secrets(error.to_string()))
    }

    /// Saves a token.
    ///
    /// # Errors
    /// [`CloudError::Secrets`] when the store cannot be written.
    pub fn save(&self, token: &str) -> Result<(), CloudError> {
        if let Some(dir) = &self.private_dir {
            make_private_dir(dir)?;
        }
        self.backend
            .set(&self.key(), token)
            .map_err(|error| CloudError::Secrets(error.to_string()))
    }

    /// Forgets the token.
    ///
    /// # Errors
    /// [`CloudError::Secrets`] when the store cannot be written.
    pub fn clear(&self) -> Result<(), CloudError> {
        self.backend
            .delete(&self.key())
            .map_err(|error| CloudError::Secrets(error.to_string()))
    }
}

/// Creates `dir`, readable by the owner only on Unix.
fn make_private_dir(dir: &Path) -> Result<(), CloudError> {
    let failed = |error: std::io::Error| CloudError::Secrets(format!("{}: {error}", dir.display()));
    std::fs::create_dir_all(dir).map_err(failed)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700)).map_err(failed)?;
    }
    Ok(())
}
