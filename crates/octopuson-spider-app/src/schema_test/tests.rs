#![allow(clippy::unwrap_used)]
use std::sync::Mutex;
use std::time::Duration;

use super::*;
use octopuson_crawl::{FetchError, FetchRequest, FetchResponse, RenderRequest, RenderedPage};

#[derive(Default)]
struct Site(Mutex<Vec<String>>);
impl Fetcher for Site {
    fn fetch(&self, request: &FetchRequest<'_>) -> Result<FetchResponse, FetchError> {
        self.0.lock().unwrap().push(request.url.into());
        let (status, headers, body) = if request.url.ends_with("robots.txt") {
            (404, Vec::new(), String::new())
        } else if request.url.ends_with("/old") {
            (
                301,
                vec![("location".into(), "/page".into())],
                String::new(),
            )
        } else {
            (200, vec![("content-type".into(), "text/html".into())], r#"<a href="/never-fetch">link</a><script type="application/ld+json">{"@context":"https://schema.org","@type":"Organization","name":"Test"}</script>"#.into())
        };
        Ok(FetchResponse {
            status,
            headers,
            body: body.into_bytes(),
            truncated: false,
            http_version: "HTTP/1.1".into(),
            response_time: Duration::ZERO,
        })
    }
}
fn request() -> SchemaTestRequest {
    SchemaTestRequest {
        mode: SchemaTestMode::Url,
        input: "https://example.com/old".into(),
        rendering: Rendering::default(),
    }
}
struct Script(bool);
impl Renderer for Script {
    fn render(&self, request: &RenderRequest<'_>) -> Result<RenderedPage, FetchError> {
        if self.0 {
            return Err(FetchError::Timeout);
        }
        Ok(RenderedPage { html: r#"<script type="application/ld+json">{"@context":"https://schema.org","@type":"Product"}</script>"#.into(), url: request.url.into(), elapsed: Duration::ZERO, truncated: false })
    }
}
#[test]
fn explicit_url_test_follows_redirects_but_never_page_links_or_sitemaps() {
    let site = Site::default();
    let result = test_with(&plan(&request()).unwrap(), &site, None).unwrap();
    assert_eq!(result.url.as_deref(), Some("https://example.com/page"));
    assert_eq!(result.redirects, ["https://example.com/old"]);
    assert_eq!(result.report.unwrap().items[0].types, ["Organization"]);
    assert!(
        site.0
            .lock()
            .unwrap()
            .iter()
            .all(|url| url.ends_with("/old")
                || url.ends_with("/page")
                || url.ends_with("robots.txt"))
    );
}
#[test]
fn successful_javascript_test_validates_dom_and_failed_render_never_reports_a_pass() {
    let mut request = request();
    request.rendering.mode = RenderMode::Javascript;
    let plan = plan(&request).unwrap();
    let result = test_with(&plan, &Site::default(), Some(&Script(false))).unwrap();
    assert!(result.rendered);
    assert_eq!(result.report.unwrap().items[0].types, ["Product"]);
    let result = test_with(&plan, &Site::default(), Some(&Script(true))).unwrap();
    assert!(!result.rendered);
    assert!(result.render_error.is_some());
    assert!(result.report.is_none());
}
#[test]
fn pasted_source_never_starts_browser_even_when_rendering_is_selected() {
    let mut request = request();
    request.mode = SchemaTestMode::Jsonld;
    request.input = r#"{"@context":"https://schema.org","@type":"Service"}"#.into();
    request.rendering.mode = RenderMode::Javascript;
    request.rendering.browser_path = Some("/does/not/exist".into());
    let result = test_schema(&request).unwrap();
    assert!(!result.rendered);
    assert!(result.url.is_none());
    assert_eq!(result.report.unwrap().items[0].status, "unchecked");
}
#[test]
fn invalid_schemes_and_missing_urls_are_rejected_before_network_use() {
    for input in ["", "file:///etc/passwd", "javascript:alert(1)"] {
        let request = SchemaTestRequest {
            input: input.into(),
            ..request()
        };
        assert!(plan(&request).is_err());
    }
}
