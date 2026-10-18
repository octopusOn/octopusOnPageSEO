//! Restrictive snippet limits survive HTTP/meta/DOM merging and reach saved findings.

use std::collections::BTreeSet;
use std::time::Duration;

use super::*;
use octopuson_core::table::Cell;
use octopuson_crawl::{
    FetchError, FetchRequest, FetchResponse, Fetcher, RenderRequest, RenderedPage, Renderer,
};

const CASES: [&str; 9] = [
    "header-zero",
    "header-free",
    "duplicate-header",
    "duplicate-meta",
    "same-list",
    "invalid",
    "finite",
    "unlimited",
    "generated",
];

struct Site;
impl Fetcher for Site {
    fn fetch(&self, request: &FetchRequest<'_>) -> Result<FetchResponse, FetchError> {
        let path = request.url.strip_prefix("https://example.com/").unwrap();
        let mut headers = vec![("Content-Type", "text/html")];
        let meta = match path {
            "header-zero" => {
                headers.push(("X-Robots-Tag", "max-snippet:0"));
                "<meta name='robots' content='max-snippet:-1'>"
            }
            "header-free" => {
                headers.push(("X-Robots-Tag", "max-snippet:-1"));
                "<meta name='robots' content='max-snippet:0'>"
            }
            "duplicate-header" => {
                headers.extend([
                    ("X-Robots-Tag", "max-snippet:0"),
                    ("X-Robots-Tag", "max-snippet:-1"),
                ]);
                ""
            }
            "duplicate-meta" => {
                "<meta name='robots' content='max-snippet:0'><meta name='robots' content='max-snippet:-1'>"
            }
            "same-list" => {
                headers.push(("X-Robots-Tag", "max-snippet:0, max-snippet:-1"));
                ""
            }
            "invalid" => {
                headers.push(("X-Robots-Tag", "max-snippet:0, max-snippet:invalid"));
                ""
            }
            "finite" => {
                headers.push(("X-Robots-Tag", "max-snippet:50"));
                "<meta name='robots' content='max-snippet:150'>"
            }
            "unlimited" => {
                headers.push(("X-Robots-Tag", "max-snippet:-1"));
                "<meta name='robots' content='max-snippet:-1'>"
            }
            "generated" => "<meta name='robots' content='max-snippet:50'>",
            _ => return Ok(FetchResponse::new(404, &[], "")),
        };
        Ok(FetchResponse::new(
            200,
            &headers,
            format!("<head><title>Snippet controls</title>{meta}</head><body>Content</body>"),
        ))
    }
}

struct Script;
impl Renderer for Script {
    fn render(&self, request: &RenderRequest<'_>) -> Result<RenderedPage, FetchError> {
        Ok(RenderedPage {
            html: if request.url.ends_with("/generated") {
                "<head><title>Generated controls</title><meta name='robots' content='max-snippet:0'><meta name='robots' content='max-snippet:-1'></head><body>Content</body>".into()
            } else {
                String::from_utf8(request.response.body.clone()).unwrap()
            },
            url: request.url.into(),
            elapsed: Duration::ZERO,
            truncated: false,
        })
    }
}

fn fact<'a>(detail: &'a UrlDetail, name: &str) -> &'a Cell {
    &detail.facts.rows[0][detail
        .facts
        .columns
        .iter()
        .position(|column| column.name == name)
        .unwrap()]
}

fn verify(mode: RenderMode) {
    let dir = tempfile::tempdir().unwrap();
    let workspace = SpiderWorkspace::resolve(Some(dir.path().into())).unwrap();
    let mut config = CrawlConfig {
        mode: CrawlMode::List,
        start_urls: CASES
            .iter()
            .map(|case| format!("https://example.com/{case}"))
            .collect(),
        robots: RobotsPolicy::Ignore,
        ..CrawlConfig::default()
    };
    config.rendering.mode = mode;
    config.follow.sitemaps = false;
    config.storage.html = false;
    let plan = CrawlPlan::new(config).unwrap();
    let meta = workspace
        .crawl_with_renderer(&plan, &Site, Some(&Script), &CrawlControl::new(), |_| {})
        .unwrap();
    let reader = workspace.open(&meta.id).unwrap();
    for case in CASES {
        let detail = reader
            .detail(&format!("https://example.com/{case}"))
            .unwrap();
        let limit = match case {
            "finite" => 50,
            "unlimited" => -1,
            "generated" if mode == RenderMode::Html => 50,
            _ => 0,
        };
        assert_eq!(
            fact(&detail, "max_snippet"),
            &Cell::Integer(limit),
            "{case}"
        );
        assert_eq!(
            fact(&detail, "indexability"),
            &Cell::Text("indexable".into()),
            "{case}"
        );
        assert!(detail.html.is_none() && detail.raw_html.is_none());
    }
    verify_findings_and_export(&workspace, &reader, &meta.id, dir.path(), mode);
}

fn verify_findings_and_export(
    workspace: &SpiderWorkspace,
    reader: &CrawlReader,
    id: &str,
    directory: &Path,
    mode: RenderMode,
) {
    let page = reader
        .page(&PageRequest {
            tab: "internal".into(),
            issue_id: "snippet-restricted".into(),
            limit: 100,
            ..PageRequest::default()
        })
        .unwrap();
    let expected: BTreeSet<_> = CASES
        .iter()
        .filter(|case| {
            !matches!(**case, "finite" | "unlimited")
                && (**case != "generated" || mode == RenderMode::Javascript)
        })
        .map(|case| format!("https://example.com/{case}"))
        .collect();
    assert_eq!(
        page.total,
        if mode == RenderMode::Javascript { 7 } else { 6 }
    );
    assert_eq!(
        page.rows
            .iter()
            .map(|row| row[0].to_string())
            .collect::<BTreeSet<_>>(),
        expected
    );
    let path = directory.join("issues.json");
    workspace.export(id, CrawlTab::Issues, &path, None).unwrap();
    let exported: serde_json::Value =
        serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let actual: BTreeSet<_> = exported
        .as_array()
        .unwrap()
        .iter()
        .filter(|row| row["issue_id"] == "snippet-restricted")
        .map(|row| row["address"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(actual, expected);
}

#[test]
fn html_snippet_limits_reach_saved_facts_findings_and_exports_without_html_storage() {
    verify(RenderMode::Html);
}

#[test]
fn rendered_snippet_limits_preserve_http_restrictions_and_analyze_dom_changes() {
    verify(RenderMode::Javascript);
}
