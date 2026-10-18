//! Ignored robots extensions cannot change a shared group's HTTP/browser verdicts.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use super::*;
use seonexus_crawl::{
    FetchError, FetchRequest, FetchResponse, Fetcher, RenderRequest, RenderedPage, Renderer,
};

const RULES: &str = "User-agent: Googlebot\nCrawl-delay: 10\nHost: example.com\nUser-agent: Bingbot\nX-custom: ignored\nDisallow: /private/\nAllow: /private/public/\nCrawl-delay: 5\nUser-agent: GPTBot\nRequest-rate: 1/5\nUser-agent: CCBot\nDisallow: /\nSitemap: https://example.com/map.xml\nUser-agent: *\nDisallow: /fallback/\n";
const PATHS: [&str; 4] = [
    "/private/page",
    "/private/public/page",
    "/fallback/page",
    "/free",
];

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
            PATHS
                .iter()
                .flat_map(|&path| ["<a href='", path, "'>Link</a>"])
                .collect::<String>()
        } else {
            String::new()
        };
        Ok(FetchResponse::new(
            200,
            &[("Content-Type", "text/html")],
            format!("<title>Grouped robots</title><h1>Page</h1>{links}"),
        ))
    }
}

struct Script;
impl Renderer for Script {
    fn render(&self, request: &RenderRequest<'_>) -> Result<RenderedPage, FetchError> {
        let policy = request.request_allowed.unwrap();
        let deadline = Instant::now() + Duration::from_secs(1);
        assert!(!policy(
            "https://example.com/private/asset.js",
            seonexus_crawl::BrowserRequest::Resource,
            deadline
        ));
        assert!(policy(
            "https://example.com/private/public/asset.js",
            seonexus_crawl::BrowserRequest::Resource,
            deadline
        ));
        assert!(policy(
            "https://example.com/fallback/asset.js",
            seonexus_crawl::BrowserRequest::Resource,
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

fn exported(
    workspace: &SpiderWorkspace,
    id: &str,
    tab: CrawlTab,
    path: &Path,
) -> Vec<serde_json::Value> {
    workspace.export(id, tab, path, None).unwrap();
    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
}

fn verify(mode: RenderMode, policy: RobotsPolicy, agent: AgentPreset) {
    let dir = tempfile::tempdir().unwrap();
    let workspace = SpiderWorkspace::resolve(Some(dir.path().into())).unwrap();
    let mut config = CrawlConfig {
        start_urls: vec!["https://example.com/".into()],
        agent,
        robots: policy,
        ..CrawlConfig::default()
    };
    config.rendering.mode = mode;
    config.follow.sitemaps = false;
    config.storage.html = false;
    config.storage.headers = false;
    config.speed.max_requests_per_second = 0.0;
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
    assert_eq!(meta.progress.crawled, 5);
    assert_eq!(meta.progress.errors, 0);
    let rows = exported(
        &workspace,
        &meta.id,
        CrawlTab::Internal,
        &dir.path().join("internal.json"),
    );
    assert_eq!(rows.len(), 5);
    for path in PATHS.into_iter().chain(["/"]) {
        let url = format!("https://example.com{path}");
        let row = rows.iter().find(|row| row["address"] == url).unwrap();
        let blocked = path == "/private/page";
        let fetched = !blocked || policy == RobotsPolicy::IgnoreButReport;
        assert_eq!(row["blocked_by_robots"], blocked, "{agent:?} {path}");
        assert_eq!(row["error"], serde_json::Value::Null);
        assert_eq!(
            row["status_code"],
            if fetched {
                serde_json::json!(200)
            } else {
                serde_json::Value::Null
            }
        );
        if blocked {
            assert_eq!(row["robots_rule"], "Disallow: /private/");
        }
        assert_eq!(
            site.0
                .lock()
                .unwrap()
                .iter()
                .filter(|request| *request == &url)
                .count(),
            usize::from(fetched)
        );
    }
    assert_eq!(
        site.0
            .lock()
            .unwrap()
            .iter()
            .filter(|request| request.ends_with("/robots.txt"))
            .count(),
        1
    );
    verify_saved_evidence(&workspace, &meta.id, dir.path());
}

fn verify_saved_evidence(workspace: &SpiderWorkspace, id: &str, directory: &Path) {
    let robots = exported(
        workspace,
        id,
        CrawlTab::Robots,
        &directory.join("robots.json"),
    );
    assert_eq!(robots.len(), 1);
    assert_eq!(robots[0]["text"], RULES);
    assert!(
        robots[0]["sitemaps"]
            .as_str()
            .unwrap()
            .contains("https://example.com/map.xml")
    );
    let issues = exported(
        workspace,
        id,
        CrawlTab::Issues,
        &directory.join("issues.json"),
    );
    let blocked: Vec<_> = issues
        .iter()
        .filter(|row| row["issue_id"] == "internal-blocked-robots")
        .collect();
    assert_eq!(blocked.len(), 1);
    assert_eq!(blocked[0]["address"], "https://example.com/private/page");
    assert!(
        !issues
            .iter()
            .any(|row| row["issue_id"] == "internal-no-response"
                || row["issue_id"] == "render-error")
    );
}

#[test]
fn html_shared_groups_keep_rules_for_both_googlebot_and_bingbot() {
    for agent in [AgentPreset::GooglebotDesktop, AgentPreset::Bingbot] {
        verify(RenderMode::Html, RobotsPolicy::Respect, agent);
    }
}

#[test]
fn javascript_policy_keeps_shared_rules_and_allow_exceptions() {
    for agent in [AgentPreset::GooglebotDesktop, AgentPreset::Bingbot] {
        verify(RenderMode::Javascript, RobotsPolicy::Respect, agent);
    }
}

#[test]
fn ignore_but_report_fetches_the_blocked_page_and_keeps_original_evidence() {
    for agent in [AgentPreset::GooglebotDesktop, AgentPreset::Bingbot] {
        verify(RenderMode::Html, RobotsPolicy::IgnoreButReport, agent);
    }
}
