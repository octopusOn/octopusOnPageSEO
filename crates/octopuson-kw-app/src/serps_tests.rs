use std::collections::VecDeque;
use std::fmt::Write as _;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use octopuson_core::http::testing::FakeTransport;
use octopuson_keywords::Level;
use octopuson_kw_store::{KeywordQuery, MetricRecord, ProjectSpec};
use octopuson_secrets::{FileBackend, Secrets};
use octopuson_serp::challenge::ChallengeKind;
use octopuson_serp::profile::builtin;
use octopuson_serp::provider::{BrowserPort, BrowserProvider, BrowserView, Clock};
use serde_json::json;

use super::*;
use crate::cluster::ClusterRequest;
use crate::discover::{DiscoverRequest, DiscoverySource};
use crate::settings::Method;

fn workspace() -> (tempfile::TempDir, KeywordWorkspace, i64) {
    let dir = tempfile::tempdir().unwrap();
    let secrets = Secrets::new(
        Box::new(FileBackend::new(dir.path().join("secrets.json"))),
        dir.path().join("secret-bindings.json"),
    )
    .with_environment(|_| None);
    let workspace = KeywordWorkspace::with_secrets(dir.path().to_owned(), secrets);
    let id = workspace
        .create_project(&ProjectSpec {
            name: "Kahve".to_owned(),
            country: "tr".to_owned(),
            language: "tr".to_owned(),
            engines: vec!["google".to_owned()],
            site: Some("ornek.com.tr".to_owned()),
            competitors: Vec::new(),
        })
        .unwrap()
        .id;
    let texts = ["kahve makinesi", "espresso makinesi", "çay demliği"]
        .map(str::to_owned)
        .to_vec();
    workspace.add_keywords(id, &texts, "manual", None).unwrap();
    let index = workspace.store().unwrap().key_index(id).unwrap();
    let volume = |key: &str, volume: u64| MetricRecord {
        keyword_id: index[key],
        source: "import".to_owned(),
        engine: "google".to_owned(),
        country: "tr".to_owned(),
        volume: Some(volume),
        volume_low: None,
        volume_high: None,
        cpc_usd: None,
        competition: None,
        difficulty: None,
        monthly: Vec::new(),
        fetched_at: "2026-10-18T10:00:00Z".to_owned(),
    };
    workspace
        .store()
        .unwrap()
        .put_metrics(&[
            volume("kahve makinesi", 9_000),
            volume("espresso makinesi", 5_000),
            volume("cay demligi", 1_000),
        ])
        .unwrap();
    (dir, workspace, id)
}

fn serper(links: &[&str]) -> String {
    json!({
        "organic": links
            .iter()
            .enumerate()
            .map(|(index, link)| json!({"title": link, "link": format!("https://{link}"), "position": index + 1}))
            .collect::<Vec<_>>(),
        "peopleAlsoAsk": [{"question": "Hangi kahve makinesi?"}],
        "relatedSearches": [{"query": "kahve makinesi fiyat"}],
        "credits": 1
    })
    .to_string()
}

fn quiet() -> RunControl<'static> {
    RunControl {
        cancelled: &|| false,
        progress: &|_| {},
        sleep: &|_| {},
    }
}

#[test]
fn fetches_result_pages_through_the_chain_and_reuses_fresh_ones() {
    let (_dir, workspace, id) = workspace();
    let estimate = workspace
        .estimate_serps(id, &SerpRequest::default())
        .unwrap();
    assert_eq!(estimate.keywords, 3);
    assert_eq!(estimate.provider, None);
    assert_eq!(estimate.skipped.len(), 3);

    workspace
        .set_key("serper", "api_key", "serper-key-1")
        .unwrap();
    let estimate = workspace
        .estimate_serps(id, &SerpRequest::default())
        .unwrap();
    assert_eq!(estimate.provider.as_deref(), Some("serper"));
    assert!(estimate.cost_usd > 0.0);

    let shared = [
        "a.com/1",
        "b.com/1",
        "c.com/1",
        "ornek.com.tr/kahve-makinesi",
    ];
    let transport = FakeTransport::new()
        .reply(200, serper(&shared))
        .reply(200, serper(&shared))
        .reply(200, serper(&["t.com/1", "u.com/1"]));
    let outcome = workspace
        .fetch_serps_with(id, &SerpRequest::default(), &transport, &quiet())
        .unwrap();
    assert_eq!(outcome.run.status, RunStatus::Done);
    assert_eq!(outcome.run.found, 3);
    assert_eq!(outcome.site_pages, 2);
    assert!(
        transport.requests()[0]
            .headers
            .contains(&("X-API-KEY".to_owned(), "serper-key-1".to_owned()))
    );
    let store = workspace.store().unwrap();
    let machine = store.key_index(id).unwrap()["kahve makinesi"];
    let page = store.serp(machine, "google", "desktop").unwrap().unwrap();
    assert_eq!(page.urls.len(), 4);
    assert_eq!(page.questions, ["Hangi kahve makinesi?"]);
    assert_eq!(page.related, ["kahve makinesi fiyat"]);
    assert_eq!(store.usage().unwrap()[0].provider, "serper");
    drop(store);

    let again = workspace
        .estimate_serps(id, &SerpRequest::default())
        .unwrap();
    assert_eq!((again.keywords, again.cached), (0, 3));
    assert!(matches!(
        workspace.fetch_serps_with(id, &SerpRequest::default(), &transport, &quiet()),
        Err(KwAppError::Invalid(_))
    ));
    let top = SerpRequest {
        top: Some(1),
        refresh: true,
        ..SerpRequest::default()
    };
    assert_eq!(workspace.estimate_serps(id, &top).unwrap().keywords, 1);

    // The result pages verify the clusters, and mapping finds the site's page.
    let outcome = workspace
        .cluster(
            id,
            &ClusterRequest {
                method: Method::Serp,
                ..ClusterRequest::default()
            },
        )
        .unwrap();
    assert_eq!(outcome.clustering.clusters, 2);
    let store = workspace.store().unwrap();
    let clusters = store.cluster_records(outcome.clustering.id).unwrap();
    assert_eq!(
        clusters[0].target_url.as_deref(),
        Some("https://ornek.com.tr/kahve-makinesi")
    );
    assert!(clusters[1].gap);
    drop(store);
    let remapped = workspace.remap(id).unwrap();
    assert_eq!(remapped.clusters, 2);
}

/// A project whose three keywords have result pages: two sharing four results, one apart.
fn fetched() -> (tempfile::TempDir, KeywordWorkspace, i64) {
    let (dir, workspace, id) = workspace();
    workspace
        .set_key("serper", "api_key", "serper-key-1")
        .unwrap();
    let shared = [
        "a.com/1",
        "b.com/1",
        "c.com/1",
        "ornek.com.tr/kahve-makinesi",
    ];
    let transport = FakeTransport::new()
        .reply(200, serper(&shared))
        .reply(200, serper(&shared))
        .reply(200, serper(&["t.com/1", "u.com/1"]));
    workspace
        .fetch_serps_with(id, &SerpRequest::default(), &transport, &quiet())
        .unwrap();
    workspace
        .cluster(
            id,
            &ClusterRequest {
                method: Method::Serp,
                ..ClusterRequest::default()
            },
        )
        .unwrap();
    (dir, workspace, id)
}

#[test]
fn manual_placements_survive_a_new_clustering() {
    let (_dir, workspace, id) = fetched();
    // The tea pot moved to the machines by hand: one cluster, kept on the next clustering.
    let ids = workspace.store().unwrap().key_index(id).unwrap();
    let placed = workspace
        .place_keywords(id, &[ids["cay demligi"]], ids["kahve makinesi"])
        .unwrap();
    assert_eq!(placed.clusters, 1);
    let again = workspace
        .cluster(
            id,
            &ClusterRequest {
                method: Method::Serp,
                ..ClusterRequest::default()
            },
        )
        .unwrap();
    assert_eq!(again.clustering.clusters, 1);
    assert_eq!(again.placed, 1);
    // A split: the espresso machine leads a cluster of its own.
    let split = workspace
        .place_keywords(id, &[ids["espresso makinesi"]], ids["espresso makinesi"])
        .unwrap();
    assert_eq!(split.clusters, 2);
    workspace
        .unplace_keywords(id, &[ids["cay demligi"], ids["espresso makinesi"]])
        .unwrap();
    assert_eq!(workspace.store().unwrap().pins(id).unwrap(), Vec::new());
}

#[test]
fn stops_when_every_provider_fails() {
    let (_dir, workspace, id) = workspace();
    workspace
        .set_key("serper", "api_key", "serper-key-1")
        .unwrap();
    let transport = FakeTransport::new()
        .reply(401, r#"{"message":"Unauthorized."}"#)
        .reply(401, r#"{"message":"Unauthorized."}"#)
        .reply(401, r#"{"message":"Unauthorized."}"#);
    let outcome = workspace
        .fetch_serps_with(id, &SerpRequest::default(), &transport, &quiet())
        .unwrap();
    assert_eq!(outcome.run.status, RunStatus::Failed);
    assert_eq!(outcome.run.found, 0);
    assert_eq!(transport.requests().len(), 3);
    let rows = workspace
        .keywords(id, &KeywordQuery::default())
        .unwrap()
        .rows;
    assert!(rows.iter().all(|row| !row.serp));
}

#[test]
fn free_suggestions_are_off_until_accepted_and_paced_when_on() {
    let (_dir, workspace, id) = workspace();
    let request = DiscoverRequest {
        seeds: vec!["kahve".to_owned()],
        sources: vec![DiscoverySource::GoogleSuggest],
        expansions: vec![octopuson_keywords::Expansion::Commercial],
        ..DiscoverRequest::default()
    };
    let estimate = workspace.estimate_discovery(id, &request).unwrap();
    assert!(
        estimate.sources[0]
            .reason
            .as_deref()
            .unwrap()
            .contains("off")
    );

    let mut settings = workspace.settings().unwrap();
    settings.suggest.google = true;
    settings.suggest.accepted_at = Some("2026-10-18T10:00:00Z".to_owned());
    workspace.save_settings(&settings).unwrap();
    let transport = FakeTransport::new()
        .reply(200, r#"["kahve",["kahve falı","kahve makinesi"]]"#)
        .reply(429, "");
    let pauses = AtomicU32::new(0);
    let control = RunControl {
        cancelled: &|| false,
        progress: &|_| {},
        sleep: &|pause| {
            assert!(pause.as_millis() >= 2_000);
            pauses.fetch_add(1, Ordering::SeqCst);
        },
    };
    let outcome = workspace
        .discover_with(id, &request, &transport, &control)
        .unwrap();
    assert_eq!(outcome.run.status, RunStatus::Partial);
    assert_eq!(outcome.added.added, 1);
    assert_eq!(transport.requests().len(), 2);
    assert!(pauses.load(Ordering::SeqCst) >= 1);
    // Refused today: not asked again until tomorrow.
    let estimate = workspace.estimate_discovery(id, &request).unwrap();
    let reason = estimate.sources[0].reason.clone().unwrap_or_default();
    assert!(reason.contains("tomorrow"), "{reason}");
}

/// Virtual time: sleeping advances it.
#[derive(Clone)]
struct FakeClock {
    start: Instant,
    elapsed_ms: Arc<AtomicU64>,
}

impl Clock for FakeClock {
    fn now(&self) -> Instant {
        self.start + Duration::from_millis(self.elapsed_ms.load(Ordering::SeqCst))
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
    fronted: Arc<AtomicU32>,
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

/// A Google result page as the browser shows it in 2026: encrypted links, sites in `cite`.
fn google_page(sites: &[&str]) -> BrowserView {
    let mut results = String::new();
    for (index, site) in sites.iter().enumerate() {
        let _ = write!(
            results,
            r#"<div class="MjjYud"><div class="tF2Cxc" data-hveid="x"><div class="yuRUbf">
            <a jsname="UWckNb" href="/goto?url=CAES{index}" data-ved="v"><h3>{site} {index}</h3>
            <cite>https://{site} › sayfa</cite></a></div><div class="VwiC3b">Özet</div></div></div>"#
        );
    }
    BrowserView {
        url: "https://www.google.com/search?q=kahve".to_owned(),
        html: format!(r#"<html><body><div id="rso">{results}</div></body></html>"#),
    }
}

fn captcha_page() -> BrowserView {
    BrowserView {
        url: "https://www.google.com/sorry/index?continue=x".to_owned(),
        html: "<html><body><form id='captcha-form'></form></body></html>".to_owned(),
    }
}

/// What the fake window was asked to do.
struct Window {
    opened: Arc<Mutex<Vec<String>>>,
    fronted: Arc<AtomicU32>,
}

/// Puts the browser in the Google chain, with a stand-in executable and a fake window.
fn use_browser(
    dir: &tempfile::TempDir,
    workspace: &KeywordWorkspace,
    daily_cap: u32,
    opens: Vec<BrowserView>,
    currents: Vec<BrowserView>,
) -> Window {
    let executable = dir.path().join("chrome");
    std::fs::write(&executable, "").unwrap();
    let mut settings = workspace.settings().unwrap();
    settings.browser.executable = Some(executable.display().to_string());
    settings.browser.daily_cap = daily_cap;
    settings
        .serp
        .chains
        .insert("google".to_owned(), vec![BROWSER_ID.to_owned()]);
    workspace.save_settings(&settings).unwrap();
    let opened = Arc::new(Mutex::new(Vec::new()));
    let fronted = Arc::new(AtomicU32::new(0));
    let profiles = Engine::ALL
        .into_iter()
        .map(|engine| {
            let profile = builtin(engine).unwrap();
            (engine, (profile.compile().unwrap(), profile.key()))
        })
        .collect();
    let provider = BrowserProvider::new(
        Box::new(FakeWindow {
            opens: opens.into(),
            currents: currents.into(),
            opened: Arc::clone(&opened),
            fronted: Arc::clone(&fronted),
        }),
        profiles,
        BrowserPacing {
            delay: Duration::from_secs(settings.browser.delay_secs),
            ..BrowserPacing::default()
        },
        Box::new(FakeClock {
            start: Instant::now(),
            elapsed_ms: Arc::new(AtomicU64::new(0)),
        }),
    );
    workspace.browser.lock().unwrap().fixed = Some(Arc::new(provider));
    Window { opened, fronted }
}

#[test]
fn the_browser_is_left_out_of_the_chains_until_the_user_adds_it() {
    let (_dir, workspace, id) = workspace();
    let settings = workspace.settings().unwrap();
    assert!(
        settings
            .serp
            .chains
            .values()
            .flatten()
            .all(|provider| provider != BROWSER_ID)
    );
    let estimate = workspace
        .estimate_serps(id, &SerpRequest::default())
        .unwrap();
    assert!(estimate.skipped.iter().all(|(id, _)| id != BROWSER_ID));
    assert_eq!(
        workspace.browser_status().unwrap().engines,
        Vec::<String>::new()
    );

    // Added, but no Chrome or Edge at the given path.
    let mut settings = workspace.settings().unwrap();
    settings.browser.executable = Some("/nowhere/chrome".to_owned());
    settings
        .serp
        .chains
        .get_mut("google")
        .unwrap()
        .push(BROWSER_ID.to_owned());
    workspace.save_settings(&settings).unwrap();
    let estimate = workspace
        .estimate_serps(id, &SerpRequest::default())
        .unwrap();
    assert!(estimate.skipped.contains(&(
        BROWSER_ID.to_owned(),
        "Chrome or Edge was not found".to_owned()
    )));
    let status = workspace.browser_status().unwrap();
    assert_eq!(status.engines, ["google"]);
    assert_eq!(status.executable, None);
    // Mobile results are not read in the browser.
    let mobile = SerpRequest {
        device: "mobile".to_owned(),
        ..SerpRequest::default()
    };
    let estimate = workspace.estimate_serps(id, &mobile).unwrap();
    assert!(
        estimate
            .skipped
            .iter()
            .any(|(id, reason)| id == BROWSER_ID && reason.contains("mobile"))
    );
}

#[test]
fn reads_google_in_the_browser_as_sites_and_waits_for_check_pages() {
    let (dir, workspace, id) = workspace();
    let window = use_browser(
        &dir,
        &workspace,
        100,
        vec![
            google_page(&["a.com", "b.com", "c.com", "ornek.com.tr"]),
            captcha_page(),
            google_page(&["t.com", "u.com"]),
        ],
        // The person solves the CAPTCHA; the window shows the results.
        vec![
            captcha_page(),
            google_page(&["a.com", "b.com", "c.com", "ornek.com.tr"]),
        ],
    );
    let estimate = workspace
        .estimate_serps(id, &SerpRequest::default())
        .unwrap();
    assert_eq!(estimate.provider.as_deref(), Some(BROWSER_ID));
    assert!(estimate.cost_usd.abs() < f64::EPSILON);

    let seen: Mutex<Vec<Progress>> = Mutex::new(Vec::new());
    let control = RunControl {
        cancelled: &|| false,
        progress: &|progress| seen.lock().unwrap().push(progress.clone()),
        sleep: &|_| {},
    };
    let outcome = workspace
        .fetch_serps_with(id, &SerpRequest::default(), &FakeTransport::new(), &control)
        .unwrap();
    assert_eq!(outcome.run.status, RunStatus::Done);
    assert_eq!(outcome.run.found, 3);
    assert_eq!(window.opened.lock().unwrap().len(), 3);
    assert_eq!(window.fronted.load(Ordering::SeqCst), 1);
    // The wait was reported, then cleared.
    let seen = seen.into_inner().unwrap();
    let waits: Vec<&CheckPage> = seen.iter().filter_map(|p| p.check.as_ref()).collect();
    assert_eq!(waits.len(), 1);
    assert_eq!(waits[0].kind, ChallengeKind::Captcha);
    assert!(waits[0].url.contains("google."));
    assert!(seen.last().unwrap().check.is_none());

    // Google hides the addresses: sites only, so mapping leaves them out and clustering
    // compares sites.
    let store = workspace.store().unwrap();
    let machine = store.key_index(id).unwrap()["kahve makinesi"];
    let page = store.serp(machine, "google", "desktop").unwrap().unwrap();
    assert_eq!(page.provider, BROWSER_ID);
    assert!(page.site_only);
    assert_eq!(page.urls[0], "https://a.com/");
    drop(store);
    assert_eq!(outcome.site_pages, 0);
    let clustered = workspace
        .cluster(
            id,
            &ClusterRequest {
                method: Method::Serp,
                ..ClusterRequest::default()
            },
        )
        .unwrap();
    assert_eq!(clustered.level, Level::Site);
    assert_eq!(clustered.clustering.clusters, 2);

    let status = workspace.browser_status().unwrap();
    assert_eq!(status.used_today["google"], 3);
    assert_eq!(status.used_today["bing"], 0);
}

#[test]
fn the_daily_limit_of_the_browser_counts_pages_per_engine() {
    let (dir, workspace, id) = workspace();
    let window = use_browser(
        &dir,
        &workspace,
        2,
        vec![
            google_page(&["a.com", "b.com"]),
            google_page(&["a.com", "c.com"]),
            google_page(&["d.com"]),
        ],
        Vec::new(),
    );
    let outcome = workspace
        .fetch_serps_with(id, &SerpRequest::default(), &FakeTransport::new(), &quiet())
        .unwrap();
    assert_eq!(outcome.run.status, RunStatus::Partial);
    assert_eq!(outcome.run.found, 2);
    assert_eq!(window.opened.lock().unwrap().len(), 2);
    assert!(
        outcome.run.failures[0]
            .error
            .contains("daily limit of 2 google pages"),
        "{:?}",
        outcome.run.failures
    );
    let estimate = workspace
        .estimate_serps(id, &SerpRequest::default())
        .unwrap();
    assert_eq!(estimate.provider, None);
}

#[test]
fn a_check_page_nobody_clears_stops_the_run() {
    let (dir, workspace, id) = workspace();
    let window = use_browser(&dir, &workspace, 100, vec![captcha_page()], Vec::new());
    let outcome = workspace
        .fetch_serps_with(id, &SerpRequest::default(), &FakeTransport::new(), &quiet())
        .unwrap();
    assert_eq!(outcome.run.status, RunStatus::Failed);
    assert_eq!(outcome.run.found, 0);
    // Waited ten minutes on the first keyword, then stopped instead of asking again.
    assert_eq!(window.opened.lock().unwrap().len(), 1);
    assert_eq!(window.fronted.load(Ordering::SeqCst), 1);
    assert!(outcome.run.failures[0].error.contains("check"));
    let attempts = workspace
        .store()
        .unwrap()
        .engine_requests_on(BROWSER_ID, "google", &crate::now()[..10])
        .unwrap();
    assert_eq!(attempts, 1);
}
