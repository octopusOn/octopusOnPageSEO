//! A rejected base must not erase relative evidence; fragment links can leave a document.

use std::sync::Mutex;
use std::time::Duration;

use super::*;
use seonexus_crawl::{
    FetchError, FetchRequest, FetchResponse, Fetcher, RenderRequest, RenderedPage, Renderer,
};

const ROOT: &str = "https://example.com/suite/start";
const TARGET: &str = "https://example.com/target?keep=2&encoded=a%23b";
const SOURCE: &str = "<html><head><base href='data:,unused'><base href='/wrong/'>\
    <title>Source root</title><link rel='canonical' href='start'>\
    <link rel='alternate' hreflang='en' href='child'>\
    <link rel='stylesheet' href='style.css'></head><body><h1>Root</h1>\
    <a href='child'>Child</a><a href='/fragment'>Fragment</a>\
    <a href='/self?keep=1'>Self</a><a href='/javascript'>JS base</a>\
    <img src='image.png' alt='Image' width='8' height='8'><script src='script.js'></script></body></html>";

#[derive(Default)]
struct Site(Mutex<Vec<String>>);
impl Fetcher for Site {
    fn fetch(&self, request: &FetchRequest<'_>) -> Result<FetchResponse, FetchError> {
        self.0.lock().unwrap().push(request.url.into());
        let body = match request.url.strip_prefix("https://example.com") {
            Some("/suite/start") => SOURCE,
            Some("/fragment") => {
                "<base href='/target?keep=2&amp;encoded=a%23b#base'><title>Fragment source</title><a href='#one'>One</a><a href='#two'>Two</a><a href='  '>Empty</a>"
            }
            Some("/self?keep=1") => {
                "<base href='/self?keep=1#base'><title>Self source</title><a href='#one'>One</a><a href=''>Empty</a>"
            }
            Some("/javascript") => {
                "<base href='JaVaScRiPt:unused'><base href='/wrong/'><title>JS base</title><link rel='canonical' href='javascript'><a href='js-child'>Child</a>"
            }
            Some("/suite/child" | "/js-child" | "/target?keep=2&encoded=a%23b" | "/js-only") => {
                "<title>Destination</title><h1>Destination</h1>"
            }
            Some("/suite/image.png") => {
                return Ok(FetchResponse::new(
                    200,
                    &[("Content-Type", "image/png")],
                    "image",
                ));
            }
            Some("/llms.txt") => return Ok(FetchResponse::new(200, &[], "# Local QA")),
            _ => return Ok(FetchResponse::new(404, &[], "Unexpected request")),
        };
        Ok(FetchResponse::new(
            200,
            &[("Content-Type", "text/html")],
            body,
        ))
    }
}

struct Script(bool);
impl Renderer for Script {
    fn render(&self, request: &RenderRequest<'_>) -> Result<RenderedPage, FetchError> {
        if self.0 && request.url == ROOT {
            return Err(FetchError::Other("intentional rendering failure".into()));
        }
        let mut html = String::from_utf8(request.response.body.clone()).unwrap();
        if request.url == ROOT {
            html = html
                .replace("Source root", "Rendered root")
                .replace("</body>", "<a href='/js-only'>JS only</a></body>");
        }
        Ok(RenderedPage {
            html,
            url: request.url.into(),
            elapsed: Duration::ZERO,
            truncated: false,
        })
    }
}

fn export(
    workspace: &SpiderWorkspace,
    id: &str,
    directory: &Path,
    tab: CrawlTab,
) -> Vec<serde_json::Value> {
    let path = directory.join(format!("{}.json", tab.name()));
    workspace.export(id, tab, &path, None).unwrap();
    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
}

fn verify(mode: RenderMode, fail_root: bool) {
    let directory = tempfile::tempdir().unwrap();
    let workspace = SpiderWorkspace::resolve(Some(directory.path().into())).unwrap();
    let mut config = CrawlConfig {
        start_urls: vec![ROOT.into()],
        robots: RobotsPolicy::Ignore,
        ..CrawlConfig::default()
    };
    config.rendering.mode = mode;
    config.follow.sitemaps = false;
    config.follow.scripts = false;
    config.follow.stylesheets = false;
    config.storage.headers = false;
    config.storage.html = false;
    let active = mode == RenderMode::Javascript && !fail_root;
    let site = Site::default();
    let meta = workspace
        .crawl_with_renderer(
            &CrawlPlan::new(config).unwrap(),
            &site,
            Some(&Script(fail_root)),
            &CrawlControl::new(),
            |_| {},
        )
        .unwrap();
    assert_eq!(meta.status, CrawlStatus::Completed);
    assert_eq!(meta.progress.errors, 0);
    assert_eq!(meta.progress.crawled, if active { 9 } else { 8 });
    let reader = workspace.open(&meta.id).unwrap();
    verify_facts(&workspace, &reader, &meta.id, directory.path(), active);
    for (rule, search, total) in [
        ("canonical-missing", "/suite/start", 0),
        (
            "render-error",
            "",
            u64::from(mode == RenderMode::Javascript && fail_root),
        ),
    ] {
        assert_eq!(
            reader
                .page(&PageRequest {
                    tab: "internal".into(),
                    issue_id: rule.into(),
                    search: search.into(),
                    ..PageRequest::default()
                })
                .unwrap()
                .total,
            total,
            "{rule}"
        );
    }
    verify_links(&workspace, &meta.id, directory.path());
    let requests = site.0.lock().unwrap();
    assert_eq!(
        requests.iter().filter(|url| url.as_str() == TARGET).count(),
        1
    );
    assert!(
        requests.iter().all(|url| !url.contains('#')
            && !url.contains("/wrong/")
            && url.starts_with("https://"))
    );
}

fn verify_facts(
    workspace: &SpiderWorkspace,
    reader: &CrawlReader,
    id: &str,
    directory: &Path,
    active: bool,
) {
    let pages = export(workspace, id, directory, CrawlTab::Internal);
    for page in &pages {
        assert_eq!(page["status_code"], 200, "{page}");
        assert!(page["error"].is_null());
    }
    let root = pages.iter().find(|page| page["address"] == ROOT).unwrap();
    assert_eq!(root["canonical"], ROOT);
    assert_eq!(
        root["title"],
        if active {
            "Rendered root"
        } else {
            "Source root"
        }
    );
    let target = pages.iter().find(|page| page["address"] == TARGET).unwrap();
    assert_eq!(target["inlinks"], 3);
    assert_eq!(target["unique_inlinks"], 1);
    let own = pages
        .iter()
        .find(|page| page["address"] == "https://example.com/self?keep=1")
        .unwrap();
    assert_eq!(own["inlinks"], 1);
    let root_detail = reader.detail(ROOT).unwrap();
    let base_column = root_detail
        .facts
        .columns
        .iter()
        .position(|column| column.name == "base_href")
        .unwrap();
    assert_eq!(
        root_detail.facts.rows[0][base_column].to_string(),
        "data:,unused"
    );
    assert!(root_detail.html.is_none() && root_detail.raw_html.is_none());
}

fn verify_links(workspace: &SpiderWorkspace, id: &str, directory: &Path) {
    let links = export(workspace, id, directory, CrawlTab::Links);
    let fragments: Vec<_> = links
        .iter()
        .filter(|row| row["source"] == "https://example.com/fragment")
        .collect();
    assert_eq!(fragments.len(), 3);
    assert!(
        fragments
            .iter()
            .all(|row| row["destination"] == TARGET && row["destination_status"] == 200)
    );
    assert_eq!(
        fragments
            .iter()
            .map(|row| row["text"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["One", "Two", "Empty"]
    );
    assert!(
        !links
            .iter()
            .any(|row| row["source"] == "https://example.com/self?keep=1")
    );
    for (kind, destination) in [
        ("hyperlink", "/suite/child"),
        ("hreflang", "/suite/child"),
        ("canonical", "/suite/start"),
        ("stylesheet", "/suite/style.css"),
        ("script", "/suite/script.js"),
        ("image", "/suite/image.png"),
    ] {
        assert!(
            links.iter().any(|row| row["source"] == ROOT
                && row["kind"] == kind
                && row["destination"] == format!("https://example.com{destination}")),
            "{kind}"
        );
    }
}

#[test]
fn html_base_resolution_keeps_fetched_pages_and_exported_evidence_consistent() {
    verify(RenderMode::Html, false);
}
#[test]
fn javascript_base_resolution_keeps_source_and_rendered_link_discovery() {
    verify(RenderMode::Javascript, false);
}
#[test]
fn failed_rendering_uses_the_corrected_original_document_base() {
    verify(RenderMode::Javascript, true);
}
