//! Keep raw sitemap evidence while matching the frontier's page identities.

use std::sync::Mutex;
use std::time::Duration;

use super::*;
use seonexus_crawl::{
    FetchError, FetchRequest, FetchResponse, Fetcher, RenderRequest, RenderedPage, Renderer,
};

const MAP: &str = "https://example.com/maps.xml?part=1&utm_source=map";
const ENTRIES: [(&str, &str); 7] = [
    (
        "https://EXAMPLE.com:443/a/../orphan?keep=1&utm_source=one#first",
        "https://example.com/orphan?keep=1",
    ),
    (
        "https://example.com/orphan?keep=1&utm_source=two#second",
        "https://example.com/orphan?keep=1",
    ),
    (
        "https://example.com/blocked?keep=2&utm_source=x#third",
        "https://example.com/blocked?keep=2",
    ),
    (
        "https://example.com/missing?keep=3&utm_source=x#fourth",
        "https://example.com/missing?keep=3",
    ),
    (
        "https://example.com/excluded?keep=4&utm_source=x#fifth",
        "https://example.com/excluded?keep=4",
    ),
    (
        "https://example.com/map%23name?token=a%23b&utm_source=x#sixth",
        "https://example.com/map%23name?token=a%23b",
    ),
    ("ftp://example.com/unsupported#original", ""),
];

#[derive(Default)]
struct Site {
    requests: Mutex<Vec<String>>,
}
impl Fetcher for Site {
    fn fetch(&self, request: &FetchRequest<'_>) -> Result<FetchResponse, FetchError> {
        self.requests.lock().unwrap().push(request.url.into());
        if request.url == MAP {
            let entries = ENTRIES
                .iter()
                .map(|(raw, _)| {
                    format!(
                        "<url><loc>{}</loc><lastmod>2026-10-17</lastmod></url>",
                        raw.replace('&', "&amp;")
                    )
                })
                .collect::<Vec<_>>()
                .join("\n");
            return Ok(FetchResponse::new(
                200,
                &[("Content-Type", "application/xml")],
                format!("<urlset>{entries}</urlset>"),
            ));
        }
        let path = request.url.strip_prefix("https://example.com").unwrap();
        let (status, kind, body) = if path == "/robots.txt" {
            (200, "text/plain", "User-agent: *\n")
        } else if path == "/llms.txt" {
            (200, "text/plain", "# Local QA")
        } else if path == "/" || path.starts_with("/orphan") || path.starts_with("/map%23name") {
            (200, "text/html", "<title>Source page</title><h1>Page</h1>")
        } else if path.starts_with("/blocked") {
            (
                200,
                "text/html",
                "<title>Noindex</title><meta name='robots' content='noindex'>",
            )
        } else if path.starts_with("/missing") {
            (404, "text/plain", "Missing")
        } else {
            panic!("unexpected page identity: {path}")
        };
        Ok(FetchResponse::new(status, &[("Content-Type", kind)], body))
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
fn export(
    workspace: &SpiderWorkspace,
    id: &str,
    tab: CrawlTab,
    path: &Path,
) -> Vec<serde_json::Value> {
    let path = path.join(format!("{}.json", tab.name()));
    workspace.export(id, tab, &path, None).unwrap();
    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
}
fn verify(mode: RenderMode, strip_all: bool) {
    let dir = tempfile::tempdir().unwrap();
    let workspace = SpiderWorkspace::resolve(Some(dir.path().into())).unwrap();
    let mut config = CrawlConfig {
        start_urls: vec!["https://example.com/".into()],
        robots: RobotsPolicy::Ignore,
        ..CrawlConfig::default()
    };
    config.rendering.mode = mode;
    config.urls.strip_all_params = strip_all;
    config.urls.strip_params = vec!["utm_source".into()];
    config.urls.exclude = vec!["/excluded".into()];
    config.follow.sitemap_urls = vec![MAP.into()];
    config.limits.max_sitemaps = 1;
    config.speed.max_requests_per_second = 0.0;
    config.storage.headers = false;
    config.storage.html = false;
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
    assert_eq!(meta.progress.errors, 1);
    let rows = export(&workspace, &meta.id, CrawlTab::SitemapUrls, dir.path());
    assert_eq!(rows.len(), 7);
    verify_entries(&rows, strip_all);
    let issues = export(&workspace, &meta.id, CrawlTab::Issues, dir.path());
    let expected = |path: &str| {
        if strip_all {
            format!("https://example.com{}", path.split('?').next().unwrap())
        } else {
            format!("https://example.com{path}")
        }
    };
    for (rule, paths) in [
        (
            "orphan-page",
            vec![
                "/orphan?keep=1",
                "/blocked?keep=2",
                "/map%23name?token=a%23b",
            ],
        ),
        (
            "sitemap-non-indexable",
            vec!["/blocked?keep=2", "/missing?keep=3"],
        ),
        ("sitemap-missing-indexable", vec!["/"]),
    ] {
        let mut actual: Vec<_> = issues
            .iter()
            .filter(|row| row["issue_id"] == rule)
            .map(|row| row["address"].as_str().unwrap().to_owned())
            .collect();
        let mut wanted: Vec<_> = paths.iter().map(|path| expected(path)).collect();
        actual.sort_unstable();
        wanted.sort_unstable();
        assert_eq!(actual, wanted, "{rule}");
    }
    let requests = site.requests.lock().unwrap();
    assert_eq!(requests.iter().filter(|url| url.as_str() == MAP).count(), 1);
    assert!(
        requests
            .iter()
            .filter(|url| url.as_str() != MAP)
            .all(|url| !url.contains('#')
                && !url.contains("utm_source")
                && !url.starts_with("ftp:"))
    );
    verify_filtered_report(&workspace, &meta.id);
}

fn verify_entries(rows: &[serde_json::Value], strip_all: bool) {
    for (raw, identity) in ENTRIES {
        let row = rows.iter().find(|row| row["url"] == raw).unwrap();
        let identity = if strip_all {
            identity.split('?').next().unwrap()
        } else {
            identity
        };
        let status = if identity.contains("excluded") || identity.is_empty() {
            None
        } else if identity.contains("missing") {
            Some(404)
        } else {
            Some(200)
        };
        assert_eq!(
            row["status_code"],
            status.map_or(serde_json::Value::Null, serde_json::Value::from),
            "{raw}"
        );
        if identity.is_empty() {
            assert!(row["crawl_url"].is_null());
        } else {
            assert_eq!(row["crawl_url"], identity);
        }
        assert_eq!(row["sitemap"], MAP);
        assert_eq!(row["lastmod"], "2026-10-17");
        if identity.contains("blocked") {
            assert_eq!(row["indexability"], "noindex");
        }
    }
}

fn verify_filtered_report(workspace: &SpiderWorkspace, id: &str) {
    let reader = workspace.open(id).unwrap();
    for (search, total) in [("utm_source=two", 1), ("part=1", 7)] {
        let page = reader
            .page(&PageRequest {
                tab: "sitemap-urls".into(),
                search: search.into(),
                limit: 1,
                ..PageRequest::default()
            })
            .unwrap();
        assert_eq!(page.total, total);
        assert_eq!(page.rows.len(), 1);
    }
    for (issue, total) in [("orphan-page", 4), ("sitemap-non-indexable", 2)] {
        let request = PageRequest {
            tab: "sitemap-urls".into(),
            issue_id: issue.into(),
            limit: 1,
            offset: 1,
            ..PageRequest::default()
        };
        let page = reader.page(&request).unwrap();
        assert_eq!(page.total, total);
        assert_eq!(page.rows.len(), 1);
        let mut rows = seonexus_core::table::TableBuffer::default();
        let count = reader
            .export_selection(
                &seonexus_crawl_store::TableExportRequest {
                    request,
                    columns: vec!["url".into(), "crawl_url".into(), "status_code".into()],
                },
                &mut rows,
            )
            .unwrap();
        assert_eq!(count, total);
        assert_eq!(rows.rows.len() as u64, total);
    }
    let page = reader
        .page(&PageRequest {
            tab: "sitemap-urls".into(),
            status_class: 4,
            ..PageRequest::default()
        })
        .unwrap();
    assert_eq!(page.total, 1);
}
#[test]
fn html_sitemap_page_statuses_and_findings_match_normalized_frontier_urls() {
    verify(RenderMode::Html, false);
}
#[test]
fn javascript_sitemap_page_statuses_keep_raw_evidence_and_normalized_facts() {
    verify(RenderMode::Javascript, false);
}
#[test]
fn stripping_all_page_queries_preserves_the_original_map_and_entry_evidence() {
    verify(RenderMode::Html, true);
}
