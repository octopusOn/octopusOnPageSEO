//! The browser window of attended searches and its parser profiles.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use octopuson_render::{BrowserOptions, BrowserSession};
use octopuson_serp::mapping::CompiledMapping;
use octopuson_serp::profile::{ProfileOverride, apply_override, builtin};
use octopuson_serp::provider::{
    BrowserPacing, BrowserPort, BrowserProvider, BrowserView, Capabilities, CostEstimate,
    ProviderError, SearchContext, SerpFetch, SerpProvider, SystemClock,
};
use octopuson_serp::{Engine, SerpQuery};
use serde::{Deserialize, Serialize};

use crate::files::write_atomically;
use crate::settings::Settings;
use crate::{SerpAppError, SerpWorkspace};

/// Time a result page may take to load.
const LOAD_TIMEOUT: Duration = Duration::from_secs(45);
/// Time after loading for scripts to finish drawing the page.
const SETTLE: Duration = Duration::from_millis(1500);

/// A parser profile as used, with what changed it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParserInfo {
    /// Engine.
    pub engine: Engine,
    /// Key recorded with snapshots (`google-html@1`, `google-html@1+local`).
    pub key: String,
    /// Whether a user override is applied.
    pub patched: bool,
    /// The override was written for another revision of the built-in profile.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stale_base: Option<String>,
    /// Why the override is not applied.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// The effective mapping as JSON.
    pub mapping: serde_json::Value,
    /// Key of the built-in profile (`google-html@1`), which overrides are written against.
    pub base: String,
    /// The user's override (a JSON merge patch of the mapping), applied or not.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub patch: Option<serde_json::Value>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct OverridesFile {
    version: u32,
    overrides: Vec<ProfileOverride>,
}

fn overrides_path(data_dir: &Path) -> PathBuf {
    data_dir.join("parser-overrides.json")
}

fn load_overrides(path: &Path) -> Result<Vec<ProfileOverride>, SerpAppError> {
    match fs::read_to_string(path) {
        Ok(text) => serde_json::from_str::<OverridesFile>(&text)
            .map(|file| file.overrides)
            .map_err(|error| SerpAppError::invalid_file(path, error)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(source) => Err(SerpAppError::Io {
            path: path.to_owned(),
            source,
        }),
    }
}

/// The browser window, opened on first use and reopened if the person closed it.
pub(crate) struct BrowserWindow {
    options: BrowserOptions,
    session: Option<BrowserSession>,
}

impl BrowserWindow {
    fn session(&mut self) -> Result<&mut BrowserSession, String> {
        if self.session.is_none() {
            self.session =
                Some(BrowserSession::launch(&self.options).map_err(|error| error.to_string())?);
        }
        self.session
            .as_mut()
            .ok_or_else(|| "the browser window is not open".to_owned())
    }
}

/// One window shared by every provider built for this workspace.
#[derive(Clone)]
struct SharedWindow(Arc<Mutex<BrowserWindow>>);

impl BrowserPort for SharedWindow {
    fn open(&mut self, url: &str) -> Result<BrowserView, String> {
        let mut window = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        let first = window.session()?.navigate(url, LOAD_TIMEOUT, SETTLE);
        let page = if let Ok(page) = first {
            page
        } else {
            // The person may have closed the window: open it again once.
            window.session = None;
            window
                .session()?
                .navigate(url, LOAD_TIMEOUT, SETTLE)
                .map_err(|error| error.to_string())?
        };
        Ok(BrowserView {
            url: page.url,
            html: page.html,
        })
    }

    fn current(&mut self) -> Result<BrowserView, String> {
        let mut window = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        let page = window
            .session()?
            .current()
            .map_err(|error| error.to_string())?;
        Ok(BrowserView {
            url: page.url,
            html: page.html,
        })
    }

    fn bring_to_front(&mut self) {
        let mut window = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        if let Ok(session) = window.session() {
            let _ = session.bring_to_front();
        }
    }
}

/// The workspace's browser: one window, and a provider rebuilt when settings or parser
/// profiles change.
#[derive(Default)]
pub(crate) struct BrowserState {
    window: Option<Arc<Mutex<BrowserWindow>>>,
    provider: Option<(String, Arc<BrowserProvider>)>,
}

/// A provider handle the chain can own.
pub(crate) struct SharedBrowser(pub(crate) Arc<BrowserProvider>);

impl SerpProvider for SharedBrowser {
    fn id(&self) -> &str {
        self.0.id()
    }

    fn name(&self) -> &str {
        self.0.name()
    }

    fn capabilities(&self) -> Capabilities {
        self.0.capabilities()
    }

    fn estimate(&self, query: &SerpQuery) -> CostEstimate {
        self.0.estimate(query)
    }

    fn search(
        &self,
        query: &SerpQuery,
        context: &SearchContext<'_>,
    ) -> Result<SerpFetch, ProviderError> {
        self.0.search(query, context)
    }
}

impl SerpWorkspace {
    /// The parser profiles in use, with overrides applied where they are valid.
    ///
    /// # Errors
    /// [`SerpAppError`] for an unreadable overrides file.
    pub fn parsers(&self) -> Result<Vec<ParserInfo>, SerpAppError> {
        let overrides = load_overrides(&overrides_path(self.data_dir()))?;
        Engine::ALL
            .into_iter()
            .map(|engine| {
                let base = builtin(engine)
                    .map_err(|error| SerpAppError::InvalidProvider(error.to_string()))?;
                let mut info = ParserInfo {
                    engine,
                    key: base.key(),
                    patched: false,
                    stale_base: None,
                    error: None,
                    mapping: serde_json::to_value(&base.mapping).unwrap_or_default(),
                    base: base.key(),
                    patch: overrides
                        .iter()
                        .find(|patch| patch.engine == engine)
                        .map(|patch| patch.mapping.clone()),
                };
                if let Some(patch) = overrides
                    .iter()
                    .find(|patch| patch.engine == engine && patch.enabled)
                {
                    match apply_override(&base, patch) {
                        Ok(effective) => {
                            info.key = effective.key(true);
                            info.patched = true;
                            info.stale_base = effective.stale_base;
                            info.mapping = serde_json::to_value(&effective.profile.mapping)
                                .unwrap_or_default();
                        }
                        Err(error) => info.error = Some(error.to_string()),
                    }
                }
                Ok(info)
            })
            .collect()
    }

    /// Saves (or replaces) the override of an engine's parser profile after checking it.
    ///
    /// # Errors
    /// [`SerpAppError::InvalidProvider`] when the patched profile is invalid.
    pub fn save_parser_override(&self, patch: ProfileOverride) -> Result<(), SerpAppError> {
        let base = builtin(patch.engine)
            .map_err(|error| SerpAppError::InvalidProvider(error.to_string()))?;
        apply_override(&base, &patch)
            .map_err(|error| SerpAppError::InvalidProvider(error.to_string()))?;
        let path = overrides_path(self.data_dir());
        let mut overrides = load_overrides(&path)?;
        overrides.retain(|existing| existing.engine != patch.engine);
        overrides.push(patch);
        save_overrides(&path, overrides)
    }

    /// Removes the override of an engine's parser profile.
    ///
    /// # Errors
    /// I/O errors.
    pub fn remove_parser_override(&self, engine: Engine) -> Result<(), SerpAppError> {
        let path = overrides_path(self.data_dir());
        let mut overrides = load_overrides(&path)?;
        overrides.retain(|existing| existing.engine != engine);
        save_overrides(&path, overrides)
    }

    /// Compiled profiles by engine, with the keys snapshots record.
    pub(crate) fn compiled_parsers(
        &self,
    ) -> Result<BTreeMap<Engine, (CompiledMapping, String)>, SerpAppError> {
        let mut compiled = BTreeMap::new();
        for info in self.parsers()? {
            let mapping: octopuson_serp::mapping::Mapping = serde_json::from_value(info.mapping)
                .map_err(|error| SerpAppError::InvalidProvider(error.to_string()))?;
            let mapping = mapping
                .compile()
                .map_err(|error| SerpAppError::InvalidProvider(error.to_string()))?;
            compiled.insert(info.engine, (mapping, info.key));
        }
        Ok(compiled)
    }

    /// The browser provider for these settings, sharing one window across searches.
    pub(crate) fn browser_provider(
        &self,
        settings: &Settings,
    ) -> Result<Arc<BrowserProvider>, SerpAppError> {
        let parsers = self.compiled_parsers()?;
        let fingerprint = format!(
            "{}|{}|{:?}|{:?}|{}",
            settings.browser.delay_secs,
            parsers
                .values()
                .map(|(_, key)| key.as_str())
                .collect::<Vec<_>>()
                .join(","),
            settings.browser.executable,
            settings.proxy,
            settings.defaults.language,
        );
        let mut state = self.browser.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some((current, provider)) = &state.provider
            && *current == fingerprint
        {
            return Ok(Arc::clone(provider));
        }
        let window = Arc::clone(state.window.get_or_insert_with(|| {
            let mut options = BrowserOptions::new(self.data_dir().join("browser-profile"));
            options.executable.clone_from(&settings.browser.executable);
            options.proxy.clone_from(&settings.proxy);
            options.language = Some(settings.defaults.language.clone());
            Arc::new(Mutex::new(BrowserWindow {
                options,
                session: None,
            }))
        }));
        let provider = Arc::new(BrowserProvider::new(
            Box::new(SharedWindow(window)),
            parsers,
            BrowserPacing {
                delay: Duration::from_secs(settings.browser.delay_secs),
                ..BrowserPacing::default()
            },
            Box::new(SystemClock),
        ));
        state.provider = Some((fingerprint, Arc::clone(&provider)));
        Ok(provider)
    }

    /// Closes the browser window, if open.
    pub fn close_browser(&self) {
        let mut state = self.browser.lock().unwrap_or_else(PoisonError::into_inner);
        state.provider = None;
        if let Some(window) = state.window.take() {
            let mut window = window.lock().unwrap_or_else(PoisonError::into_inner);
            if let Some(session) = window.session.take() {
                session.close();
            }
        }
    }

    /// Deletes the browser profile (cookies, consent choices); the window must be closed.
    ///
    /// # Errors
    /// I/O errors, e.g. while the profile is in use.
    pub fn reset_browser_profile(&self) -> Result<(), SerpAppError> {
        self.close_browser();
        let path = self.data_dir().join("browser-profile");
        match fs::remove_dir_all(&path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(source) => Err(SerpAppError::Io { path, source }),
        }
    }
}

fn save_overrides(path: &Path, overrides: Vec<ProfileOverride>) -> Result<(), SerpAppError> {
    let text = serde_json::to_string_pretty(&OverridesFile {
        version: 1,
        overrides,
    })
    .map_err(|error| SerpAppError::InvalidProvider(error.to_string()))?;
    write_atomically(path, text.as_bytes())
}
