//! `settings.json`: defaults of new projects and clusterings, budgets, the free suggestion
//! endpoints, the attended browser mode and the network.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use octopuson_keywords::{Level, Mode};
use serde::{Deserialize, Serialize};

use crate::KwAppError;

/// Most suggestion requests per engine and day; the setting can only lower it.
pub const SUGGEST_DAILY_LIMIT: u32 = 2_000;
/// Shortest pause between suggestion requests, milliseconds.
pub const SUGGEST_MIN_PAUSE_MS: u64 = 2_000;
/// Shortest pause between two result pages in the browser, seconds.
pub const BROWSER_MIN_DELAY_SECS: u64 = 10;

/// How keywords are grouped.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Method {
    /// By wording: free and instant.
    Lexical,
    /// By shared results of their result pages.
    Serp,
    /// Result pages where fetched, wording elsewhere.
    #[default]
    Hybrid,
}

impl Method {
    /// Stored name.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Lexical => "lexical",
            Self::Serp => "serp",
            Self::Hybrid => "hybrid",
        }
    }
}

/// Market of new projects.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ProjectDefaults {
    /// Country code.
    pub country: String,
    /// Language code.
    pub language: String,
    /// Engines, primary first.
    pub engines: Vec<String>,
}

impl Default for ProjectDefaults {
    fn default() -> Self {
        Self {
            country: "tr".to_owned(),
            language: "tr".to_owned(),
            engines: vec!["google".to_owned()],
        }
    }
}

/// Defaults of a clustering.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ClusteringDefaults {
    /// Method.
    pub method: Method,
    /// Least wording similarity (0–1).
    pub threshold: f64,
    /// Least shared results (1–9).
    pub serp_threshold: u16,
    /// Soft or hard.
    pub mode: Mode,
    /// Pages or sites.
    pub level: Level,
}

impl Default for ClusteringDefaults {
    fn default() -> Self {
        Self {
            method: Method::Hybrid,
            threshold: 0.6,
            serp_threshold: 3,
            mode: Mode::Soft,
            level: Level::Page,
        }
    }
}

/// The free Google and Yandex suggestion endpoints: off until the user switches them on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct SuggestSettings {
    /// Google suggestions switched on.
    pub google: bool,
    /// Yandex suggestions switched on.
    pub yandex: bool,
    /// When the user accepted the terms warning (RFC 3339).
    pub accepted_at: Option<String>,
    /// Pause between requests, milliseconds (at least [`SUGGEST_MIN_PAUSE_MS`]).
    pub pause_ms: u64,
    /// Requests per engine and day (at most [`SUGGEST_DAILY_LIMIT`]).
    pub daily_limit: u32,
}

impl Default for SuggestSettings {
    fn default() -> Self {
        Self {
            google: false,
            yandex: false,
            accepted_at: None,
            pause_ms: SUGGEST_MIN_PAUSE_MS,
            daily_limit: SUGGEST_DAILY_LIMIT,
        }
    }
}

/// The attended browser mode: result pages read in a visible Chrome or Edge window of the
/// program's own, one at a time. It is a result-page source only when the user adds
/// `browser` to an engine's chain.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct BrowserSettings {
    /// Chrome or Edge executable; found automatically when absent.
    pub executable: Option<String>,
    /// Pause between two result pages, seconds (at least [`BROWSER_MIN_DELAY_SECS`]).
    pub delay_secs: u64,
    /// Most result pages per engine and day.
    pub daily_cap: u32,
    /// One proxy of the user's for the window (`http://`, `https://` or `socks5://`); the
    /// network proxy is not used by the window.
    pub proxy: Option<String>,
}

impl Default for BrowserSettings {
    fn default() -> Self {
        Self {
            executable: None,
            delay_secs: 30,
            daily_cap: 100,
            proxy: None,
        }
    }
}

/// Network settings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct NetworkSettings {
    /// One proxy of the user's (`http://`, `https://` or `socks5://`).
    pub proxy: Option<String>,
    /// Request timeout, seconds.
    pub timeout_secs: u64,
}

impl Default for NetworkSettings {
    fn default() -> Self {
        Self {
            proxy: None,
            timeout_secs: 30,
        }
    }
}

/// Result pages for clustering.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct SerpSettings {
    /// Providers asked in order, per engine.
    pub chains: BTreeMap<String, Vec<String>>,
    /// Stored result pages younger than this many days are reused.
    pub max_age_days: u32,
}

impl Default for SerpSettings {
    fn default() -> Self {
        let chain = |ids: &[&str]| ids.iter().map(|id| (*id).to_owned()).collect::<Vec<_>>();
        Self {
            chains: BTreeMap::from([
                (
                    "google".to_owned(),
                    chain(&["dataforseo", "serper", "serpapi"]),
                ),
                (
                    "yandex".to_owned(),
                    chain(&["yandex-cloud", "dataforseo", "serpapi"]),
                ),
                ("bing".to_owned(), chain(&["dataforseo", "serpapi"])),
            ]),
            max_age_days: 30,
        }
    }
}

/// The user's Google connection (the secrets live in the keychain).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct GoogleSettings {
    /// Client id of the user's own "Desktop app" OAuth client.
    pub client_id: Option<String>,
    /// Scopes granted at the last sign-in.
    pub scopes: Vec<String>,
    /// Google Ads customer id (ten digits).
    pub ads_customer_id: Option<String>,
    /// Manager account the customer is reached through, if any.
    pub ads_login_customer_id: Option<String>,
}

/// Everything in `settings.json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// File version.
    pub version: u32,
    /// Market of new projects.
    pub projects: ProjectDefaults,
    /// Defaults of a clustering.
    pub clustering: ClusteringDefaults,
    /// Monthly budget per provider, US dollars.
    pub budgets: BTreeMap<String, f64>,
    /// Runs estimated above this cost ask first, US dollars.
    pub confirm_above_usd: f64,
    /// Providers switched off.
    pub disabled: BTreeSet<String>,
    /// How each paid source is paid: own key, OctopusOn SEO credits, or `auto` (missing).
    pub credit: BTreeMap<String, octopuson_cloud::SourceMode>,
    /// Wordstat requests per hour (the Yandex Search API's default quota is 100).
    pub wordstat_hourly_quota: u32,
    /// The free suggestion endpoints.
    pub suggest: SuggestSettings,
    /// The Google connection.
    pub google: GoogleSettings,
    /// Result pages for clustering.
    pub serp: SerpSettings,
    /// The attended browser mode.
    pub browser: BrowserSettings,
    /// Network.
    pub network: NetworkSettings,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            version: 1,
            projects: ProjectDefaults::default(),
            clustering: ClusteringDefaults::default(),
            budgets: BTreeMap::new(),
            confirm_above_usd: 1.0,
            disabled: BTreeSet::new(),
            credit: BTreeMap::new(),
            wordstat_hourly_quota: 100,
            suggest: SuggestSettings::default(),
            google: GoogleSettings::default(),
            serp: SerpSettings::default(),
            browser: BrowserSettings::default(),
            network: NetworkSettings::default(),
        }
    }
}

impl Settings {
    /// Reads `path`; defaults when it does not exist.
    ///
    /// # Errors
    /// [`KwAppError::InvalidFile`] for invalid JSON or values; I/O errors.
    pub fn load(path: &Path) -> Result<Self, KwAppError> {
        match fs::read_to_string(path) {
            Ok(text) => {
                let settings: Self = serde_json::from_str(&text)
                    .map_err(|error| KwAppError::invalid_file(path, error))?;
                settings
                    .validate()
                    .map_err(|error| KwAppError::invalid_file(path, error))?;
                Ok(settings)
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(source) => Err(KwAppError::Io {
                path: path.to_owned(),
                source,
            }),
        }
    }

    /// Writes `path` atomically.
    ///
    /// # Errors
    /// [`KwAppError::InvalidSettings`] for invalid values; I/O errors.
    pub fn save(&self, path: &Path) -> Result<(), KwAppError> {
        self.validate().map_err(KwAppError::InvalidSettings)?;
        let text = serde_json::to_string_pretty(self)
            .map_err(|error| KwAppError::InvalidSettings(error.to_string()))?;
        octopuson_secrets::write_atomically(path, text.as_bytes())
            .map_err(|error| KwAppError::Secrets(error.to_string()))
    }

    /// Checks value ranges.
    ///
    /// # Errors
    /// What is out of range.
    pub fn validate(&self) -> Result<(), String> {
        let clustering = &self.clustering;
        if !(clustering.threshold > 0.0 && clustering.threshold <= 1.0) {
            return Err("clustering.threshold must be above 0 and at most 1".to_owned());
        }
        if !(1..=9).contains(&clustering.serp_threshold) {
            return Err("clustering.serp_threshold must be 1–9".to_owned());
        }
        if let Some((provider, _)) = self
            .budgets
            .iter()
            .find(|(_, budget)| !budget.is_finite() || **budget < 0.0)
        {
            return Err(format!("budgets.{provider} must be 0 or more"));
        }
        if !self.confirm_above_usd.is_finite() || self.confirm_above_usd < 0.0 {
            return Err("confirm_above_usd must be 0 or more".to_owned());
        }
        if !(1..=100_000).contains(&self.wordstat_hourly_quota) {
            return Err("wordstat_hourly_quota must be 1–100000".to_owned());
        }
        if self.suggest.pause_ms < SUGGEST_MIN_PAUSE_MS {
            return Err(format!(
                "suggest.pause_ms must be at least {SUGGEST_MIN_PAUSE_MS}"
            ));
        }
        if !(1..=SUGGEST_DAILY_LIMIT).contains(&self.suggest.daily_limit) {
            return Err(format!(
                "suggest.daily_limit must be 1–{SUGGEST_DAILY_LIMIT}"
            ));
        }
        if (self.suggest.google || self.suggest.yandex) && self.suggest.accepted_at.is_none() {
            return Err(
                "the suggestion endpoints need the terms warning accepted first".to_owned(),
            );
        }
        if let Some(id) = &self.google.client_id
            && !id.trim().ends_with(".apps.googleusercontent.com")
        {
            return Err("google.client_id must end with .apps.googleusercontent.com".to_owned());
        }
        for id in [
            &self.google.ads_customer_id,
            &self.google.ads_login_customer_id,
        ]
        .into_iter()
        .flatten()
        {
            if octopuson_keyword_sources::google_ads::GoogleAds::customer(id).is_none() {
                return Err(format!("`{id}` is not a ten-digit Google Ads customer id"));
            }
        }
        if !(1..=365).contains(&self.serp.max_age_days) {
            return Err("serp.max_age_days must be 1–365".to_owned());
        }
        if self.browser.delay_secs < BROWSER_MIN_DELAY_SECS {
            return Err(format!(
                "browser.delay_secs must be at least {BROWSER_MIN_DELAY_SECS}"
            ));
        }
        if self.browser.daily_cap == 0 {
            return Err("browser.daily_cap must be at least 1".to_owned());
        }
        if self
            .browser
            .executable
            .as_deref()
            .is_some_and(|path| path.trim().is_empty())
        {
            return Err(
                "browser.executable must not be empty (leave it out to find Chrome or Edge)"
                    .to_owned(),
            );
        }
        if !(5..=300).contains(&self.network.timeout_secs) {
            return Err("network.timeout_secs must be 5–300".to_owned());
        }
        check_proxy("network.proxy", self.network.proxy.as_deref())?;
        check_proxy("browser.proxy", self.browser.proxy.as_deref())?;
        crate::projects::check_market(
            &self.projects.country,
            &self.projects.language,
            &self.projects.engines,
        )
        .map(|_| ())
    }
}

/// A single proxy address: `http://`, `https://` or `socks5://` and a host.
fn check_proxy(name: &str, proxy: Option<&str>) -> Result<(), String> {
    let Some(proxy) = proxy else {
        return Ok(());
    };
    let valid = ["http://", "https://", "socks5://"]
        .iter()
        .any(|scheme| proxy.starts_with(scheme) && proxy.len() > scheme.len());
    if valid {
        Ok(())
    } else {
        Err(format!(
            "{name} must start with http://, https:// or socks5://"
        ))
    }
}
