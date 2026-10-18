//! `settings.json`: defaults, provider chains, budgets and network settings.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use octopuson_serp::{Device, Engine, RESULTS_PER_PAGE};
use serde::{Deserialize, Serialize};

use crate::SerpAppError;

/// Defaults of new searches.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueryDefaults {
    /// Country code.
    pub country: String,
    /// Language code.
    pub language: String,
    /// Device.
    pub device: Device,
    /// Depth.
    pub depth: u16,
}

impl Default for QueryDefaults {
    fn default() -> Self {
        Self {
            country: "tr".to_owned(),
            language: "tr".to_owned(),
            device: Device::Desktop,
            depth: RESULTS_PER_PAGE,
        }
    }
}

/// Settings of the attended browser mode.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct BrowserSettings {
    /// Seconds between two result pages (at least 10).
    pub delay_secs: u64,
    /// Most result pages per engine and day.
    pub daily_cap: u32,
    /// Chrome or Edge executable; found automatically when empty.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub executable: Option<String>,
}

impl Default for BrowserSettings {
    fn default() -> Self {
        Self {
            delay_secs: 30,
            daily_cap: 100,
            executable: None,
        }
    }
}

/// Everything in `settings.json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Defaults of new searches.
    pub defaults: QueryDefaults,
    /// Providers tried in order, per engine.
    pub chains: BTreeMap<Engine, Vec<String>>,
    /// Providers switched off.
    #[serde(skip_serializing_if = "BTreeSet::is_empty")]
    pub disabled: BTreeSet<String>,
    /// Monthly budgets in US dollars per provider; a provider over its budget is skipped.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub budgets: BTreeMap<String, f64>,
    /// Estimated cost of one action (US dollars) above which the user is asked first.
    pub confirm_above_usd: f64,
    /// HTTP(S) proxy for API requests (`http://host:port`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proxy: Option<String>,
    /// Seconds before an API request is given up.
    pub timeout_secs: u64,
    /// Browser mode.
    pub browser: BrowserSettings,
    /// How each paid source is paid: own key, OctopusOn SEO credits, or `auto` (missing).
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub credit: BTreeMap<String, octopuson_cloud::SourceMode>,
}

impl Default for Settings {
    fn default() -> Self {
        let chain = |ids: &[&str]| ids.iter().map(|id| (*id).to_owned()).collect::<Vec<_>>();
        Self {
            defaults: QueryDefaults::default(),
            chains: BTreeMap::from([
                (
                    Engine::Google,
                    chain(&["serper", "dataforseo", "serpapi", "browser"]),
                ),
                (Engine::Bing, chain(&["dataforseo", "serpapi", "browser"])),
                (
                    Engine::Yandex,
                    chain(&["yandex-cloud", "dataforseo", "serpapi", "browser"]),
                ),
            ]),
            disabled: BTreeSet::new(),
            budgets: BTreeMap::new(),
            confirm_above_usd: 1.0,
            proxy: None,
            timeout_secs: 60,
            browser: BrowserSettings::default(),
            credit: BTreeMap::new(),
        }
    }
}

impl Settings {
    /// Reads `path`, or the defaults when it does not exist.
    ///
    /// # Errors
    /// [`SerpAppError::InvalidFile`] for unreadable or invalid JSON.
    pub fn load(path: &Path) -> Result<Self, SerpAppError> {
        match fs::read_to_string(path) {
            Ok(text) => {
                let settings: Self = serde_json::from_str(&text)
                    .map_err(|error| SerpAppError::invalid_file(path, error))?;
                settings
                    .validate()
                    .map_err(|error| SerpAppError::invalid_file(path, error))?;
                Ok(settings)
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(source) => Err(SerpAppError::Io {
                path: path.to_owned(),
                source,
            }),
        }
    }

    /// Writes `path` atomically.
    ///
    /// # Errors
    /// [`SerpAppError::InvalidSettings`] for invalid values; I/O errors.
    pub fn save(&self, path: &Path) -> Result<(), SerpAppError> {
        self.validate().map_err(SerpAppError::InvalidSettings)?;
        let text = serde_json::to_string_pretty(self)
            .map_err(|error| SerpAppError::InvalidSettings(error.to_string()))?;
        crate::files::write_atomically(path, text.as_bytes())
    }

    /// Checks value ranges.
    ///
    /// # Errors
    /// A message naming the first invalid value.
    pub fn validate(&self) -> Result<(), String> {
        if self.browser.delay_secs < 10 {
            return Err("browser.delay_secs must be at least 10".to_owned());
        }
        if self.browser.daily_cap == 0 {
            return Err("browser.daily_cap must be at least 1".to_owned());
        }
        if !(5..=600).contains(&self.timeout_secs) {
            return Err("timeout_secs must be between 5 and 600".to_owned());
        }
        if !self.confirm_above_usd.is_finite() || self.confirm_above_usd < 0.0 {
            return Err("confirm_above_usd must be zero or more".to_owned());
        }
        if let Some((provider, _)) = self
            .budgets
            .iter()
            .find(|(_, budget)| !budget.is_finite() || **budget < 0.0)
        {
            return Err(format!("the budget of `{provider}` must be zero or more"));
        }
        Ok(())
    }

    /// Providers of an engine's chain that are switched on.
    pub fn chain(&self, engine: Engine) -> Vec<String> {
        self.chains
            .get(&engine)
            .into_iter()
            .flatten()
            .filter(|id| !self.disabled.contains(*id))
            .cloned()
            .collect()
    }
}
