//! Rate-limited REP observations must not become crawl permission or fake page failures.

use std::sync::{
    Mutex,
    atomic::{AtomicUsize, Ordering},
};
use std::time::{Duration, Instant};

use super::*;
use octopuson_crawl::{
    FetchError, FetchRequest, FetchResponse, Fetcher, RenderRequest, RenderedPage, Renderer,
};
use octopuson_crawl_store::ResponseGroup;

const ORIGIN: &str = "https://example.com";
const HEALTHY: &str = "https://example.com:8443";

struct Site {
    status: u16,
    redirect: bool,
    calls: Mutex<Vec<String>>,
}
impl Fetcher for Site {
    fn fetch(&self, request: &FetchRequest<'_>) -> Result<FetchResponse, FetchError> {
        self.calls.lock().unwrap().push(request.url.into());
        let healthy = request.url.starts_with(HEALTHY);
        if request.url.ends_with("/robots.txt") && self.redirect && !healthy {
            return Ok(FetchResponse::new(
                302,
                &[("Location", "/limited-rules")],
                "",
            ));
        }
        if request.url.ends_with("/robots.txt") || request.url.ends_with("/limited-rules") {
            return Ok(FetchResponse::new(
                if healthy { 200 } else { self.status },
                &[("Retry-After", "1")],
                if healthy {
                    "User-agent: *\nAllow: /\n"
                } else {
                    "User-agent: *\nDisallow: /\nSitemap: https://example.com/unused.xml\n"
                },
            ));
        }
        Ok(FetchResponse::new(
            200,
            &[("Content-Type", "text/html")],
            "<title>Observed page</title><h1>Observed</h1>",
        ))
    }
}

struct Script {
    guard: bool,
    calls: AtomicUsize,
}
impl Renderer for Script {
    fn render(&self, request: &RenderRequest<'_>) -> Result<RenderedPage, FetchError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if self.guard {
            assert!(
                request.url.starts_with(HEALTHY),
                "the limited page must not render"
            );
            let policy = request.request_allowed.unwrap();
            let deadline = Instant::now() + Duration::from_secs(1);
            assert!(!policy(
                "https://example.com/asset.js",
                octopuson_crawl::BrowserRequest::Resource,
                deadline
            ));
            assert!(policy(
                "https://example.com:8443/asset.js",
                octopuson_crawl::BrowserRequest::Resource,
                deadline
            ));
        }
        Ok(RenderedPage {
            html: "<title>Rendered page</title><h1>Observed</h1>".into(),
            url: request.url.into(),
            elapsed: Duration::ZERO,
            truncated: false,
        })
    }
}

fn exported(
    workspace: &SpiderWorkspace,
    id: &str,
    directory: &Path,
    tab: CrawlTab,
) -> Vec<serde_json::Value> {
    let path = directory.join(format!("{}.json", tab.name()));
    workspace.export(id, tab, &path, None).unwrap();
    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
}

fn issue_total(reader: &CrawlReader, tab: &str, issue: &str) -> u64 {
    reader
        .page(&PageRequest {
            tab: tab.into(),
            issue_id: issue.into(),
            ..PageRequest::default()
        })
        .unwrap()
        .total
}

fn verify(status: u16, policy: RobotsPolicy, mode: RenderMode, redirect: bool) {
    let dir = tempfile::tempdir().unwrap();
    let workspace = SpiderWorkspace::resolve(Some(dir.path().into())).unwrap();
    let mut config = CrawlConfig {
        start_urls: vec![format!("{ORIGIN}/one"), format!("{ORIGIN}/two")],
        mode: CrawlMode::List,
        robots: policy,
        ..CrawlConfig::default()
    };
    config.follow.sitemaps = false;
    config.rendering.mode = mode;
    config.speed.threads = 2;
    let site = Site {
        status,
        redirect,
        calls: Mutex::new(Vec::new()),
    };
    let script = Script {
        guard: false,
        calls: AtomicUsize::new(0),
    };
    let meta = workspace
        .crawl_with_renderer(
            &CrawlPlan::new(config).unwrap(),
            &site,
            Some(&script),
            &CrawlControl::new(),
            |_| {},
        )
        .unwrap();
    assert_eq!(meta.status, CrawlStatus::Completed);
    assert_eq!(meta.progress.crawled, 2);
    assert_eq!(meta.progress.errors, 0);
    let limited = status == 429 && policy != RobotsPolicy::Ignore;
    let fetched = !(limited && policy == RobotsPolicy::Respect);
    let reader = workspace.open(&meta.id).unwrap();
    verify_overview(&reader, limited, fetched, status, policy);
    verify_rows(
        &workspace,
        &meta.id,
        dir.path(),
        limited,
        fetched,
        status,
        policy,
    );
    assert_eq!(
        script.calls.load(Ordering::SeqCst),
        if fetched && mode == RenderMode::Javascript {
            2
        } else {
            0
        }
    );
    let calls = site.calls.lock().unwrap();
    assert_eq!(
        calls
            .iter()
            .filter(|url| url.ends_with("/robots.txt"))
            .count(),
        usize::from(policy != RobotsPolicy::Ignore)
    );
    assert_eq!(
        calls
            .iter()
            .filter(|url| url.ends_with("/limited-rules"))
            .count(),
        usize::from(redirect && policy != RobotsPolicy::Ignore)
    );
    for path in ["/one", "/two"] {
        assert_eq!(
            calls
                .iter()
                .filter(|url| *url == &format!("{ORIGIN}{path}"))
                .count(),
            usize::from(fetched)
        );
    }
}

fn verify_overview(
    reader: &CrawlReader,
    limited: bool,
    fetched: bool,
    status: u16,
    policy: RobotsPolicy,
) {
    let overview = reader.overview().unwrap();
    assert_eq!(overview.internal, 2);
    assert_eq!(overview.errors, 0);
    assert_eq!(overview.blocked, if fetched { 0 } else { 2 });
    assert_eq!(
        overview
            .responses
            .iter()
            .map(|group| group.count)
            .sum::<u64>(),
        2
    );
    assert_eq!(
        overview
            .responses
            .iter()
            .find(|row| row.group
                == if fetched {
                    ResponseGroup::Success
                } else {
                    ResponseGroup::RobotsBlocked
                })
            .unwrap()
            .count,
        2
    );
    assert_eq!(issue_total(reader, "internal", "internal-no-response"), 0);
    assert_eq!(issue_total(reader, "internal", "internal-client-error"), 0);
    assert_eq!(
        issue_total(reader, "internal", "internal-blocked-robots"),
        if limited { 2 } else { 0 }
    );
    assert_eq!(
        issue_total(reader, "robots", "robots-txt-unreachable"),
        u64::from(limited)
    );
    assert_eq!(
        issue_total(reader, "robots", "robots-txt-missing"),
        u64::from(status == 404 && policy != RobotsPolicy::Ignore)
    );
}

fn verify_rows(
    workspace: &SpiderWorkspace,
    id: &str,
    directory: &Path,
    limited: bool,
    fetched: bool,
    status: u16,
    policy: RobotsPolicy,
) {
    for row in exported(workspace, id, directory, CrawlTab::Internal) {
        assert_eq!(row["blocked_by_robots"], limited);
        assert_eq!(
            row["status_code"],
            if fetched {
                serde_json::json!(200)
            } else {
                serde_json::Value::Null
            }
        );
        assert!(row["error"].is_null());
        if limited {
            assert_eq!(row["robots_rule"], "robots.txt rate limited (HTTP 429)");
        } else {
            assert!(row["robots_rule"].is_null());
        }
    }
    let records = exported(workspace, id, directory, CrawlTab::Robots);
    if policy == RobotsPolicy::Ignore {
        assert_eq!(records, [] as [serde_json::Value; 0]);
    } else {
        assert_eq!(records.len(), 1);
        assert_eq!(records[0]["http_status"], status);
        assert_eq!(
            records[0]["status"],
            if limited { "unreachable" } else { "missing" }
        );
        assert_eq!(records[0]["text"], "");
        assert_eq!(records[0]["sitemaps"], "");
    }
}

#[test]
fn html_429_blocks_unsent_pages_without_inventing_http_failures() {
    verify(429, RobotsPolicy::Respect, RenderMode::Html, false);
}
#[test]
fn javascript_429_pages_are_not_fetched_or_rendered() {
    verify(429, RobotsPolicy::Respect, RenderMode::Javascript, false);
}
#[test]
fn ignore_but_report_fetches_and_flags_rate_limited_pages() {
    verify(429, RobotsPolicy::IgnoreButReport, RenderMode::Html, false);
}
#[test]
fn ignore_policy_skips_robots_checks_and_does_not_invent_blocks() {
    verify(429, RobotsPolicy::Ignore, RenderMode::Html, false);
}
#[test]
fn a_redirect_to_429_keeps_the_rate_limit_diagnostic() {
    verify(429, RobotsPolicy::Respect, RenderMode::Html, true);
}
#[test]
fn genuine_404_robots_remains_missing_and_allows_pages() {
    verify(404, RobotsPolicy::Respect, RenderMode::Html, false);
}

#[test]
fn browser_resource_policy_uses_the_same_cached_origin_rate_limit() {
    let dir = tempfile::tempdir().unwrap();
    let workspace = SpiderWorkspace::resolve(Some(dir.path().into())).unwrap();
    let mut config = CrawlConfig {
        start_urls: vec![format!("{ORIGIN}/one"), format!("{HEALTHY}/one")],
        mode: CrawlMode::List,
        robots: RobotsPolicy::Respect,
        ..CrawlConfig::default()
    };
    config.follow.sitemaps = false;
    config.speed.threads = 1;
    config.rendering.mode = RenderMode::Javascript;
    let site = Site {
        status: 429,
        redirect: false,
        calls: Mutex::new(Vec::new()),
    };
    let script = Script {
        guard: true,
        calls: AtomicUsize::new(0),
    };
    let meta = workspace
        .crawl_with_renderer(
            &CrawlPlan::new(config).unwrap(),
            &site,
            Some(&script),
            &CrawlControl::new(),
            |_| {},
        )
        .unwrap();
    assert_eq!(meta.progress.crawled, 2);
    assert_eq!(meta.progress.errors, 0);
    assert_eq!(script.calls.load(Ordering::SeqCst), 1);
    let reader = workspace.open(&meta.id).unwrap();
    assert_eq!(reader.overview().unwrap().blocked, 1);
    assert_eq!(issue_total(&reader, "robots", "robots-txt-unreachable"), 1);
    assert_eq!(issue_total(&reader, "internal", "internal-no-response"), 0);
    let robots = exported(&workspace, &meta.id, dir.path(), CrawlTab::Robots);
    assert_eq!(robots.len(), 2);
    assert!(
        robots
            .iter()
            .any(|row| row["http_status"] == 429 && row["status"] == "unreachable")
    );
    assert!(
        robots
            .iter()
            .any(|row| row["http_status"] == 200 && row["status"] == "found")
    );
    assert_eq!(
        site.calls
            .lock()
            .unwrap()
            .iter()
            .filter(|url| url.ends_with("/robots.txt"))
            .count(),
        2
    );
}

#[test]
fn browser_only_429_observations_reach_the_robots_report_without_fake_page_rows() {
    verify_browser_only_record(false);
}

#[test]
fn stopping_keeps_robots_responses_first_observed_by_browser_policy() {
    verify_browser_only_record(true);
}

fn verify_browser_only_record(stopped: bool) {
    let dir = tempfile::tempdir().unwrap();
    let workspace = SpiderWorkspace::resolve(Some(dir.path().into())).unwrap();
    let mut config = CrawlConfig {
        start_urls: vec![format!("{HEALTHY}/one")],
        mode: CrawlMode::List,
        robots: RobotsPolicy::Respect,
        ..CrawlConfig::default()
    };
    config.follow.sitemaps = false;
    config.rendering.mode = RenderMode::Javascript;
    let site = Site {
        status: 429,
        redirect: false,
        calls: Mutex::new(Vec::new()),
    };
    let script = Script {
        guard: true,
        calls: AtomicUsize::new(0),
    };
    let control = CrawlControl::new();
    let meta = workspace
        .crawl_with_renderer(
            &CrawlPlan::new(config).unwrap(),
            &site,
            Some(&script),
            &control,
            |progress| {
                if stopped && progress.crawled == 1 {
                    control.stop();
                }
            },
        )
        .unwrap();
    assert_eq!(
        meta.status,
        if stopped {
            CrawlStatus::Stopped
        } else {
            CrawlStatus::Completed
        }
    );
    let reader = workspace.open(&meta.id).unwrap();
    assert_eq!(meta.progress.crawled, 1);
    assert_eq!(meta.progress.errors, 0);
    assert_eq!(reader.overview().unwrap().internal, 1);
    assert_eq!(reader.overview().unwrap().blocked, 0);
    assert_eq!(issue_total(&reader, "robots", "robots-txt-unreachable"), 1);
    assert_eq!(issue_total(&reader, "internal", "internal-no-response"), 0);
    let robots = exported(&workspace, &meta.id, dir.path(), CrawlTab::Robots);
    assert_eq!(robots.len(), 2);
    assert!(
        robots
            .iter()
            .any(|row| row["url"] == "https://example.com/robots.txt" && row["http_status"] == 429)
    );
    assert_eq!(
        site.calls
            .lock()
            .unwrap()
            .iter()
            .filter(|url| url.ends_with("/robots.txt"))
            .count(),
        2
    );
}
