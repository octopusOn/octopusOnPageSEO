//! Canonical eligibility propagates through HTML/DOM facts, discovery, audits and exports.

use std::sync::Mutex;
use std::time::Duration;

use super::*;
use octopuson_core::table::Cell;
use octopuson_crawl::{
    FetchError, FetchRequest, FetchResponse, Fetcher, RenderRequest, RenderedPage, Renderer,
};

const CASES: [&str; 6] = [
    "body",
    "qualified",
    "head",
    "header",
    "javascript",
    "multiple",
];
const GENERATED: &str = "<html><head><title>Generated canonical</title><link rel='canonical' href='/target'></head><body><link rel='canonical' href='/ignored-dom'></body></html>";

#[derive(Default)]
struct Site(Mutex<Vec<String>>);

impl Fetcher for Site {
    fn fetch(&self, request: &FetchRequest<'_>) -> Result<FetchResponse, FetchError> {
        self.0.lock().unwrap().push(request.url.to_owned());
        let path = request.url.strip_prefix("https://example.com").unwrap();
        let mut headers = vec![("Content-Type", "text/html")];
        let body = match path {
            "/body" => {
                "<html><head><title>Body canonical</title></head><body><link rel='canonical' href='/ignored-body'></body></html>"
            }
            "/qualified" => {
                "<head><title>Qualified canonical</title><link rel='canonical' type='text/html' href='/ignored-attribute'></head>"
            }
            "/head" => {
                "<head><title>Head canonical</title><link rel='CANONICAL canonical' href='/target'></head>"
            }
            "/header" => {
                headers.push(("Link", "<https://example.com/target>; rel=canonical"));
                "<html><head><title>Header canonical</title></head><body><link rel='canonical' href='/ignored-header-body'></body></html>"
            }
            "/javascript" => {
                "<html><head><title>Before JavaScript</title></head><body><link rel='canonical' href='/ignored-raw'></body></html>"
            }
            "/multiple" => {
                "<head><title>Multiple declarations</title><link rel='canonical' href='/target'><link rel='canonical' href='/target-two'></head>"
            }
            "/target" => "<head><title>Target</title><link rel='canonical' href='/target'></head>",
            "/target-two" => {
                "<head><title>Second target</title><link rel='canonical' href='/target-two'></head>"
            }
            _ => return Ok(FetchResponse::new(404, &[], "")),
        };
        Ok(FetchResponse::new(200, &headers, body))
    }
}

struct Script;

impl Renderer for Script {
    fn render(&self, request: &RenderRequest<'_>) -> Result<RenderedPage, FetchError> {
        Ok(RenderedPage {
            html: if request.url.ends_with("/javascript") {
                GENERATED.to_owned()
            } else {
                String::from_utf8(request.response.body.clone()).unwrap()
            },
            url: request.url.to_owned(),
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
    let site = Site::default();
    let meta = workspace
        .crawl_with_renderer(&plan, &site, Some(&Script), &CrawlControl::new(), |_| {})
        .unwrap();
    let reader = workspace.open(&meta.id).unwrap();
    assert!(
        !site
            .0
            .lock()
            .unwrap()
            .iter()
            .any(|url| url.contains("/ignored-"))
    );
    verify_facts(&reader, mode);
    verify_findings(&reader, mode);
    verify_export(&workspace, &meta.id, dir.path());
}

fn verify_facts(reader: &CrawlReader, mode: RenderMode) {
    for case in ["body", "qualified", "header"] {
        let detail = reader
            .detail(&format!("https://example.com/{case}"))
            .unwrap();
        assert_eq!(fact(&detail, "canonical"), &Cell::Null, "{case}");
        assert_eq!(
            fact(&detail, "canonical_count"),
            &Cell::Integer(0),
            "{case}"
        );
        assert_eq!(
            fact(&detail, "indexability"),
            &Cell::Text(
                if case == "header" {
                    "canonicalised"
                } else {
                    "indexable"
                }
                .into()
            )
        );
        if mode == RenderMode::Javascript {
            assert_eq!(
                fact(&detail, "raw_canonical"),
                &if case == "header" {
                    Cell::Text("https://example.com/target".into())
                } else {
                    Cell::Null
                }
            );
        }
        assert!(detail.html.is_none() && detail.raw_html.is_none());
    }
    let head = reader.detail("https://example.com/head").unwrap();
    assert_eq!(fact(&head, "canonical_count"), &Cell::Integer(1));
    assert_eq!(
        fact(&head, "canonical"),
        &Cell::Text("https://example.com/target".into())
    );
    let javascript = reader.detail("https://example.com/javascript").unwrap();
    assert_eq!(
        fact(&javascript, "canonical"),
        &if mode == RenderMode::Javascript {
            Cell::Text("https://example.com/target".into())
        } else {
            Cell::Null
        }
    );
    if mode == RenderMode::Javascript {
        assert_eq!(fact(&javascript, "raw_canonical"), &Cell::Null);
    }
}

fn verify_findings(reader: &CrawlReader, mode: RenderMode) {
    let multiple = reader
        .page(&PageRequest {
            tab: "internal".into(),
            issue_id: "canonical-multiple".into(),
            ..PageRequest::default()
        })
        .unwrap();
    assert_eq!(multiple.total, 1);
    assert_eq!(
        multiple.rows[0][0],
        Cell::Text("https://example.com/multiple".into())
    );
    assert_eq!(
        reader
            .page(&PageRequest {
                tab: "internal".into(),
                issue_id: "canonical-mismatch".into(),
                ..PageRequest::default()
            })
            .unwrap()
            .total,
        0
    );
    let missing = reader
        .page(&PageRequest {
            tab: "internal".into(),
            issue_id: "canonical-missing".into(),
            ..PageRequest::default()
        })
        .unwrap();
    assert_eq!(
        missing.total,
        if mode == RenderMode::Javascript { 2 } else { 3 }
    );
}

fn verify_export(workspace: &SpiderWorkspace, id: &str, directory: &Path) {
    let path = directory.join("canonicals.json");
    workspace
        .export(id, CrawlTab::Internal, &path, None)
        .unwrap();
    let exported: serde_json::Value =
        serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let body = exported
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["address"] == "https://example.com/body")
        .unwrap();
    assert!(body.get("canonical").unwrap().is_null());
    assert_eq!(body["indexability"], "indexable");
    assert_eq!(exported.as_array().unwrap().len(), 8);
    let header = exported
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["address"] == "https://example.com/header")
        .unwrap();
    assert_eq!(header["header_canonical"], "https://example.com/target");
}

#[test]
fn html_canonical_eligibility_drives_discovery_indexability_audits_and_exports() {
    verify(RenderMode::Html);
}

#[test]
fn javascript_canonical_eligibility_preserves_raw_facts_and_header_fallback() {
    verify(RenderMode::Javascript);
}
