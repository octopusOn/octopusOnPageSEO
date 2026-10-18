//! Header annotations must survive storage settings and participate in hreflang audits.

use std::collections::BTreeSet;
use std::sync::Mutex;
use std::time::Duration;

use super::*;
use octopuson_core::table::Cell;
use octopuson_crawl::{
    FetchError, FetchRequest, FetchResponse, Fetcher, RenderRequest, RenderedPage, Renderer,
};

const SEEDS: [&str; 4] = ["html-tr", "pdf-tr.pdf", "problem.pdf", "js"];
const DOM_ALTERNATE: &str = "<link rel='alternate' hreflang='fr' href='/js-alt'>";

#[derive(Default)]
struct Site(Mutex<Vec<String>>);

impl Fetcher for Site {
    fn fetch(&self, request: &FetchRequest<'_>) -> Result<FetchResponse, FetchError> {
        self.0.lock().unwrap().push(request.url.into());
        let path = request.url.strip_prefix("https://example.com/").unwrap();
        let links = match path {
            "html-tr" => {
                "</html-tr>; rel=alternate; hreflang=TR, </html-en>; rel=alternate; hreflang=en, </html-tr>; rel=alternate; hreflang=x-default"
            }
            "html-en" => {
                "</html-tr>; rel=alternate; hreflang=tr, </html-tr>; rel=alternate; hreflang=x-default"
            }
            "pdf-tr.pdf" => {
                "</pdf-tr.pdf>; rel=alternate; hreflang=tr, </pdf-en.pdf>; rel=alternate; hreflang=en, </pdf-tr.pdf>; rel=alternate; hreflang=x-default"
            }
            "pdf-en.pdf" => {
                "</pdf-en.pdf>; rel=alternate; hreflang=en; hreflang=en-GB, </pdf-tr.pdf>; rel=alternate; hreflang=tr, </pdf-tr.pdf>; rel=alternate; hreflang=x-default"
            }
            "problem.pdf" => {
                "</unreturned.pdf>; rel=alternate; hreflang=xx_BAD, </broken>; rel=alternate; hreflang=en, </noindex>; rel=alternate; hreflang=de, </ignored>; rel=alternate; hreflang=en; anchor=\"/other\", </ignored-help>; rel=help; hreflang=fr"
            }
            "js" => "</js>; rel=alternate; hreflang=en, </js>; rel=alternate; hreflang=x-default",
            "js-alt" => {
                "</js-alt>; rel=alternate; hreflang=fr, </js>; rel=alternate; hreflang=en, </js>; rel=alternate; hreflang=x-default"
            }
            "unreturned.pdf" | "noindex" => "",
            _ => return Ok(FetchResponse::new(404, &[], "")),
        };
        let pdf = Path::new(path)
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("pdf"));
        let mut headers = vec![
            (
                "Content-Type",
                if pdf { "application/pdf" } else { "text/html" },
            ),
            ("Link", links),
        ];
        if path == "pdf-tr.pdf" {
            headers.push(("Link", "</pdf-tr.pdf>; rel=alternate; hreflang=TR"));
        }
        let body = if pdf {
            "%PDF-1.4\n%%EOF".into()
        } else {
            let lang = if path == "html-tr" {
                "tr"
            } else if path == "js-alt" {
                "fr"
            } else {
                "en"
            };
            format!(
                "<html lang='{lang}'><head><title>{path}</title>{}</head><body><h1>{path}</h1></body></html>",
                match path {
                    "html-tr" => "<link rel='alternate' hreflang='tr' href='/html-tr'>",
                    "html-en" => "<link rel='alternate' hreflang='en' href='/html-en'>",
                    "noindex" => "<meta name='robots' content='noindex'>",
                    _ => "",
                }
            )
        };
        Ok(FetchResponse::new(200, &headers, body))
    }
}

struct Script {
    fail: bool,
}

impl Renderer for Script {
    fn render(&self, request: &RenderRequest<'_>) -> Result<RenderedPage, FetchError> {
        if self.fail && request.url.ends_with("/js") {
            return Err(FetchError::Other("intentional rendering failure".into()));
        }
        let mut html = String::from_utf8(request.response.body.clone()).unwrap();
        if request.url.ends_with("/js") {
            html = html.replace("</head>", &format!("{DOM_ALTERNATE}</head>"));
        }
        Ok(RenderedPage {
            html,
            url: request.url.into(),
            elapsed: Duration::ZERO,
            truncated: false,
        })
    }
}

fn expected(javascript: bool, list: bool) -> BTreeSet<(String, String, String)> {
    let mut entries = vec![
        ("html-tr", "tr", "html-tr"),
        ("html-tr", "en", "html-en"),
        ("html-tr", "x-default", "html-tr"),
        ("pdf-tr.pdf", "tr", "pdf-tr.pdf"),
        ("pdf-tr.pdf", "en", "pdf-en.pdf"),
        ("pdf-tr.pdf", "x-default", "pdf-tr.pdf"),
        ("problem.pdf", "xx_bad", "unreturned.pdf"),
        ("problem.pdf", "en", "broken"),
        ("problem.pdf", "de", "noindex"),
        ("js", "en", "js"),
        ("js", "x-default", "js"),
    ];
    if !list {
        entries.extend([
            ("html-en", "en", "html-en"),
            ("html-en", "tr", "html-tr"),
            ("html-en", "x-default", "html-tr"),
            ("pdf-en.pdf", "en", "pdf-en.pdf"),
            ("pdf-en.pdf", "en-gb", "pdf-en.pdf"),
            ("pdf-en.pdf", "tr", "pdf-tr.pdf"),
            ("pdf-en.pdf", "x-default", "pdf-tr.pdf"),
        ]);
    }
    if javascript {
        entries.extend([
            ("js", "fr", "js-alt"),
            ("js-alt", "fr", "js-alt"),
            ("js-alt", "en", "js"),
            ("js-alt", "x-default", "js"),
        ]);
    }
    entries
        .into_iter()
        .map(|(page, lang, target)| {
            (
                format!("https://example.com/{page}"),
                lang.into(),
                format!("https://example.com/{target}"),
            )
        })
        .collect()
}

fn verify(mode: RenderMode, fail: bool, list: bool) {
    let dir = tempfile::tempdir().unwrap();
    let workspace = SpiderWorkspace::resolve(Some(dir.path().into())).unwrap();
    let mut config = CrawlConfig {
        start_urls: SEEDS
            .iter()
            .map(|path| format!("https://example.com/{path}"))
            .collect(),
        mode: if list {
            CrawlMode::List
        } else {
            CrawlMode::Spider
        },
        robots: RobotsPolicy::Ignore,
        ..CrawlConfig::default()
    };
    config.storage.html = false;
    config.storage.headers = false;
    config.follow.sitemaps = false;
    config.follow.hreflang = !list;
    config.rendering.mode = mode;
    let site = Site::default();
    let meta = workspace
        .crawl_with_renderer(
            &CrawlPlan::new(config).unwrap(),
            &site,
            Some(&Script { fail }),
            &CrawlControl::new(),
            |_| {},
        )
        .unwrap();
    let reader = workspace.open(&meta.id).unwrap();
    let javascript = mode == RenderMode::Javascript && !fail;
    assert_eq!(
        reader.overview().unwrap().internal,
        if list {
            4
        } else if javascript {
            10
        } else {
            9
        }
    );
    assert!(
        !site
            .0
            .lock()
            .unwrap()
            .iter()
            .any(|url| url.contains("/ignored"))
    );
    for seed in SEEDS {
        let detail = reader
            .detail(&format!("https://example.com/{seed}"))
            .unwrap();
        assert!(detail.html.is_none() && detail.raw_html.is_none());
        assert_eq!(detail.headers.total, 0);
    }
    verify_report(&workspace, &reader, &meta.id, dir.path(), javascript, list);
    verify_findings(&workspace, &reader, &meta.id, dir.path(), list);
}

fn verify_report(
    workspace: &SpiderWorkspace,
    reader: &CrawlReader,
    id: &str,
    directory: &Path,
    javascript: bool,
    list: bool,
) {
    let expected = expected(javascript, list);
    let page = reader
        .page(&PageRequest {
            tab: "hreflang".into(),
            limit: 100,
            ..PageRequest::default()
        })
        .unwrap();
    assert_eq!(page.total, expected.len() as u64);
    let path = directory.join("hreflang.json");
    workspace
        .export(id, CrawlTab::Hreflang, &path, None)
        .unwrap();
    let rows: serde_json::Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let rows = rows.as_array().unwrap();
    assert_eq!(
        rows.len(),
        expected.len(),
        "identical HTML/header and repeated header annotations are merged"
    );
    assert_eq!(
        rows.iter()
            .map(|row| (
                row["page"].as_str().unwrap().to_owned(),
                row["lang"].as_str().unwrap().to_ascii_lowercase(),
                row["target"].as_str().unwrap().to_owned()
            ))
            .collect::<BTreeSet<_>>(),
        expected
    );
    for row in rows {
        let target = row["target"].as_str().unwrap();
        let status = if list
            && !SEEDS
                .iter()
                .any(|seed| target.ends_with(&format!("/{seed}")))
        {
            serde_json::Value::Null
        } else {
            serde_json::json!(if target.ends_with("/broken") {
                404
            } else {
                200
            })
        };
        assert_eq!(row["target_status"], status, "{target}");
    }
}

fn verify_findings(
    workspace: &SpiderWorkspace,
    reader: &CrawlReader,
    id: &str,
    directory: &Path,
    list: bool,
) {
    for issue in [
        "hreflang-invalid-code",
        "hreflang-missing-self",
        "hreflang-missing-x-default",
        "hreflang-missing-return",
        "hreflang-to-non-200",
        "hreflang-to-non-indexable",
    ] {
        let findings = reader
            .page(&PageRequest {
                tab: "internal".into(),
                issue_id: issue.into(),
                limit: 100,
                ..PageRequest::default()
            })
            .unwrap();
        let observed = !list
            || matches!(
                issue,
                "hreflang-invalid-code" | "hreflang-missing-self" | "hreflang-missing-x-default"
            );
        assert_eq!(findings.total, u64::from(observed), "{issue}");
        if observed {
            assert_eq!(
                findings.rows[0][0],
                Cell::Text("https://example.com/problem.pdf".into()),
                "{issue}"
            );
        }
    }
    let path = directory.join("issues.json");
    workspace.export(id, CrawlTab::Issues, &path, None).unwrap();
    let issues: serde_json::Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    assert_eq!(
        issues
            .as_array()
            .unwrap()
            .iter()
            .filter(|row| row["issue_id"].as_str().unwrap().starts_with("hreflang-"))
            .count(),
        if list { 3 } else { 6 }
    );
}

#[test]
fn header_hreflang_reaches_reports_and_pdf_audits_without_saved_sources() {
    verify(RenderMode::Html, false, false);
}
#[test]
fn header_hreflang_and_dom_annotations_share_discovery_audits_and_exports() {
    verify(RenderMode::Javascript, false, false);
}
#[test]
fn header_hreflang_survives_failed_render_fallback() {
    verify(RenderMode::Javascript, true, false);
}
#[test]
fn list_crawls_store_unfollowed_annotations_without_assuming_target_failures() {
    verify(RenderMode::Html, false, true);
}
