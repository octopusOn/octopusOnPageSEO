//! Only eligible HTML alternates may affect discovery, reciprocity or saved hreflang.

use std::collections::BTreeSet;
use std::sync::Mutex;
use std::time::Duration;

use super::*;
use octopuson_crawl::{
    FetchError, FetchRequest, FetchResponse, Fetcher, RenderRequest, RenderedPage, Renderer,
};

const ORIGIN: &str = "https://example.com";
const SOURCE: &str = "<html lang='en'><head><title>Source</title><link rel='canonical' href='/source'>\
    <link rel='alternate' hreflang='en' href='/source'><link rel='alternate' hreflang='x-default' href='/source'>\
    <link rel='ALTERNATE alternate' hreflang='fr' href='/head-target'>\
    <link rel='alternate' hreflang='xx_BAD' media='screen' href='/ignored-media'>\
    <link rel='alternate stylesheet' hreflang='xx_BAD' media='print' href='/style.css'>\
    <template><link rel='alternate' hreflang='xx_BAD' href='/ignored-template'></template></head>\
    <body><h1>Source</h1><link rel='alternate' hreflang='xx_BAD' href='/ignored-body'></body></html>";

#[derive(Default)]
struct Site(Mutex<Vec<String>>);
impl Fetcher for Site {
    fn fetch(&self, request: &FetchRequest<'_>) -> Result<FetchResponse, FetchError> {
        self.0.lock().unwrap().push(request.url.into());
        let (kind, links, body) = match request.url.strip_prefix(ORIGIN).unwrap() {
            "/source" => (
                "text/html",
                "</header.pdf>; rel=alternate; hreflang=tr",
                SOURCE,
            ),
            "/head-target" => (
                "text/html",
                "",
                "<html lang='fr'><head><title>Head target</title><link rel='alternate' hreflang='fr' href='/head-target'></head><body><h1>Target</h1><link rel='alternate' hreflang='en' href='/source'></body></html>",
            ),
            "/header.pdf" => (
                "application/pdf",
                "</header.pdf>; rel=alternate; hreflang=tr, </source>; rel=alternate; hreflang=en, </source>; rel=alternate; hreflang=x-default",
                "%PDF-1.4\n%%EOF",
            ),
            "/js-target" => (
                "text/html",
                "",
                "<html lang='de'><head><title>JS target</title><link rel='alternate' hreflang='de' href='/js-target'><link rel='alternate' hreflang='en' href='/source'><link rel='alternate' hreflang='x-default' href='/source'></head><body><h1>JS target</h1></body></html>",
            ),
            "/style.css" => ("text/css", "", "body { color: black; }"),
            "/llms.txt" => ("text/plain", "", "# Local fixture"),
            _ => return Ok(FetchResponse::new(404, &[], "Unexpected request")),
        };
        Ok(FetchResponse::new(
            200,
            &[("Content-Type", kind), ("Link", links)],
            body,
        ))
    }
}

struct Script(bool);
impl Renderer for Script {
    fn render(&self, request: &RenderRequest<'_>) -> Result<RenderedPage, FetchError> {
        let mut html = String::from_utf8(request.response.body.clone()).unwrap();
        if request.url.ends_with("/source") {
            if self.0 {
                return Err(FetchError::Other("intentional failure".into()));
            }
            html = html.replace("</head>","<link rel='alternate' hreflang='de' href='/js-target'><link rel='alternate' hreflang='xx_BAD' media='' href='/ignored-dom-media'></head>")
                .replace("</body>","<link rel='alternate' hreflang='xx_BAD' href='/ignored-dom-body'></body>");
        }
        Ok(RenderedPage {
            html,
            url: request.url.into(),
            elapsed: Duration::ZERO,
            truncated: false,
        })
    }
}

fn exported(
    workspace: &SpiderWorkspace,
    id: &str,
    dir: &Path,
    tab: CrawlTab,
) -> Vec<serde_json::Value> {
    let path = dir.join(format!("{}.json", tab.name()));
    workspace.export(id, tab, &path, None).unwrap();
    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
}

fn expected(javascript: bool) -> BTreeSet<(String, String, String)> {
    let mut rows = vec![
        ("source", "en", "source"),
        ("source", "x-default", "source"),
        ("source", "fr", "head-target"),
        ("source", "tr", "header.pdf"),
        ("head-target", "fr", "head-target"),
        ("header.pdf", "tr", "header.pdf"),
        ("header.pdf", "en", "source"),
        ("header.pdf", "x-default", "source"),
    ];
    if javascript {
        rows.extend([
            ("source", "de", "js-target"),
            ("js-target", "de", "js-target"),
            ("js-target", "en", "source"),
            ("js-target", "x-default", "source"),
        ]);
    }
    rows.into_iter()
        .map(|(page, lang, target)| {
            (
                format!("{ORIGIN}/{page}"),
                lang.into(),
                format!("{ORIGIN}/{target}"),
            )
        })
        .collect()
}

fn verify_reports(
    workspace: &SpiderWorkspace,
    id: &str,
    directory: &Path,
    javascript: bool,
    fail: bool,
) {
    let rows = exported(workspace, id, directory, CrawlTab::Hreflang);
    let triples = rows
        .iter()
        .map(|row| {
            (
                row["page"].as_str().unwrap().into(),
                row["lang"].as_str().unwrap().into(),
                row["target"].as_str().unwrap().into(),
            )
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(triples, expected(javascript));
    assert_eq!(rows.len(), triples.len());
    assert!(rows.iter().all(|row| row["target_status"] == 200));
    let links = exported(workspace, id, directory, CrawlTab::Links);
    assert_eq!(
        links.iter().filter(|row| row["kind"] == "hreflang").count(),
        rows.len()
    );
    assert!(
        links.iter().any(|row| row["kind"] == "stylesheet"
            && row["destination"] == format!("{ORIGIN}/style.css"))
    );
    let findings = exported(workspace, id, directory, CrawlTab::Issues);
    let affected = |rule: &str| {
        findings
            .iter()
            .filter(|row| row["issue_id"] == rule)
            .map(|row| row["address"].as_str().unwrap())
            .collect::<Vec<_>>()
    };
    assert_eq!(
        affected("hreflang-missing-return"),
        [format!("{ORIGIN}/source")]
    );
    for rule in [
        "hreflang-invalid-code",
        "hreflang-missing-self",
        "hreflang-to-non-200",
        "hreflang-to-non-indexable",
        "internal-client-error",
    ] {
        assert_eq!(affected(rule), Vec::<&str>::new(), "{rule}");
    }
    assert_eq!(affected("render-error").len(), usize::from(fail));
}

fn verify(mode: RenderMode, fail: bool) {
    let dir = tempfile::tempdir().unwrap();
    let workspace = SpiderWorkspace::resolve(Some(dir.path().into())).unwrap();
    let mut config = CrawlConfig {
        start_urls: vec![format!("{ORIGIN}/source")],
        robots: RobotsPolicy::Ignore,
        ..CrawlConfig::default()
    };
    config.rendering.mode = mode;
    config.follow.sitemaps = false;
    config.follow.stylesheets = true;
    config.storage.headers = false;
    config.storage.html = false;
    let site = Site::default();
    let meta = workspace
        .crawl_with_renderer(
            &CrawlPlan::new(config).unwrap(),
            &site,
            Some(&Script(fail)),
            &CrawlControl::new(),
            |_| {},
        )
        .unwrap();
    let javascript = mode == RenderMode::Javascript && !fail;
    assert_eq!(meta.status, CrawlStatus::Completed);
    assert_eq!(meta.progress.crawled, if javascript { 5 } else { 4 });
    assert_eq!(meta.progress.errors, 0);
    verify_reports(&workspace, &meta.id, dir.path(), javascript, fail);
    let reader = workspace.open(&meta.id).unwrap();
    let detail = reader.detail(&format!("{ORIGIN}/source")).unwrap();
    assert_eq!(detail.headers.total, 0);
    assert!(detail.html.is_none() && detail.raw_html.is_none());
    let calls = site.0.lock().unwrap();
    assert!(calls.iter().all(|url| !url.contains("/ignored-")));
    assert_eq!(
        calls
            .iter()
            .filter(|url| url.ends_with("/head-target"))
            .count(),
        1
    );
    assert_eq!(
        calls
            .iter()
            .filter(|url| url.ends_with("/style.css"))
            .count(),
        1
    );
}

#[test]
fn html_hreflang_eligibility_controls_discovery_reciprocity_and_exports() {
    verify(RenderMode::Html, false);
}
#[test]
fn javascript_hreflang_eligibility_applies_to_the_observed_dom() {
    verify(RenderMode::Javascript, false);
}
#[test]
fn failed_javascript_hreflang_eligibility_uses_source_and_preserves_headers() {
    verify(RenderMode::Javascript, true);
}
