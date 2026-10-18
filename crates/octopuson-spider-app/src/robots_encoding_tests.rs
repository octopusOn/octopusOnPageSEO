//! Shared REP normalization must govern HTTP pages, browser policy and saved evidence.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use super::*;
use octopuson_crawl::{
    FetchError, FetchRequest, FetchResponse, Fetcher, RenderRequest, RenderedPage, Renderer,
};

const RULES: &str = "\u{feff}User-agent: *\rDisallow: /özel/\rAllow: /özel/açık$\rDisallow: /~secret/\rDisallow: /encoded%2fpart$\rDisallow: /caf%C3%A9$\rAllow: /café$\rDisallow: /search?q=özel\r";
const BLOCKED: [&str; 4] = [
    "/%C3%B6zel/gizli",
    "/%7Esecret/data",
    "/encoded%2Fpart",
    "/search?q=%C3%B6zel",
];
const ALLOWED: [&str; 3] = ["/%C3%B6zel/a%C3%A7%C4%B1k", "/encoded/part", "/caf%C3%A9"];

#[derive(Default)]
struct Site(Mutex<Vec<String>>);
impl Fetcher for Site {
    fn fetch(&self, request: &FetchRequest<'_>) -> Result<FetchResponse, FetchError> {
        self.0.lock().unwrap().push(request.url.into());
        if request.url == "https://example.com/robots.txt" {
            return Ok(FetchResponse::new(
                200,
                &[("Content-Type", "text/plain")],
                RULES,
            ));
        }
        let links = if request.url == "https://example.com/" {
            BLOCKED
                .iter()
                .chain(ALLOWED.iter())
                .flat_map(|&path| ["<a href='", path, "'>Link</a>"])
                .collect::<String>()
        } else {
            String::new()
        };
        Ok(FetchResponse::new(
            200,
            &[("Content-Type", "text/html")],
            format!("<title>Observed</title><h1>Page</h1>{links}"),
        ))
    }
}

struct Script;
impl Renderer for Script {
    fn render(&self, request: &RenderRequest<'_>) -> Result<RenderedPage, FetchError> {
        let policy = request.request_allowed.unwrap();
        let deadline = Instant::now() + Duration::from_secs(1);
        assert!(!policy(
            "https://example.com/özel/asset.js",
            octopuson_crawl::BrowserRequest::Resource,
            deadline
        ));
        assert!(!policy(
            "https://example.com/%C3%B6zel/asset.js",
            octopuson_crawl::BrowserRequest::Resource,
            deadline
        ));
        assert!(policy(
            "https://example.com/%C3%B6zel/a%C3%A7%C4%B1k",
            octopuson_crawl::BrowserRequest::Resource,
            deadline
        ));
        assert!(policy(
            "https://example.com/encoded/part",
            octopuson_crawl::BrowserRequest::Resource,
            deadline
        ));
        Ok(RenderedPage {
            html: String::from_utf8(request.response.body.clone()).unwrap(),
            url: request.url.into(),
            elapsed: Duration::ZERO,
            truncated: false,
        })
    }
}

fn verify(mode: RenderMode, policy: RobotsPolicy) {
    let dir = tempfile::tempdir().unwrap();
    let workspace = SpiderWorkspace::resolve(Some(dir.path().into())).unwrap();
    let mut config = CrawlConfig {
        start_urls: vec!["https://example.com/".into()],
        robots: policy,
        ..CrawlConfig::default()
    };
    config.rendering.mode = mode;
    config.follow.sitemaps = false;
    let site = Site::default();
    let meta = workspace
        .crawl_with_renderer(
            &CrawlPlan::new(config).unwrap(),
            &site,
            Some(&Script),
            &CrawlControl::new(),
            |_| {},
        )
        .unwrap();
    assert_eq!(meta.status, CrawlStatus::Completed);
    assert_eq!(meta.progress.crawled, 8);
    assert_eq!(meta.progress.errors, 0);
    let out = dir.path().join("internal.json");
    workspace
        .export(&meta.id, CrawlTab::Internal, &out, None)
        .unwrap();
    let rows: serde_json::Value = serde_json::from_slice(&std::fs::read(out).unwrap()).unwrap();
    let rows = rows.as_array().unwrap();
    assert_eq!(rows.len(), 8);
    for path in BLOCKED {
        let url = format!("https://example.com{path}");
        let row = rows.iter().find(|row| row["address"] == url).unwrap();
        assert_eq!(row["blocked_by_robots"], true);
        assert_eq!(row["error"], serde_json::Value::Null);
        assert_eq!(
            row["status_code"],
            if policy == RobotsPolicy::Respect {
                serde_json::Value::Null
            } else {
                serde_json::json!(200)
            }
        );
        let rule = row["robots_rule"].as_str().unwrap();
        assert!(RULES.contains(rule.strip_prefix("Disallow: ").unwrap()));
        let calls = site
            .0
            .lock()
            .unwrap()
            .iter()
            .filter(|request| *request == &url)
            .count();
        assert_eq!(calls, usize::from(policy != RobotsPolicy::Respect));
    }
    for path in ALLOWED {
        let url = format!("https://example.com{path}");
        let row = rows.iter().find(|row| row["address"] == url).unwrap();
        assert_eq!(row["blocked_by_robots"], false);
        assert_eq!(row["status_code"], 200);
    }
    assert_eq!(
        site.0
            .lock()
            .unwrap()
            .iter()
            .filter(|url| url.ends_with("/robots.txt"))
            .count(),
        1
    );
    let reader = workspace.open(&meta.id).unwrap();
    assert_eq!(
        reader
            .page(&PageRequest {
                tab: "internal".into(),
                issue_id: "internal-blocked-robots".into(),
                ..PageRequest::default()
            })
            .unwrap()
            .total,
        4
    );
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
    let robots = dir.path().join("robots.json");
    workspace
        .export(&meta.id, CrawlTab::Robots, &robots, None)
        .unwrap();
    let robots: serde_json::Value =
        serde_json::from_slice(&std::fs::read(robots).unwrap()).unwrap();
    assert_eq!(robots[0]["text"], RULES);
}

#[test]
fn html_respects_encoded_robots_rules_and_preserves_original_evidence() {
    verify(RenderMode::Html, RobotsPolicy::Respect);
}

#[test]
fn javascript_policy_obeys_the_same_robots_decisions_as_http_pages() {
    verify(RenderMode::Javascript, RobotsPolicy::Respect);
}

#[test]
fn ignore_but_report_still_fetches_blocked_pages_and_retains_their_rules() {
    verify(RenderMode::Html, RobotsPolicy::IgnoreButReport);
}
