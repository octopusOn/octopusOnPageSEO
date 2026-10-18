//! Original SEO signals survive rendering independently of stored HTML.
use super::*;
use seonexus_core::table::Cell;
use seonexus_crawl::{
    FetchError, FetchRequest, FetchResponse, Fetcher, RenderRequest, RenderedPage, Renderer,
};
use std::time::Duration;

const RAW: &str = "<title>Before</title><meta name='description' content='Before description'><meta name='robots' content='noindex,nofollow'><link rel='canonical' href='/before'><a href='/old'>Old link</a>";
const DOM: &str = "<title>After</title><meta name='description' content='After description'><link rel='canonical' href='/after'><a href='/one'>One</a><a href='/two'>Two</a>";
const CASES: [&str; 6] = [
    "removed",
    "header",
    "failed",
    "truncated",
    "unchanged",
    "added",
];

struct Site;
impl Fetcher for Site {
    fn fetch(&self, r: &FetchRequest<'_>) -> Result<FetchResponse, FetchError> {
        let mut headers = vec![("Content-Type", "text/html")];
        let body = if r.url.ends_with("/header") {
            headers.extend([
                ("X-Robots-Tag", "noindex,nofollow"),
                (
                    "Link",
                    "<https://example.com/http-canonical>; rel=canonical",
                ),
            ]);
            "<title>Header directives</title>"
        } else if r.url.ends_with("/added") {
            "<title>Originally indexable</title>"
        } else {
            RAW
        };
        Ok(FetchResponse::new(200, &headers, body))
    }
}
struct Script;
impl Renderer for Script {
    fn render(&self, r: &RenderRequest<'_>) -> Result<RenderedPage, FetchError> {
        if r.url.ends_with("/failed") {
            return Err(FetchError::Other("browser failed".into()));
        }
        let html = if r.url.ends_with("/unchanged") || r.url.ends_with("/added") {
            RAW
        } else if r.url.ends_with("/header") {
            "<title>Header still applies</title>"
        } else {
            DOM
        };
        Ok(RenderedPage {
            html: html.into(),
            url: r.url.into(),
            elapsed: Duration::ZERO,
            truncated: r.url.ends_with("/truncated"),
        })
    }
}
fn crawl(workspace: &SpiderWorkspace) -> CrawlMeta {
    let mut config = CrawlConfig {
        mode: CrawlMode::List,
        start_urls: CASES
            .iter()
            .map(|case| format!("https://example.com/{case}"))
            .collect(),
        ..CrawlConfig::default()
    };
    config.rendering.mode = RenderMode::Javascript;
    config.storage.html = false;
    config.robots = RobotsPolicy::Ignore;
    config.follow.sitemaps = false;
    let plan = CrawlPlan::new(config).unwrap();
    workspace
        .crawl_observed_with(&plan, &Site, Some(&Script), &CrawlControl::new(), |_| {})
        .unwrap()
}
fn value<'a>(page: &'a TablePage, column: &str) -> &'a Cell {
    &page.rows[0][page.columns.iter().position(|c| c.name == column).unwrap()]
}

#[test]
fn comparison_persists_seo_metadata_without_saving_html() {
    let dir = tempfile::tempdir().unwrap();
    let workspace = SpiderWorkspace::resolve(Some(dir.path().into())).unwrap();
    let meta = crawl(&workspace);
    let reader = workspace.open(&meta.id).unwrap();
    let detail = reader.detail("https://example.com/removed").unwrap();
    assert!(detail.html.is_none() && detail.raw_html.is_none());
    for (column, expected) in [
        ("raw_title", "Before"),
        ("title", "After"),
        ("raw_meta_description", "Before description"),
        ("meta_description", "After description"),
        ("raw_canonical", "https://example.com/before"),
        ("canonical", "https://example.com/after"),
    ] {
        assert_eq!(
            value(&detail.facts, column),
            &Cell::Text(expected.into()),
            "{column}"
        );
    }
    for (column, expected) in [
        ("raw_noindex", true),
        ("raw_nofollow", true),
        ("noindex", false),
        ("nofollow", false),
    ] {
        assert_eq!(
            value(&detail.facts, column),
            &Cell::Boolean(expected),
            "{column}"
        );
    }
    assert_eq!(value(&detail.facts, "raw_link_count"), &Cell::Integer(1));
    assert_eq!(
        value(&detail.facts, "rendered_link_count"),
        &Cell::Integer(2)
    );
    let header = reader.detail("https://example.com/header").unwrap();
    assert_eq!(
        value(&header.facts, "raw_canonical"),
        &Cell::Text("https://example.com/http-canonical".into())
    );
    assert_eq!(value(&header.facts, "raw_noindex"), &Cell::Boolean(true));
    assert_eq!(value(&header.facts, "noindex"), &Cell::Boolean(true));
    let added = reader.detail("https://example.com/added").unwrap();
    assert_eq!(value(&added.facts, "raw_noindex"), &Cell::Boolean(false));
    assert_eq!(value(&added.facts, "raw_meta_description"), &Cell::Null);
    let path = dir.path().join("comparison.json");
    workspace
        .export(&meta.id, CrawlTab::Rendering, &path, None)
        .unwrap();
    let exported = std::fs::read_to_string(path).unwrap();
    assert!(
        exported.contains("raw_noindex")
            && exported.contains("Before description")
            && exported.contains("rendered_canonical")
    );
}

#[test]
fn noindex_removal_requires_a_complete_successful_render_and_excludes_http_noindex() {
    let dir = tempfile::tempdir().unwrap();
    let workspace = SpiderWorkspace::resolve(Some(dir.path().into())).unwrap();
    let meta = crawl(&workspace);
    let reader = workspace.open(&meta.id).unwrap();
    let findings = reader
        .page(&PageRequest {
            tab: "internal".into(),
            issue_id: "render-noindex-removed".into(),
            limit: 100,
            ..PageRequest::default()
        })
        .unwrap();
    let affected: Vec<_> = findings.rows.iter().map(|row| row[0].to_string()).collect();
    assert_eq!(affected, ["https://example.com/removed"]);
    let failed = reader
        .page(&PageRequest {
            tab: "rendering".into(),
            search: "/failed".into(),
            ..PageRequest::default()
        })
        .unwrap();
    assert_eq!(value(&failed, "rendered"), &Cell::Boolean(false));
    for column in [
        "rendered_title",
        "rendered_word_count",
        "rendered_meta_description",
        "rendered_canonical",
        "rendered_noindex",
        "rendered_nofollow",
        "rendered_link_count",
    ] {
        assert_eq!(value(&failed, column), &Cell::Null, "{column}");
    }
    assert_eq!(value(&failed, "raw_noindex"), &Cell::Boolean(true));
}
