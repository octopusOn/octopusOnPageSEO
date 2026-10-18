//! API keys, OAuth tokens and other secrets of an OctopusOn SEO program.
//!
//! Values live in the operating system's credential store (macOS Keychain, Windows
//! Credential Manager), or in `secrets.json` readable by the owner only where there is none
//! (Linux) or when `OCTOPUSSEO_SECRET_STORE=file`. An environment variable
//! `OCTOPUSSEO_SECRET_<PROVIDER>_<NAME>` (upper case, `-` as `_`) overrides both, for scripts.
//!
//! Every saved secret is bound to the origins its provider sent requests to when it was
//! saved (`secret-bindings.json`, no values). If a provider spec is edited or imported so that
//! it points elsewhere, the secret counts as unset until it is entered again, so a spec can
//! never quietly send a key to a new host.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

mod files;

pub use files::{write_atomically, write_private};

/// Why a secret cannot be read or saved.
#[derive(Debug, thiserror::Error)]
pub enum SecretError {
    /// A file could not be read or written.
    #[error("cannot access `{}`: {source}", path.display())]
    Io {
        /// The file.
        path: PathBuf,
        /// The error.
        source: std::io::Error,
    },
    /// A secrets or bindings file is not valid.
    #[error("`{}` is invalid: {message}", path.display())]
    InvalidFile {
        /// The file.
        path: PathBuf,
        /// What is wrong.
        message: String,
    },
    /// The credential store refused, or the value is not acceptable.
    #[error("secret store: {0}")]
    Store(String),
}

/// Where a secret's value comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SecretState {
    /// Not entered.
    Unset,
    /// Saved for the current origins.
    Saved,
    /// Saved for other origins; must be entered again.
    Rebind,
    /// Set by an environment variable.
    Environment,
}

/// A store of secret values by key.
pub trait SecretBackend: Send + Sync {
    /// Short name shown to the user (`keychain`, `file`).
    fn name(&self) -> &'static str;
    /// The value of `key`.
    ///
    /// # Errors
    /// [`SecretError::Store`] when the store cannot be read.
    fn get(&self, key: &str) -> Result<Option<String>, SecretError>;
    /// Saves `value` under `key`.
    ///
    /// # Errors
    /// [`SecretError::Store`] when the store cannot be written.
    fn set(&self, key: &str, value: &str) -> Result<(), SecretError>;
    /// Removes `key`.
    ///
    /// # Errors
    /// [`SecretError::Store`] when the store cannot be written.
    fn delete(&self, key: &str) -> Result<(), SecretError>;
}

/// `secrets.json`, owner-readable only.
#[derive(Debug)]
pub struct FileBackend {
    path: PathBuf,
    lock: Mutex<()>,
}

impl FileBackend {
    /// A store in `path`.
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            lock: Mutex::new(()),
        }
    }

    fn read(&self) -> Result<BTreeMap<String, String>, SecretError> {
        match fs::read_to_string(&self.path) {
            Ok(text) => serde_json::from_str(&text).map_err(|error| SecretError::InvalidFile {
                path: self.path.clone(),
                message: error.to_string(),
            }),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(BTreeMap::new()),
            Err(source) => Err(SecretError::Io {
                path: self.path.clone(),
                source,
            }),
        }
    }

    fn write(&self, values: &BTreeMap<String, String>) -> Result<(), SecretError> {
        let text = serde_json::to_string_pretty(values)
            .map_err(|error| SecretError::Store(error.to_string()))?;
        write_private(&self.path, text.as_bytes())
    }
}

impl SecretBackend for FileBackend {
    fn name(&self) -> &'static str {
        "file"
    }

    fn get(&self, key: &str) -> Result<Option<String>, SecretError> {
        let _guard = self
            .lock
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        Ok(self.read()?.remove(key))
    }

    fn set(&self, key: &str, value: &str) -> Result<(), SecretError> {
        let _guard = self
            .lock
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut values = self.read()?;
        values.insert(key.to_owned(), value.to_owned());
        self.write(&values)
    }

    fn delete(&self, key: &str) -> Result<(), SecretError> {
        let _guard = self
            .lock
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut values = self.read()?;
        if values.remove(key).is_some() {
            self.write(&values)?;
        }
        Ok(())
    }
}

/// The operating system's credential store, under one service name per program.
#[cfg(any(target_os = "macos", windows))]
#[derive(Debug)]
pub struct KeychainBackend {
    service: &'static str,
}

#[cfg(any(target_os = "macos", windows))]
impl KeychainBackend {
    /// Entries under `service` (`OctopusOn SEO Keyword Research`).
    pub const fn new(service: &'static str) -> Self {
        Self { service }
    }

    fn entry(&self, key: &str) -> Result<keyring::Entry, SecretError> {
        keyring::Entry::new(self.service, key)
            .map_err(|error| SecretError::Store(error.to_string()))
    }
}

#[cfg(any(target_os = "macos", windows))]
impl SecretBackend for KeychainBackend {
    fn name(&self) -> &'static str {
        "keychain"
    }

    fn get(&self, key: &str) -> Result<Option<String>, SecretError> {
        match self.entry(key)?.get_password() {
            Ok(value) => Ok(Some(value)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(error) => Err(SecretError::Store(error.to_string())),
        }
    }

    fn set(&self, key: &str, value: &str) -> Result<(), SecretError> {
        self.entry(key)?
            .set_password(value)
            .map_err(|error| SecretError::Store(error.to_string()))
    }

    fn delete(&self, key: &str) -> Result<(), SecretError> {
        match self.entry(key)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(error) => Err(SecretError::Store(error.to_string())),
        }
    }
}

/// The platform's default backend: the OS credential store where there is one, under
/// `service`; `secrets.json` in `data_dir` otherwise or with `OCTOPUSSEO_SECRET_STORE=file`.
pub fn default_backend(data_dir: &Path, service: &'static str) -> Box<dyn SecretBackend> {
    let file =
        || Box::new(FileBackend::new(data_dir.join("secrets.json"))) as Box<dyn SecretBackend>;
    if std::env::var("OCTOPUSSEO_SECRET_STORE").is_ok_and(|store| store == "file") {
        return file();
    }
    #[cfg(any(target_os = "macos", windows))]
    {
        Box::new(KeychainBackend::new(service))
    }
    #[cfg(not(any(target_os = "macos", windows)))]
    {
        let _ = service;
        file()
    }
}

/// Looks up an environment variable.
type Environment = Box<dyn Fn(&str) -> Option<String> + Send + Sync>;

/// Secrets with origin bindings.
pub struct Secrets {
    backend: Box<dyn SecretBackend>,
    bindings_path: PathBuf,
    environment: Environment,
}

impl std::fmt::Debug for Secrets {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Secrets")
            .field("backend", &self.backend.name())
            .field("bindings_path", &self.bindings_path)
            .finish_non_exhaustive()
    }
}

impl Secrets {
    /// Secrets in `backend`, bindings in `bindings_path`, overridden by process environment
    /// variables.
    pub fn new(backend: Box<dyn SecretBackend>, bindings_path: PathBuf) -> Self {
        Self {
            backend,
            bindings_path,
            environment: Box::new(|name| std::env::var(name).ok()),
        }
    }

    /// Replaces the environment lookup (tests).
    #[must_use]
    pub fn with_environment(
        mut self,
        environment: impl Fn(&str) -> Option<String> + Send + Sync + 'static,
    ) -> Self {
        self.environment = Box::new(environment);
        self
    }

    /// Name of the backend.
    pub fn backend_name(&self) -> &'static str {
        self.backend.name()
    }

    /// Environment variable that overrides a secret.
    pub fn variable(provider: &str, name: &str) -> String {
        format!(
            "OCTOPUSSEO_SECRET_{}_{}",
            provider.to_ascii_uppercase().replace('-', "_"),
            name.to_ascii_uppercase()
        )
    }

    fn key(provider: &str, name: &str) -> String {
        format!("{provider}/{name}")
    }

    fn bindings(&self) -> Result<BTreeMap<String, Vec<String>>, SecretError> {
        match fs::read_to_string(&self.bindings_path) {
            Ok(text) => serde_json::from_str(&text).map_err(|error| SecretError::InvalidFile {
                path: self.bindings_path.clone(),
                message: error.to_string(),
            }),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(BTreeMap::new()),
            Err(source) => Err(SecretError::Io {
                path: self.bindings_path.clone(),
                source,
            }),
        }
    }

    fn save_bindings(&self, bindings: &BTreeMap<String, Vec<String>>) -> Result<(), SecretError> {
        let text = serde_json::to_string_pretty(bindings)
            .map_err(|error| SecretError::Store(error.to_string()))?;
        write_atomically(&self.bindings_path, text.as_bytes())
    }

    fn sorted(origins: &[&str]) -> Vec<String> {
        let mut origins: Vec<String> = origins.iter().map(|origin| (*origin).to_owned()).collect();
        origins.sort_unstable();
        origins.dedup();
        origins
    }

    /// Whether a secret is available for a provider sending requests to `origins`.
    ///
    /// # Errors
    /// [`SecretError`] when the bindings cannot be read.
    pub fn state(
        &self,
        provider: &str,
        name: &str,
        origins: &[&str],
    ) -> Result<SecretState, SecretError> {
        if (self.environment)(&Self::variable(provider, name))
            .is_some_and(|value| !value.is_empty())
        {
            return Ok(SecretState::Environment);
        }
        Ok(match self.bindings()?.get(&Self::key(provider, name)) {
            None => SecretState::Unset,
            Some(bound) if *bound == Self::sorted(origins) => SecretState::Saved,
            Some(_) => SecretState::Rebind,
        })
    }

    /// The value of a secret for a provider sending requests to `origins`; `None` when unset
    /// or bound to other origins.
    ///
    /// # Errors
    /// [`SecretError`] when the store cannot be read.
    pub fn get(
        &self,
        provider: &str,
        name: &str,
        origins: &[&str],
    ) -> Result<Option<String>, SecretError> {
        if let Some(value) =
            (self.environment)(&Self::variable(provider, name)).filter(|value| !value.is_empty())
        {
            return Ok(Some(value));
        }
        if self.state(provider, name, origins)? != SecretState::Saved {
            return Ok(None);
        }
        self.backend.get(&Self::key(provider, name))
    }

    /// Saves a secret and binds it to `origins`.
    ///
    /// # Errors
    /// [`SecretError`] when the value is empty or the store cannot be written.
    pub fn set(
        &self,
        provider: &str,
        name: &str,
        value: &str,
        origins: &[&str],
    ) -> Result<(), SecretError> {
        let value = value.trim();
        if value.is_empty() {
            return Err(SecretError::Store("the value is empty".to_owned()));
        }
        self.backend.set(&Self::key(provider, name), value)?;
        let mut bindings = self.bindings()?;
        bindings.insert(Self::key(provider, name), Self::sorted(origins));
        self.save_bindings(&bindings)
    }

    /// Removes a secret.
    ///
    /// # Errors
    /// [`SecretError`] when the store cannot be written.
    pub fn delete(&self, provider: &str, name: &str) -> Result<(), SecretError> {
        self.backend.delete(&Self::key(provider, name))?;
        let mut bindings = self.bindings()?;
        if bindings.remove(&Self::key(provider, name)).is_some() {
            self.save_bindings(&bindings)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn secrets(dir: &Path) -> Secrets {
        Secrets::new(
            Box::new(FileBackend::new(dir.join("secrets.json"))),
            dir.join("secret-bindings.json"),
        )
        .with_environment(|name| {
            (name == "OCTOPUSSEO_SECRET_SERP_API_API_KEY").then(|| "from-env".to_owned())
        })
    }

    #[test]
    fn binds_secrets_to_origins() {
        let dir = tempfile::tempdir().unwrap();
        let secrets = secrets(dir.path());
        let origins = ["https://api.acme.example"];
        assert_eq!(
            secrets.state("acme", "token", &origins).unwrap(),
            SecretState::Unset
        );
        secrets.set("acme", "token", " t0ken ", &origins).unwrap();
        assert_eq!(
            secrets.get("acme", "token", &origins).unwrap().as_deref(),
            Some("t0ken")
        );
        let moved = ["https://evil.example"];
        assert_eq!(
            secrets.state("acme", "token", &moved).unwrap(),
            SecretState::Rebind
        );
        assert_eq!(secrets.get("acme", "token", &moved).unwrap(), None);
        secrets.delete("acme", "token").unwrap();
        assert_eq!(secrets.get("acme", "token", &origins).unwrap(), None);
        assert!(secrets.set("acme", "token", "  ", &origins).is_err());
    }

    #[test]
    fn environment_variables_win() {
        let dir = tempfile::tempdir().unwrap();
        let secrets = secrets(dir.path());
        assert_eq!(
            Secrets::variable("serp-api", "api_key"),
            "OCTOPUSSEO_SECRET_SERP_API_API_KEY"
        );
        assert_eq!(
            secrets.state("serp-api", "api_key", &[]).unwrap(),
            SecretState::Environment
        );
        assert_eq!(
            secrets.get("serp-api", "api_key", &[]).unwrap().as_deref(),
            Some("from-env")
        );
    }

    #[cfg(unix)]
    #[test]
    fn the_file_is_private() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let secrets = secrets(dir.path());
        secrets
            .set("acme", "token", "x", &["https://a.example"])
            .unwrap();
        let mode = fs::metadata(dir.path().join("secrets.json"))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
        assert!(
            !fs::read_to_string(dir.path().join("secret-bindings.json"))
                .unwrap()
                .contains("\"x\"")
        );
    }
}
