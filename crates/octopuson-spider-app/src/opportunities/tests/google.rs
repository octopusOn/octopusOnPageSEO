use super::*;
use octopuson_core::http::testing::FakeTransport;
#[allow(clippy::unnecessary_wraps)] // Implements the fallible credential-provider test seam.
fn token(_id: &str, _http: &dyn HttpTransport) -> Result<String, String> {
    Ok("fixture-access".into())
}
fn client<'a>(http: &'a FakeTransport, cancel: &'a AtomicBool) -> Client<'a> {
    Client {
        token: &token,
        invalidate: &|_| {},
        http,
        connection: "fixture",
        cancel,
        requests: 0,
        progress: None,
        progress_state: SyncProgress::default(),
    }
}
fn project() -> OpportunityProject {
    OpportunityProject {
        id: "fixture".into(),
        name: "Fixture".into(),
        site_url: "https://example.com/".into(),
        gsc_property: "sc-domain:example.com".into(),
        ga4_property: "123".into(),
        ga4_origin_confirmed: true,
        ..Default::default()
    }
}
#[test]
fn quota_backoff_is_bounded_and_cancelled_requests_are_never_sent() {
    let http = FakeTransport::new()
        .reply_with_headers(429, vec![("Retry-After", "0")], Vec::new())
        .reply(200, b"{}".to_vec());
    let cancel = AtomicBool::new(false);
    let mut c = client(&http, &cancel);
    assert!(
        c.send("https://www.googleapis.com/webmasters/v3/sites", None)
            .is_ok()
    );
    assert_eq!(http.requests().len(), 2);
    cancel.store(true, Ordering::Relaxed);
    assert!(
        c.send("https://www.googleapis.com/webmasters/v3/sites", None)
            .is_err()
    );
    assert_eq!(http.requests().len(), 2);
}
#[test]
fn unauthorized_access_refreshes_once_and_errors_do_not_expose_response_body() {
    let http = FakeTransport::new()
        .reply(401, b"secret".to_vec())
        .reply(200, b"{}".to_vec());
    let cancel = AtomicBool::new(false);
    let mut c = client(&http, &cancel);
    assert!(
        c.send("https://www.googleapis.com/webmasters/v3/sites", None)
            .is_ok()
    );
    assert_eq!(http.requests().len(), 2);
    let http = FakeTransport::new().reply(403, b"private-token".to_vec());
    let error = client(&http, &cancel)
        .send("https://www.googleapis.com/webmasters/v3/sites", None)
        .unwrap_err();
    assert!(error.contains("permission"));
    assert!(!error.contains("private-token"));
}
#[test]
fn gsc_requests_final_data_and_keeps_top_row_coverage_warning() {
    let http=FakeTransport::new().reply(200,serde_json::to_vec(&json!({"rows":[{"keys":["https://example.com/tr/?p=2"],"clicks":20,"impressions":1000,"position":8.5}]})).unwrap());
    let cancel = AtomicBool::new(false);
    let output = gsc(
        &mut client(&http, &cancel),
        &project(),
        AnalysisRequest::default().current,
        DataGrain::Pages,
    )
    .unwrap();
    assert_eq!(
        output.rows[0].url.as_deref(),
        Some("https://example.com/tr/?p=2")
    );
    assert!(output.coverage.warnings.contains(&"gsc-top-rows".into()));
    let body: Value = serde_json::from_slice(http.requests()[0].body.as_ref().unwrap()).unwrap();
    assert_eq!(body["dataState"], "final");
    assert_eq!(body["rowLimit"], 25000);
    assert_eq!(body["dimensions"], json!(["page"]));
    assert_eq!(body["aggregationType"], "auto");
}
#[test]
fn ga4_organic_filter_goal_rate_and_timezone_are_explicit() {
    let http=FakeTransport::new().reply(200,br#"{"timeZone":"Europe/Istanbul","currencyCode":"TRY"}"#.to_vec()).reply(200,br#"{"metrics":[{"apiName":"sessionKeyEventRate"}]}"#.to_vec()).reply(200,b"{}".to_vec()).reply(200,br#"{"rows":[{"dimensionValues":[{"value":"/tr/?page=2"}],"metricValues":[{"value":"120"},{"value":"4"},{"value":"0.025"},{"value":"500"}]}],"metadata":{"subjectToThresholding":true}}"#.to_vec());
    let cancel = AtomicBool::new(false);
    let output = ga4(
        &mut client(&http, &cancel),
        &project(),
        AnalysisRequest::default().current,
    )
    .unwrap();
    assert_eq!(output.rows[0].goal_rate, Some(0.025));
    assert_eq!(output.coverage.timezone, "Europe/Istanbul");
    assert!(output.coverage.limited);
    let body: Value = serde_json::from_slice(http.requests()[3].body.as_ref().unwrap()).unwrap();
    assert!(
        body["dimensionFilter"]
            .to_string()
            .contains("sessionSource")
    );
    assert!(body["dimensionFilter"].to_string().contains("organic"));
}
#[test]
fn interrupted_transport_cannot_produce_a_dataset() {
    let http = FakeTransport::new().fail(octopuson_core::http::TransportError::Timeout);
    let cancel = AtomicBool::new(false);
    assert!(
        gsc(
            &mut client(&http, &cancel),
            &project(),
            AnalysisRequest::default().current,
            DataGrain::Pages
        )
        .is_err()
    );
}

#[test]
fn ga4_pagination_uses_fetched_offset_even_when_landing_paths_are_unmatched() {
    let bad = json!({"dimensionValues":[{"value":"(not set)"}],"metricValues":[{"value":"1"},{"value":"0"},{"value":"0"},{"value":"0"}]});
    let http=FakeTransport::new().reply(200,b"{}".to_vec()).reply(200,br#"{"metrics":[{"apiName":"sessionKeyEventRate"}]}"#.to_vec()).reply(200,b"{}".to_vec()).reply(200,serde_json::to_vec(&json!({"rows":vec![bad;10000]})).unwrap()).reply(200,br#"{"rows":[{"dimensionValues":[{"value":"/valid"}],"metricValues":[{"value":"1"},{"value":"0"},{"value":"0"},{"value":"0"}]}]}"#.to_vec());
    let cancel = AtomicBool::new(false);
    let result = ga4(
        &mut client(&http, &cancel),
        &project(),
        AnalysisRequest::default().current,
    )
    .unwrap();
    assert_eq!(result.rows.len(), 1);
    let body: Value = serde_json::from_slice(http.requests()[4].body.as_ref().unwrap()).unwrap();
    assert_eq!(body["offset"], "10000");
}
#[test]
fn selected_key_event_counts_and_session_rate_use_distinct_compatible_reports() {
    let http=FakeTransport::new().reply(200,b"{}".to_vec()).reply(200,br#"{"metrics":[{"apiName":"sessionKeyEventRate:generate_lead"}]}"#.to_vec()).reply(200,b"{}".to_vec()).reply(200,br#"{"rows":[{"dimensionValues":[{"value":"/tr/"}],"metricValues":[{"value":"100"},{"value":"9"},{"value":"0.02"},{"value":"0"}]}]}"#.to_vec()).reply(200,br#"{"rows":[{"dimensionValues":[{"value":"/tr/"}],"metricValues":[{"value":"2"}]}]}"#.to_vec());
    let mut p = project();
    p.goal = BusinessGoal::Event;
    p.event_name = "generate_lead".into();
    let cancel = AtomicBool::new(false);
    let result = ga4(
        &mut client(&http, &cancel),
        &p,
        AnalysisRequest::default().current,
    )
    .unwrap();
    assert_eq!(result.rows[0].key_events, Some(2.0));
    assert_eq!(result.rows[0].goal_rate, Some(0.02));
    let body: Value = serde_json::from_slice(http.requests()[4].body.as_ref().unwrap()).unwrap();
    assert!(
        body["dimensionFilter"]
            .to_string()
            .contains("generate_lead")
    );
}

#[test]
fn gsc_reports_row_and_request_progress_before_report_completion() {
    let row = json!({"keys":["https://example.com/a"],"clicks":1,"impressions":20,"position":8});
    let http = FakeTransport::new()
        .reply(
            200,
            serde_json::to_vec(&json!({"rows":vec![row;25_000]})).unwrap(),
        )
        .reply(200, b"{}".to_vec());
    let cancel = AtomicBool::new(false);
    let mut updates = Vec::new();
    let mut observe = |progress| updates.push(progress);
    let mut c = client(&http, &cancel);
    c.progress = Some(&mut observe);
    let range = AnalysisRequest::default().current;
    let dataset = gsc(&mut c, &project(), range, DataGrain::Pages).unwrap();
    drop(c);
    assert_eq!(dataset.rows.len(), 25_000);
    assert!(updates.iter().any(|p| p.requests == 1 && p.rows == 25_000));
    assert!(updates.iter().any(|p| p.requests == 2 && p.rows == 25_000));
    assert!(
        updates
            .iter()
            .all(|p| p.phase == "gsc-pages" && p.period == Some(range))
    );
}
#[test]
fn ga4_short_pages_continue_until_reported_row_count_and_use_stable_order() {
    let row = |path: &str| json!({"dimensionValues":[{"value":path}],"metricValues":[{"value":"100"},{"value":"2"},{"value":"0.02"},{"value":"10"}]});
    let http = FakeTransport::new()
        .reply(200, b"{}".to_vec())
        .reply(
            200,
            br#"{"metrics":[{"apiName":"sessionKeyEventRate"}]}"#.to_vec(),
        )
        .reply(200, b"{}".to_vec())
        .reply(
            200,
            serde_json::to_vec(&json!({"rows":[row("/a")],"rowCount":2})).unwrap(),
        )
        .reply(
            200,
            serde_json::to_vec(&json!({"rows":[row("/b")],"rowCount":2})).unwrap(),
        );
    let cancel = AtomicBool::new(false);
    let data = ga4(
        &mut client(&http, &cancel),
        &project(),
        AnalysisRequest::default().current,
    )
    .unwrap();
    assert_eq!(data.rows.len(), 2);
    let body: Value = serde_json::from_slice(http.requests()[4].body.as_ref().unwrap()).unwrap();
    assert_eq!(body["offset"], "1");
    assert_eq!(
        body["orderBys"][0]["dimension"]["dimensionName"],
        "landingPagePlusQueryString"
    );
}
#[test]
fn ga4_early_empty_page_is_limited_and_quota_delay_is_visible() {
    let mut coverage = DataCoverage::default();
    assert!(!ga4_has_more(
        &json!({"rowCount":100}),
        0,
        10,
        &mut coverage
    ));
    assert!(coverage.limited);
    assert!(coverage.warnings.contains(&"ga4-incomplete".into()));
    let http = FakeTransport::new()
        .reply_with_headers(429, vec![("Retry-After", "0")], Vec::new())
        .reply(200, b"{}".to_vec());
    let cancel = AtomicBool::new(false);
    let mut updates = Vec::new();
    let mut observe = |p| updates.push(p);
    let mut c = client(&http, &cancel);
    c.progress = Some(&mut observe);
    c.send("https://www.googleapis.com/webmasters/v3/sites", None)
        .unwrap();
    drop(c);
    assert!(updates.iter().any(|p| p.retry_after_seconds == Some(0)));
    assert!(
        updates
            .iter()
            .any(|p| p.requests == 2 && p.retry_after_seconds.is_none())
    );
}
