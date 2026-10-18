//! Rendering integration without a browser: the same path persists DOM facts and links.
use std::sync::Mutex;
use std::time::Duration;

use super::*;
use seonexus_core::table::Cell;
use seonexus_crawl::{
    FetchError, FetchRequest, FetchResponse, Fetcher, RenderRequest, RenderedPage, Renderer,
};

const RAW: &str =
    "<title>Before JavaScript</title><a href='/raw-only'>Raw only</a><div id='app'></div>";
const DOM: &str = "<title>After JavaScript</title><h1>Dynamic content</h1><a href='/from-js'>JS page</a><a href='/private/generated'>Blocked</a><a href='/failed'>Failure</a><a href='/excluded/test'>Excluded</a>";
struct Site;
impl Fetcher for Site {
    fn fetch(&self, r: &FetchRequest<'_>) -> Result<FetchResponse, FetchError> {
        Ok(match r.url {
            "https://example.com/robots.txt" => {
                FetchResponse::new(200, &[], "User-agent: *\nDisallow: /private/\n")
            }
            "https://example.com/" => {
                FetchResponse::new(200, &[("Content-Type", "text/html")], RAW)
            }
            "https://example.com/from-js" => FetchResponse::new(
                200,
                &[("Content-Type", "text/html")],
                "<title>Generated destination</title>",
            ),
            "https://example.com/failed" => FetchResponse::new(
                200,
                &[("Content-Type", "text/html")],
                "<title>Fallback page</title>",
            ),
            _ => FetchResponse::new(404, &[], ""),
        })
    }
}
#[derive(Default)]
struct Script(Mutex<Vec<String>>);
impl Renderer for Script {
    fn render(&self, r: &RenderRequest<'_>) -> Result<RenderedPage, FetchError> {
        self.0.lock().unwrap().push(r.url.to_owned());
        let policy = r.request_allowed.unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(30);
        assert!(!policy(
            "https://example.com/private/script.js",
            seonexus_crawl::BrowserRequest::Resource,
            deadline
        ));
        assert!(!policy(
            "https://elsewhere.example/",
            seonexus_crawl::BrowserRequest::Document,
            deadline
        ));
        assert!(!policy(
            "https://example.com/excluded/frame",
            seonexus_crawl::BrowserRequest::Document,
            deadline
        ));
        assert!(
            !policy(
                "https://example.com/private/script.js",
                seonexus_crawl::BrowserRequest::CachedResource,
                deadline
            ),
            "cached resources must still respect robots.txt"
        );
        assert!(policy(
            "https://example.com/public/script.js",
            seonexus_crawl::BrowserRequest::CachedResource,
            deadline
        ));
        if r.url.ends_with("/failed") {
            return Err(FetchError::Other("intentional browser failure".into()));
        }
        let html = if r.url == "https://example.com/" {
            DOM.to_owned()
        } else {
            String::from_utf8(r.response.body.clone()).unwrap()
        };
        Ok(RenderedPage {
            html,
            url: r.url.to_owned(),
            elapsed: Duration::ZERO,
            truncated: false,
        })
    }
}

#[test]
fn rendered_content_drives_facts_frontier_audits_and_exports() {
    let dir = tempfile::tempdir().unwrap();
    let workspace = SpiderWorkspace::resolve(Some(dir.path().to_owned())).unwrap();
    let mut config = CrawlConfig {
        start_urls: vec!["https://example.com/".into()],
        ..CrawlConfig::default()
    };
    config.rendering.mode = RenderMode::Javascript;
    config.follow.sitemaps = false;
    config.storage.html = true;
    config.urls.exclude = vec!["/excluded/".into()];
    let plan = CrawlPlan::new(config).unwrap();
    let renderer = Script::default();
    let mut activity = CrawlActivity::default();
    let meta = workspace
        .crawl_observed_with(
            &plan,
            &Site,
            Some(&renderer),
            &CrawlControl::new(),
            |event| {
                if let CrawlEvent::Activity(snapshot) = event {
                    activity = (*snapshot).clone();
                }
            },
        )
        .unwrap();
    assert_live_rendering(&activity);
    let reader = workspace.open(&meta.id).unwrap();
    let page = reader
        .page(&PageRequest {
            tab: "internal".into(),
            limit: 100,
            ..PageRequest::default()
        })
        .unwrap();
    let addresses: Vec<_> = page.rows.iter().map(|row| row[0].to_string()).collect();
    assert!(addresses.contains(&"https://example.com/from-js".into()));
    assert!(addresses.contains(&"https://example.com/private/generated".into()));
    assert!(!addresses.contains(&"https://example.com/raw-only".into()));
    assert!(!addresses.contains(&"https://example.com/excluded/test".into()));
    assert!(
        !renderer
            .0
            .lock()
            .unwrap()
            .iter()
            .any(|url| url.contains("/private/"))
    );
    let detail = reader.detail("https://example.com/").unwrap();
    assert_eq!(detail.raw_html.as_deref(), Some(RAW));
    assert_eq!(detail.html.as_deref(), Some(DOM));
    let title = detail
        .facts
        .columns
        .iter()
        .position(|column| column.name == "title")
        .unwrap();
    assert_eq!(
        detail.facts.rows[0][title],
        Cell::Text("After JavaScript".into())
    );
    let rendering = reader
        .page(&PageRequest {
            tab: "rendering".into(),
            limit: 100,
            ..PageRequest::default()
        })
        .unwrap();
    assert_eq!(rendering.total, 3);
    assert_eq!(
        reader
            .overview()
            .unwrap()
            .issues
            .iter()
            .find(|issue| issue.id == "render-error")
            .unwrap()
            .count,
        1
    );
    let failed = reader.detail("https://example.com/failed").unwrap();
    assert!(failed.html.unwrap().contains("Fallback page"));
    let path = dir.path().join("rendering.json");
    workspace
        .export(&meta.id, CrawlTab::Rendering, &path, None)
        .unwrap();
    assert!(
        std::fs::read_to_string(path)
            .unwrap()
            .contains("After JavaScript")
    );
}

fn assert_live_rendering(activity: &CrawlActivity) {
    assert_eq!(activity.active, Vec::<UrlActivity>::new());
    assert_eq!(
        (activity.rendered, activity.render_errors, activity.blocked),
        (2, 1, 1)
    );
    let home = activity
        .recent
        .iter()
        .find(|entry| entry.url == "https://example.com/")
        .unwrap();
    assert!(home.rendered);
    assert_eq!(home.title.as_deref(), Some("After JavaScript"));
    let failed = activity
        .recent
        .iter()
        .find(|entry| entry.url.ends_with("/failed"))
        .unwrap();
    assert_eq!(failed.state, ActivityState::Error);
    assert_eq!(failed.status, Some(200));
    assert_eq!(failed.title.as_deref(), Some("Fallback page"));
    assert!(failed.render_error.is_some());
}
