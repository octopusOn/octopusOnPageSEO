//! Sitemap fetch identities differ from page normalization and fragment references.

use std::sync::Mutex;
use std::time::Duration;

use super::*;
use seonexus_crawl::{
    FetchError, FetchRequest, FetchResponse, Fetcher, RenderRequest, RenderedPage, Renderer,
};

const MAPS: [(&str, &str); 3] = [
    ("/pages.xml", "/one"),
    ("/pages.xml?part=2&token=a%23b", "/two"),
    ("/map%23name.xml", "/hash"),
];
const PAGE: &str = "<title>Source page</title><h1>Page</h1>";

fn robots(fallback: bool) -> String {
    format!(
        "User-agent: *\nSitemap: ftp://example.com/unused.xml\n{}",
        if fallback {
            ""
        } else {
            "Sitemap: https://example.com/index.xml#robots-a\nSitemap: https://example.com/index.xml#robots-b\n"
        }
    )
}

fn urlset(page: &str) -> String {
    format!(
        "<urlset><url><loc>https://example.com{page}</loc><lastmod>2026-10-17</lastmod></url></urlset>"
    )
}

struct Site {
    fallback: bool,
    requests: Mutex<Vec<String>>,
}
impl Fetcher for Site {
    fn fetch(&self, request: &FetchRequest<'_>) -> Result<FetchResponse, FetchError> {
        self.requests.lock().unwrap().push(request.url.into());
        let path = request
            .url
            .strip_prefix("https://example.com/")
            .unwrap_or_default();
        let mut content_type = "application/xml";
        let body = if path == "robots.txt" {
            content_type = "text/plain";
            robots(self.fallback)
        } else if path == "llms.txt" {
            content_type = "text/plain";
            "# Local QA".into()
        } else if path.is_empty() || MAPS.iter().any(|(_, page)| &page[1..] == path) {
            content_type = "text/html";
            PAGE.into()
        } else if path == "sitemap.xml" && self.fallback {
            urlset("/one")
        } else if path == "index.xml" {
            "<sitemapindex><sitemap><loc>data:text/plain,unused</loc></sitemap><sitemap><loc>file:///tmp/unused.xml</loc></sitemap>\
                 <sitemap><loc>https://example.com/index.xml#cycle</loc></sitemap>\
                 <sitemap><loc>https://example.com/pages.xml#one</loc></sitemap>\
                 <sitemap><loc>https://example.com/pages.xml#two</loc></sitemap>\
                 <sitemap><loc>https://example.com/pages.xml?part=2&amp;token=a%23b#two</loc></sitemap>\
                 <sitemap><loc>https://example.com/map%23name.xml#hash</loc></sitemap></sitemapindex>".into()
        } else if let Some((_, page)) = MAPS.iter().find(|(map, _)| &map[1..] == path) {
            urlset(page)
        } else {
            return Ok(FetchResponse::new(404, &[], "unexpected URL"));
        };
        Ok(FetchResponse::new(
            200,
            &[("Content-Type", content_type)],
            body,
        ))
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

fn verify(mode: RenderMode, fallback: bool) {
    let dir = tempfile::tempdir().unwrap();
    let workspace = SpiderWorkspace::resolve(Some(dir.path().into())).unwrap();
    let mut config = CrawlConfig {
        start_urls: vec!["https://example.com/".into()],
        robots: RobotsPolicy::Respect,
        ..CrawlConfig::default()
    };
    config.rendering.mode = mode;
    config.follow.sitemap_urls = vec!["data:text/plain,unused".into()];
    if !fallback {
        config.follow.sitemap_urls.extend([
            "https://example.com/index.xml#config-a".into(),
            "https://example.com/index.xml#config-b".into(),
        ]);
    }
    config.limits.max_sitemaps = if fallback { 1 } else { 4 };
    // Page URL rewriting must never collapse separate sitemap query resources.
    config.urls.strip_all_params = true;
    config.storage.html = false;
    config.storage.headers = false;
    config.speed.max_requests_per_second = 0.0;
    let site = Site {
        fallback,
        requests: Mutex::new(Vec::new()),
    };
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
    assert_eq!(meta.progress.errors, 0);
    assert_eq!(meta.progress.crawled, if fallback { 2 } else { 4 });
    assert_eq!(meta.progress.sitemaps, if fallback { 1 } else { 4 });
    verify_maps(&workspace, &meta.id, dir.path(), &site);
    let pages = exported(&workspace, &meta.id, CrawlTab::Internal, dir.path());
    assert_eq!(pages.len(), if fallback { 2 } else { 4 });
    for row in pages {
        assert_eq!(row["status_code"], 200);
        assert_eq!(
            row["title"],
            if mode == RenderMode::Javascript {
                "Rendered page"
            } else {
                "Source page"
            }
        );
        if row["address"] != "https://example.com/" {
            assert_eq!(row["crawl_depth"], serde_json::Value::Null);
        }
        assert!(row["render_error"].is_null());
    }
    let issues = exported(&workspace, &meta.id, CrawlTab::Issues, dir.path());
    assert!(!issues.iter().any(|row| row["issue_id"] == "sitemap-error"));
    assert_eq!(
        issues
            .iter()
            .filter(|row| row["issue_id"] == "orphan-page")
            .count(),
        if fallback { 1 } else { 3 }
    );
    let files = exported(&workspace, &meta.id, CrawlTab::Robots, dir.path());
    assert_eq!(files.len(), 1);
    assert_eq!(files[0]["text"], robots(fallback));
    assert!(
        files[0]["sitemaps"]
            .as_str()
            .unwrap()
            .contains("ftp://example.com/unused.xml")
    );
    // Robots '#' suffixes are comments; the original text above still records them.
    let hints: Vec<_> = files[0]["sitemaps"].as_str().unwrap().lines().collect();
    assert_eq!(
        hints,
        if fallback {
            vec!["ftp://example.com/unused.xml"]
        } else {
            vec![
                "ftp://example.com/unused.xml",
                "https://example.com/index.xml",
                "https://example.com/index.xml",
            ]
        }
    );
}

fn verify_maps(workspace: &SpiderWorkspace, id: &str, directory: &Path, site: &Site) {
    let rows = exported(workspace, id, CrawlTab::Sitemaps, directory);
    let mut expected: Vec<_> = if site.fallback {
        vec!["/sitemap.xml"]
    } else {
        vec!["/index.xml", MAPS[0].0, MAPS[1].0, MAPS[2].0]
    };
    expected.sort_unstable();
    let mut actual: Vec<_> = rows
        .iter()
        .map(|row| {
            row["url"]
                .as_str()
                .unwrap()
                .strip_prefix("https://example.com")
                .unwrap()
        })
        .collect();
    actual.sort_unstable();
    assert_eq!(actual, expected);
    assert!(
        rows.iter()
            .all(|row| row["status_code"] == 200 && row["error"].is_null())
    );
    let requested = site.requests.lock().unwrap();
    assert!(
        requested
            .iter()
            .all(|url| url.starts_with("https://") && !url.contains('#'))
    );
    for path in expected {
        assert_eq!(
            requested
                .iter()
                .filter(|url| *url == &format!("https://example.com{path}"))
                .count(),
            1
        );
    }
    let entries = exported(workspace, id, CrawlTab::SitemapUrls, directory);
    assert_eq!(entries.len(), if site.fallback { 1 } else { 3 });
    for row in entries {
        let page = row["url"].as_str().unwrap();
        let map = if site.fallback {
            "/sitemap.xml"
        } else {
            MAPS.iter()
                .find(|(_, target)| page.ends_with(target))
                .unwrap()
                .0
        };
        assert_eq!(row["sitemap"], format!("https://example.com{map}"));
        assert_eq!(row["lastmod"], "2026-10-17");
        assert_eq!(row["status_code"], 200);
    }
}

#[test]
fn html_sitemap_identity_preserves_budget_queries_and_encoded_hashes() {
    verify(RenderMode::Html, false);
}

#[test]
fn javascript_sitemap_only_pages_use_the_same_normalized_map_queue() {
    verify(RenderMode::Javascript, false);
}

#[test]
fn unsupported_hints_allow_default_sitemap_discovery_without_false_errors() {
    verify(RenderMode::Html, true);
}
