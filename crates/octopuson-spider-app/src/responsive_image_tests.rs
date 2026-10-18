//! Responsive candidates must enter resource scheduling, saved facts and page audits.

use std::sync::Mutex;
use std::time::Duration;

use super::*;
use octopuson_crawl::{
    FetchError, FetchRequest, FetchResponse, Fetcher, RenderRequest, RenderedPage, Renderer,
};

const SOURCE: &str = "<title>Responsive QA</title><h1>Images</h1>\
    <picture><source srcset='/wide.avif 1x' media='(min-width: 2000px)' type='image/avif'>\
    <img src='/small.png' srcset='/small.png 1x, /broken.png 2x, /crop,a.png 3x, /invalid.png 0w' alt='Hero' width='40' height='20'></picture>\
    <img srcset='/missing-alt.png 1x, /other.png 2x'>\
    <template><img srcset='/inert.png 1x'></template>";
const ACTIVATED: &str = "<img srcset='/dom.png 1x, /dom-large.png 2x' alt='DOM image' loading='lazy' width='40' height='20'>";
const IMAGES: [&str; 6] = [
    "/small.png",
    "/broken.png",
    "/crop,a.png",
    "/wide.avif",
    "/missing-alt.png",
    "/other.png",
];

#[derive(Default)]
struct Site(Mutex<Vec<String>>);
impl Fetcher for Site {
    fn fetch(&self, request: &FetchRequest<'_>) -> Result<FetchResponse, FetchError> {
        self.0.lock().unwrap().push(request.url.into());
        if request.url == "https://example.com/" {
            return Ok(FetchResponse::new(
                200,
                &[("Content-Type", "text/html")],
                SOURCE,
            ));
        }
        Ok(FetchResponse::new(
            if request.url.ends_with("/broken.png") {
                404
            } else {
                200
            },
            &[("Content-Type", "image/png")],
            "image",
        ))
    }
}

struct Script {
    fail: bool,
}
impl Renderer for Script {
    fn render(&self, request: &RenderRequest<'_>) -> Result<RenderedPage, FetchError> {
        assert_eq!(request.url, "https://example.com/");
        if self.fail {
            return Err(FetchError::Other("intentional render failure".into()));
        }
        Ok(RenderedPage {
            html: format!("{SOURCE}{ACTIVATED}"),
            url: request.url.into(),
            elapsed: Duration::ZERO,
            truncated: false,
        })
    }
}

fn exported(
    workspace: &SpiderWorkspace,
    id: &str,
    tab: CrawlTab,
    path: &Path,
) -> Vec<serde_json::Value> {
    workspace.export(id, tab, path, None).unwrap();
    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
}

fn verify(mode: RenderMode, fail: bool, follow: bool) {
    let activated = mode == RenderMode::Javascript && !fail;
    let dir = tempfile::tempdir().unwrap();
    let workspace = SpiderWorkspace::resolve(Some(dir.path().into())).unwrap();
    let mut config = CrawlConfig {
        start_urls: vec!["https://example.com/".into()],
        robots: RobotsPolicy::Ignore,
        ..CrawlConfig::default()
    };
    config.rendering.mode = mode;
    config.follow.sitemaps = false;
    config.follow.images = follow;
    config.storage.html = false;
    config.storage.headers = false;
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
    assert_eq!(meta.progress.errors, u32::from(follow));
    let rows = exported(
        &workspace,
        &meta.id,
        CrawlTab::Images,
        &dir.path().join("images.json"),
    );
    verify_images(&rows, &site, activated, follow);
    let links = exported(
        &workspace,
        &meta.id,
        CrawlTab::Links,
        &dir.path().join("links.json"),
    );
    assert_eq!(links.len(), rows.len());
    assert!(links.iter().all(|row| row["kind"] == "image"));
    let issues = exported(
        &workspace,
        &meta.id,
        CrawlTab::Issues,
        &dir.path().join("issues.json"),
    );
    for (issue, expected) in [
        ("image-broken", usize::from(follow)),
        ("image-missing-alt", 1),
        ("image-missing-dimensions", 1),
        ("render-error", usize::from(fail)),
    ] {
        assert_eq!(
            issues.iter().filter(|row| row["issue_id"] == issue).count(),
            expected,
            "{issue}"
        );
    }
}

fn verify_images(rows: &[serde_json::Value], site: &Site, activated: bool, follow: bool) {
    assert_eq!(rows.len(), 6 + if activated { 2 } else { 0 });
    for path in IMAGES.into_iter().chain(if activated {
        vec!["/dom.png", "/dom-large.png"]
    } else {
        Vec::new()
    }) {
        let url = format!("https://example.com{path}");
        let row = rows.iter().find(|row| row["image"] == url).unwrap();
        let missing = matches!(path, "/missing-alt.png" | "/other.png");
        assert_eq!(row["page"], "https://example.com/");
        assert_eq!(row["missing_alt"], missing);
        assert_eq!(row["has_dimensions"], !missing);
        assert_eq!(row["lazy"], path.starts_with("/dom"));
        assert_eq!(
            row["alt"],
            if missing {
                serde_json::Value::Null
            } else {
                serde_json::json!(if path.starts_with("/dom") {
                    "DOM image"
                } else {
                    "Hero"
                })
            }
        );
        assert_eq!(
            row["status_code"],
            if follow {
                serde_json::json!(if path == "/broken.png" { 404 } else { 200 })
            } else {
                serde_json::Value::Null
            }
        );
        assert_eq!(
            site.0
                .lock()
                .unwrap()
                .iter()
                .filter(|request| *request == &url)
                .count(),
            usize::from(follow)
        );
    }
    let requests = site.0.lock().unwrap();
    assert!(
        !requests
            .iter()
            .any(|request| request.contains("invalid") || request.contains("inert"))
    );
    assert_eq!(
        requests
            .iter()
            .filter(|request| *request == "https://example.com/")
            .count(),
        1
    );
}

#[test]
fn html_responsive_images_are_fetched_audited_and_exported() {
    verify(RenderMode::Html, false, true);
}

#[test]
fn javascript_responsive_images_use_the_rendered_dom() {
    verify(RenderMode::Javascript, false, true);
}

#[test]
fn failed_render_keeps_source_responsive_candidates() {
    verify(RenderMode::Javascript, true, true);
}

#[test]
fn disabled_image_crawling_keeps_metadata_without_inventing_broken_images() {
    verify(RenderMode::Html, false, false);
}
