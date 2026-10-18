//! Non-web references retain their evidence without inventing HTTP observations.

use std::fmt::Write as _;
use std::sync::Mutex;
use std::time::Duration;

use super::*;
use seonexus_crawl::{
    FetchError, FetchRequest, FetchResponse, Fetcher, RenderRequest, RenderedPage, Renderer,
};

const TARGETS: [&str; 7] = [
    "ftp://example.com/from-redirect",
    "ws://example.com/from-redirect",
    "wss://other.example/from-redirect",
    "file://example.com/from-redirect",
    "custom://example.com/from-redirect",
    "mailto:hello@example.com",
    "data:text/plain,redirect",
];
const EXTERNAL: &str = "http://outside.example/landing";

fn source_page() -> String {
    let mut links = String::new();
    for i in 0..TARGETS.len() {
        write!(&mut links, "<a href='/r/{i}'>Redirect</a>").unwrap();
    }
    format!(
        "<title>Source page</title><h1>Page</h1>{links}<a href='/http'>HTTP</a><a href='/external'>External</a>"
    )
}

#[derive(Default)]
struct Site(Mutex<Vec<String>>);
impl Fetcher for Site {
    fn fetch(&self, request: &FetchRequest<'_>) -> Result<FetchResponse, FetchError> {
        self.0.lock().unwrap().push(request.url.into());
        if request.url == EXTERNAL {
            return Ok(FetchResponse::new(
                200,
                &[("Content-Type", "text/plain")],
                "",
            ));
        }
        let path = request
            .url
            .strip_prefix("https://example.com")
            .unwrap_or_default();
        let (kind, body) = match path {
            "/" => ("text/html", source_page()),
            "/final?version=2" | "/orphan?keep=1" => (
                "text/html",
                "<title>Source page</title><h1>Page</h1>".into(),
            ),
            "/robots.txt" => (
                "text/plain",
                "User-agent: *\nSitemap: https://example.com/sitemap.xml\n".into(),
            ),
            "/llms.txt" => ("text/plain", "# HTTP scope QA\n".into()),
            "/sitemap.xml" => {
                let mut entries = String::new();
                for target in TARGETS {
                    write!(
                        &mut entries,
                        "<url><loc>{}</loc></url>",
                        target.replace("from-redirect", "from-map")
                    )
                    .unwrap();
                }
                (
                    "application/xml",
                    format!(
                        "<urlset>{entries}<url><loc>https://example.com/orphan?keep=1</loc><lastmod>2026-10-17</lastmod></url></urlset>"
                    ),
                )
            }
            "/http" => {
                return Ok(FetchResponse::new(
                    307,
                    &[("Location", "/final?version=2&utm_source=qa#section")],
                    "",
                ));
            }
            "/external" => return Ok(FetchResponse::new(302, &[("Location", EXTERNAL)], "")),
            path if path.starts_with("/r/") => {
                let index: usize = path[3..].parse().unwrap();
                return Ok(FetchResponse::new(302, &[("Location", TARGETS[index])], ""));
            }
            _ => return Ok(FetchResponse::new(404, &[], "Unexpected request")),
        };
        Ok(FetchResponse::new(200, &[("Content-Type", kind)], body))
    }
}

struct Script;
impl Renderer for Script {
    fn render(&self, request: &RenderRequest<'_>) -> Result<RenderedPage, FetchError> {
        Ok(RenderedPage {
            html: String::from_utf8(request.response.body.clone())
                .unwrap()
                .replace("Source page", "Rendered page"),
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
    directory: &Path,
) -> Vec<serde_json::Value> {
    let path = directory.join(format!("{}.json", tab.name()));
    workspace.export(id, tab, &path, None).unwrap();
    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
}

fn verify(mode: RenderMode, check_external: bool) {
    let directory = tempfile::tempdir().unwrap();
    let workspace = SpiderWorkspace::resolve(Some(directory.path().into())).unwrap();
    let mut config = CrawlConfig {
        start_urls: vec!["https://example.com/".into()],
        robots: RobotsPolicy::Respect,
        ..CrawlConfig::default()
    };
    config.rendering.mode = mode;
    config.follow.check_external = check_external;
    config.speed.max_requests_per_second = 0.0;
    config.limits.max_urls = if check_external { 13 } else { 12 };
    config.storage.headers = false;
    config.storage.html = false;
    config.urls.strip_params = vec!["utm_source".into()];
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
    assert_eq!(meta.progress.crawled, if check_external { 13 } else { 12 });
    assert_eq!(meta.progress.errors, 0);
    assert_eq!(meta.progress.sitemaps, 1);
    let overview = workspace.open(&meta.id).unwrap().overview().unwrap();
    assert_eq!(overview.internal, 12);
    assert_eq!(overview.external, u64::from(check_external));
    assert_eq!(overview.errors, 0);
    assert_eq!(overview.redirects, 9);
    assert_eq!(overview.html, 3);
    assert_eq!(
        overview
            .responses
            .iter()
            .map(|group| group.count)
            .sum::<u64>(),
        u64::from(meta.progress.crawled)
    );
    let requests = site.0.lock().unwrap();
    assert!(
        requests
            .iter()
            .all(|url| url.starts_with("https://") || url.starts_with("http://"))
    );
    assert_eq!(
        requests.iter().filter(|url| *url == EXTERNAL).count(),
        usize::from(check_external)
    );
    verify_pages(&workspace, &meta.id, directory.path(), mode, check_external);
    verify_evidence(&workspace, &meta.id, directory.path());
}

fn verify_pages(
    workspace: &SpiderWorkspace,
    id: &str,
    directory: &Path,
    mode: RenderMode,
    check_external: bool,
) {
    let pages = exported(workspace, id, CrawlTab::Internal, directory);
    assert_eq!(pages.len(), 12);
    assert!(pages.iter().all(
        |row| row["address"].as_str().unwrap().starts_with("https://") && row["error"].is_null()
    ));
    let title = if mode == RenderMode::Javascript {
        "Rendered page"
    } else {
        "Source page"
    };
    for path in ["/", "/final?version=2", "/orphan?keep=1"] {
        let row = pages
            .iter()
            .find(|row| row["address"] == format!("https://example.com{path}"))
            .unwrap();
        assert_eq!(row["title"], title);
        assert_eq!(row["status_code"], 200);
        assert!(row["render_error"].is_null());
        if mode == RenderMode::Javascript {
            assert_eq!(row["rendered"], true);
        }
    }
    let orphan = pages
        .iter()
        .find(|row| row["address"] == "https://example.com/orphan?keep=1")
        .unwrap();
    assert!(orphan["crawl_depth"].is_null());
    let external = exported(workspace, id, CrawlTab::External, directory);
    assert_eq!(external.len(), usize::from(check_external));
    if check_external {
        assert_eq!(external[0]["address"], EXTERNAL);
        assert_eq!(external[0]["status_code"], 200);
    }
    let issues = exported(workspace, id, CrawlTab::Issues, directory);
    assert!(
        issues
            .iter()
            .all(|row| row["address"].as_str().unwrap().starts_with("https://"))
    );
    let orphans: Vec<_> = issues
        .iter()
        .filter(|row| row["issue_id"] == "orphan-page")
        .collect();
    assert_eq!(orphans.len(), 1);
    assert_eq!(orphans[0]["address"], "https://example.com/orphan?keep=1");
}

fn verify_evidence(workspace: &SpiderWorkspace, id: &str, directory: &Path) {
    let links = exported(workspace, id, CrawlTab::Links, directory);
    let maps = exported(workspace, id, CrawlTab::SitemapUrls, directory);
    assert_eq!(maps.len(), 8);
    let reader = workspace.open(id).unwrap();
    for (i, target) in TARGETS.iter().enumerate() {
        let link = links
            .iter()
            .find(|row| {
                row["source"] == format!("https://example.com/r/{i}")
                    && row["destination"] == *target
            })
            .unwrap();
        assert_eq!(link["kind"], "redirect");
        assert_eq!(link["internal"], false);
        assert!(link["destination_status"].is_null());
        let entry = maps
            .iter()
            .find(|row| row["url"] == target.replace("from-redirect", "from-map"))
            .unwrap();
        assert!(entry["status_code"].is_null() && entry["indexability"].is_null());
        let trace = reader
            .redirect_trace(&format!("https://example.com/r/{i}"))
            .unwrap();
        assert_eq!(trace.outcome, RedirectOutcome::Unchecked);
        assert_eq!(trace.hops.len(), 2);
        assert_eq!(trace.hops[0].status, Some(302));
        assert_eq!(trace.hops[1].url, *target);
        assert!(!trace.hops[1].recorded);
    }
    let trace = reader.redirect_trace("https://example.com/http").unwrap();
    assert_eq!(trace.outcome, RedirectOutcome::Complete);
    assert_eq!(trace.hops[1].url, "https://example.com/final?version=2");
}

#[test]
fn html_non_http_targets_do_not_spend_budget_or_create_false_failures() {
    verify(RenderMode::Html, false);
}

#[test]
fn external_http_checks_still_run_while_non_http_targets_remain_unchecked() {
    verify(RenderMode::Html, true);
}

#[test]
fn javascript_crawls_use_the_same_http_queue_boundary_and_saved_evidence() {
    verify(RenderMode::Javascript, true);
}

struct NormalizedChain;
impl Fetcher for NormalizedChain {
    fn fetch(&self, request: &FetchRequest<'_>) -> Result<FetchResponse, FetchError> {
        let path = request.url.strip_prefix("https://example.com").unwrap();
        let (status, headers, body) = match path {
            "/" => (
                200,
                vec![("Content-Type", "text/html")],
                "<title>Home</title><a href='/loop'>Loop</a><a href='/old'>Old</a>",
            ),
            "/loop" => (301, vec![("Location", "/loop?utm_source=qa#section")], ""),
            "/old" => (302, vec![("Location", "/middle?utm_source=qa#section")], ""),
            "/middle" => (
                308,
                vec![("Location", "/missing?utm_source=qa#section")],
                "",
            ),
            "/missing" => (404, vec![], "Missing"),
            "/llms.txt" => (200, vec![("Content-Type", "text/plain")], "# Local QA"),
            _ => panic!("unexpected HTTP identity: {path}"),
        };
        Ok(FetchResponse::new(status, &headers, body))
    }
}

#[test]
fn normalized_redirects_keep_loops_and_downstream_failures_visible() {
    let directory = tempfile::tempdir().unwrap();
    let workspace = SpiderWorkspace::resolve(Some(directory.path().into())).unwrap();
    let mut config = CrawlConfig {
        start_urls: vec!["https://example.com/".into()],
        robots: RobotsPolicy::Ignore,
        ..CrawlConfig::default()
    };
    config.follow.sitemaps = false;
    config.storage.headers = false;
    config.storage.html = false;
    config.speed.max_requests_per_second = 0.0;
    config.urls.strip_params = vec!["utm_source".into()];
    config.limits.max_urls = 5;
    let meta = workspace
        .crawl_with(
            &CrawlPlan::new(config).unwrap(),
            &NormalizedChain,
            &CrawlControl::new(),
            |_| {},
        )
        .unwrap();
    assert_eq!(meta.progress.crawled, 5);
    assert_eq!(meta.progress.errors, 1);
    let reader = workspace.open(&meta.id).unwrap();
    let looped = reader.redirect_trace("https://example.com/loop").unwrap();
    assert_eq!(looped.outcome, RedirectOutcome::Loop);
    assert_eq!(looped.hops.len(), 2);
    assert!(
        looped
            .hops
            .iter()
            .all(|hop| hop.url == "https://example.com/loop" && hop.recorded)
    );
    let failed = reader.redirect_trace("https://example.com/old").unwrap();
    assert_eq!(failed.outcome, RedirectOutcome::HttpError);
    assert_eq!(
        failed.hops.iter().map(|hop| hop.status).collect::<Vec<_>>(),
        [Some(302), Some(308), Some(404)]
    );
    let issues = exported(&workspace, &meta.id, CrawlTab::Issues, directory.path());
    for (issue, paths) in [
        ("redirect-loop", vec!["/loop"]),
        ("redirect-to-error", vec!["/old", "/middle"]),
    ] {
        let mut actual: Vec<_> = issues
            .iter()
            .filter(|row| row["issue_id"] == issue)
            .map(|row| row["address"].as_str().unwrap())
            .collect();
        let mut expected: Vec<_> = paths
            .iter()
            .map(|path| format!("https://example.com{path}"))
            .collect();
        actual.sort_unstable();
        expected.sort_unstable();
        assert_eq!(actual, expected);
    }
}
