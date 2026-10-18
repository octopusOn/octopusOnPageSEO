//! Use cases of the OctopusOn SERP Checker.
//!
//! A [`SerpWorkspace`] is one data directory: `settings.json` (defaults, provider chains per
//! engine, budgets, proxy), `providers.json` (the user's own provider specs), the secrets of
//! the providers, and the store of runs and snapshots. [`SerpWorkspace::search`] runs a query
//! through the engine's provider chain, skipping providers that are switched off, not set up
//! or over their monthly budget, stores what came back and every attempt with its cost.

pub mod analysis;
pub mod browser;
pub mod cloud;
pub mod crawl;
pub mod domains;
pub mod export;
mod files;
pub mod providers;
pub mod rank;
pub mod reparse;
mod search;
pub mod secrets;
pub mod settings;

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use seonexus_serp::SerpSnapshot;
use seonexus_serp::provider::Attempt;
use seonexus_serp::provider::ProviderError;
use seonexus_serp::spec::ProviderSpec;
use seonexus_serp_store::{
    ProviderUsage, SerpStore, SerpStoreError, SnapshotFilter, SnapshotSummary,
};

pub use search::{ProviderEstimate, SearchOutcome, SearchRequest};
pub use seonexus_cloud as cloud_api;
pub use seonexus_serp_store as store;
pub use seonexus_spider_app::CrawlControl;

use crate::providers::{
    ProviderInfo, ProviderKind, describe, load_custom, reserved_ids, save_custom,
};
use crate::secrets::{Secrets, default_backend};
use crate::settings::Settings;

const QUALIFIER: &str = "com";
const ORGANIZATION: &str = "octopusSEO";
const APPLICATION: &str = "octopusSEO SERP Finder";
/// How long [`SerpWorkspace::store`] waits for a store in use.
const STORE_WAIT: Duration = Duration::from_secs(5);

/// Errors of the SERP Checker.
#[derive(Debug, thiserror::Error)]
pub enum SerpAppError {
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
    /// A settings or provider file is invalid.
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
    /// A provider spec is invalid or unknown.
    #[error("{0}")]
    InvalidProvider(String),
    /// The query is invalid.
    #[error("{0}")]
    InvalidQuery(String),
    /// The secret store failed.
    #[error("secret store: {0}")]
    Secrets(String),
    /// No provider returned results.
    #[error("{error}")]
    Search {
        /// The error that ended the chain.
        error: ProviderError,
        /// Every attempt.
        attempts: Vec<Attempt>,
        /// Where an unreadable page was saved for fixing the parser selectors.
        diagnostic: Option<PathBuf>,
    },
    /// The store failed.
    #[error(transparent)]
    Store(#[from] SerpStoreError),
    /// Crawling result pages failed.
    #[error("crawl failed: {0}")]
    Crawl(String),
    /// The OctopusOn SEO account cannot be used.
    #[error("{0}")]
    Cloud(String),
}

impl From<secrets::SecretError> for SerpAppError {
    fn from(error: secrets::SecretError) -> Self {
        match error {
            secrets::SecretError::Io { path, source } => Self::Io { path, source },
            secrets::SecretError::InvalidFile { path, message } => {
                Self::InvalidFile { path, message }
            }
            secrets::SecretError::Store(message) => Self::Secrets(message),
        }
    }
}

impl SerpAppError {
    pub(crate) fn invalid_file(path: &Path, error: impl std::fmt::Display) -> Self {
        Self::InvalidFile {
            path: path.to_owned(),
            message: error.to_string(),
        }
    }
}

/// One data directory of the SERP Checker.
pub struct SerpWorkspace {
    data_dir: PathBuf,
    secrets: Secrets,
    browser: std::sync::Mutex<browser::BrowserState>,
    cloud: cloud::CloudState,
}

impl std::fmt::Debug for SerpWorkspace {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SerpWorkspace")
            .field("data_dir", &self.data_dir)
            .field("secrets", &self.secrets)
            .finish_non_exhaustive()
    }
}

impl SerpWorkspace {
    /// The workspace in `data_dir`, or in the platform's application data directory.
    ///
    /// # Errors
    /// [`SerpAppError::NoDataDir`] when the platform reports none.
    pub fn resolve(data_dir: Option<PathBuf>) -> Result<Self, SerpAppError> {
        let data_dir = match data_dir {
            Some(dir) => dir,
            None => directories::ProjectDirs::from(QUALIFIER, ORGANIZATION, APPLICATION)
                .ok_or(SerpAppError::NoDataDir)?
                .data_dir()
                .to_owned(),
        };
        let secrets = Secrets::new(
            default_backend(&data_dir),
            data_dir.join("secret-bindings.json"),
        );
        let api =
            seonexus_cloud::api_base(std::env::var(seonexus_cloud::API_VARIABLE).ok().as_deref())
                .map_err(|error| SerpAppError::InvalidSettings(error.to_string()))?;
        // One account file for every OctopusOn SEO program: connecting once connects them all.
        let tokens = seonexus_cloud::TokenStore::shared(api)
            .map_err(|error| SerpAppError::InvalidSettings(error.to_string()))?;
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
            browser: std::sync::Mutex::new(browser::BrowserState::default()),
            cloud: cloud::CloudState::new(tokens),
        }
    }

    /// Keeps the OctopusOn SEO account token in `tokens`.
    #[must_use]
    pub fn with_tokens(mut self, tokens: seonexus_cloud::TokenStore) -> Self {
        self.cloud = cloud::CloudState::new(tokens);
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

    fn providers_path(&self) -> PathBuf {
        self.data_dir.join("providers.json")
    }

    /// Current settings.
    ///
    /// # Errors
    /// [`SerpAppError`] for an unreadable or invalid file.
    pub fn settings(&self) -> Result<Settings, SerpAppError> {
        Settings::load(&self.settings_path())
    }

    /// Saves settings.
    ///
    /// # Errors
    /// [`SerpAppError`] for invalid values or I/O errors.
    pub fn save_settings(&self, settings: &Settings) -> Result<(), SerpAppError> {
        settings.save(&self.settings_path())
    }

    /// Opens the store.
    ///
    /// Work holds the store for moments only (searches and lookups release it while they
    /// wait for the network), so a store in use is waited for briefly before giving up.
    ///
    /// # Errors
    /// [`SerpAppError::Store`], e.g. while another program keeps using it.
    pub fn store(&self) -> Result<SerpStore, SerpAppError> {
        let deadline = Instant::now() + STORE_WAIT;
        loop {
            match SerpStore::open(&self.data_dir) {
                Err(SerpStoreError::Busy(_)) if Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(20));
                }
                other => return Ok(other?),
            }
        }
    }

    /// Every provider, built-in and custom, with its state.
    ///
    /// # Errors
    /// [`SerpAppError`] for unreadable files.
    pub fn providers(&self) -> Result<Vec<ProviderInfo>, SerpAppError> {
        let settings = self.settings()?;
        self.warm_catalog(&settings)?;
        let (entries, invalid) = providers::catalog(load_custom(&self.providers_path())?);
        let mut infos = entries
            .iter()
            .map(|entry| {
                let mut info = describe(entry, &self.secrets, &settings)?;
                info.via = self.entry_via(&settings, entry)?;
                info.ready = info.via.is_some();
                info.mode = self.source_mode(&settings, entry.provider.id());
                info.credit = self.credit_offer(entry);
                Ok(info)
            })
            .collect::<Result<Vec<_>, SerpAppError>>()?;
        infos.push(Self::browser_info(&settings));
        infos.extend(invalid.into_iter().map(|(spec, error)| ProviderInfo {
            id: spec.id.clone(),
            name: spec.name.clone(),
            kind: ProviderKind::Custom,
            engines: spec.engines.clone(),
            devices: spec.devices.clone(),
            secrets: Vec::new(),
            origins: Vec::new(),
            ready: false,
            enabled: !settings.disabled.contains(&spec.id),
            per_request_usd: spec.cost.map_or(0.0, |cost| cost.per_request_usd),
            budget_usd: settings.budgets.get(&spec.id).copied(),
            notes: spec.notes.clone(),
            error: Some(error),
            via: None,
            mode: seonexus_cloud::SourceMode::Auto,
            credit: None,
        }));
        Ok(infos)
    }

    /// The browser provider as listed.
    fn browser_info(settings: &Settings) -> ProviderInfo {
        let found = seonexus_render::find_browser(settings.browser.executable.as_deref());
        ProviderInfo {
            id: providers::BROWSER.to_owned(),
            name: "Browser (attended)".to_owned(),
            kind: ProviderKind::Browser,
            engines: seonexus_serp::Engine::ALL.to_vec(),
            devices: vec![seonexus_serp::Device::Desktop],
            secrets: Vec::new(),
            origins: Vec::new(),
            ready: found.is_some(),
            enabled: !settings.disabled.contains(providers::BROWSER),
            per_request_usd: 0.0,
            budget_usd: None,
            notes: Some(format!(
                "Your Chrome or Edge in a visible window, one page every {} s, at most {} pages per engine and day; you solve consent pages and CAPTCHAs yourself.{}",
                settings.browser.delay_secs,
                settings.browser.daily_cap,
                found.map_or_else(
                    || " No browser was found.".to_owned(),
                    |path| format!(" ({})", path.display())
                )
            )),
            error: None,
            via: None,
            mode: seonexus_cloud::SourceMode::Auto,
            credit: None,
        }
    }

    /// The user's provider specs.
    ///
    /// # Errors
    /// [`SerpAppError`] for an unreadable file.
    pub fn custom_specs(&self) -> Result<Vec<ProviderSpec>, SerpAppError> {
        load_custom(&self.providers_path())
    }

    /// Adds or replaces a user spec after validating it.
    ///
    /// # Errors
    /// [`SerpAppError::InvalidProvider`] for an invalid spec or a reserved id.
    pub fn save_spec(&self, spec: ProviderSpec) -> Result<(), SerpAppError> {
        if reserved_ids().contains(&spec.id) {
            return Err(SerpAppError::InvalidProvider(format!(
                "`{}` is the id of a built-in provider",
                spec.id
            )));
        }
        spec.compile()
            .map_err(|error| SerpAppError::InvalidProvider(error.to_string()))?;
        let mut specs = self.custom_specs()?;
        specs.retain(|existing| existing.id != spec.id);
        specs.push(spec);
        specs.sort_by(|a, b| a.id.cmp(&b.id));
        save_custom(&self.providers_path(), &specs)
    }

    /// Imports specs from JSON (`{"providers": [...]}`, an array or one spec). Returns the ids
    /// imported; nothing is saved when one is invalid.
    ///
    /// # Errors
    /// [`SerpAppError::InvalidProvider`] naming the first invalid spec.
    pub fn import_specs(&self, json: &str) -> Result<Vec<String>, SerpAppError> {
        let specs = providers::parse_specs(json).map_err(SerpAppError::InvalidProvider)?;
        let reserved = reserved_ids();
        for spec in &specs {
            if reserved.contains(&spec.id) {
                return Err(SerpAppError::InvalidProvider(format!(
                    "`{}` is the id of a built-in provider",
                    spec.id
                )));
            }
            spec.compile().map_err(|error| {
                SerpAppError::InvalidProvider(format!("`{}`: {error}", spec.id))
            })?;
        }
        let ids: Vec<String> = specs.iter().map(|spec| spec.id.clone()).collect();
        let mut existing = self.custom_specs()?;
        existing.retain(|spec| !ids.contains(&spec.id));
        existing.extend(specs);
        existing.sort_by(|a, b| a.id.cmp(&b.id));
        save_custom(&self.providers_path(), &existing)?;
        Ok(ids)
    }

    /// The user's specs as JSON for sharing; secrets are never part of a spec.
    ///
    /// # Errors
    /// [`SerpAppError`] for an unreadable file or unknown ids.
    pub fn export_specs(&self, ids: &[String]) -> Result<String, SerpAppError> {
        let specs: Vec<ProviderSpec> = self
            .custom_specs()?
            .into_iter()
            .filter(|spec| ids.is_empty() || ids.contains(&spec.id))
            .collect();
        if let Some(missing) = ids
            .iter()
            .find(|id| !specs.iter().any(|spec| spec.id == **id))
        {
            return Err(SerpAppError::InvalidProvider(format!(
                "no custom provider `{missing}`"
            )));
        }
        serde_json::to_string_pretty(&serde_json::json!({ "version": 1, "providers": specs }))
            .map_err(|error| SerpAppError::InvalidProvider(error.to_string()))
    }

    /// Removes a user spec and its secrets.
    ///
    /// # Errors
    /// [`SerpAppError::InvalidProvider`] for an unknown id.
    pub fn remove_spec(&self, id: &str) -> Result<(), SerpAppError> {
        let mut specs = self.custom_specs()?;
        let Some(index) = specs.iter().position(|spec| spec.id == id) else {
            return Err(SerpAppError::InvalidProvider(format!(
                "no custom provider `{id}`"
            )));
        };
        let spec = specs.remove(index);
        for secret in &spec.secrets {
            self.secrets.delete(id, &secret.name)?;
        }
        save_custom(&self.providers_path(), &specs)
    }

    /// Saves a provider secret, bound to the provider's current origins.
    ///
    /// # Errors
    /// [`SerpAppError::InvalidProvider`] for an unknown provider or secret name; store errors.
    pub fn set_secret(&self, provider: &str, name: &str, value: &str) -> Result<(), SerpAppError> {
        if let Some((_, secret, origin)) = domains::DATA_SOURCES
            .iter()
            .find(|(source, _, _)| *source == provider)
        {
            if *secret != name {
                return Err(SerpAppError::InvalidProvider(format!(
                    "`{provider}` has no secret `{name}` (it has: {secret})"
                )));
            }
            return Ok(self.secrets.set(provider, name, value, &[origin])?);
        }
        let (entries, _) = providers::catalog(self.custom_specs()?);
        let entry = entries
            .iter()
            .find(|entry| entry.provider.id() == provider)
            .ok_or_else(|| SerpAppError::InvalidProvider(format!("no provider `{provider}`")))?;
        if !entry.secrets.iter().any(|secret| secret.name == name) {
            let names: Vec<&str> = entry
                .secrets
                .iter()
                .map(|secret| secret.name.as_str())
                .collect();
            return Err(SerpAppError::InvalidProvider(format!(
                "`{provider}` has no secret `{name}` (it has: {})",
                names.join(", ")
            )));
        }
        Ok(self
            .secrets
            .set(provider, name, value, &entry.origin_refs())?)
    }

    /// Removes a provider secret.
    ///
    /// # Errors
    /// Store errors.
    pub fn remove_secret(&self, provider: &str, name: &str) -> Result<(), SerpAppError> {
        Ok(self.secrets.delete(provider, name)?)
    }

    /// Stored snapshots, newest first.
    ///
    /// # Errors
    /// Store errors.
    pub fn snapshots(&self, filter: &SnapshotFilter) -> Result<Vec<SnapshotSummary>, SerpAppError> {
        Ok(self.store()?.snapshots(filter)?)
    }

    /// A stored snapshot.
    ///
    /// # Errors
    /// Store errors, [`SerpStoreError::NotFound`] for an unknown id.
    pub fn snapshot(&self, id: i64) -> Result<SerpSnapshot, SerpAppError> {
        Ok(self.store()?.snapshot(id)?)
    }

    /// Deletes a stored snapshot.
    ///
    /// # Errors
    /// Store errors.
    pub fn delete_snapshot(&self, id: i64) -> Result<(), SerpAppError> {
        Ok(self.store()?.delete_snapshot(id)?)
    }

    /// Requests and spending per provider and month.
    ///
    /// # Errors
    /// Store errors.
    pub fn usage(&self) -> Result<Vec<ProviderUsage>, SerpAppError> {
        Ok(self.store()?.usage()?)
    }
}

#[cfg(test)]
mod tests;
