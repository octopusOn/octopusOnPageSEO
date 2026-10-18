//! Use cases of the OctopusOn SEO Keyword Research.
//!
//! A [`KeywordWorkspace`] is one data directory, independent of the SERP Checker's:
//! `settings.json` (defaults, budgets, the free suggestion endpoints, the browser mode,
//! network), the keys of the sources in the system keychain, the store of projects, keywords,
//! numbers, result pages and clusterings (`kw.duckdb`), and the browser mode's profile
//! (`browser-profile`).

pub mod browser;
pub mod cloud;
pub mod cluster;
pub mod discover;
pub mod enrich;
pub mod export;
pub mod google;
pub mod import;
pub mod keywords;
pub mod projects;
pub mod runner;
pub mod serps;
pub mod settings;
pub mod sources;

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use seonexus_core::Timestamp;
use seonexus_kw_store::{KwStore, KwStoreError};
use seonexus_secrets::{Secrets, default_backend};

pub use seonexus_cloud as cloud_api;
pub use seonexus_kw_store as store;
pub use seonexus_secrets as secrets;

use crate::browser::BrowserState;
use crate::settings::Settings;

const QUALIFIER: &str = "com";
const ORGANIZATION: &str = "octopusSEO";
/// Name of the program: its data directory and its keychain service.
pub const APPLICATION: &str = "octopusSEO Keyword Miner";
/// Environment variable naming the data directory.
pub const DATA_DIR_VARIABLE: &str = "OCTOPUSSEO_KW_DATA_DIR";
/// How long [`KeywordWorkspace::store`] waits for a store in use.
const STORE_WAIT: Duration = Duration::from_secs(5);

/// Errors of the SEO Keyword Research.
#[derive(Debug, thiserror::Error)]
pub enum KwAppError {
    /// The platform reports no data directory.
    #[error("cannot determine the application data directory; pass --data-dir")]
    NoDataDir,
    /// A file could not be read or written.
    #[error("cannot access `{}`: {source}", path.display())]
    Io {
        /// The file.
        path: PathBuf,
        /// The error.
        source: std::io::Error,
    },
    /// A settings or input file is invalid.
    #[error("`{}` is invalid: {message}", path.display())]
    InvalidFile {
        /// The file.
        path: PathBuf,
        /// What is wrong.
        message: String,
    },
    /// Settings values are out of range.
    #[error("invalid settings: {0}")]
    InvalidSettings(String),
    /// A request is invalid (unknown engine, empty name, nothing to cluster …).
    #[error("{0}")]
    Invalid(String),
    /// The secret store failed.
    #[error("secret store: {0}")]
    Secrets(String),
    /// The OctopusOn SEO account cannot be used.
    #[error("{0}")]
    Cloud(String),
    /// The store failed.
    #[error(transparent)]
    Store(#[from] KwStoreError),
}

impl KwAppError {
    pub(crate) fn invalid_file(path: &Path, error: impl std::fmt::Display) -> Self {
        Self::InvalidFile {
            path: path.to_owned(),
            message: error.to_string(),
        }
    }

    pub(crate) fn io(path: &Path) -> impl FnOnce(std::io::Error) -> Self + '_ {
        move |source| Self::Io {
            path: path.to_owned(),
            source,
        }
    }
}

/// One data directory of the SEO Keyword Research.
pub struct KeywordWorkspace {
    data_dir: PathBuf,
    secrets: Secrets,
    browser: Mutex<BrowserState>,
    cloud: crate::cloud::CloudState,
}

impl std::fmt::Debug for KeywordWorkspace {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KeywordWorkspace")
            .field("data_dir", &self.data_dir)
            .field("secrets", &self.secrets)
            .finish_non_exhaustive()
    }
}

impl KeywordWorkspace {
    /// The workspace in `data_dir`, or in the platform's application data directory.
    ///
    /// # Errors
    /// [`KwAppError::NoDataDir`] when the platform reports none.
    pub fn resolve(data_dir: Option<PathBuf>) -> Result<Self, KwAppError> {
        let data_dir = match data_dir {
            Some(dir) => dir,
            None => directories::ProjectDirs::from(QUALIFIER, ORGANIZATION, APPLICATION)
                .ok_or(KwAppError::NoDataDir)?
                .data_dir()
                .to_owned(),
        };
        let secrets = Secrets::new(
            default_backend(&data_dir, APPLICATION),
            data_dir.join("secret-bindings.json"),
        );
        let api =
            seonexus_cloud::api_base(std::env::var(seonexus_cloud::API_VARIABLE).ok().as_deref())
                .map_err(|error| KwAppError::InvalidSettings(error.to_string()))?;
        // One account file for every OctopusOn SEO program: connecting once connects them all.
        let tokens = seonexus_cloud::TokenStore::shared(api)
            .map_err(|error| KwAppError::InvalidSettings(error.to_string()))?;
        Ok(Self::with_secrets(data_dir, secrets).with_tokens(tokens))
    }

    /// A workspace with a given secret store (tests, embedding). The account token lives in the
    /// data directory until [`Self::with_tokens`] says otherwise.
    pub fn with_secrets(data_dir: PathBuf, secrets: Secrets) -> Self {
        let tokens = seonexus_cloud::TokenStore::new(
            Box::new(seonexus_secrets::FileBackend::new(
                data_dir.join("account.json"),
            )),
            seonexus_cloud::DEFAULT_API,
        );
        Self {
            data_dir,
            secrets,
            browser: Mutex::new(BrowserState::default()),
            cloud: crate::cloud::CloudState::new(tokens),
        }
    }

    /// Keeps the OctopusOn SEO account token in `tokens`.
    #[must_use]
    pub fn with_tokens(mut self, tokens: seonexus_cloud::TokenStore) -> Self {
        self.cloud = crate::cloud::CloudState::new(tokens);
        self
    }

    /// The data directory.
    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    /// The secret store.
    pub fn secrets(&self) -> &Secrets {
        &self.secrets
    }

    fn settings_path(&self) -> PathBuf {
        self.data_dir.join("settings.json")
    }

    /// Current settings.
    ///
    /// # Errors
    /// [`KwAppError`] for an unreadable or invalid file.
    pub fn settings(&self) -> Result<Settings, KwAppError> {
        Settings::load(&self.settings_path())
    }

    /// Saves settings.
    ///
    /// # Errors
    /// [`KwAppError`] for invalid values or I/O errors.
    pub fn save_settings(&self, settings: &Settings) -> Result<(), KwAppError> {
        settings.save(&self.settings_path())
    }

    /// Opens the store.
    ///
    /// Work holds the store for moments only, so a store in use is waited for briefly before
    /// giving up.
    ///
    /// # Errors
    /// [`KwAppError::Store`], e.g. while another program keeps using it.
    pub fn store(&self) -> Result<KwStore, KwAppError> {
        let deadline = Instant::now() + STORE_WAIT;
        loop {
            match KwStore::open(&self.data_dir) {
                Err(KwStoreError::Busy(_)) if Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(20));
                }
                other => return Ok(other?),
            }
        }
    }

    /// Requests and spending per provider and month.
    ///
    /// # Errors
    /// Store errors.
    pub fn usage(&self) -> Result<Vec<store::ProviderUsage>, KwAppError> {
        Ok(self.store()?.usage()?)
    }
}

/// The text language of a project's language code.
pub fn text_language(code: &str) -> seonexus_text::Language {
    seonexus_text::Language::from_code(code)
}

/// The current time, RFC 3339 in whole seconds (for records written by callers).
pub fn timestamp() -> String {
    now()
}

/// The current time, RFC 3339 in whole seconds.
pub(crate) fn now() -> String {
    ago(0)
}

/// The time `seconds` ago, RFC 3339 in whole seconds.
pub(crate) fn ago(seconds: i64) -> String {
    let micros = Timestamp::now().as_unix_micros() - seconds * 1_000_000;
    Timestamp::from_unix_micros(micros - micros.rem_euclid(1_000_000)).to_string()
}

/// The current month, `YYYY-MM`.
pub(crate) fn month() -> String {
    now().chars().take(7).collect()
}

#[cfg(test)]
mod tests;
