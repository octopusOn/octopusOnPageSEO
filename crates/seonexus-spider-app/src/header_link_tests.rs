//! HTTP annotations must drive the same stored facts, discovery and exports in both modes.

use std::collections::BTreeSet;
use std::sync::Mutex;
use std::time::Duration;

use super::*;
use seonexus_core::table::Cell;
use seonexus_crawl::{
    FetchError, FetchRequest, FetchResponse, Fetcher, RenderRequest, RenderedPage, Renderer,
};

const CASES: [&str; 9] = [
    "quoted",
    "escaped",
    "duplicate-ignore",
    "duplicate-keep",
    "scoped",
    "self",
    "document.pdf",
    "malformed",
    "nonweb",
];
const MISSING: [&str; 5] = [
    "quoted",
    "duplicate-ignore",
    "scoped",
    "malformed",
    "nonweb",
];

#[derive(Default)]
struct Site(Mutex<BTreeSet<String>>);

impl Fetcher for Site {
    fn fetch(&self, request: &FetchRequest<'_>) -> Result<FetchResponse, FetchError> {
        self.0.lock().unwrap().insert(request.url.into());
        let path = request.url.strip_prefix("https://example.com/").unwrap();
        let link = match path {
            "quoted" => {
                r#"</ignored-title>; rel=preload; title="Manual; rel=canonical; still a title""#
            }
            "escaped" => {
                r#"</canonical>; title="A \"quoted, manual\""; rel="can\onical", </en>; rel=alternate; hreflang="e\n""#
            }
            "duplicate-ignore" => "</ignored-duplicate>; rel=preload; rel=canonical",
            "duplicate-keep" => "</canonical>; rel=canonical; rel=preload",
            "scoped" => r#"</ignored-anchor>; rel=canonical; anchor="/other-document""#,
            "self" => r#"</canonical>; rel=canonical; anchor="/self""#,
            "document.pdf" => {
                r##"</canonical>; rel=canonical; anchor="/document.pdf", </en>; rel=alternate; hreflang=en, </ignored-pdf>; rel=canonical; anchor="#section""##
            }
            "malformed" => r#"</ignored-malformed>; rel=canonical; title="unterminated"#,
            "nonweb" => {
                "<javascript:alert(1)>; rel=canonical, <ftp://example.com/file>; rel=alternate; hreflang=en"
            }
            "canonical" | "en" | "dom" => "",
            _ => return Ok(FetchResponse::new(404, &[], "")),
        };
        let pdf = path == "document.pdf";
        let headers = [
            (
                "Content-Type",
                if pdf { "application/pdf" } else { "text/html" },
            ),
            ("Link", link),
        ];
        let body = if pdf {
            "%PDF-1.4\n%%EOF".into()
        } else {
            format!(
                "<head><base href='/wrong/'><title>{path}</title>{}</head><body><h1>{path}</h1></body>",
                if matches!(path, "canonical" | "en" | "dom") {
                    format!("<link rel='canonical' href='/{path}'>")
                } else {
                    String::new()
                }
            )
        };
        Ok(FetchResponse::new(200, &headers, body))
    }
}

struct Script;

impl Renderer for Script {
    fn render(&self, request: &RenderRequest<'_>) -> Result<RenderedPage, FetchError> {
        let mut html = String::from_utf8(request.response.body.clone()).unwrap();
        if request.url.ends_with("/escaped") {
            html = html.replace("</head>", "<link rel='canonical' href='/dom'></head>");
        }
        Ok(RenderedPage {
            html,
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
    let reader = workspace.open(&meta.id).unwrap();
    let mut expected: BTreeSet<String> = CASES.into_iter().map(str::to_owned).collect();
    expected.extend(["canonical".into(), "en".into()]);
    let javascript = mode == RenderMode::Javascript;
    if javascript {
        expected.insert("dom".into());
    }
    let mut requests: BTreeSet<String> = expected
        .iter()
        .map(|path| format!("https://example.com/{path}"))
        .collect();
    requests.insert("https://example.com/llms.txt".into());
    assert_eq!(*site.0.lock().unwrap(), requests);
    let overview = reader.overview().unwrap();
    assert_eq!(overview.internal, expected.len() as u64);
    assert_eq!(overview.external, 0);
    assert_eq!(overview.indexable, if javascript { 8 } else { 7 });
    for case in CASES {
        let detail = reader
            .detail(&format!("https://example.com/{case}"))
            .unwrap();
        let annotated = !MISSING.contains(&case);
        assert_eq!(
            fact(&detail, "header_canonical"),
            &if annotated {
                Cell::Text("https://example.com/canonical".into())
            } else {
                Cell::Null
            },
            "{case}"
        );
        assert_eq!(
            fact(&detail, "indexability"),
            &Cell::Text(
                if annotated {
                    "canonicalised"
                } else {
                    "indexable"
                }
                .into()
            ),
            "{case}"
        );
        if javascript && case != "document.pdf" {
            assert_eq!(
                fact(&detail, "raw_canonical"),
                fact(&detail, "header_canonical")
            );
            assert_eq!(fact(&detail, "rendered"), &Cell::Boolean(true));
        }
        assert!(detail.html.is_none() && detail.raw_html.is_none());
    }
    let escaped = reader.detail("https://example.com/escaped").unwrap();
    assert_eq!(
        fact(&escaped, "canonical"),
        &if javascript {
            Cell::Text("https://example.com/dom".into())
        } else {
            Cell::Null
        }
    );
    verify_findings_and_exports(&workspace, &reader, &meta.id, dir.path(), javascript);
}

fn verify_findings_and_exports(
    workspace: &SpiderWorkspace,
    reader: &CrawlReader,
    id: &str,
    directory: &Path,
    javascript: bool,
) {
    let missing = reader
        .page(&PageRequest {
            tab: "internal".into(),
            issue_id: "canonical-missing".into(),
            limit: 100,
            ..PageRequest::default()
        })
        .unwrap();
    assert_eq!(missing.total, MISSING.len() as u64);
    assert_eq!(
        missing
            .rows
            .iter()
            .map(|row| row[0].to_string())
            .collect::<BTreeSet<_>>(),
        MISSING
            .iter()
            .map(|path| format!("https://example.com/{path}"))
            .collect()
    );
    let path = directory.join("links.json");
    workspace.export(id, CrawlTab::Links, &path, None).unwrap();
    let exported: serde_json::Value =
        serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let links = exported.as_array().unwrap();
    assert_eq!(links.len(), if javascript { 10 } else { 8 });
    for row in links {
        let target = row["destination"].as_str().unwrap();
        assert!(
            [
                "https://example.com/canonical",
                "https://example.com/en",
                "https://example.com/dom"
            ]
            .contains(&target)
        );
        assert_eq!(row["destination_status"], 200);
        if row["kind"] == "hreflang" {
            assert_eq!(target, "https://example.com/en");
            assert_eq!(row["text"], "en");
        }
    }
    assert_eq!(
        links.iter().filter(|row| row["kind"] == "hreflang").count(),
        2
    );
    let path = directory.join("urls.json");
    workspace
        .export(id, CrawlTab::Internal, &path, None)
        .unwrap();
    let exported: serde_json::Value =
        serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let quoted = exported
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["address"] == "https://example.com/quoted")
        .unwrap();
    assert!(quoted.get("header_canonical").unwrap().is_null());
    assert_eq!(quoted["indexability"], "indexable");
}

#[test]
fn http_annotations_drive_html_facts_discovery_audits_and_exports() {
    verify(RenderMode::Html);
}

#[test]
fn http_annotations_preserve_raw_baselines_and_dom_canonical_precedence() {
    verify(RenderMode::Javascript);
}
