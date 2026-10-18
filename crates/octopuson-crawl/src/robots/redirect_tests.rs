use super::*;
use crate::{FetchError, FetchResponse};

const ORIGIN: &str = "https://example.com";
const SOURCE: &str = "https://example.com/robots.txt";
const RULES: &str = "User-agent: *\nDisallow: /private\n";

#[derive(Default)]
struct Site {
    responses: HashMap<String, Result<FetchResponse, FetchError>>,
    calls: Mutex<Vec<String>>,
}
impl Site {
    fn response(mut self, url: &str, status: u16, headers: &[(&str, &str)], body: &str) -> Self {
        self.responses
            .insert(url.into(), Ok(FetchResponse::new(status, headers, body)));
        self
    }
}
impl Fetcher for Site {
    fn fetch(&self, request: &FetchRequest<'_>) -> Result<FetchResponse, FetchError> {
        self.calls.lock().unwrap().push(request.url.into());
        assert_eq!(request.body_limit, MAX_BYTES);
        self.responses
            .get(request.url)
            .cloned()
            .unwrap_or_else(|| Ok(FetchResponse::new(200, &[], "User-agent: *\nAllow: /")))
    }
}

fn check(site: &Site) -> (Verdict, RobotsRecord) {
    let (verdict, record) = RobotsCache::new(&[]).check(
        site,
        &Url::parse(&format!("{ORIGIN}/private/page")).unwrap(),
    );
    (verdict, record.unwrap())
}

fn assert_unreachable(site: &Site, expected_status: Option<u16>, expected_calls: &[&str]) {
    let (verdict, record) = check(site);
    assert!(!verdict.allowed);
    assert_eq!(verdict.rule.as_deref(), Some("robots.txt unreachable"));
    assert_eq!(record.url, SOURCE);
    assert_eq!(record.status, RobotsStatus::Unreachable);
    assert_eq!(record.http_status, expected_status);
    assert!(record.text.is_empty() && record.sitemaps.is_empty());
    assert_eq!(*site.calls.lock().unwrap(), expected_calls);
}

#[test]
fn supported_redirect_codes_apply_the_destination_rules() {
    for status in [301, 302, 303, 307, 308] {
        let site = Site::default()
            .response(SOURCE, status, &[("LoCaTiOn", "/rules")], "Allow: /")
            .response("https://example.com/rules", 200, &[], RULES);
        let (verdict, record) = check(&site);
        assert!(!verdict.allowed, "HTTP {status}");
        assert_eq!(verdict.rule.as_deref(), Some("Disallow: /private"));
        assert_eq!(record.status, RobotsStatus::Found);
        assert_eq!(record.http_status, Some(200));
        assert_eq!(record.text, RULES);
        assert_eq!(site.calls.lock().unwrap().len(), 2);
    }
}

#[test]
fn other_3xx_codes_never_follow_location_or_parse_the_body() {
    for status in (300..=399).filter(|status| !matches!(status, 301 | 302 | 303 | 307 | 308)) {
        let site = Site::default().response(SOURCE, status, &[("Location", "/allow")], RULES);
        assert_unreachable(&site, Some(status), &[SOURCE]);
    }
}

#[test]
fn invalid_or_non_http_redirect_targets_are_not_requested() {
    for location in [
        "ftp://example.com/rules",
        "file:///tmp/rules",
        "data:,User-agent:*",
        "javascript:unused",
        "mailto:rules@example.com",
        "https://[invalid/rules",
    ] {
        let site = Site::default().response(SOURCE, 302, &[("Location", location)], RULES);
        assert_unreachable(&site, Some(302), &[SOURCE]);
    }
}

#[test]
fn redirect_identity_discards_fragments_and_preserves_query_and_escaped_hash() {
    let target = "https://example.com/rules?keep=2&encoded=a%23b";
    let site = Site::default()
        .response(
            SOURCE,
            302,
            &[("Location", " /rules?keep=2&encoded=a%23b#unused ")],
            "",
        )
        .response(target, 200, &[], RULES);
    let (verdict, record) = check(&site);
    assert!(!verdict.allowed);
    assert_eq!(record.status, RobotsStatus::Found);
    assert_eq!(*site.calls.lock().unwrap(), [SOURCE, target]);
}

#[test]
fn a_cycle_stops_before_requesting_a_previous_url_again() {
    let site = Site::default()
        .response(SOURCE, 302, &[("Location", "/folder/rules#one")], "")
        .response(
            "https://example.com/folder/rules",
            307,
            &[("Location", "../robots.txt#two")],
            RULES,
        );
    assert_unreachable(
        &site,
        Some(307),
        &[SOURCE, "https://example.com/folder/rules"],
    );
}

#[test]
fn missing_empty_and_fragment_only_locations_cannot_refetch_the_same_file() {
    for location in [None, Some(""), Some("  "), Some("#changed")] {
        let headers = location.map(|value| ("Location", value));
        let site = Site::default().response(SOURCE, 301, headers.as_slice(), RULES);
        assert_unreachable(&site, Some(301), &[SOURCE]);
    }
}

fn chain(hops: usize) -> Site {
    let mut site = Site::default();
    let mut current = SOURCE.to_owned();
    for hop in 1..=hops {
        let next = format!("/rules-{hop}");
        site = site.response(&current, 302, &[("Location", &next)], "");
        current = format!("{ORIGIN}{next}");
    }
    site.response(&current, 200, &[], RULES)
}

#[test]
fn five_redirect_hops_are_followed_including_the_final_rules_response() {
    let site = chain(5);
    let (verdict, record) = check(&site);
    assert!(!verdict.allowed);
    assert_eq!(verdict.rule.as_deref(), Some("Disallow: /private"));
    assert_eq!(record.http_status, Some(200));
    assert_eq!(record.status, RobotsStatus::Found);
    assert_eq!(site.calls.lock().unwrap().len(), 6);
}

#[test]
fn a_sixth_hop_is_not_requested_and_remains_conservatively_unreachable() {
    let site = chain(6);
    let (verdict, record) = check(&site);
    assert!(!verdict.allowed);
    assert_eq!(record.status, RobotsStatus::Unreachable);
    assert_eq!(record.http_status, Some(302));
    let calls = site.calls.lock().unwrap();
    assert_eq!(calls.len(), 6);
    assert_eq!(calls.last().unwrap(), "https://example.com/rules-5");
    assert!(record.text.is_empty() && record.sitemaps.is_empty());
}

#[test]
fn cross_origin_redirect_rules_apply_to_the_initial_origin_only() {
    let destination = "http://other.example:8080/shared-rules";
    let site = Site::default()
        .response(SOURCE, 308, &[("Location", destination)], "")
        .response(destination, 200, &[], RULES)
        .response("http://other.example:8080/robots.txt", 404, &[], RULES);
    let cache = RobotsCache::new(&[]);
    for (url, allowed, status) in [
        ("https://example.com/private/page", false, Some(200)),
        ("http://other.example:8080/private/page", true, Some(404)),
        ("https://example.com/public", true, None),
        ("https://example.com/private/again", false, None),
    ] {
        let (verdict, record) = cache.check(&site, &Url::parse(url).unwrap());
        assert_eq!(verdict.allowed, allowed);
        assert_eq!(record.map(|record| record.http_status.unwrap()), status);
    }
    assert_eq!(
        *site.calls.lock().unwrap(),
        [SOURCE, destination, "http://other.example:8080/robots.txt"]
    );
}

#[test]
fn network_failure_keeps_the_last_observed_http_code_without_parsing_redirect_body() {
    let target = "https://example.com/rules";
    let mut site = Site::default().response(SOURCE, 302, &[("Location", "/rules")], RULES);
    site.responses
        .insert(target.into(), Err(FetchError::Timeout));
    assert_unreachable(&site, Some(302), &[SOURCE, target]);
    let mut site = Site::default();
    site.responses.insert(SOURCE.into(), Err(FetchError::Dns));
    assert_unreachable(&site, None, &[SOURCE]);
}
