//! Attended browser SERP adapter powered by `octopuson-render` and `octopuson-serp`.
//!
//! Uses the same persistent `BrowserSession` and declarative SERP parser profiles
//! (`google-html.json`, `yandex-html.json`, `bing-html.json`) as `OctopusOn SERP Checker`,
//! handling JavaScript rendering, cookie consent banners, and interactive CAPTCHA checks.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock, PoisonError};
use std::time::{Duration, Instant};

use directories::ProjectDirs;
use octopuson_render::{BrowserOptions, BrowserPage, BrowserSession};
use octopuson_serp::locale::search_url;
use octopuson_serp::mapping::{CompiledMapping, ParsedPage};
use octopuson_serp::profile::builtin;
use octopuson_serp::{Engine, SerpQuery};
use url::Url;

use crate::model::{IndexEngine, ScanControl, ScanProgress};
use crate::serp::{
    RawSerpHit, parse_bing_serp, parse_duckduckgo_serp, parse_google_serp, parse_yandex_serp,
};

const LOAD_TIMEOUT: Duration = Duration::from_secs(45);
const SETTLE_DELAY: Duration = Duration::from_millis(1600);
const PAGE_DELAY: Duration = Duration::from_millis(2500);
const CHALLENGE_TIMEOUT: Duration = Duration::from_secs(600);
const CHALLENGE_POLL: Duration = Duration::from_secs(2);

static SHARED_BROWSER: OnceLock<Mutex<Option<BrowserSession>>> = OnceLock::new();

fn browser_slot() -> &'static Mutex<Option<BrowserSession>> {
    SHARED_BROWSER.get_or_init(|| Mutex::new(None))
}

fn candidate_profile_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(serp_proj) = ProjectDirs::from("com", "octopusSEO", "octopusSEO SERP Finder") {
        dirs.push(serp_proj.data_dir().join("browser-profile"));
    }
    if let Some(finder_proj) = ProjectDirs::from("com", "octopusseo", "finder") {
        dirs.push(finder_proj.data_local_dir().join("browser-profile"));
    }
    dirs.push(std::env::temp_dir().join("octopusseo-index-check-browser-profile"));
    dirs
}

fn launch_session() -> Option<BrowserSession> {
    if cfg!(test) {
        return None;
    }
    for profile_dir in candidate_profile_dirs() {
        let mut options = BrowserOptions::new(profile_dir);
        options.language = Some("tr".to_owned());
        options.visible = true;
        if let Ok(session) = BrowserSession::launch(&options) {
            return Some(session);
        }
    }
    None
}

fn open_browser_url(url: &str) -> Option<BrowserPage> {
    let mut guard = browser_slot()
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    if guard.is_none() {
        *guard = launch_session();
    }
    let first = guard
        .as_mut()?
        .navigate(url, LOAD_TIMEOUT, SETTLE_DELAY)
        .ok();
    if let Some(page) = first {
        return Some(page);
    }
    // User may have closed the browser window; reopen once.
    *guard = launch_session();
    guard
        .as_mut()?
        .navigate(url, LOAD_TIMEOUT, SETTLE_DELAY)
        .ok()
}

fn current_browser_page() -> Option<BrowserPage> {
    let mut guard = browser_slot()
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    guard.as_mut()?.current().ok()
}

fn bring_browser_to_front() {
    let mut guard = browser_slot()
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    if let Some(session) = guard.as_mut() {
        let _ = session.bring_to_front();
    }
}

/// Stateful browser SERP runner that reuses compiled `octopuson-serp` profiles and paces requests.
pub struct SerpBrowserRunner {
    profiles: BTreeMap<Engine, CompiledMapping>,
    last_opened: Option<Instant>,
    penalty: u32,
}

impl Default for SerpBrowserRunner {
    fn default() -> Self {
        Self::new()
    }
}

impl SerpBrowserRunner {
    /// Creates a new runner with compiled Google, Yandex, and Bing HTML parser profiles.
    pub fn new() -> Self {
        let mut profiles = BTreeMap::new();
        for engine in Engine::ALL {
            if let Ok(profile) = builtin(engine)
                && let Ok(compiled) = profile.compile()
            {
                profiles.insert(engine, compiled);
            }
        }
        Self {
            profiles,
            last_opened: None,
            penalty: 0,
        }
    }

    /// Fetches a single SERP page using the real browser session and `octopuson-serp` parsers.
    pub fn fetch_page(
        &mut self,
        engine: IndexEngine,
        query: &str,
        page: u16,
        control: &ScanControl,
        progress: &mut ScanProgress,
        on_progress: &mut dyn FnMut(&ScanProgress),
    ) -> Option<Vec<RawSerpHit>> {
        match engine {
            IndexEngine::Google => self.fetch_structured_engine(
                Engine::Google,
                engine,
                query,
                page,
                control,
                progress,
                on_progress,
            ),
            IndexEngine::Yandex => self.fetch_structured_engine(
                Engine::Yandex,
                engine,
                query,
                page,
                control,
                progress,
                on_progress,
            ),
            IndexEngine::Bing => self.fetch_structured_engine(
                Engine::Bing,
                engine,
                query,
                page,
                control,
                progress,
                on_progress,
            ),
            IndexEngine::DuckDuckGo => {
                self.fetch_duckduckgo_browser(query, page, control, progress, on_progress)
            }
            _ => None,
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn fetch_structured_engine(
        &mut self,
        serp_engine: Engine,
        index_engine: IndexEngine,
        query: &str,
        page: u16,
        control: &ScanControl,
        progress: &mut ScanProgress,
        on_progress: &mut dyn FnMut(&ScanProgress),
    ) -> Option<Vec<RawSerpHit>> {
        let mapping = self.profiles.get(&serp_engine)?.clone();
        let page_1 = page.saturating_add(1);
        let serp_query = SerpQuery::new(serp_engine, query);
        let target_url = search_url(&serp_query, page_1).ok()?;
        let url_str = target_url.to_string();

        if !self.wait_turn(index_engine, query, page_1, control, progress, on_progress) {
            return Some(Vec::new());
        }

        let mut view = open_browser_url(&url_str)?;
        self.last_opened = Some(Instant::now());
        let mut parsed = parse_browser_view(&mapping, &view, page_1)?;

        if should_wait_for_challenge(index_engine, &parsed, &view.html) {
            self.penalty = (self.penalty + 1).min(3);
            let cleared = self.wait_for_challenge_clearance(
                &mapping,
                index_engine,
                &url_str,
                page_1,
                control,
                progress,
                on_progress,
            )?;
            view = cleared.0;
            parsed = cleared.1;
        }

        Some(collect_merged_hits(index_engine, &parsed, &view.html))
    }

    fn fetch_duckduckgo_browser(
        &mut self,
        query: &str,
        page: u16,
        control: &ScanControl,
        progress: &mut ScanProgress,
        on_progress: &mut dyn FnMut(&ScanProgress),
    ) -> Option<Vec<RawSerpHit>> {
        let page_1 = page.saturating_add(1);
        if !self.wait_turn(
            IndexEngine::DuckDuckGo,
            query,
            page_1,
            control,
            progress,
            on_progress,
        ) {
            return Some(Vec::new());
        }
        let encoded = crate::serp::url_encode(query);
        let offset = u32::from(page) * 30;
        let url = if page == 0 {
            format!("https://duckduckgo.com/?q={encoded}&ia=web")
        } else {
            format!("https://html.duckduckgo.com/html/?q={encoded}&s={offset}")
        };
        let view = open_browser_url(&url)?;
        self.last_opened = Some(Instant::now());
        let mut hits = parse_duckduckgo_serp(&view.html);
        if hits.is_empty() {
            hits = parse_yandex_serp(&view.html);
        }
        Some(hits)
    }

    fn wait_turn(
        &self,
        engine: IndexEngine,
        query: &str,
        page_1: u16,
        control: &ScanControl,
        progress: &mut ScanProgress,
        on_progress: &mut dyn FnMut(&ScanProgress),
    ) -> bool {
        let Some(last) = self.last_opened else {
            return control.wait_if_paused();
        };
        let wait_dur = PAGE_DELAY.saturating_mul(2u32.pow(self.penalty));
        let ready_at = last + wait_dur;
        while Instant::now() < ready_at {
            if !control.wait_if_paused() {
                return false;
            }
            let left = ready_at.saturating_duration_since(Instant::now());
            progress.current_item = format!(
                "{engine}: {query} (p.{page_1} — {} sn bekleniyor)",
                left.as_secs().max(1)
            );
            on_progress(progress);
            std::thread::sleep(left.min(Duration::from_millis(250)));
        }
        progress.current_item = format!("{engine}: {query} (p.{page_1})");
        on_progress(progress);
        control.wait_if_paused()
    }

    #[allow(clippy::too_many_arguments)]
    fn wait_for_challenge_clearance(
        &mut self,
        mapping: &CompiledMapping,
        engine: IndexEngine,
        url_str: &str,
        page_1: u16,
        control: &ScanControl,
        progress: &mut ScanProgress,
        on_progress: &mut dyn FnMut(&ScanProgress),
    ) -> Option<(BrowserPage, ParsedPage)> {
        bring_browser_to_front();
        progress.current_item = format!(
            "{engine}: Tarayıcıda doğrulama (CAPTCHA / Çerez onayı) bekleniyor — lütfen açılan pencereden geçin..."
        );
        on_progress(progress);

        let deadline = Instant::now() + CHALLENGE_TIMEOUT;
        let mut reopened = false;
        while Instant::now() < deadline {
            if !control.wait_if_paused() {
                return None;
            }
            std::thread::sleep(CHALLENGE_POLL);
            let Some(view) = current_browser_page() else {
                continue;
            };
            let Some(parsed) = parse_browser_view(mapping, &view, page_1) else {
                continue;
            };
            if should_wait_for_challenge(engine, &parsed, &view.html) {
                continue;
            }
            if !collect_merged_hits(engine, &parsed, &view.html).is_empty() || reopened {
                return Some((view, parsed));
            }
            // Cleared on a consent page that did not lead back to results: navigate again once.
            reopened = true;
            let view = open_browser_url(url_str)?;
            self.last_opened = Some(Instant::now());
            let parsed = parse_browser_view(mapping, &view, page_1)?;
            if !should_wait_for_challenge(engine, &parsed, &view.html) {
                return Some((view, parsed));
            }
        }
        None
    }
}

fn should_wait_for_challenge(index_engine: IndexEngine, parsed: &ParsedPage, html: &str) -> bool {
    let Some(challenge) = &parsed.challenge else {
        return false;
    };
    if challenge.kind != octopuson_serp::challenge::ChallengeKind::Consent {
        return true;
    }
    collect_merged_hits(index_engine, parsed, html).is_empty()
}

fn parse_browser_view(
    mapping: &CompiledMapping,
    view: &BrowserPage,
    page_1: u16,
) -> Option<ParsedPage> {
    let parsed_url = Url::parse(&view.url).ok()?;
    mapping.parse(&view.html, &parsed_url, page_1).ok()
}

fn collect_merged_hits(engine: IndexEngine, parsed: &ParsedPage, html: &str) -> Vec<RawSerpHit> {
    let mut hits = Vec::new();
    let mut seen = BTreeSet::new();

    for item in &parsed.organic {
        if seen.insert(item.url.clone()) {
            hits.push(RawSerpHit {
                url: item.url.clone(),
                title: item.title.clone(),
                snippet: item.snippet.clone().unwrap_or_default(),
            });
        }
        for sitelink in &item.sitelinks {
            if seen.insert(sitelink.url.clone()) {
                hits.push(RawSerpHit {
                    url: sitelink.url.clone(),
                    title: sitelink.title.clone(),
                    snippet: String::new(),
                });
            }
        }
    }

    let fallback = match engine {
        IndexEngine::Google => parse_google_serp(html),
        IndexEngine::Yandex => parse_yandex_serp(html),
        IndexEngine::Bing => parse_bing_serp(html),
        _ => Vec::new(),
    };
    for hit in fallback {
        if seen.insert(hit.url.clone()) {
            hits.push(hit);
        }
    }

    hits
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn browser_profiles_extract_organic_and_sitelinks_and_detect_blocking_captcha() {
        let runner = SerpBrowserRunner::new();
        assert_eq!(runner.profiles.len(), 3);

        let bing_html = r#"
            <html><body>
                <div id="bnp_container"><button id="bnp_btn_accept">Kabul et</button></div>
                <ol id="b_results">
                    <li class="b_algo">
                        <h2><a href="https://acme.com/urun-1">Ürün 1</a></h2>
                        <div class="b_caption"><p>Açıklama 1</p></div>
                        <ul class="b_vList"><li><a href="https://acme.com/alt-sayfa">Alt Sayfa</a></li></ul>
                    </li>
                </ol>
            </body></html>
        "#;
        let view = BrowserPage {
            url: "https://www.bing.com/search?q=site%3Aacme.com".to_owned(),
            title: "site:acme.com - Bing".to_owned(),
            html: bing_html.to_owned(),
        };
        let mapping = &runner.profiles[&Engine::Bing];
        let parsed = parse_browser_view(mapping, &view, 1).unwrap();
        assert!(!should_wait_for_challenge(
            IndexEngine::Bing,
            &parsed,
            bing_html
        ));
        let hits = collect_merged_hits(IndexEngine::Bing, &parsed, bing_html);
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].url, "https://acme.com/urun-1");
        assert_eq!(hits[1].url, "https://acme.com/alt-sayfa");

        let google_sorry = BrowserPage {
            url: "https://www.google.com/sorry/index".to_owned(),
            title: "Sorry".to_owned(),
            html: "<html><body><form id='captcha-form'></form></body></html>".to_owned(),
        };
        let g_parsed =
            parse_browser_view(&runner.profiles[&Engine::Google], &google_sorry, 1).unwrap();
        assert!(should_wait_for_challenge(
            IndexEngine::Google,
            &g_parsed,
            &google_sorry.html
        ));
    }
}
