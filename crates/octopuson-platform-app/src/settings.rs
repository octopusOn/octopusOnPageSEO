//! Settings of a workspace: how each paid source is paid, monthly budgets, the proxy.

use std::collections::BTreeMap;
use std::path::Path;

use octopuson_cloud::SourceMode;
use serde::{Deserialize, Serialize};

use crate::{AppError, Host, Workspace};

/// File name in the data folder.
const FILE: &str = "settings.json";

/// What the user set.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Settings {
    /// Own key or credits, per source id; `auto` when missing.
    #[serde(default)]
    pub modes: BTreeMap<String, SourceMode>,
    /// Most US dollars a month per source id; no limit when missing.
    #[serde(default)]
    pub budgets: BTreeMap<String, f64>,
    /// HTTP(S) proxy for provider requests (desktop only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proxy: Option<String>,
    /// Always ask providers anew on credits: no discounted answers the OctopusOn SEO gateway
    /// keeps for every account (ADR 0079).
    #[serde(default)]
    pub fresh_only: bool,
}

impl Settings {
    /// The mode of a source.
    pub fn mode(&self, source: &str) -> SourceMode {
        self.modes.get(source).copied().unwrap_or_default()
    }

    /// The settings cleaned for `host`.
    ///
    /// # Errors
    /// [`AppError::Invalid`] for a negative budget, or a proxy on the web, where requests leave
    /// the server and must not be sent elsewhere.
    pub fn checked(mut self, host: Host) -> Result<Self, AppError> {
        if self
            .budgets
            .values()
            .any(|budget| !budget.is_finite() || *budget < 0.0)
        {
            return Err(AppError::Invalid(
                "budgets: a budget is a positive amount of US dollars".to_owned(),
            ));
        }
        self.proxy = self
            .proxy
            .map(|proxy| proxy.trim().to_owned())
            .filter(|proxy| !proxy.is_empty());
        if host == Host::Web && self.proxy.is_some() {
            return Err(AppError::Invalid(
                "proxy: the web version sends requests from its own server".to_owned(),
            ));
        }
        Ok(self)
    }

    fn load(path: &Path) -> Result<Self, AppError> {
        match std::fs::read(path) {
            Ok(bytes) => serde_json::from_slice(&bytes)
                .map_err(|error| AppError::Internal(format!("{}: {error}", path.display()))),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(error) => Err(AppError::Internal(format!("{}: {error}", path.display()))),
        }
    }
}

impl Workspace {
    /// Current settings.
    ///
    /// # Errors
    /// [`AppError::Internal`] for an unreadable file.
    pub fn settings(&self) -> Result<Settings, AppError> {
        Settings::load(&self.data_dir.join(FILE))
    }

    /// Saves settings after checking them.
    ///
    /// # Errors
    /// [`AppError::Invalid`] for unusable values, [`AppError::Internal`] when the file cannot
    /// be written.
    pub fn save_settings(&self, settings: Settings) -> Result<Settings, AppError> {
        let settings = settings.checked(self.host)?;
        let bytes = serde_json::to_vec_pretty(&settings)
            .map_err(|error| AppError::Internal(error.to_string()))?;
        octopuson_secrets::write_atomically(&self.data_dir.join(FILE), &bytes)?;
        Ok(settings)
    }

    /// Changes the settings with `change` and saves them.
    ///
    /// # Errors
    /// As [`Self::save_settings`].
    pub fn update_settings(
        &self,
        change: impl FnOnce(&mut Settings),
    ) -> Result<Settings, AppError> {
        let mut settings = self.settings()?;
        change(&mut settings);
        self.save_settings(settings)
    }
}
