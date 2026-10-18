//! The attended browser mode: result pages read in a visible Chrome or Edge window with the
//! program's own profile (`<data>/browser-profile`), one query at a time, with a fixed pause
//! and a daily limit per engine. A consent page or CAPTCHA brings the window forward and
//! waits for the person; nothing is solved, disguised or rotated.

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use seonexus_render::{BrowserOptions, BrowserSession, find_browser};
use seonexus_serp::mapping::CompiledMapping;
use seonexus_serp::profile::builtin;
use seonexus_serp::provider::{
    BROWSER_ID, BrowserPacing, BrowserPort, BrowserProvider, BrowserView, Capabilities,
    CostEstimate, ProviderError, SearchContext, SerpFetch, SerpProvider, SystemClock,
};
use seonexus_serp::{Engine, SerpQuery};
use serde::{Deserialize, Serialize};

use crate::settings::Settings;
use crate::{KeywordWorkspace, KwAppError};

/// Provider id of the browser mode in the chains, switches and usage.
pub const BROWSER: &str = BROWSER_ID;

/// Time a result page may take to load.
const LOAD_TIMEOUT: Duration = Duration::from_secs(45);
/// Time after loading for scripts to finish drawing the page.
const SETTLE: Duration = Duration::from_millis(1500);
/// The window's profile, under the data directory.
const PROFILE_DIR: &str = "browser-profile";

/// Where the browser mode stands.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BrowserStatus {
    /// The Chrome or Edge executable that would be used; none when not found.
    pub executable: Option<String>,
    /// Not switched off.
    pub enabled: bool,
    /// Engines whose result-page chain lists the browser.
    pub engines: Vec<String>,
    /// Result pages opened today (UTC) per engine.
    pub used_today: BTreeMap<String, u64>,
    /// Most result pages per engine and day.
    pub daily_cap: u32,
    /// Pause between two result pages, seconds.
    pub delay_secs: u64,
    /// The window's profile directory.
    pub profile_dir: String,
}

/// The browser window, opened on first use and reopened if the person closed it.
struct BrowserWindow {
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

    fn close(&mut self) {
        if let Some(session) = self.session.take() {
            session.close();
        }
    }
}

/// One window shared by every provider built for the workspace.
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

/// The workspace's browser: one window, and a provider rebuilt when the pause changes.
#[derive(Default)]
pub(crate) struct BrowserState {
    window: Option<Arc<Mutex<BrowserWindow>>>,
    provider: Option<(BrowserPacing, Arc<BrowserProvider>)>,
    /// A provider tests put in place of the window.
    #[cfg(test)]
    pub(crate) fixed: Option<Arc<BrowserProvider>>,
}

/// A provider handle the chain can own.
struct SharedBrowser(Arc<BrowserProvider>);

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

/// The built-in parser profiles by engine, with the keys snapshots record.
fn parsers() -> Result<BTreeMap<Engine, (CompiledMapping, String)>, KwAppError> {
    Engine::ALL
        .into_iter()
        .map(|engine| {
            let profile =
                builtin(engine).map_err(|error| KwAppError::Invalid(error.to_string()))?;
            let mapping = profile
                .compile()
                .map_err(|error| KwAppError::Invalid(error.to_string()))?;
            Ok((engine, (mapping, profile.key())))
        })
        .collect()
}

/// Today (UTC), `YYYY-MM-DD`.
fn today() -> String {
    crate::now().chars().take(10).collect()
}

impl KeywordWorkspace {
    fn browser_profile(&self) -> PathBuf {
        self.data_dir().join(PROFILE_DIR)
    }

    fn browser_options(&self, settings: &Settings) -> BrowserOptions {
        let mut options = BrowserOptions::new(self.browser_profile());
        options.executable.clone_from(&settings.browser.executable);
        options.proxy.clone_from(&settings.browser.proxy);
        options.language = Some(settings.projects.language.clone());
        options
    }

    /// The browser as a result-page provider, sharing one window across runs.
    pub(crate) fn browser_provider(
        &self,
        settings: &Settings,
    ) -> Result<Box<dyn SerpProvider>, KwAppError> {
        let mut state = self.browser.lock().unwrap_or_else(PoisonError::into_inner);
        #[cfg(test)]
        if let Some(fixed) = &state.fixed {
            return Ok(Box::new(SharedBrowser(Arc::clone(fixed))));
        }
        let options = self.browser_options(settings);
        let window = if let Some(window) = &state.window {
            let mut open = window.lock().unwrap_or_else(PoisonError::into_inner);
            if open.options != options {
                // A new executable, proxy or language: the next page opens a new window.
                open.close();
                open.options = options;
            }
            Arc::clone(window)
        } else {
            let window = Arc::new(Mutex::new(BrowserWindow {
                options,
                session: None,
            }));
            state.window = Some(Arc::clone(&window));
            window
        };
        let pacing = BrowserPacing {
            delay: Duration::from_secs(settings.browser.delay_secs),
            ..BrowserPacing::default()
        };
        if let Some((current, provider)) = &state.provider
            && *current == pacing
        {
            return Ok(Box::new(SharedBrowser(Arc::clone(provider))));
        }
        let provider = Arc::new(BrowserProvider::new(
            Box::new(SharedWindow(window)),
            parsers()?,
            pacing,
            Box::new(SystemClock),
        ));
        state.provider = Some((pacing, Arc::clone(&provider)));
        Ok(Box::new(SharedBrowser(provider)))
    }

    /// Why the browser cannot read `pages` more result pages of `engine` now, if it cannot:
    /// no Chrome or Edge, or the day's limit reached.
    pub(crate) fn browser_unusable(
        &self,
        settings: &Settings,
        engine: &str,
        pages: u16,
    ) -> Result<Option<String>, KwAppError> {
        if find_browser(settings.browser.executable.as_deref()).is_none() {
            return Ok(Some("Chrome or Edge was not found".to_owned()));
        }
        let used = self
            .store()?
            .engine_requests_on(BROWSER_ID, engine, &today())?;
        Ok(
            (used + u64::from(pages) > u64::from(settings.browser.daily_cap)).then(|| {
                format!(
                    "the daily limit of {} {engine} pages in browser mode is reached",
                    settings.browser.daily_cap
                )
            }),
        )
    }

    /// Where the browser mode stands: the executable, the chains that list it and today's
    /// pages per engine.
    ///
    /// # Errors
    /// Settings and store errors.
    pub fn browser_status(&self) -> Result<BrowserStatus, KwAppError> {
        let settings = self.settings()?;
        let store = self.store()?;
        let day = today();
        let used_today = Engine::ALL
            .into_iter()
            .map(|engine| {
                let id = engine.as_str();
                Ok((
                    id.to_owned(),
                    store.engine_requests_on(BROWSER_ID, id, &day)?,
                ))
            })
            .collect::<Result<_, KwAppError>>()?;
        Ok(BrowserStatus {
            executable: find_browser(settings.browser.executable.as_deref())
                .map(|path| path.display().to_string()),
            enabled: !settings.disabled.contains(BROWSER_ID),
            engines: settings
                .serp
                .chains
                .iter()
                .filter(|(_, chain)| chain.iter().any(|id| id == BROWSER_ID))
                .map(|(engine, _)| engine.clone())
                .collect(),
            used_today,
            daily_cap: settings.browser.daily_cap,
            delay_secs: settings.browser.delay_secs,
            profile_dir: self.browser_profile().display().to_string(),
        })
    }

    /// Closes the browser window, if open.
    pub fn close_browser(&self) {
        let mut state = self.browser.lock().unwrap_or_else(PoisonError::into_inner);
        state.provider = None;
        if let Some(window) = state.window.take() {
            window
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .close();
        }
    }

    /// Closes the window and deletes its profile (cookies, consent choices).
    ///
    /// # Errors
    /// I/O errors, e.g. while another program uses the profile.
    pub fn reset_browser_profile(&self) -> Result<(), KwAppError> {
        self.close_browser();
        let path = self.browser_profile();
        match fs::remove_dir_all(&path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(source) => Err(KwAppError::Io { path, source }),
        }
    }
}
