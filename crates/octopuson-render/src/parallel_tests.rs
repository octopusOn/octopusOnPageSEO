//! Real-browser concurrency, isolation and owner-cancellation regressions.

use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{
    Arc, Barrier,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};
use std::time::{Duration, Instant};

use octopuson_crawl::{
    CrawlConfig, CrawlControl, CrawlPlan, FetchResponse, RenderMode, RenderRequest, Renderer,
};

use super::ChromeRenderer;

#[derive(Default)]
struct State {
    stop: AtomicBool,
    public: AtomicUsize,
    private: AtomicUsize,
    cancel: AtomicUsize,
    failed: AtomicUsize,
    private_gate_expired: AtomicBool,
    release_owner: AtomicBool,
}
struct Site {
    url: String,
    state: Arc<State>,
    thread: Option<std::thread::JoinHandle<()>>,
}
impl Site {
    fn new() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        listener.set_nonblocking(true).unwrap();
        let state = Arc::new(State::default());
        let worker = Arc::clone(&state);
        let thread = std::thread::spawn(move || {
            let mut children = Vec::new();
            while !worker.stop.load(Ordering::Acquire) {
                if let Ok((stream, _)) = listener.accept() {
                    let state = Arc::clone(&worker);
                    children.push(std::thread::spawn(move || serve(stream, &state)));
                } else {
                    std::thread::sleep(Duration::from_millis(5));
                }
            }
            for child in children {
                child.join().unwrap();
            }
        });
        Self {
            url,
            state,
            thread: Some(thread),
        }
    }
}
impl Drop for Site {
    fn drop(&mut self) {
        self.state.stop.store(true, Ordering::Release);
        if let Some(thread) = self.thread.take() {
            thread.join().unwrap();
        }
    }
}

fn serve(mut stream: TcpStream, state: &State) {
    stream
        .set_read_timeout(Some(Duration::from_millis(500)))
        .unwrap();
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let mut line = String::new();
    if reader.read_line(&mut line).unwrap_or(0) == 0 {
        return;
    }
    let path = line.split_whitespace().nth(1).unwrap_or("/").to_owned();
    loop {
        line.clear();
        if reader.read_line(&mut line).unwrap_or(0) <= 2 {
            break;
        }
    }
    let (status, cache, body, count) = match path.as_str() {
        "/public.js" => {
            state.public.fetch_add(1, Ordering::SeqCst);
            (
                200,
                "public, max-age=3600, immutable",
                "window.shared='Shared';".to_owned(),
                0,
            )
        }
        "/private.js" => {
            let n = state.private.fetch_add(1, Ordering::SeqCst) + 1;
            (200, "no-store", format!("window.privateValue={n};"), n)
        }
        "/cancel.js" => {
            let n = state.cancel.fetch_add(1, Ordering::SeqCst) + 1;
            (
                200,
                "public, max-age=3600, immutable",
                "window.shared='Recovered';".to_owned(),
                n,
            )
        }
        "/failed.js" => {
            let n = state.failed.fetch_add(1, Ordering::SeqCst) + 1;
            (
                if n == 1 { 503 } else { 200 },
                "public, max-age=3600, immutable",
                "window.shared='Recovered';".to_owned(),
                n,
            )
        }
        _ => (404, "no-store", String::new(), 0),
    };
    let date = time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc2822)
        .unwrap();
    let headers = format!(
        "HTTP/1.1 {status} Test\r\nContent-Type: application/javascript; charset=utf-8\r\nCache-Control: {cache}\r\nDate: {date}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let _ = stream.write_all(headers.as_bytes());
    let _ = stream.flush();
    if path == "/public.js" {
        std::thread::sleep(Duration::from_millis(250));
    }
    if path == "/private.js" {
        let deadline = Instant::now() + Duration::from_secs(1);
        while state.private.load(Ordering::SeqCst) < 3
            && Instant::now() < deadline
            && !state.stop.load(Ordering::Acquire)
        {
            std::thread::sleep(Duration::from_millis(5));
        }
        if state.private.load(Ordering::SeqCst) < 3 {
            state.private_gate_expired.store(true, Ordering::Release);
        }
    }
    if path == "/cancel.js" && count == 1 {
        while !state.release_owner.load(Ordering::Acquire) && !state.stop.load(Ordering::Acquire) {
            std::thread::sleep(Duration::from_millis(5));
        }
    }
    let _ = stream.write_all(body.as_bytes());
}

fn renderer(site: &Site) -> ChromeRenderer {
    let mut config = CrawlConfig {
        start_urls: vec![site.url.clone()],
        ..CrawlConfig::default()
    };
    config.rendering.mode = RenderMode::Javascript;
    config.rendering.wait_ms = 50;
    config.rendering.timeout_seconds = 10;
    config.rendering.concurrency = 3;
    ChromeRenderer::launch(&CrawlPlan::new(config).unwrap()).unwrap()
}
fn document(scripts: &str) -> FetchResponse {
    FetchResponse::new(
        200,
        &[("Content-Type", "text/html; charset=utf-8")],
        format!(
            "<html><head><link rel='icon' href='data:,'><title>Raw</title>{scripts}<script>document.addEventListener('DOMContentLoaded',()=>{{document.title=(window.shared||'Raw')+' '+(window.privateValue||'none')+' '+(localStorage.getItem('leak')||'clean');localStorage.setItem('leak','dirty');document.body.innerHTML='<h1>'+document.title+'</h1><a href=\"/generated\">JS link</a>';}});</script></head><body>Shell</body></html>"
        ),
    )
}
fn render(
    renderer: &ChromeRenderer,
    site: &Site,
    n: usize,
    response: &FetchResponse,
    control: &CrawlControl,
) -> Result<octopuson_crawl::RenderedPage, octopuson_crawl::FetchError> {
    renderer.render(&RenderRequest {
        url: &format!("{}/page/{n}", site.url),
        response,
        body_limit: 1024 * 1024,
        request_allowed: None,
        control,
    })
}
fn wait_until(mut condition: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(4);
    while !condition() {
        assert!(
            Instant::now() < deadline,
            "browser did not reach the expected request stage"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
#[ignore = "requires an installed Chrome/Chromium/Edge and loopback access"]
fn browser_parallel_cache_shares_public_files_but_releases_private_headers_immediately() {
    let site = Site::new();
    let renderer = renderer(&site);
    let control = CrawlControl::new();
    let response = document(
        "<script defer src='/public.js'></script><script defer src='/private.js'></script>",
    );
    let barrier = Barrier::new(3);
    let pages = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..3)
            .map(|n| {
                let (site, renderer, response, control, barrier) =
                    (&site, &renderer, &response, &control, &barrier);
                scope.spawn(move || {
                    barrier.wait();
                    render(renderer, site, n, response, control).unwrap()
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|h| h.join().unwrap())
            .collect::<Vec<_>>()
    });
    assert_eq!(site.state.public.load(Ordering::SeqCst), 1);
    assert_eq!(site.state.private.load(Ordering::SeqCst), 3);
    assert!(
        !site.state.private_gate_expired.load(Ordering::Acquire),
        "no-store headers must release waiters before the body completes"
    );
    for n in 1..=3 {
        assert!(
            pages.iter().any(|page| page
                .html
                .contains(&format!("<title>Shared {n} clean</title>"))),
            "private response {n} or isolated storage was lost"
        );
    }
    assert!(pages.iter().all(|page| page.html.contains("/generated")));
}

#[test]
#[ignore = "requires an installed Chrome/Chromium/Edge and loopback access"]
fn browser_cancelled_cache_owner_releases_a_live_waiting_page() {
    let site = Site::new();
    let renderer = renderer(&site);
    let response = document("<script defer src='/cancel.js'></script>");
    let owner_control = CrawlControl::new();
    let other_control = CrawlControl::new();
    std::thread::scope(|scope| {
        let owner = scope.spawn(|| render(&renderer, &site, 0, &response, &owner_control));
        wait_until(|| site.state.cancel.load(Ordering::SeqCst) == 1);
        let other = scope.spawn(|| render(&renderer, &site, 1, &response, &other_control));
        wait_until(|| {
            renderer
                .downloads
                .has_waiter(&format!("{}/cancel.js", site.url))
        });
        let cancelled = Instant::now();
        owner_control.stop();
        assert!(owner.join().unwrap().is_err());
        site.state.release_owner.store(true, Ordering::Release);
        let page = other.join().unwrap().unwrap();
        assert!(page.html.contains("<title>Recovered none clean</title>"));
        assert!(
            cancelled.elapsed() < Duration::from_millis(1900),
            "waiter must be released by owner disposal, before the two-second sharing fallback"
        );
    });
    assert_eq!(site.state.cancel.load(Ordering::SeqCst), 2);
}

#[test]
#[ignore = "requires an installed Chrome/Chromium/Edge and loopback access"]
fn browser_waiting_for_a_slow_fill_falls_back_before_the_owners_render_budget() {
    let site = Site::new();
    let renderer = renderer(&site);
    let response = document("<script defer src='/cancel.js'></script>");
    let owner_control = CrawlControl::new();
    let other_control = CrawlControl::new();
    std::thread::scope(|scope| {
        let owner = scope.spawn(|| render(&renderer, &site, 0, &response, &owner_control));
        wait_until(|| site.state.cancel.load(Ordering::SeqCst) == 1);
        let other = scope.spawn(|| render(&renderer, &site, 1, &response, &other_control));
        wait_until(|| {
            renderer
                .downloads
                .has_waiter(&format!("{}/cancel.js", site.url))
        });
        let started = Instant::now();
        let result = other.join().unwrap();
        owner_control.stop();
        assert!(owner.join().unwrap().is_err());
        site.state.release_owner.store(true, Ordering::Release);
        let page = result.unwrap();
        assert!(page.html.contains("<title>Recovered none clean</title>"));
        assert!(
            started.elapsed() < Duration::from_secs(4),
            "sharing must not consume the owner's ten-second render budget"
        );
    });
    assert_eq!(site.state.cancel.load(Ordering::SeqCst), 2);
}

#[test]
#[ignore = "requires an installed Chrome/Chromium/Edge and loopback access"]
fn browser_failed_fill_never_substitutes_an_error_body_for_another_page() {
    let site = Site::new();
    let renderer = renderer(&site);
    let response = document("<script defer src='/failed.js'></script>");
    let control = CrawlControl::new();
    let barrier = Barrier::new(2);
    let pages = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..2)
            .map(|n| {
                let (site, renderer, response, control, barrier) =
                    (&site, &renderer, &response, &control, &barrier);
                scope.spawn(move || {
                    barrier.wait();
                    render(renderer, site, n, response, control).unwrap()
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|h| h.join().unwrap())
            .collect::<Vec<_>>()
    });
    assert!(
        pages
            .iter()
            .any(|p| p.html.contains("<title>Raw none clean</title>"))
    );
    assert!(
        pages
            .iter()
            .any(|p| p.html.contains("<title>Recovered none clean</title>"))
    );
    let next = render(&renderer, &site, 2, &response, &control).unwrap();
    assert!(next.html.contains("<title>Recovered none clean</title>"));
    assert_eq!(site.state.failed.load(Ordering::SeqCst), 2);
}
