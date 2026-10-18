//! Character decoding must preserve SEO text, URL discovery and raw/DOM comparison facts.

use std::sync::Mutex;
use std::time::Duration;

use super::*;
use octopuson_core::table::Cell;
use octopuson_crawl::{
    FetchError, FetchRequest, FetchResponse, Fetcher, RenderRequest, RenderedPage, Renderer,
};

const CASES: [&str; 9] = [
    "comment",
    "invalid-header",
    "prefix-bytes",
    "pragma",
    "quoted-header",
    "utf8-decoy",
    "meta-utf16",
    "bom",
    "xml-signature",
];
const TITLE: &str = "Çağrı Şirketi";
const DESCRIPTION: &str = "İletişim açıklaması";
const TARGET: &str = "https://example.com/ileti%C5%9Fim";
const DOCUMENT: &str = "<html><head><title>Çağrı Şirketi</title><meta name='description' content='İletişim açıklaması'><link rel='canonical' href='/iletişim'></head><body><h1>Çağrı Şirketi</h1><a href='/iletişim'>İletişim</a></body></html>";

fn legacy(text: &str) -> Vec<u8> {
    text.chars()
        .map(|ch| match ch {
            'Ç' => 0xc7,
            'ğ' => 0xf0,
            'ı' => 0xfd,
            'Ş' => 0xde,
            'İ' => 0xdd,
            'ş' => 0xfe,
            'ç' => 0xe7,
            ch if ch.is_ascii() => ch as u8,
            _ => panic!("unsupported fixture character {ch}"),
        })
        .collect()
}

fn response(case: &str) -> FetchResponse {
    let (header, prefix, legacy_body) = match case {
        "comment" => (
            "text/html",
            "<!-- charset=utf-8 --><META CHARSET = 'windows-1254'>",
            true,
        ),
        "invalid-header" => (
            "text/html; charset=invalid",
            "<meta charset=windows-1254>",
            true,
        ),
        "prefix-bytes" => ("text/html", "<!-- ş --><meta charset=windows-1254>", true),
        "pragma" => (
            "text/html",
            "<meta content=\"text/html; CHARSET = 'windows-1254'\" http-equiv=content-type>",
            true,
        ),
        "quoted-header" => (
            "text/html; note=\"x;charset=utf-8\"; charset=\"windows-1254\"",
            "<meta charset=utf-8>",
            true,
        ),
        "utf8-decoy" => (
            "text/html",
            "<!-- charset=windows-1254 --><meta charset=utf-8>",
            false,
        ),
        "meta-utf16" => ("text/html", "<meta charset=utf-16>", false),
        "bom" => (
            "text/html; charset=windows-1254",
            "\u{feff}<meta charset=windows-1254>",
            false,
        ),
        "xml-signature" => ("text/html", "<?xml version='1.0'?>", false),
        _ => ("text/html; charset=utf-8", "", false),
    };
    let text = format!("{prefix}{DOCUMENT}");
    let bytes = if case == "xml-signature" {
        text.encode_utf16().flat_map(u16::to_le_bytes).collect()
    } else if legacy_body {
        legacy(&text)
    } else {
        text.into_bytes()
    };
    FetchResponse::new(200, &[("Content-Type", header)], bytes)
}

#[derive(Default)]
struct Site(Mutex<Vec<String>>);
impl Fetcher for Site {
    fn fetch(&self, request: &FetchRequest<'_>) -> Result<FetchResponse, FetchError> {
        self.0.lock().unwrap().push(request.url.into());
        let case = request.url.strip_prefix("https://example.com/").unwrap();
        if CASES.contains(&case) || request.url == TARGET {
            Ok(response(case))
        } else {
            Ok(FetchResponse::new(404, &[], ""))
        }
    }
}

struct Script {
    fail: bool,
}
impl Renderer for Script {
    fn render(&self, request: &RenderRequest<'_>) -> Result<RenderedPage, FetchError> {
        if self.fail && request.url.ends_with("/comment") {
            return Err(FetchError::Other("intentional render failure".into()));
        }
        Ok(RenderedPage {
            html: DOCUMENT.into(),
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

fn verify(mode: RenderMode, fail: bool) {
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
    config.storage.headers = false;
    config.speed.max_requests_per_second = 0.0;
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
    assert_eq!(meta.status, CrawlStatus::Completed);
    assert_eq!(meta.progress.crawled, 10);
    assert_eq!(meta.progress.errors, 0);
    let reader = workspace.open(&meta.id).unwrap();
    for url in CASES
        .iter()
        .map(|case| format!("https://example.com/{case}"))
        .chain([TARGET.into()])
    {
        let detail = reader.detail(&url).unwrap();
        for (name, text) in [
            ("title", TITLE),
            ("meta_description", DESCRIPTION),
            ("h1", TITLE),
            ("canonical", TARGET),
        ] {
            assert_eq!(
                fact(&detail, name),
                &Cell::Text(text.into()),
                "{url} {name}"
            );
        }
        if mode == RenderMode::Javascript {
            assert_eq!(
                fact(&detail, "raw_title"),
                &Cell::Text(TITLE.into()),
                "{url}"
            );
            assert_eq!(
                fact(&detail, "raw_meta_description"),
                &Cell::Text(DESCRIPTION.into()),
                "{url}"
            );
            assert_eq!(
                fact(&detail, "raw_canonical"),
                &Cell::Text(TARGET.into()),
                "{url}"
            );
        }
        assert!(detail.html.is_none() && detail.raw_html.is_none());
        assert_eq!(
            site.0
                .lock()
                .unwrap()
                .iter()
                .filter(|request| *request == &url)
                .count(),
            1
        );
    }
    verify_exports(&workspace, &meta.id, dir.path(), mode, fail);
}

fn exported(
    workspace: &SpiderWorkspace,
    id: &str,
    tab: CrawlTab,
    directory: &Path,
) -> Vec<serde_json::Value> {
    let path = directory.join(format!("{}.json", tab.name()));
    workspace.export(id, tab, &path, None).unwrap();
    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
}

fn verify_exports(
    workspace: &SpiderWorkspace,
    id: &str,
    directory: &Path,
    mode: RenderMode,
    fail: bool,
) {
    let rows = exported(workspace, id, CrawlTab::Internal, directory);
    assert_eq!(rows.len(), 10);
    assert!(rows.iter().all(|row| row["title"] == TITLE
        && row["meta_description"] == DESCRIPTION
        && row["status_code"] == 200));
    let links = exported(workspace, id, CrawlTab::Links, directory);
    assert_eq!(links.len(), 20);
    assert!(links.iter().all(|row| row["destination"] == TARGET));
    assert!(
        links
            .iter()
            .filter(|row| row["kind"] == "hyperlink")
            .all(|row| row["text"] == "İletişim")
    );
    let renders = exported(workspace, id, CrawlTab::Rendering, directory);
    if mode == RenderMode::Javascript {
        assert_eq!(renders.len(), 10);
        for row in renders {
            assert_eq!(row["raw_title"], TITLE);
            assert_eq!(row["raw_meta_description"], DESCRIPTION);
            assert_eq!(row["raw_canonical"], TARGET);
            if !fail || !row["address"].as_str().unwrap().ends_with("/comment") {
                assert_eq!(row["rendered_title"], TITLE);
                assert_eq!(row["rendered_meta_description"], DESCRIPTION);
                assert_eq!(row["rendered_canonical"], TARGET);
            }
        }
    } else {
        assert_eq!(renders, Vec::<serde_json::Value>::new());
    }
    let issues = exported(workspace, id, CrawlTab::Issues, directory);
    assert_eq!(
        issues
            .iter()
            .filter(|row| row["issue_id"] == "render-error")
            .count(),
        usize::from(fail)
    );
    assert!(
        !issues
            .iter()
            .any(|row| row["issue_id"] == "internal-client-error")
    );
}

#[test]
fn html_encoding_preserves_turkish_facts_and_discovers_the_correct_url() {
    verify(RenderMode::Html, false);
}

#[test]
fn javascript_encoding_preserves_matching_raw_and_rendered_facts() {
    verify(RenderMode::Javascript, false);
}

#[test]
fn failed_render_uses_correctly_decoded_source_facts() {
    verify(RenderMode::Javascript, true);
}
