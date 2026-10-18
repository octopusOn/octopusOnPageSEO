//! Crawler-specific HTTP directives must not leak into the browser preset.

use std::collections::BTreeSet;
use std::sync::Mutex;
use std::time::Duration;

use super::*;
use octopuson_core::table::Cell;
use octopuson_crawl::{
    AgentPreset, FetchError, FetchRequest, FetchResponse, Fetcher, RenderRequest, RenderedPage,
    Renderer,
};

const CASES: [&str; 8] = [
    "google-only",
    "bing-only",
    "generic",
    "mixed",
    "separate",
    "meta",
    "document.pdf",
    "generated",
];
const GOOGLE: &str = "GoOgLeBoT: noindex, nofollow, max-snippet:0";
const META: &str = "<meta name='robots' content='max-snippet:100'><meta name='googlebot' content='noindex,nofollow,max-snippet:0'><meta name='bingbot' content='noindex,nofollow,max-snippet:20'><meta name='octopusseo-spider' content='noindex,nofollow,max-snippet:30'>";

#[derive(Default)]
struct Site(Mutex<Vec<String>>);
impl Fetcher for Site {
    fn fetch(&self, request: &FetchRequest<'_>) -> Result<FetchResponse, FetchError> {
        self.0.lock().unwrap().push(request.url.into());
        let path = request.url.strip_prefix("https://example.com/").unwrap();
        let mut headers = vec![(
            "Content-Type",
            if path == "document.pdf" {
                "application/pdf"
            } else {
                "text/html"
            },
        )];
        let body = match path {
            "google-only" => {
                headers.push(("X-Robots-Tag", GOOGLE));
                "<a href='/child'>Child</a>"
            }
            "document.pdf" => {
                headers.push(("X-Robots-Tag", GOOGLE));
                "%PDF-test"
            }
            "bing-only" => {
                headers.push(("X-Robots-Tag", "BingBot: noindex, nofollow, max-snippet:20"));
                ""
            }
            "generic" => {
                headers.push(("X-Robots-Tag", "noindex, nofollow, max-snippet:50"));
                ""
            }
            "mixed" => {
                headers.push(("X-Robots-Tag", "max-snippet:150, googlebot: noindex, max-snippet:0, bingbot: nofollow, max-snippet:20"));
                ""
            }
            "separate" => {
                headers.extend([
                    ("X-Robots-Tag", GOOGLE),
                    ("X-Robots-Tag", "nosnippet, max-snippet:50"),
                    ("X-Robots-Tag", "index, follow, max-snippet:-1"),
                ]);
                ""
            }
            "meta" => META,
            "generated" => "<meta name='robots' content='max-snippet:100'>",
            "child" => "",
            _ => return Ok(FetchResponse::new(404, &[], "")),
        };
        Ok(FetchResponse::new(
            200,
            &headers,
            format!("<head><title>Scopes</title>{body}</head><body>Content</body>"),
        ))
    }
}

struct Script {
    fail: bool,
}
impl Renderer for Script {
    fn render(&self, request: &RenderRequest<'_>) -> Result<RenderedPage, FetchError> {
        if self.fail {
            return Err(FetchError::Other("intentional render failure".into()));
        }
        Ok(RenderedPage {
            html: if request.url.ends_with("/generated") {
                format!("<title>DOM</title>{META}")
            } else {
                String::from_utf8(request.response.body.clone()).unwrap()
            },
            url: request.url.into(),
            elapsed: Duration::ZERO,
            truncated: false,
        })
    }
}

fn expected(case: &str, agent: AgentPreset, generated: bool) -> (bool, bool, bool, Option<i32>) {
    let google = matches!(
        agent,
        AgentPreset::GooglebotDesktop | AgentPreset::GooglebotSmartphone
    );
    let bing = agent == AgentPreset::Bingbot;
    match case {
        "google-only" | "document.pdf" => (google, google, false, google.then_some(0)),
        "bing-only" => (bing, bing, false, bing.then_some(20)),
        "generic" => (true, true, false, Some(50)),
        "mixed" => (
            google,
            bing,
            false,
            Some(if google {
                0
            } else if bing {
                20
            } else {
                150
            }),
        ),
        "separate" => (google, google, true, Some(if google { 0 } else { 50 })),
        "meta" | "generated" if case == "meta" || generated => match agent {
            AgentPreset::GooglebotDesktop | AgentPreset::GooglebotSmartphone => {
                (true, true, false, Some(0))
            }
            AgentPreset::Bingbot => (true, true, false, Some(20)),
            AgentPreset::Octopus => (true, true, false, Some(30)),
            AgentPreset::Chrome => (false, false, false, Some(100)),
        },
        "generated" => (false, false, false, Some(100)),
        _ => panic!("unexpected case {case}"),
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

fn verify(mode: RenderMode, agent: AgentPreset, fail: bool) {
    let dir = tempfile::tempdir().unwrap();
    let workspace = SpiderWorkspace::resolve(Some(dir.path().into())).unwrap();
    let mut config = CrawlConfig {
        start_urls: CASES
            .iter()
            .map(|case| format!("https://example.com/{case}"))
            .collect(),
        agent,
        robots: RobotsPolicy::Ignore,
        ..CrawlConfig::default()
    };
    config.rendering.mode = mode;
    config.follow.sitemaps = false;
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
    assert_eq!(meta.progress.errors, 0);
    let google = matches!(
        agent,
        AgentPreset::GooglebotDesktop | AgentPreset::GooglebotSmartphone
    );
    assert_eq!(
        site.0
            .lock()
            .unwrap()
            .iter()
            .filter(|url| url.ends_with("/child"))
            .count(),
        usize::from(!google)
    );
    let reader = workspace.open(&meta.id).unwrap();
    let generated = mode == RenderMode::Javascript && !fail;
    for case in CASES {
        verify_facts(
            &reader
                .detail(&format!("https://example.com/{case}"))
                .unwrap(),
            case,
            agent,
            mode,
            generated,
        );
    }
    verify_exports(&workspace, &meta.id, dir.path(), agent, generated);
}

fn verify_facts(
    detail: &UrlDetail,
    case: &str,
    agent: AgentPreset,
    mode: RenderMode,
    generated: bool,
) {
    let (noindex, nofollow, nosnippet, snippet) = expected(case, agent, generated);
    for (name, value) in [
        ("noindex", noindex),
        ("nofollow", nofollow),
        ("nosnippet", nosnippet),
    ] {
        assert_eq!(
            fact(detail, name),
            &Cell::Boolean(value),
            "{agent:?} {case} {name}"
        );
    }
    assert_eq!(
        fact(detail, "max_snippet"),
        &snippet.map_or(Cell::Null, |value| Cell::Integer(i64::from(value))),
        "{agent:?} {case}"
    );
    assert_eq!(
        fact(detail, "indexability"),
        &Cell::Text(if noindex { "noindex" } else { "indexable" }.into()),
        "{agent:?} {case}"
    );
    if mode == RenderMode::Javascript && case != "document.pdf" {
        let (raw_noindex, raw_nofollow, _, _) = expected(case, agent, false);
        assert_eq!(fact(detail, "raw_noindex"), &Cell::Boolean(raw_noindex));
        assert_eq!(fact(detail, "raw_nofollow"), &Cell::Boolean(raw_nofollow));
    }
    assert!(detail.html.is_none() && detail.raw_html.is_none());
}

fn verify_exports(
    workspace: &SpiderWorkspace,
    id: &str,
    directory: &Path,
    agent: AgentPreset,
    generated: bool,
) {
    let path = directory.join("internal.json");
    workspace
        .export(id, CrawlTab::Internal, &path, None)
        .unwrap();
    let rows: Vec<serde_json::Value> =
        serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    for case in CASES {
        let url = format!("https://example.com/{case}");
        let row = rows.iter().find(|row| row["address"] == url).unwrap();
        assert_eq!(
            row["indexability"],
            if expected(case, agent, generated).0 {
                "noindex"
            } else {
                "indexable"
            }
        );
        if matches!(case, "google-only" | "document.pdf") {
            assert_eq!(row["x_robots_tag"], GOOGLE);
        }
    }
    let path = directory.join("issues.json");
    workspace.export(id, CrawlTab::Issues, &path, None).unwrap();
    let rows: Vec<serde_json::Value> =
        serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    for (issue, field) in [
        ("noindex", 0),
        ("nofollow-page", 1),
        ("snippet-restricted", 2),
    ] {
        let wanted: BTreeSet<_> = CASES
            .iter()
            .filter(|case| {
                let (noindex, nofollow, nosnippet, snippet) = expected(case, agent, generated);
                **case != "document.pdf"
                    && match field {
                        0 => noindex,
                        1 => nofollow,
                        _ => nosnippet || snippet == Some(0),
                    }
            })
            .map(|case| format!("https://example.com/{case}"))
            .collect();
        let actual: BTreeSet<_> = rows
            .iter()
            .filter(|row| row["issue_id"] == issue)
            .map(|row| row["address"].as_str().unwrap().to_owned())
            .collect();
        assert_eq!(actual, wanted, "{agent:?} {issue}");
    }
}

#[test]
fn html_header_scopes_match_all_five_crawler_presets() {
    for agent in [
        AgentPreset::Chrome,
        AgentPreset::GooglebotDesktop,
        AgentPreset::GooglebotSmartphone,
        AgentPreset::Bingbot,
        AgentPreset::Octopus,
    ] {
        verify(RenderMode::Html, agent, false);
    }
}

#[test]
fn javascript_header_scopes_preserve_raw_and_rendered_crawler_identity() {
    for agent in [
        AgentPreset::Chrome,
        AgentPreset::GooglebotDesktop,
        AgentPreset::GooglebotSmartphone,
        AgentPreset::Bingbot,
        AgentPreset::Octopus,
    ] {
        verify(RenderMode::Javascript, agent, false);
    }
}

#[test]
fn failed_render_fallback_does_not_inherit_foreign_header_scopes() {
    verify(RenderMode::Javascript, AgentPreset::Chrome, true);
}
