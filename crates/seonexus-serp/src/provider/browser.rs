//! Attended searches in the user's browser.
//!
//! [`BrowserProvider`] opens result pages one at a time in a browser window the person using
//! the program can see ([`BrowserPort`]), waits a fixed delay between pages (doubled after
//! each check page, up to eight times), reads the pages with the engine's parser profile and,
//! when the engine shows a consent page or a CAPTCHA, brings the window to the front and waits
//! for the person to deal with it. It never answers a check itself.

use std::collections::BTreeMap;
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

use url::Url;

use super::{
    Capabilities, CostEstimate, ProviderError, RawPage, SearchContext, SearchEvent, SerpFetch,
    SerpProvider,
};
use crate::challenge::Challenge;
use crate::locale::search_url;
use crate::mapping::{CompiledMapping, ParsedPage};
use crate::{Device, Engine, MAX_DEPTH, SerpQuery, SnapshotBuilder};

/// Provider id.
pub const BROWSER_ID: &str = "browser";

/// Most doublings of the delay after check pages.
const MAX_PENALTY: u32 = 3;

/// A page the window shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowserView {
    /// Address after redirects.
    pub url: String,
    /// Serialized document.
    pub html: String,
}

/// A browser window.
pub trait BrowserPort: Send {
    /// Loads `url` and returns the document.
    ///
    /// # Errors
    /// A message when the page cannot be loaded.
    fn open(&mut self, url: &str) -> Result<BrowserView, String>;

    /// The document the window shows now.
    ///
    /// # Errors
    /// A message when the window cannot be read.
    fn current(&mut self) -> Result<BrowserView, String>;

    /// Shows the window above others.
    fn bring_to_front(&mut self);
}

/// Time, so tests can run without waiting.
pub trait Clock: Send + Sync {
    /// The current instant.
    fn now(&self) -> Instant;
    /// Waits.
    fn sleep(&self, duration: Duration);
}

/// The system clock.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> Instant {
        Instant::now()
    }

    fn sleep(&self, duration: Duration) {
        std::thread::sleep(duration);
    }
}

/// Pacing of attended searches.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BrowserPacing {
    /// Wait between two result pages.
    pub delay: Duration,
    /// How long a check page may wait for the person.
    pub challenge_timeout: Duration,
    /// How often a check page is looked at again.
    pub poll: Duration,
}

impl Default for BrowserPacing {
    fn default() -> Self {
        Self {
            delay: Duration::from_secs(30),
            challenge_timeout: Duration::from_secs(600),
            poll: Duration::from_secs(2),
        }
    }
}

#[derive(Debug, Default)]
struct Gate {
    last: Option<Instant>,
    penalty: u32,
}

/// Searches in a browser window with the engines' parser profiles.
pub struct BrowserProvider {
    port: Mutex<Box<dyn BrowserPort>>,
    profiles: BTreeMap<Engine, (CompiledMapping, String)>,
    pacing: BrowserPacing,
    clock: Box<dyn Clock>,
    gate: Mutex<Gate>,
}

impl std::fmt::Debug for BrowserProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BrowserProvider")
            .field("engines", &self.profiles.keys().collect::<Vec<_>>())
            .field("pacing", &self.pacing)
            .finish_non_exhaustive()
    }
}

impl BrowserProvider {
    /// A provider reading each engine with its compiled profile and the profile's key.
    pub fn new(
        port: Box<dyn BrowserPort>,
        profiles: BTreeMap<Engine, (CompiledMapping, String)>,
        pacing: BrowserPacing,
        clock: Box<dyn Clock>,
    ) -> Self {
        Self {
            port: Mutex::new(port),
            profiles,
            pacing,
            clock,
            gate: Mutex::new(Gate::default()),
        }
    }

    /// Waits until the next page may be opened.
    fn wait_turn(&self, context: &SearchContext<'_>) -> Result<(), ProviderError> {
        let ready_at = {
            let gate = self.gate.lock().unwrap_or_else(PoisonError::into_inner);
            gate.last
                .map(|last| last + self.pacing.delay * 2u32.pow(gate.penalty))
        };
        let Some(ready_at) = ready_at else {
            return Ok(());
        };
        let mut announced = false;
        loop {
            let now = self.clock.now();
            if now >= ready_at {
                return Ok(());
            }
            if (context.cancelled)() {
                return Err(ProviderError::Cancelled);
            }
            let left = ready_at - now;
            if !announced {
                (context.events)(&SearchEvent::Waiting {
                    provider: BROWSER_ID.to_owned(),
                    seconds: left.as_secs().max(1),
                });
                announced = true;
            }
            self.clock.sleep(left.min(Duration::from_millis(250)));
        }
    }

    fn opened(&self) {
        self.gate
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .last = Some(self.clock.now());
    }

    fn penalise(&self) {
        let mut gate = self.gate.lock().unwrap_or_else(PoisonError::into_inner);
        gate.penalty = (gate.penalty + 1).min(MAX_PENALTY);
    }

    /// Waits for the person to clear a check page; returns the page they end up on.
    fn wait_for_person(
        &self,
        port: &mut dyn BrowserPort,
        mapping: &CompiledMapping,
        challenge: Challenge,
        url: &str,
        page: u16,
        context: &SearchContext<'_>,
    ) -> Result<(BrowserView, ParsedPage), ProviderError> {
        self.penalise();
        (context.events)(&SearchEvent::Challenge {
            provider: BROWSER_ID.to_owned(),
            kind: challenge.kind,
            url: url.to_owned(),
        });
        port.bring_to_front();
        let deadline = self.clock.now() + self.pacing.challenge_timeout;
        let mut reopened = false;
        while self.clock.now() < deadline {
            if (context.cancelled)() {
                return Err(ProviderError::Cancelled);
            }
            self.clock.sleep(self.pacing.poll);
            let Ok(view) = port.current() else {
                continue;
            };
            let parsed = parse(mapping, &view, page)?;
            if parsed.challenge.is_some() {
                continue;
            }
            if !parsed.organic.is_empty() || reopened {
                (context.events)(&SearchEvent::Resumed {
                    provider: BROWSER_ID.to_owned(),
                });
                return Ok((view, parsed));
            }
            // Cleared, but on another page (a consent page may not lead back): search again.
            reopened = true;
            let view = port.open(url).map_err(ProviderError::Network)?;
            self.opened();
            let parsed = parse(mapping, &view, page)?;
            if parsed.challenge.is_none() {
                (context.events)(&SearchEvent::Resumed {
                    provider: BROWSER_ID.to_owned(),
                });
                return Ok((view, parsed));
            }
        }
        Err(ProviderError::Blocked(challenge))
    }
}

fn parse(
    mapping: &CompiledMapping,
    view: &BrowserView,
    page: u16,
) -> Result<ParsedPage, ProviderError> {
    let url = Url::parse(&view.url).map_err(|error| ProviderError::Parse {
        message: format!("the browser reported an invalid address: {error}"),
        coverage: None,
    })?;
    Ok(mapping.parse(&view.html, &url, page)?)
}

impl SerpProvider for BrowserProvider {
    fn id(&self) -> &'static str {
        BROWSER_ID
    }

    fn name(&self) -> &'static str {
        "Browser (attended)"
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            engines: self.profiles.keys().copied().collect(),
            devices: vec![Device::Desktop],
            max_depth: MAX_DEPTH,
            interactive: true,
        }
    }

    fn estimate(&self, query: &SerpQuery) -> CostEstimate {
        CostEstimate {
            requests: u32::from(query.pages()),
            usd: 0.0,
        }
    }

    fn search(
        &self,
        query: &SerpQuery,
        context: &SearchContext<'_>,
    ) -> Result<SerpFetch, ProviderError> {
        if query.device != Device::Desktop {
            return Err(ProviderError::Unsupported(
                "browser mode returns desktop results only".to_owned(),
            ));
        }
        let (mapping, key) = self.profiles.get(&query.engine).ok_or_else(|| {
            ProviderError::Unsupported(format!("no parser profile for {}", query.engine))
        })?;
        let mut port = self.port.lock().unwrap_or_else(PoisonError::into_inner);
        let mut builder =
            SnapshotBuilder::new(query.clone(), BROWSER_ID, context.fetched_at.clone())
                .provider_rev("browser@1")
                .parser_rev(key.clone());
        let mut pages = Vec::new();
        let mut requests = 0;
        let total = query.pages();
        for page in 1..=total {
            self.wait_turn(context)?;
            let url = search_url(query, page)?.to_string();
            (context.events)(&SearchEvent::Page {
                provider: BROWSER_ID.to_owned(),
                page,
                pages: total,
            });
            let view = match port.open(&url) {
                Ok(view) => view,
                Err(error) if page > 1 => {
                    builder.warn(format!("page {page}: {error}"));
                    break;
                }
                Err(error) => return Err(ProviderError::Network(error)),
            };
            self.opened();
            requests += 1;
            let mut parsed = parse(mapping, &view, page)?;
            let mut view = view;
            if let Some(challenge) = parsed.challenge.clone() {
                let (cleared, read) =
                    self.wait_for_person(port.as_mut(), mapping, challenge, &url, page, context)?;
                view = cleared;
                parsed = read;
            }
            let raw = RawPage {
                page,
                url: view.url.clone(),
                status: 0,
                content_type: Some("text/html".to_owned()),
                body: view.html.into_bytes(),
            };
            if page == 1 && parsed.organic.is_empty() {
                return Err(ProviderError::Unreadable {
                    message: format!(
                        "no result could be read from the page; the {key} selectors may be outdated"
                    ),
                    coverage: (parsed.coverage.matched > 0).then(|| parsed.coverage.ratio()),
                    page: Box::new(raw),
                });
            }
            pages.push(raw);
            if builder.add_page(parsed) == 0 || builder.is_full() {
                break;
            }
        }
        Ok(SerpFetch {
            snapshot: builder.finish(),
            pages,
            requests,
            reported_cost_usd: Some(0.0),
            estimated_cost_usd: 0.0,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU64, Ordering};

    use seonexus_core::http::testing::FakeTransport;

    use super::*;
    use crate::challenge::ChallengeKind;
    use crate::profile::builtin;

    const RESULTS: &str = include_str!("../../tests/fixtures/google-tr.html");
    const SORRY: &str = "<html><body><form id='captcha-form'></form></body></html>";

    /// Virtual time: sleeping advances it.
    #[derive(Clone, Default)]
    struct FakeClock {
        start: Option<Instant>,
        elapsed_ms: Arc<AtomicU64>,
    }

    impl Clock for FakeClock {
        fn now(&self) -> Instant {
            self.start.unwrap_or_else(Instant::now)
                + Duration::from_millis(self.elapsed_ms.load(Ordering::SeqCst))
        }

        fn sleep(&self, duration: Duration) {
            self.elapsed_ms.fetch_add(
                u64::try_from(duration.as_millis()).unwrap_or(u64::MAX),
                Ordering::SeqCst,
            );
        }
    }

    /// Answers `open` and `current` from queues and records what was opened.
    struct FakeWindow {
        opens: VecDeque<BrowserView>,
        currents: VecDeque<BrowserView>,
        opened: Arc<Mutex<Vec<String>>>,
        fronted: Arc<AtomicU64>,
    }

    impl BrowserPort for FakeWindow {
        fn open(&mut self, url: &str) -> Result<BrowserView, String> {
            self.opened.lock().unwrap().push(url.to_owned());
            self.opens.pop_front().ok_or_else(|| "closed".to_owned())
        }

        fn current(&mut self) -> Result<BrowserView, String> {
            self.currents.pop_front().ok_or_else(|| "closed".to_owned())
        }

        fn bring_to_front(&mut self) {
            self.fronted.fetch_add(1, Ordering::SeqCst);
        }
    }

    fn view(url: &str, html: &str) -> BrowserView {
        BrowserView {
            url: url.to_owned(),
            html: html.to_owned(),
        }
    }

    struct Setup {
        provider: BrowserProvider,
        opened: Arc<Mutex<Vec<String>>>,
        fronted: Arc<AtomicU64>,
        clock: FakeClock,
    }

    fn setup(opens: Vec<BrowserView>, currents: Vec<BrowserView>) -> Setup {
        let opened = Arc::new(Mutex::new(Vec::new()));
        let fronted = Arc::new(AtomicU64::new(0));
        let clock = FakeClock {
            start: Some(Instant::now()),
            ..FakeClock::default()
        };
        let google = builtin(Engine::Google).unwrap();
        let profiles =
            BTreeMap::from([(Engine::Google, (google.compile().unwrap(), google.key()))]);
        let provider = BrowserProvider::new(
            Box::new(FakeWindow {
                opens: opens.into(),
                currents: currents.into(),
                opened: Arc::clone(&opened),
                fronted: Arc::clone(&fronted),
            }),
            profiles,
            BrowserPacing::default(),
            Box::new(clock.clone()),
        );
        Setup {
            provider,
            opened,
            fronted,
            clock,
        }
    }

    fn run(
        provider: &BrowserProvider,
        query: &SerpQuery,
        events: &Mutex<Vec<SearchEvent>>,
    ) -> Result<SerpFetch, ProviderError> {
        let transport = FakeTransport::new();
        let record = |event: &SearchEvent| events.lock().unwrap().push(event.clone());
        provider.search(
            query,
            &SearchContext {
                transport: &transport,
                secrets: &|_, _| None,
                fetched_at: "2026-10-18T10:00:00Z".to_owned(),
                cancelled: &|| false,
                events: &record,
            },
        )
    }

    #[test]
    fn pages_with_a_fixed_delay() {
        let page = view("https://www.google.com/search?q=seo", RESULTS);
        let mut second = page.clone();
        second.html = second
            .html
            .replace("forum.example.org", "forum2.example.org");
        let setup = setup(vec![page, second], Vec::new());
        let events = Mutex::new(Vec::new());
        let query = SerpQuery::new(Engine::Google, "seo ajansı").with_depth(20);
        let fetch = run(&setup.provider, &query, &events).unwrap();
        assert_eq!(fetch.snapshot.organic.len(), 4);
        assert_eq!(fetch.pages.len(), 2);
        assert_eq!(fetch.snapshot.parser_rev.as_deref(), Some("google-html@1"));
        let opened = setup.opened.lock().unwrap().clone();
        assert!(opened[1].contains("start=10"));
        // The second page waited the full delay of virtual time.
        assert!(setup.clock.elapsed_ms.load(Ordering::SeqCst) >= 30_000);
        assert!(
            events
                .lock()
                .unwrap()
                .iter()
                .any(|event| matches!(event, SearchEvent::Waiting { .. }))
        );
    }

    #[test]
    fn waits_for_the_person_to_solve_a_check() {
        let sorry = view("https://www.google.com/sorry/index?continue=x", SORRY);
        let solved = view("https://www.google.com/search?q=seo", RESULTS);
        let setup = setup(vec![sorry.clone()], vec![sorry, solved]);
        let events = Mutex::new(Vec::new());
        let fetch = run(
            &setup.provider,
            &SerpQuery::new(Engine::Google, "seo"),
            &events,
        )
        .unwrap();
        assert_eq!(fetch.snapshot.organic.len(), 3);
        assert_eq!(setup.fronted.load(Ordering::SeqCst), 1);
        let events = events.lock().unwrap();
        assert!(events.iter().any(|event| matches!(
            event,
            SearchEvent::Challenge {
                kind: ChallengeKind::Captcha,
                ..
            }
        )));
        assert!(
            events
                .iter()
                .any(|event| matches!(event, SearchEvent::Resumed { .. }))
        );
        // The next delay is doubled.
        assert_eq!(setup.provider.gate.lock().unwrap().penalty, 1);
    }

    #[test]
    fn gives_up_on_unsolved_checks_and_reports_unreadable_pages() {
        let sorry = view("https://www.google.com/sorry/index", SORRY);
        let currents = vec![sorry.clone(); 400];
        let setup = setup(vec![sorry], currents);
        let events = Mutex::new(Vec::new());
        let blocked = run(
            &setup.provider,
            &SerpQuery::new(Engine::Google, "seo"),
            &events,
        );
        assert!(matches!(blocked, Err(ProviderError::Blocked(_))));

        let empty = setup_empty();
        let unreadable = run(
            &empty.provider,
            &SerpQuery::new(Engine::Google, "seo"),
            &events,
        );
        match unreadable {
            Err(ProviderError::Unreadable { page, .. }) => {
                assert!(String::from_utf8_lossy(&page.body).contains("new layout"));
            }
            other => panic!("expected an unreadable page, got {other:?}"),
        }
        let mobile = SerpQuery {
            device: Device::Mobile,
            ..SerpQuery::new(Engine::Google, "seo")
        };
        assert!(matches!(
            run(&empty.provider, &mobile, &events),
            Err(ProviderError::Unsupported(_))
        ));
    }

    fn setup_empty() -> Setup {
        setup(
            vec![view(
                "https://www.google.com/search?q=seo",
                "<html><body><div id=rso>new layout</div></body></html>",
            )],
            Vec::new(),
        )
    }
}
