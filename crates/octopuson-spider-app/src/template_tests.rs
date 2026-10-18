//! Inert templates must not affect observations until their content enters the document.

use std::sync::Mutex;
use std::time::Duration;

use super::*;
use octopuson_core::table::Cell;
use octopuson_crawl::{
    FetchError, FetchRequest, FetchResponse, Fetcher, RenderRequest, RenderedPage, Renderer,
};

const INERT: &str = "<template><base href='/wrong/'><title>Unused title</title><meta name='robots' content='nofollow'><link rel='stylesheet' href='/unused.css'><img src='/unused.png'><script src='/unused.js'></script><script type='application/ld+json'>invalid JSON</script><template><a href='/unused'>Unused</a></template></template>";
const CONTENT: &str = "<meta name='robots' content='noindex'><h2>Activated heading</h2><a href='/activated'>Activated link</a><script type='application/ld+json'>{\"@type\":\"Product\"}</script>";

fn source(activate: bool) -> String {
    format!(
        "<html><head><title>Live page</title>{INERT}</head><body><h1>Live heading</h1><a href='live'>Live link</a><template>{CONTENT}</template>{}</body></html>",
        if activate { CONTENT } else { "" }
    )
}

#[derive(Default)]
struct Site(Mutex<Vec<String>>);
impl Fetcher for Site {
    fn fetch(&self, request: &FetchRequest<'_>) -> Result<FetchResponse, FetchError> {
        self.0.lock().unwrap().push(request.url.into());
        let body = match request.url {
            "https://example.com/" => source(false),
            "https://example.com/live" | "https://example.com/activated" => {
                "<head><title>Destination</title></head><body><h1>Destination</h1></body>".into()
            }
            _ => return Ok(FetchResponse::new(404, &[], "")),
        };
        Ok(FetchResponse::new(
            200,
            &[("Content-Type", "text/html")],
            body,
        ))
    }
}

struct Script {
    fail_root: bool,
}
impl Renderer for Script {
    fn render(&self, request: &RenderRequest<'_>) -> Result<RenderedPage, FetchError> {
        if self.fail_root && request.url == "https://example.com/" {
            return Err(FetchError::Other("intentional rendering failure".into()));
        }
        Ok(RenderedPage {
            html: if request.url == "https://example.com/" {
                source(true)
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

fn verify(mode: RenderMode, fail_root: bool) {
    let activated = mode == RenderMode::Javascript && !fail_root;
    let dir = tempfile::tempdir().unwrap();
    let workspace = SpiderWorkspace::resolve(Some(dir.path().into())).unwrap();
    let mut config = CrawlConfig {
        start_urls: vec!["https://example.com/".into()],
        robots: RobotsPolicy::Ignore,
        ..CrawlConfig::default()
    };
    config.rendering.mode = mode;
    config.follow.sitemaps = false;
    config.storage.html = false;
    let plan = CrawlPlan::new(config).unwrap();
    let site = Site::default();
    let meta = workspace
        .crawl_with_renderer(
            &plan,
            &site,
            Some(&Script { fail_root }),
            &CrawlControl::new(),
            |_| {},
        )
        .unwrap();
    let reader = workspace.open(&meta.id).unwrap();
    assert!(
        !site
            .0
            .lock()
            .unwrap()
            .iter()
            .any(|url| url.contains("/unused") || url.contains("/wrong/"))
    );
    let urls = reader
        .page(&PageRequest {
            tab: "internal".into(),
            limit: 100,
            ..PageRequest::default()
        })
        .unwrap();
    assert_eq!(urls.total, if activated { 3 } else { 2 });
    assert_eq!(
        urls.rows
            .iter()
            .filter(|row| row[0].to_string().ends_with("/activated"))
            .count(),
        usize::from(activated)
    );
    verify_facts(&reader, mode, activated);
    verify_findings_and_export(&workspace, &reader, &meta.id, dir.path(), activated);
    assert_eq!(
        reader
            .page(&PageRequest {
                tab: "internal".into(),
                issue_id: "render-error".into(),
                ..PageRequest::default()
            })
            .unwrap()
            .total,
        u64::from(mode == RenderMode::Javascript && fail_root)
    );
}

fn verify_facts(reader: &CrawlReader, mode: RenderMode, activated: bool) {
    let detail = reader.detail("https://example.com/").unwrap();
    assert_eq!(fact(&detail, "title"), &Cell::Text("Live page".into()));
    assert_eq!(fact(&detail, "title_count"), &Cell::Integer(1));
    assert_eq!(fact(&detail, "base_href"), &Cell::Null);
    assert_eq!(fact(&detail, "nofollow"), &Cell::Boolean(false));
    assert_eq!(fact(&detail, "noindex"), &Cell::Boolean(activated));
    assert_eq!(fact(&detail, "h1_count"), &Cell::Integer(1));
    assert_eq!(
        fact(&detail, "h2_count"),
        &Cell::Integer(i64::from(activated))
    );
    if mode == RenderMode::Javascript {
        assert_eq!(fact(&detail, "raw_noindex"), &Cell::Boolean(false));
        assert_eq!(fact(&detail, "raw_nofollow"), &Cell::Boolean(false));
        assert_eq!(fact(&detail, "raw_link_count"), &Cell::Integer(1));
        assert_eq!(
            fact(&detail, "rendered_link_count"),
            &if activated {
                Cell::Integer(2)
            } else {
                Cell::Null
            }
        );
    }
    assert!(detail.html.is_none() && detail.raw_html.is_none());
    let structured = reader
        .page(&PageRequest {
            tab: "structured-data".into(),
            limit: 100,
            ..PageRequest::default()
        })
        .unwrap();
    assert_eq!(structured.total, u64::from(activated));
}

fn verify_findings_and_export(
    workspace: &SpiderWorkspace,
    reader: &CrawlReader,
    id: &str,
    directory: &Path,
    activated: bool,
) {
    for (issue, expected) in [
        ("noindex", u64::from(activated)),
        ("structured-data-invalid", 0),
        ("title-multiple", 0),
    ] {
        assert_eq!(
            reader
                .page(&PageRequest {
                    tab: "internal".into(),
                    issue_id: issue.into(),
                    limit: 100,
                    ..PageRequest::default()
                })
                .unwrap()
                .total,
            expected,
            "{issue}"
        );
    }
    let path = directory.join("links.json");
    workspace.export(id, CrawlTab::Links, &path, None).unwrap();
    let exported: serde_json::Value =
        serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let destinations: Vec<_> = exported
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row["destination"].as_str().unwrap())
        .collect();
    assert_eq!(
        destinations,
        if activated {
            vec!["https://example.com/live", "https://example.com/activated"]
        } else {
            vec!["https://example.com/live"]
        }
    );
}

#[test]
fn inert_templates_do_not_change_html_metadata_discovery_or_exported_links() {
    verify(RenderMode::Html, false);
}

#[test]
fn javascript_activated_template_content_changes_facts_audits_and_discovery() {
    verify(RenderMode::Javascript, false);
}

#[test]
fn failed_render_fallback_keeps_template_metadata_and_links_inert() {
    verify(RenderMode::Javascript, true);
}
