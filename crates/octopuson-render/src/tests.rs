use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;

use octopuson_crawl::{
    CrawlConfig, CrawlControl, CrawlPlan, FetchRequest, Fetcher, RenderMode, RenderRequest,
    Renderer,
};
use octopuson_net::CrawlFetcher;

use super::ChromeRenderer;

struct Site {
    url: String,
    stop: Arc<AtomicBool>,
    seen: Arc<Mutex<Vec<String>>>,
    thread: Option<std::thread::JoinHandle<()>>,
}
impl Site {
    fn new() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        listener.set_nonblocking(true).unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let worker_stop = stop.clone();
        let seen = Arc::new(Mutex::new(Vec::new()));
        let worker_seen = seen.clone();
        let thread = std::thread::spawn(move || {
            while !worker_stop.load(Ordering::Relaxed) {
                let Ok((mut stream, _)) = listener.accept() else {
                    std::thread::sleep(Duration::from_millis(5));
                    continue;
                };
                stream.set_nonblocking(false).unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut first = String::new();
                if reader.read_line(&mut first).is_err() {
                    continue;
                }
                let path = first.split_whitespace().nth(1).unwrap_or("/").to_owned();
                let mut line = String::new();
                while reader.read_line(&mut line).unwrap_or(0) > 2 {
                    line.clear();
                }
                worker_seen.lock().unwrap().push(path.clone());
                let (content_type, body) = site_body(&path);
                let body = if path == "/fresh" {
                    worker_seen
                        .lock()
                        .unwrap()
                        .iter()
                        .filter(|p| p.as_str() == "/fresh")
                        .count()
                        .to_string()
                } else {
                    body.to_owned()
                };
                let cache = if path == "/cached.js" {
                    "public, max-age=3600, immutable"
                } else {
                    "no-store"
                };
                let date = time::OffsetDateTime::now_utc()
                    .format(&time::format_description::well_known::Rfc2822)
                    .unwrap();
                let compressed = path == "/cached.js";
                let body = if compressed {
                    let mut encoder =
                        flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
                    encoder.write_all(body.as_bytes()).unwrap();
                    encoder.finish().unwrap()
                } else {
                    body.into_bytes()
                };
                let encoding = if compressed {
                    "Content-Encoding: gzip\r\n"
                } else {
                    ""
                };
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nCache-Control: {cache}\r\nDate: {date}\r\n{encoding}Content-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                let _ = stream.write_all(response.as_bytes());
                let _ = stream.write_all(&body);
            }
        });
        Self {
            url,
            stop,
            seen,
            thread: Some(thread),
        }
    }
}

fn site_body(path: &str) -> (&'static str, &'static str) {
    match path {
        "/app.js" => (
            "application/javascript",
            "setTimeout(async()=>{const text=await (await fetch('/data')).text();document.title='Rendered title';document.body.innerHTML='<h1>'+text+'</h1><a href=\"/js-link\">JS link</a>';localStorage.setItem('isolated','yes');},100);",
        ),
        "/data" => ("text/plain", "Async content"),
        "/fresh" => ("text/plain", ""),
        "/cached.js" => (
            "application/javascript; charset=utf-8",
            "document.addEventListener('DOMContentLoaded',async()=>{const n=await(await fetch('/fresh')).text();document.title='Cached '+n+' '+(localStorage.getItem('session')||'clean')+' '+(document.cookie||'clean');localStorage.setItem('session','dirty');document.cookie='session=dirty';document.body.innerHTML='<h1>'+document.title+'</h1><a href=\"/generated\">Generated</a>';});",
        ),
        "/uncached.js" => ("application/javascript", "window.privateLoaded=true;"),
        "/cached" => (
            "text/html",
            "<html><head><title>Raw cached page</title><link rel='icon' href='data:,'><script defer src='/cached.js'></script><script defer src='/uncached.js'></script></head><body>Shell</body></html>",
        ),
        "/isolation" => (
            "text/html",
            "<script>document.title=localStorage.getItem('isolated')||'clean';</script>",
        ),
        "/slow" => ("text/html", "<script>while(true){}</script>"),
        _ => (
            "text/html",
            "<!doctype html><html><head><title>Raw title</title><script defer src='/app.js'></script></head><body>Shell</body></html>",
        ),
    }
}

#[test]
#[ignore = "requires an installed Chrome/Chromium/Edge and loopback access"]
fn browser_cache_reuses_only_immutable_assets_and_checks_policy_on_every_hit() {
    use octopuson_crawl::BrowserRequest;
    let site = Site::new();
    let mut config = CrawlConfig {
        start_urls: vec![format!("{}/cached", site.url)],
        ..CrawlConfig::default()
    };
    config.rendering.mode = RenderMode::Javascript;
    config.rendering.wait_ms = 100;
    let plan = CrawlPlan::new(config).unwrap();
    let renderer = ChromeRenderer::launch(&plan).unwrap();
    let http = CrawlFetcher::new(plan.user_agent(), Duration::from_secs(5), None);
    let control = CrawlControl::new();
    let url = &plan.config().start_urls[0];
    let response = fetch(&http, url, 1024 * 1024);
    let calls = Mutex::new(Vec::new());
    let policy = |url: &str, kind: BrowserRequest, _deadline| {
        calls.lock().unwrap().push((url.to_owned(), kind));
        true
    };
    for n in 1..=2 {
        let rendered = renderer
            .render(&RenderRequest {
                url,
                response: &response,
                body_limit: 1024 * 1024,
                control: &control,
                request_allowed: Some(&policy),
            })
            .unwrap();
        assert!(
            rendered
                .html
                .contains(&format!("<title>Cached {n} clean clean</title>")),
            "{}",
            rendered.html
        );
        assert!(rendered.html.contains("/generated"));
    }
    let seen = site.seen.lock().unwrap();
    assert_eq!(
        seen.iter().filter(|p| p.as_str() == "/cached.js").count(),
        1,
        "immutable script must be downloaded once"
    );
    assert_eq!(
        seen.iter().filter(|p| p.as_str() == "/uncached.js").count(),
        2,
        "no-store script must stay fresh"
    );
    assert_eq!(
        seen.iter().filter(|p| p.as_str() == "/fresh").count(),
        2,
        "API responses must stay fresh"
    );
    drop(seen);
    assert!(
        calls
            .lock()
            .unwrap()
            .iter()
            .any(|(url, kind)| url.ends_with("/cached.js")
                && *kind == BrowserRequest::CachedResource)
    );
    let blocked = |url: &str, kind: BrowserRequest, _deadline| {
        if url.ends_with("/cached.js") {
            assert_eq!(kind, BrowserRequest::CachedResource);
            false
        } else {
            true
        }
    };
    let rendered = renderer
        .render(&RenderRequest {
            url,
            response: &response,
            body_limit: 1024 * 1024,
            control: &control,
            request_allowed: Some(&blocked),
        })
        .unwrap();
    assert!(rendered.html.contains("<title>Raw cached page</title>"));
    assert_eq!(
        site.seen
            .lock()
            .unwrap()
            .iter()
            .filter(|p| p.as_str() == "/fresh")
            .count(),
        2,
        "blocked cache must not execute"
    );
}
impl Drop for Site {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            thread.join().unwrap();
        }
    }
}

#[test]
#[ignore = "requires an installed Chrome/Chromium/Edge and loopback access"]
fn browser_executes_async_scripts_once_and_isolates_pages() {
    let site = Site::new();
    let mut config = CrawlConfig {
        start_urls: vec![format!("{}/", site.url)],
        ..CrawlConfig::default()
    };
    config.rendering.mode = RenderMode::Javascript;
    config.rendering.wait_ms = 200;
    config.rendering.timeout_seconds = 10;
    let plan = CrawlPlan::new(config).unwrap();
    let renderer = ChromeRenderer::launch(&plan).unwrap();
    let http = CrawlFetcher::new(plan.user_agent(), Duration::from_secs(5), None);
    let control = CrawlControl::new();
    let url = format!("{}/", site.url);
    let response = fetch(&http, &url, 1024 * 1024);
    let rendered = renderer
        .render(&RenderRequest {
            url: &url,
            response: &response,
            body_limit: 1024 * 1024,
            control: &control,
            request_allowed: None,
        })
        .unwrap();
    assert!(rendered.html.contains("Rendered title"));
    assert!(rendered.html.contains("Async content"));
    assert!(rendered.html.contains("/js-link"));
    assert_eq!(
        site.seen
            .lock()
            .unwrap()
            .iter()
            .filter(|path| path.as_str() == "/")
            .count(),
        1,
        "the main document must not be fetched a second time"
    );
    let blocked =
        |url: &str, _kind: octopuson_crawl::BrowserRequest, _deadline| !url.ends_with("/app.js");
    let blocked_render = renderer
        .render(&RenderRequest {
            url: &url,
            response: &response,
            body_limit: 1024 * 1024,
            control: &control,
            request_allowed: Some(&blocked),
        })
        .unwrap();
    assert!(blocked_render.html.contains("Raw title"));
    assert!(!blocked_render.html.contains("Rendered title"));
    assert_eq!(
        site.seen
            .lock()
            .unwrap()
            .iter()
            .filter(|path| path.as_str() == "/app.js")
            .count(),
        1,
        "blocked resources must not reach the server"
    );
    let url = format!("{}/isolation", site.url);
    let response = fetch(&http, &url, 1024 * 1024);
    let rendered = renderer
        .render(&RenderRequest {
            url: &url,
            response: &response,
            body_limit: 1024 * 1024,
            control: &control,
            request_allowed: None,
        })
        .unwrap();
    assert!(
        rendered.html.contains("<title>clean</title>"),
        "cookies/storage must not leak between pages"
    );
    let rendered = renderer
        .render(&RenderRequest {
            url: &url,
            response: &response,
            body_limit: 17,
            control: &control,
            request_allowed: None,
        })
        .unwrap();
    assert!(rendered.truncated);
    assert!(rendered.html.len() <= 17);
    assert_unicode_truncation(&renderer, &url, &control);
    control.stop();
    assert!(
        renderer
            .render(&RenderRequest {
                url: &url,
                response: &response,
                body_limit: 1024,
                control: &control,
                request_allowed: None,
            })
            .is_err()
    );
}

#[test]
fn explicit_missing_browser_is_reported() {
    let mut config = CrawlConfig {
        start_urls: vec!["https://example.com/".into()],
        ..CrawlConfig::default()
    };
    config.rendering.browser_path = Some("/nonexistent/octopusseo-browser".into());
    let plan = CrawlPlan::new(config).unwrap();
    assert!(matches!(
        ChromeRenderer::launch(&plan),
        Err(super::RenderError::BrowserMissing)
    ));
}

#[test]
#[ignore = "requires an installed Chrome/Chromium/Edge and loopback access"]
fn browser_timeout_releases_page_and_can_render_next_url() {
    let site = Site::new();
    let mut config = CrawlConfig {
        start_urls: vec![format!("{}/", site.url)],
        ..CrawlConfig::default()
    };
    config.rendering.mode = RenderMode::Javascript;
    config.rendering.timeout_seconds = 2;
    config.rendering.wait_ms = 100;
    let plan = CrawlPlan::new(config).unwrap();
    let renderer = ChromeRenderer::launch(&plan).unwrap();
    let http = CrawlFetcher::new(plan.user_agent(), Duration::from_secs(2), None);
    let control = CrawlControl::new();
    let url = format!("{}/slow", site.url);
    let response = fetch(&http, &url, 1024);
    let result = renderer.render(&RenderRequest {
        url: &url,
        response: &response,
        body_limit: 1024,
        control: &control,
        request_allowed: None,
    });
    assert!(
        matches!(result, Err(octopuson_crawl::FetchError::Timeout)),
        "{result:?}"
    );
    let url = format!("{}/isolation", site.url);
    let response = fetch(&http, &url, 1024);
    let next = renderer
        .render(&RenderRequest {
            url: &url,
            response: &response,
            body_limit: 1024,
            control: &control,
            request_allowed: None,
        })
        .unwrap();
    assert!(next.html.contains("<title>clean</title>"));
}

#[test]
#[ignore = "requires an installed Chrome/Chromium/Edge and loopback access"]
fn browser_queue_does_not_consume_the_next_pages_render_budget() {
    let site = Site::new();
    let mut config = CrawlConfig {
        start_urls: vec![format!("{}/isolation", site.url)],
        ..CrawlConfig::default()
    };
    config.rendering.mode = RenderMode::Javascript;
    config.rendering.concurrency = 1;
    config.rendering.timeout_seconds = 2;
    config.rendering.wait_ms = 1100;
    let plan = CrawlPlan::new(config).unwrap();
    let renderer = ChromeRenderer::launch(&plan).unwrap();
    let response = octopuson_crawl::FetchResponse::new(
        200,
        &[("Content-Type", "text/html")],
        "<html><head><title>Queued page</title></head><body>Ready</body></html>",
    );
    let control = CrawlControl::new();
    let barrier = std::sync::Barrier::new(3);
    let results = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..3)
            .map(|_| {
                let (renderer, response, control, barrier, url) = (
                    &renderer,
                    &response,
                    &control,
                    &barrier,
                    &plan.config().start_urls[0],
                );
                scope.spawn(move || {
                    barrier.wait();
                    renderer.render(&RenderRequest {
                        url,
                        response,
                        body_limit: 1024,
                        control,
                        request_allowed: None,
                    })
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .collect::<Vec<_>>()
    });
    for result in &results {
        assert!(
            result.is_ok(),
            "queued pages need a full execution budget: {results:?}"
        );
        assert!(result.as_ref().unwrap().html.contains("Queued page"));
    }
    assert!(
        results
            .iter()
            .any(|result| result.as_ref().unwrap().elapsed > Duration::from_secs(3))
    );
}

#[test]
#[ignore = "requires an installed Chrome/Chromium/Edge and loopback access"]
fn browser_viewport_controls_generated_content_without_changing_the_user_agent() {
    use octopuson_crawl::{AgentPreset, FetchResponse, Viewport};
    let site = Site::new();
    let url = format!("{}/viewport", site.url);
    let response = FetchResponse::new(
        200,
        &[("Content-Type", "text/html; charset=utf-8")],
        r#"<!doctype html><html><head><meta name="viewport" content="width=device-width,initial-scale=1"><style>:root{--layout:wide}@media(max-width:600px){:root{--layout:narrow}}</style><script>document.addEventListener('DOMContentLoaded',()=>{const layout=getComputedStyle(document.documentElement).getPropertyValue('--layout').trim();document.title=innerWidth+'x'+innerHeight+' '+layout;document.body.innerHTML='<a href="/'+layout+'">'+layout+'</a><p id="agent"></p>';document.querySelector('#agent').textContent=navigator.userAgent;});</script></head><body>Original</body></html>"#,
    );
    for (agent, viewport, expected) in [
        (AgentPreset::Chrome, None, "1280x800 wide"),
        (AgentPreset::GooglebotSmartphone, None, "412x915 narrow"),
        (
            AgentPreset::Chrome,
            Some(Viewport {
                width: 390,
                height: 844,
                mobile: true,
            }),
            "390x844 narrow",
        ),
        (
            AgentPreset::GooglebotSmartphone,
            Some(Viewport {
                width: 1440,
                height: 900,
                mobile: false,
            }),
            "1440x900 wide",
        ),
        (
            AgentPreset::Chrome,
            Some(Viewport {
                width: 768,
                height: 1024,
                mobile: false,
            }),
            "768x1024 wide",
        ),
    ] {
        let mut config = CrawlConfig {
            start_urls: vec![url.clone()],
            agent,
            ..CrawlConfig::default()
        };
        config.rendering.mode = RenderMode::Javascript;
        config.rendering.wait_ms = 50;
        config.rendering.viewport = viewport;
        let plan = CrawlPlan::new(config).unwrap();
        let renderer = ChromeRenderer::launch(&plan).unwrap();
        let control = CrawlControl::new();
        let rendered = renderer
            .render(&RenderRequest {
                url: &url,
                response: &response,
                body_limit: 1024 * 1024,
                control: &control,
                request_allowed: None,
            })
            .unwrap();
        assert!(
            rendered
                .html
                .contains(&format!("<title>{expected}</title>")),
            "{expected}: {}",
            rendered.html
        );
        assert!(
            rendered
                .html
                .contains(&format!("<p id=\"agent\">{}</p>", plan.user_agent())),
            "request identity must be independent of viewport"
        );
        assert!(rendered.html.contains(if expected.ends_with("narrow") {
            "href=\"/narrow\""
        } else {
            "href=\"/wide\""
        }));
    }
    assert!(
        !site
            .seen
            .lock()
            .unwrap()
            .iter()
            .any(|path| path == "/viewport"),
        "the supplied main response must not be requested again"
    );
}

fn fetch(http: &CrawlFetcher, url: &str, limit: u64) -> octopuson_crawl::FetchResponse {
    http.fetch(&FetchRequest {
        url,
        body_limit: limit,
    })
    .unwrap()
}

fn assert_unicode_truncation(renderer: &ChromeRenderer, url: &str, control: &CrawlControl) {
    let response = octopuson_crawl::FetchResponse::new(
        200,
        &[("Content-Type", "text/html; charset=utf-8")],
        "<html><head></head><body>🙂</body></html>",
    );
    let limit = u64::try_from("<html><head></head><body>".len() + 1).unwrap();
    let rendered = renderer
        .render(&RenderRequest {
            url,
            response: &response,
            body_limit: limit,
            control,
            request_allowed: None,
        })
        .unwrap();
    assert!(rendered.truncated);
    assert!(rendered.html.len() <= usize::try_from(limit).unwrap());
}

#[test]
#[ignore = "requires an installed Chrome/Chromium/Edge"]
fn prints_a_self_contained_document_to_pdf() {
    let html = "<!doctype html><meta charset=utf-8><meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; style-src 'unsafe-inline'\"><style>@page{size:A4;margin:12mm}</style><h1>Rapor ğüşİ</h1><script>document.title='ran'</script>";
    let pdf = crate::print_pdf(html, None).unwrap();
    assert!(pdf.starts_with(b"%PDF-"));
    assert!(pdf.len() > 1_000);
    assert!(matches!(
        crate::print_pdf(&"x".repeat(crate::MAX_PDF_HTML + 1), None),
        Err(crate::RenderError::Browser(_))
    ));
}
