//! Per-hop sitemap responses and the saved graph must agree without another HTTP fetch.

use super::*;
use seonexus_crawl::{
    FetchError, FetchRequest, FetchResponse, Fetcher, RenderRequest, RenderedPage, Renderer,
};
use std::sync::Mutex;
use std::time::Duration;

struct Site<'a> {
    requests: Mutex<Vec<String>>,
    stop: Option<&'a CrawlControl>,
}
impl Fetcher for Site<'_> {
    fn fetch(&self, request: &FetchRequest<'_>) -> Result<FetchResponse, FetchError> {
        self.requests.lock().unwrap().push(request.url.into());
        let path = request.url.strip_prefix("https://example.com").unwrap();
        let (status,headers,body)=match path {
            "/"|"/orphan"=>(200,vec![("Content-Type","text/html")],"<title>Source page</title><h1>Page</h1>".into()),
            "/robots.txt"=>(200,vec![],"User-agent: *\n".into()),
            "/llms.txt"=>(200,vec![],"# Local QA".into()),
            "/sitemap.xml"=>{
                if let Some(control)=self.stop {control.stop();}
                (301,vec![("Location","/middle.xml?part=1#first")],"redirect body is ignored".into())
            },
            "/middle.xml?part=1"=>(302,vec![("Location","/index.xml?keep=2#second")],String::new()),
            "/index.xml?keep=2"=>{
                let names=["good.xml","shared.xml","loop-a.xml","loop-b.xml","fail.xml","xml.xml","unsupported.xml","missing-location.xml","not-redirect.xml","invalid-location.xml"];
                let mut body=String::from("<sitemapindex>");
                for name in names {body.push_str("<sitemap><loc>https://example.com/");body.push_str(name);body.push_str("</loc></sitemap>");}
                body.push_str("</sitemapindex>");(200,vec![],body)
            },
            "/good.xml"=>(307,vec![("Location","/pages.xml?part=2#good")],String::new()),
            "/shared.xml"=>(308,vec![("Location","/pages.xml?part=2#shared")],String::new()),
            "/pages.xml?part=2"=>(200,vec![],"<urlset><url><loc>https://example.com/orphan?utm_source=map</loc><lastmod>2026-10-17</lastmod></url></urlset>".into()),
            "/loop-a.xml"=>(301,vec![("Location","/loop-b.xml#cycle")],String::new()),
            "/loop-b.xml"=>(302,vec![("Location","/loop-a.xml#cycle")],String::new()),
            "/fail.xml"=>(303,vec![("Location","/missing.xml")],String::new()),
            "/missing.xml"=>(404,vec![],"Missing".into()),
            "/xml.xml"=>(302,vec![("Location","/broken.xml")],String::new()),
            "/broken.xml"=>(200,vec![],"<urlset>".into()),
            "/unsupported.xml"=>(302,vec![("Location","ftp://example.com/no.xml#original")],String::new()),
            "/missing-location.xml"=>(301,vec![],String::new()),
            "/not-redirect.xml"=>(304,vec![("Location","/must-not-fetch.xml")],String::new()),
            "/invalid-location.xml"=>(302,vec![("Location","http://[")],String::new()),
            _=>panic!("unexpected sitemap request: {path}"),
        };
        Ok(FetchResponse::new(status, &headers, body))
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
    dir: &Path,
) -> Vec<serde_json::Value> {
    let path = dir.join(format!("{}.json", tab.name()));
    workspace.export(id, tab, &path, None).unwrap();
    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
}
fn run(mode: RenderMode, budget: u32, stop: bool) {
    let directory = tempfile::tempdir().unwrap();
    let workspace = SpiderWorkspace::resolve(Some(directory.path().into())).unwrap();
    let mut config = CrawlConfig {
        start_urls: vec!["https://example.com/".into()],
        robots: RobotsPolicy::Ignore,
        ..CrawlConfig::default()
    };
    config.follow.sitemap_urls = vec!["https://example.com/sitemap.xml".into()];
    config.limits.max_sitemaps = budget;
    config.speed.max_requests_per_second = 0.0;
    config.speed.threads = 1;
    config.storage.html = false;
    config.storage.headers = false;
    config.urls.strip_all_params = true;
    config.rendering.mode = mode;
    let control = CrawlControl::new();
    let site = Site {
        requests: Mutex::new(vec![]),
        stop: stop.then_some(&control),
    };
    let meta = workspace
        .crawl_with_renderer(
            &CrawlPlan::new(config).unwrap(),
            &site,
            Some(&Script),
            &control,
            |_| {},
        )
        .unwrap();
    assert_eq!(
        meta.status,
        if stop {
            CrawlStatus::Stopped
        } else {
            CrawlStatus::Completed
        }
    );
    assert_eq!(meta.progress.errors, 0);
    let rows = exported(&workspace, &meta.id, CrawlTab::Sitemaps, directory.path());
    let root = rows
        .iter()
        .find(|row| row["url"] == "https://example.com/sitemap.xml")
        .unwrap();
    assert_eq!(root["status_code"], 301);
    assert_eq!(root["redirect_location"], "/middle.xml?part=1#first");
    assert_eq!(
        root["redirect_url"],
        "https://example.com/middle.xml?part=1"
    );
    assert_eq!(root["entries"], 0);
    assert!(root["kind"].is_null());
    let issues = exported(&workspace, &meta.id, CrawlTab::Issues, directory.path());
    if stop || budget == 2 {
        assert_eq!(rows.len(), if stop { 1 } else { 2 });
        assert_eq!(root["redirect_outcome"], "unchecked");
        assert!(root["error"].is_null() && root["final_status"].is_null());
        assert_eq!(root["redirect_hops"], if stop { 1 } else { 2 });
        assert!(!issues.iter().any(|row| row["issue_id"] == "sitemap-error"));
    } else {
        verify_graph(&rows, &issues);
        verify_filtered_graph(&workspace, &meta.id);
        assert_eq!(meta.progress.crawled, 2);
        assert_eq!(meta.progress.sitemaps, 16);
        let pages = exported(&workspace, &meta.id, CrawlTab::Internal, directory.path());
        assert_eq!(pages.len(), 2);
        assert!(pages.iter().all(|page| page["title"]
            == if mode == RenderMode::Javascript {
                "Rendered page"
            } else {
                "Source page"
            }));
        let entries = exported(
            &workspace,
            &meta.id,
            CrawlTab::SitemapUrls,
            directory.path(),
        );
        assert_eq!(entries.len(), 1);
        assert_eq!(
            entries[0]["sitemap"],
            "https://example.com/pages.xml?part=2"
        );
        assert_eq!(entries[0]["status_code"], 200);
    }
    let requests = site.requests.lock().unwrap();
    assert!(
        requests
            .iter()
            .all(|url| url.starts_with("https://") && !url.contains('#'))
    );
    for url in requests.iter() {
        assert_eq!(
            requests
                .iter()
                .filter(|candidate| *candidate == url)
                .count(),
            1
        );
    }
}
fn verify_graph(rows: &[serde_json::Value], issues: &[serde_json::Value]) {
    let row = |name: &str| {
        rows.iter()
            .find(|row| row["url"] == format!("https://example.com/{name}"))
            .unwrap()
    };
    for name in ["sitemap.xml", "middle.xml?part=1", "good.xml", "shared.xml"] {
        assert_eq!(row(name)["redirect_outcome"], "complete");
        assert_eq!(row(name)["final_status"], 200);
        assert!(row(name)["error"].is_null());
    }
    assert_eq!(
        row("sitemap.xml")["final_url"],
        "https://example.com/index.xml?keep=2"
    );
    assert_eq!(row("sitemap.xml")["redirect_hops"], 2);
    for name in ["loop-a.xml", "loop-b.xml"] {
        assert_eq!(row(name)["redirect_outcome"], "loop");
        assert!(row(name)["final_url"].is_null() && row(name)["redirect_hops"].is_null());
    }
    for (name, status) in [
        ("fail.xml", 404),
        ("xml.xml", 200),
        ("unsupported.xml", 302),
        ("missing-location.xml", 301),
        ("not-redirect.xml", 304),
        ("invalid-location.xml", 302),
    ] {
        assert_eq!(row(name)["redirect_outcome"], "error");
        assert_eq!(row(name)["final_status"], status);
        assert!(row(name)["error"].as_str().is_some());
    }
    assert_eq!(
        row("unsupported.xml")["redirect_url"],
        "ftp://example.com/no.xml#original"
    );
    let expected = [
        "loop-a.xml",
        "loop-b.xml",
        "fail.xml",
        "missing.xml",
        "xml.xml",
        "broken.xml",
        "unsupported.xml",
        "missing-location.xml",
        "not-redirect.xml",
        "invalid-location.xml",
    ];
    let mut actual: Vec<_> = issues
        .iter()
        .filter(|row| row["issue_id"] == "sitemap-error")
        .map(|row| row["address"].as_str().unwrap())
        .collect();
    let mut wanted: Vec<_> = expected
        .iter()
        .map(|name| format!("https://example.com/{name}"))
        .collect();
    actual.sort_unstable();
    wanted.sort_unstable();
    assert_eq!(actual, wanted);
}

fn verify_filtered_graph(workspace: &SpiderWorkspace, id: &str) {
    let reader = workspace.open(id).unwrap();
    let request = PageRequest {
        tab: "sitemaps".into(),
        search: "pages.xml?part=2".into(),
        limit: 1,
        ..PageRequest::default()
    };
    let page = reader.page(&request).unwrap();
    assert_eq!(page.total, 3);
    assert_eq!(page.rows.len(), 1);
    let issue = PageRequest {
        tab: "sitemaps".into(),
        issue_id: "sitemap-error".into(),
        limit: 1,
        ..PageRequest::default()
    };
    assert_eq!(reader.page(&issue).unwrap().total, 10);
    let mut exported = seonexus_core::table::TableBuffer::default();
    assert_eq!(
        reader
            .export_selection(
                &seonexus_crawl_store::TableExportRequest {
                    request: issue,
                    columns: vec![
                        "url".into(),
                        "status_code".into(),
                        "redirect_outcome".into(),
                        "final_status".into()
                    ]
                },
                &mut exported
            )
            .unwrap(),
        10
    );
}
#[test]
fn html_sitemap_redirects_discover_pages_and_preserve_each_hop() {
    run(RenderMode::Html, 30, false);
}
#[test]
fn javascript_sitemap_redirects_render_only_the_discovered_pages() {
    run(RenderMode::Javascript, 30, false);
}
#[test]
fn sitemap_budget_exhaustion_is_unchecked_instead_of_a_false_failure() {
    run(RenderMode::Html, 2, false);
}
#[test]
fn stopping_after_a_sitemap_redirect_retains_the_response_and_unchecked_target() {
    run(RenderMode::Html, 30, true);
}
