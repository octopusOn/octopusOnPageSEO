//! JavaScript rendering with an installed Chrome/Chromium/Edge browser via DevTools.
//!
//! One private browser per crawl, bounded parallel pages, a fresh browser context per URL.
//! The main document is fulfilled from the crawler's HTTP response; scripts/styles/XHR use
//! the browser's normal network stack, origin rules and certificate verification.
//!
//! [`BrowserSession`] is the other mode: a visible window with a persistent profile of the
//! program's own, for attended searches where a person may need to step in.

mod browser;
mod cache;
mod cdp;
mod downloads;
mod session;
mod slots;

use std::path::PathBuf;
use std::time::{Duration, Instant};

use octopuson_crawl::{
    CrawlPlan, FetchError, RenderRequest, RenderedPage, Renderer, Rendering, Viewport,
};

pub use session::{BrowserOptions, BrowserPage, BrowserSession};

/// The Chrome, Chromium or Edge executable that would be used: `explicit` if it exists, else
/// `OCTOPUSSEO_CHROME_PATH`, else the usual install locations and `PATH`.
pub fn find_browser(explicit: Option<&str>) -> Option<PathBuf> {
    browser::discover(explicit)
}

/// The rendering adapter could not start.
#[derive(Debug, thiserror::Error)]
pub enum RenderError {
    /// No supported browser executable was found.
    #[error(
        "Chrome, Chromium or Edge was not found. Install one or set rendering.browser_path / OCTOPUSSEO_CHROME_PATH."
    )]
    BrowserMissing,
    /// Starting the browser failed.
    #[error("cannot launch browser `{}`: {source}",path.display())]
    Launch {
        /// Executable path.
        path: PathBuf,
        /// Operating-system error.
        source: std::io::Error,
    },
    /// Browser startup did not complete.
    #[error("cannot start JavaScript rendering: {0}")]
    Startup(String),
    /// A profile or process operation failed.
    #[error("rendering I/O error: {0}")]
    Io(#[from] std::io::Error),
    /// Another window of this program uses the browser profile.
    #[error("the browser profile `{}` is in use by another window", .0.display())]
    Busy(PathBuf),
    /// The browser did not do what was asked.
    #[error("browser error: {0}")]
    Browser(String),
}

/// An installed browser running in a separate temporary profile for this crawl.
pub struct ChromeRenderer {
    browser: browser::Browser,
    settings: Rendering,
    user_agent: String,
    language: Option<String>,
    viewport: Viewport,
    slots: slots::Slots,
    cache: cache::ResourceCache,
    downloads: downloads::Downloads,
}

impl ChromeRenderer {
    /// Starts a private browser. The user's running browsers/profiles are never touched.
    ///
    /// # Errors
    /// Missing browser, failed process launch or startup timeout.
    pub fn launch(plan: &CrawlPlan) -> Result<Self, RenderError> {
        let settings = plan.config().rendering.clone();
        let viewport = settings.effective_viewport(plan.config().agent);
        Ok(Self {
            browser: browser::Browser::launch(settings.browser_path.as_deref())?,
            settings,
            user_agent: plan.user_agent().to_owned(),
            language: plan.config().accept_language.clone(),
            viewport,
            slots: slots::Slots::new(plan.config().rendering.concurrency),
            cache: cache::ResourceCache::default(),
            downloads: downloads::Downloads::default(),
        })
    }
}

impl Renderer for ChromeRenderer {
    fn render(&self, request: &RenderRequest<'_>) -> Result<RenderedPage, FetchError> {
        let started = Instant::now();
        let _slot = self.slots.acquire(request.control)?;
        let deadline =
            Instant::now() + Duration::from_secs(u64::from(self.settings.timeout_seconds));
        let mut session = cdp::Session::connect(
            &self.browser.endpoint,
            request,
            deadline,
            self.settings.cache_resources.then_some(&self.cache),
            &self.downloads,
        )?;
        let mut rendered = session.render(
            &self.user_agent,
            self.language.as_deref(),
            self.viewport,
            Duration::from_millis(u64::from(self.settings.wait_ms)),
        )?;
        rendered.elapsed = started.elapsed();
        Ok(rendered)
    }
}
#[cfg(test)]
mod parallel_tests;
#[cfg(test)]
mod tests;
