//! Stopping while HTTP work waits must save observations without inventing request failures.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use super::*;
use octopuson_crawl::{
    ActivityState, FetchError, FetchRequest, FetchResponse, Fetcher, RenderRequest, RenderedPage,
    Renderer,
};

const PAGE: &str = "<head><title>Observed page</title><link rel='stylesheet' href='/site.css'></head><body><h1>Observed</h1><a href='/child'>Child</a><img src='/image.png'><script src='/site.js'></script></body>";

#[derive(Default)]
struct Site(Mutex<Vec<String>>);
impl Fetcher for Site {
    fn fetch(&self, request: &FetchRequest<'_>) -> Result<FetchResponse, FetchError> {
        self.0.lock().unwrap().push(request.url.into());
        Ok(if request.url == "https://example.com/" {
            FetchResponse::new(200, &[("Content-Type", "text/html")], PAGE)
        } else {
            FetchResponse::new(200, &[("Content-Type", "text/plain")], "fixture")
        })
    }
}
struct Script;
impl Renderer for Script {
    fn render(&self, request: &RenderRequest<'_>) -> Result<RenderedPage, FetchError> {
        Ok(RenderedPage {
            html: String::from_utf8(request.response.body.clone()).unwrap(),
            url: request.url.into(),
            elapsed: Duration::ZERO,
            truncated: false,
        })
    }
}

fn verify(mode: RenderMode, rate: f64, crawl_mode: CrawlMode) {
    let dir = tempfile::tempdir().unwrap();
    let workspace = SpiderWorkspace::resolve(Some(dir.path().into())).unwrap();
    let mut config = CrawlConfig {
        start_urls: vec!["https://example.com/".into()],
        robots: RobotsPolicy::Ignore,
        mode: crawl_mode,
        ..CrawlConfig::default()
    };
    if crawl_mode == CrawlMode::List {
        config.start_urls.push("https://example.com/child".into());
        config.speed.threads = 1;
    }
    config.follow.sitemaps = false;
    config.speed.max_requests_per_second = rate;
    config.rendering.mode = mode;
    let plan = CrawlPlan::new(config).unwrap();
    let control = CrawlControl::new();
    let site = Site::default();
    let mut stopped_at = None;
    let mut finalizing_delay = None;
    let mut activity = octopuson_crawl::CrawlActivity::default();
    let meta = workspace
        .crawl_observed_with(&plan, &site, Some(&Script), &control, |event| match event {
            CrawlEvent::Progress(progress) if progress.crawled >= 1 && !control.is_stopped() => {
                stopped_at = Some(Instant::now());
                control.stop();
            }
            CrawlEvent::Activity(snapshot) => activity = (*snapshot).clone(),
            CrawlEvent::Phase(CrawlPhase::Finalizing) => {
                finalizing_delay = stopped_at.map(|start| start.elapsed());
            }
            _ => {}
        })
        .unwrap();
    assert_eq!(meta.status, CrawlStatus::Stopped);
    assert_eq!(meta.progress.crawled, 1);
    assert_eq!(meta.progress.errors, 0);
    assert!(!meta.limit_reached);
    assert!(finalizing_delay.unwrap() < Duration::from_secs(1));
    assert_eq!(activity.active, Vec::new());
    assert!(
        activity
            .recent
            .iter()
            .any(|entry| entry.state == ActivityState::Cancelled)
    );
    assert_eq!(activity.request_errors, 0);
    assert_eq!(activity.render_errors, 0);
    assert!(site.0.lock().unwrap().iter().all(|url| matches!(
        url.as_str(),
        "https://example.com/" | "https://example.com/llms.txt"
    )));
    let reader = workspace.open(&meta.id).unwrap();
    assert_eq!(reader.overview().unwrap().internal, 1);
    assert_eq!(
        reader
            .page(&PageRequest {
                tab: "internal".into(),
                issue_id: "internal-no-response".into(),
                ..PageRequest::default()
            })
            .unwrap()
            .total,
        0
    );
    let path = dir.path().join("urls.json");
    workspace
        .export(&meta.id, CrawlTab::Internal, &path, None)
        .unwrap();
    let exported: serde_json::Value =
        serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    assert_eq!(exported.as_array().unwrap().len(), 1);
    assert_eq!(exported[0]["status_code"], 200);
    assert_eq!(exported[0]["title"], "Observed page");
}

#[test]
fn stopping_a_throttled_html_crawl_saves_only_observed_responses() {
    verify(RenderMode::Html, 0.5, CrawlMode::Spider);
}
#[test]
fn stopping_a_throttled_js_crawl_preserves_rendered_results_without_false_http_errors() {
    verify(RenderMode::Javascript, 0.5, CrawlMode::Spider);
}

#[test]
fn a_subnormal_rate_can_be_stopped_after_saving_the_first_html_response() {
    verify(RenderMode::Html, f64::from_bits(1), CrawlMode::List);
}

#[test]
fn an_extremely_small_rate_can_be_stopped_after_saving_the_first_rendered_response() {
    verify(RenderMode::Javascript, 1e-300, CrawlMode::List);
}

#[test]
fn invalid_speed_is_rejected_before_browser_startup_or_archive_creation() {
    let dir = tempfile::tempdir().unwrap();
    let workspace = SpiderWorkspace::resolve(Some(dir.path().into())).unwrap();
    for (threads, rate, expected) in [
        (0, 1.0, ConfigError::InvalidConcurrency),
        (65, 1.0, ConfigError::InvalidConcurrency),
        (5, -1.0, ConfigError::InvalidRequestRate),
        (5, f64::NAN, ConfigError::InvalidRequestRate),
        (5, f64::INFINITY, ConfigError::InvalidRequestRate),
    ] {
        let mut config = CrawlConfig {
            start_urls: vec!["https://example.com/".into()],
            ..CrawlConfig::default()
        };
        config.speed.threads = threads;
        config.speed.max_requests_per_second = rate;
        config.rendering.mode = RenderMode::Javascript;
        config.rendering.browser_path = Some("/missing-browser-must-not-launch".into());
        let mut phases = Vec::new();
        let result = workspace.crawl_observed(config, &CrawlControl::new(), |event| {
            if let CrawlEvent::Phase(phase) = event {
                phases.push(*phase);
            } else {
                panic!("invalid settings must not create a crawl or dispatch work");
            }
        });
        assert!(matches!(result, Err(SpiderError::Config(actual)) if actual == expected));
        assert_eq!(phases, [CrawlPhase::Starting]);
        assert_eq!(workspace.crawls().unwrap(), Vec::<CrawlMeta>::new());
    }
}
