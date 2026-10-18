//! Naming evidence survives HTML/DOM crawling without optional source storage.

use super::*;
use seonexus_crawl::{
    FetchError, FetchRequest, FetchResponse, Fetcher, RenderRequest, RenderedPage, Renderer,
};
use std::time::Duration;

const ROOT: &str = "<html><head><title>Link fixture</title></head><body><a href='/target' data-contact><svg aria-hidden='true'><path/></svg></a><a href='/target'><svg><path/></svg></a><a href='/target' title='Download brochure'><svg aria-hidden='true'/></a></body></html>";
struct Site(bool);
impl Fetcher for Site {
    fn fetch(&self, request: &FetchRequest<'_>) -> Result<FetchResponse, FetchError> {
        let root = if self.0 {
            ROOT.into()
        } else {
            ROOT.replace("data-contact", "data-contact aria-label='WhatsApp'")
        };
        let html = if request.url == "https://example.com/" {
            root
        } else {
            "<html><head><title>Target</title></head><body>Target</body></html>".into()
        };
        Ok(FetchResponse::new(
            200,
            &[("content-type", "text/html")],
            html.into_bytes(),
        ))
    }
}
struct Script;
impl Renderer for Script {
    fn render(&self, request: &RenderRequest<'_>) -> Result<RenderedPage, FetchError> {
        Ok(RenderedPage {
            html: String::from_utf8(request.response.body.clone())
                .unwrap()
                .replace("data-contact", "data-contact aria-label='WhatsApp'"),
            url: request.url.into(),
            elapsed: Duration::ZERO,
            truncated: false,
        })
    }
}
fn verify(javascript: bool) {
    let dir = tempfile::tempdir().unwrap();
    let workspace = SpiderWorkspace::resolve(Some(dir.path().into())).unwrap();
    let mut config = CrawlConfig {
        start_urls: vec!["https://example.com/".into()],
        robots: RobotsPolicy::Ignore,
        ..Default::default()
    };
    config.rendering.mode = if javascript {
        RenderMode::Javascript
    } else {
        RenderMode::Html
    };
    config.follow.sitemaps = false;
    config.storage.html = false;
    config.storage.headers = false;
    let meta = workspace
        .crawl_with_renderer(
            &CrawlPlan::new(config).unwrap(),
            &Site(javascript),
            Some(&Script),
            &CrawlControl::new(),
            |_| {},
        )
        .unwrap();
    assert_eq!(meta.status, CrawlStatus::Completed);
    assert_eq!(meta.progress.crawled, 2);
    let reader = workspace.open(&meta.id).unwrap();
    let detail = reader.detail("https://example.com/").unwrap();
    assert!(detail.html.is_none() && detail.raw_html.is_none());
    let out = dir.path().join("links.json");
    workspace
        .export(&meta.id, CrawlTab::Links, &out, None)
        .unwrap();
    let rows: Vec<serde_json::Value> =
        serde_json::from_slice(&std::fs::read(out).unwrap()).unwrap();
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0]["text"], "WhatsApp");
    assert_eq!(rows[0]["visible_text"], "");
    assert_eq!(rows[0]["has_svg"], true);
    assert_eq!(rows[0]["name_source"], "aria-label");
    assert_eq!(rows[0]["name_observation"], "recorded");
    assert_eq!(rows[1]["text"], "");
    assert_eq!(rows[1]["name_source"], "missing");
    assert_eq!(rows[2]["text"], "Download brochure");
    let issues = dir.path().join("issues.json");
    workspace
        .export(&meta.id, CrawlTab::Issues, &issues, None)
        .unwrap();
    let issues: Vec<serde_json::Value> =
        serde_json::from_slice(&std::fs::read(issues).unwrap()).unwrap();
    assert_eq!(
        issues
            .iter()
            .filter(|r| r["issue_id"] == "anchor-empty")
            .count(),
        1
    );
}
#[test]
fn html_link_names_and_svg_presence_survive_disabled_source_storage() {
    verify(false);
}
#[test]
fn javascript_link_names_use_the_observed_dom_and_survive_disabled_source_storage() {
    verify(true);
}
