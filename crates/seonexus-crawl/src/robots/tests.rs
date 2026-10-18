use super::*;
use crate::{AgentPreset, FetchError, FetchResponse};

const ERROR_BODY: &str = "User-agent: *\nAllow: /\nSitemap: https://example.com/unused.xml\n";

struct Site {
    status: u16,
    redirected: bool,
    calls: Mutex<Vec<String>>,
}
impl Site {
    fn new(status: u16, redirected: bool) -> Self {
        Self {
            status,
            redirected,
            calls: Mutex::new(Vec::new()),
        }
    }
}
impl Fetcher for Site {
    fn fetch(&self, request: &FetchRequest<'_>) -> Result<FetchResponse, FetchError> {
        self.calls.lock().unwrap().push(request.url.into());
        if self.redirected && request.url.ends_with("/robots.txt") {
            return Ok(FetchResponse::new(
                302,
                &[("Location", "/limited-rules")],
                "",
            ));
        }
        Ok(FetchResponse::new(
            self.status,
            &[("Retry-After", "1")],
            ERROR_BODY,
        ))
    }
}

#[test]
fn rate_limited_robots_is_not_missing_for_any_crawler_preset() {
    for preset in [
        AgentPreset::Octopus,
        AgentPreset::GooglebotDesktop,
        AgentPreset::GooglebotSmartphone,
        AgentPreset::Bingbot,
        AgentPreset::Chrome,
    ] {
        let cache = RobotsCache::new(preset.robots_tokens());
        let site = Site::new(429, false);
        for (index, path) in ["/one", "/two?part=2", "/asset.js"].into_iter().enumerate() {
            let (verdict, record) = cache.check(
                &site,
                &Url::parse(&format!("https://example.com{path}")).unwrap(),
            );
            assert!(!verdict.allowed, "{preset:?}: {path}");
            assert_eq!(
                verdict.rule.as_deref(),
                Some("robots.txt rate limited (HTTP 429)")
            );
            if index == 0 {
                let record = record.unwrap();
                assert_eq!(record.status, RobotsStatus::Unreachable);
                assert_eq!(record.http_status, Some(429));
                assert!(record.text.is_empty() && record.sitemaps.is_empty());
            } else {
                assert!(record.is_none());
            }
        }
        assert_eq!(
            *site.calls.lock().unwrap(),
            ["https://example.com/robots.txt"]
        );
    }
}

#[test]
fn other_client_errors_remain_unrestricted_and_do_not_parse_error_bodies() {
    for status in (400..=499).filter(|status| *status != 429) {
        let cache = RobotsCache::new(&[]);
        let site = Site::new(status, false);
        let (verdict, record) =
            cache.check(&site, &Url::parse("https://example.com/page").unwrap());
        assert!(verdict.allowed && verdict.rule.is_none(), "HTTP {status}");
        let record = record.unwrap();
        assert_eq!(record.status, RobotsStatus::Missing);
        assert_eq!(record.http_status, Some(status));
        assert!(record.text.is_empty() && record.sitemaps.is_empty());
    }
}

#[test]
fn redirects_to_rate_limits_preserve_the_final_code_and_cached_denial() {
    let cache = RobotsCache::new(&[]);
    let site = Site::new(429, true);
    let url = Url::parse("https://example.com/page").unwrap();
    let (verdict, record) = cache.check(&site, &url);
    assert!(!verdict.allowed);
    let record = record.unwrap();
    assert_eq!(record.url, "https://example.com/robots.txt");
    assert_eq!(record.http_status, Some(429));
    assert_eq!(record.status, RobotsStatus::Unreachable);
    assert!(!cache.check(&site, &url).0.allowed);
    assert_eq!(
        *site.calls.lock().unwrap(),
        [
            "https://example.com/robots.txt",
            "https://example.com/limited-rules"
        ]
    );
}

#[test]
fn a_rate_limit_on_one_origin_does_not_block_another_port_or_scheme() {
    let cache = RobotsCache::new(&[]);
    assert!(
        !cache
            .check(
                &Site::new(429, false),
                &Url::parse("https://example.com/page").unwrap()
            )
            .0
            .allowed
    );
    for url in ["http://example.com/page", "https://example.com:8443/page"] {
        let (verdict, record) = cache.check(&Site::new(404, false), &Url::parse(url).unwrap());
        assert!(verdict.allowed);
        assert_eq!(record.unwrap().http_status, Some(404));
    }
    assert!(
        !cache
            .check(
                &Site::new(200, false),
                &Url::parse("https://example.com/another").unwrap()
            )
            .0
            .allowed,
        "429 remains cached for this crawl; no implicit retry"
    );
}

#[test]
fn resource_only_responses_are_retained_once_without_duplicating_page_owned_records() {
    for status in [200, 404, 429, 503] {
        let cache = RobotsCache::new(&[]);
        let site = Site::new(status, false);
        let url = Url::parse("https://example.com/asset.js").unwrap();
        let allowed = matches!(status, 200 | 404);
        assert_eq!(cache.check_resource(&site, &url).allowed, allowed);
        assert_eq!(cache.check_resource(&site, &url).allowed, allowed);
        let records = cache.take_deferred();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].http_status, Some(status));
        assert_eq!(cache.take_deferred().len(), 0);
        assert!(cache.check(&site, &url).1.is_none());
        assert_eq!(cache.take_deferred().len(), 0);
        assert_eq!(site.calls.lock().unwrap().len(), 1);
    }
    let cache = RobotsCache::new(&[]);
    let site = Site::new(429, false);
    let url = Url::parse("https://example.com/one").unwrap();
    assert!(cache.check(&site, &url).1.is_some());
    assert!(!cache.check_resource(&site, &url).allowed);
    assert_eq!(cache.take_deferred().len(), 0);
}
