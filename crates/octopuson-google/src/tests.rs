//! Requests and answers shaped like Google's documented examples; nothing leaves the process.

use octopuson_core::http::testing::FakeTransport;
use serde_json::{Value, json};

use crate::GoogleError;
use crate::analytics::{self, Breakdown};
use crate::search_console::{self, Dimension, Search, Site};

fn body(transport: &FakeTransport, index: usize) -> Value {
    serde_json::from_slice(transport.requests()[index].body.as_deref().unwrap()).unwrap()
}

fn header(transport: &FakeTransport, index: usize, name: &str) -> Option<String> {
    transport.requests()[index]
        .headers
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case(name))
        .map(|(_, value)| value.clone())
}

fn site(url: &str) -> Site {
    Site {
        site_url: url.to_owned(),
        permission: "siteOwner".to_owned(),
    }
}

#[test]
fn lists_readable_properties_and_matches_the_projects_domain() {
    let transport = FakeTransport::new().reply(
        200,
        json!({ "siteEntry": [
            { "siteUrl": "https://www.ornekmagaza.com/", "permissionLevel": "siteFullUser" },
            { "siteUrl": "sc-domain:ornekmagaza.com", "permissionLevel": "siteOwner" },
            { "siteUrl": "https://baska.com/", "permissionLevel": "siteUnverifiedUser" }
        ]})
        .to_string(),
    );
    let sites = search_console::sites(&transport, "ya29.token").unwrap();
    assert_eq!(sites.len(), 2);
    assert_eq!(
        transport.requests()[0].url,
        "https://www.googleapis.com/webmasters/v3/sites"
    );
    assert_eq!(
        header(&transport, 0, "authorization").as_deref(),
        Some("Bearer ya29.token")
    );
    assert_eq!(
        search_console::matching(&sites, "https://www.OrnekMagaza.com/")
            .unwrap()
            .site_url,
        "sc-domain:ornekmagaza.com"
    );
    let prefixes = [
        site("http://ornekmagaza.com/"),
        site("https://ornekmagaza.com/"),
    ];
    assert_eq!(
        search_console::matching(&prefixes, "ornekmagaza.com")
            .unwrap()
            .site_url,
        "https://ornekmagaza.com/"
    );
    assert!(search_console::matching(&prefixes, "rakip.com").is_none());
}

#[test]
fn search_analytics_page_through_rows_up_to_the_limit() {
    let row = |query: &str| json!({ "keys": [query, "https://ornekmagaza.com/"], "clicks": 12.0, "impressions": 340.0, "ctr": 0.035, "position": 4.2 });
    let transport = FakeTransport::new()
        .reply(
            200,
            json!({ "rows": [row("spor ayakkabı"), row("koşu ayakkabısı")] }).to_string(),
        )
        .reply(200, json!({ "rows": [row("sneaker")] }).to_string());
    let search = Search {
        site_url: "sc-domain:ornekmagaza.com",
        start: "2026-09-21",
        end: "2026-10-18",
        dimensions: &[Dimension::Query, Dimension::Page],
        max_rows: 3,
    };
    // Pages of the API's size: the limit of 3 asks 3, gets 2 → the last page.
    let rows = search_console::search(&transport, "t", &search).unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].keys, ["spor ayakkabı", "https://ornekmagaza.com/"]);
    assert!((rows[0].ctr - 0.035).abs() < 1e-9);
    assert_eq!(transport.requests().len(), 1);
    assert_eq!(
        transport.requests()[0].url,
        "https://www.googleapis.com/webmasters/v3/sites/sc-domain%3Aornekmagaza.com/searchAnalytics/query"
    );
    let sent = body(&transport, 0);
    assert_eq!(sent["dimensions"], json!(["query", "page"]));
    assert_eq!(sent["rowLimit"], 3);
    assert_eq!(sent["dataState"], "final");
    assert_eq!(sent["type"], "web");

    // A full page asks for the next one from where it stopped.
    let transport = FakeTransport::new()
        .reply(200, json!({ "rows": [row("a"), row("b")] }).to_string())
        .reply(200, json!({ "rows": [row("c")] }).to_string());
    let search = Search {
        max_rows: 2,
        ..search
    };
    assert_eq!(
        search_console::search(&transport, "t", &search)
            .unwrap()
            .len(),
        2
    );
    assert_eq!(transport.requests().len(), 1, "the limit is reached");
}

#[test]
fn refusals_name_their_reason_without_the_token() {
    let transport = FakeTransport::new()
        .reply(401, r#"{"error":{"code":401,"message":"Invalid Credentials ya29.secret","status":"UNAUTHENTICATED"}}"#)
        .reply(403, r#"{"error":{"code":403,"message":"User does not have sufficient permission for site","status":"PERMISSION_DENIED"}}"#)
        .reply_with_headers(429, vec![("retry-after", "30")], "{}")
        .reply(503, "busy");
    let error = search_console::sites(&transport, "ya29.secret").unwrap_err();
    assert!(matches!(&error, GoogleError::Auth(message) if !message.contains("ya29.secret")));
    assert!(matches!(
        search_console::sites(&transport, "t").unwrap_err(),
        GoogleError::Forbidden(message) if message.contains("permission")
    ));
    assert_eq!(
        search_console::sites(&transport, "t").unwrap_err(),
        GoogleError::RateLimited {
            retry_after: Some(30)
        }
    );
    assert!(matches!(
        search_console::sites(&transport, "t").unwrap_err(),
        GoogleError::Unavailable(_)
    ));
}

#[test]
fn lists_analytics_properties_across_pages() {
    let transport = FakeTransport::new()
        .reply(
            200,
            json!({ "accountSummaries": [{ "displayName": "Örnek", "propertySummaries": [
                { "property": "properties/222", "displayName": "Mağaza" }
            ]}], "nextPageToken": "next" })
            .to_string(),
        )
        .reply(
            200,
            json!({ "accountSummaries": [{ "displayName": "Ajans", "propertySummaries": [
                { "property": "properties/111", "displayName": "Blog" }
            ]}]})
            .to_string(),
        );
    let properties = analytics::properties(&transport, "t").unwrap();
    assert_eq!(properties.len(), 2);
    assert_eq!(properties[0].id, "properties/111");
    assert_eq!(properties[0].account, "Ajans");
    assert!(
        transport.requests()[1]
            .url
            .ends_with("pageSize=200&pageToken=next")
    );
    assert_eq!(
        analytics::property_id(" 123 ").as_deref(),
        Some("properties/123")
    );
    assert_eq!(
        analytics::property_id("properties/9").as_deref(),
        Some("properties/9")
    );
    assert_eq!(analytics::property_id("UA-1"), None);
}

#[test]
fn organic_reports_filter_the_channel_and_read_metrics() {
    let transport = FakeTransport::new().reply(
        200,
        json!({
            "rows": [
                { "dimensionValues": [{ "value": "/spor-ayakkabi" }], "metricValues": [{ "value": "1200" }, { "value": "800" }, { "value": "31" }, { "value": "4520.5" }] },
                { "dimensionValues": [{ "value": "/" }], "metricValues": [{ "value": "900" }, { "value": "500" }, { "value": "12" }, { "value": "0" }] }
            ],
            "rowCount": 2,
            "metadata": { "currencyCode": "TRY", "subjectToThresholding": true }
        })
        .to_string(),
    );
    let report = analytics::organic(
        &transport,
        "t",
        "222",
        ("2026-09-21", "2026-10-18"),
        Breakdown::LandingPage,
        5000,
    )
    .unwrap();
    assert_eq!(report.rows.len(), 2);
    assert_eq!(report.rows[0].key, "/spor-ayakkabi");
    assert!((report.rows[0].revenue - 4520.5).abs() < 1e-9);
    assert!(report.limited, "thresholded data is marked");
    assert_eq!(report.currency.as_deref(), Some("TRY"));
    assert!(
        transport.requests()[0]
            .url
            .ends_with("/v1beta/properties/222:runReport")
    );
    let sent = body(&transport, 0);
    assert_eq!(sent["dimensions"][0]["name"], "landingPage");
    assert_eq!(
        sent["dimensionFilter"]["filter"]["fieldName"],
        "sessionDefaultChannelGroup"
    );
    assert_eq!(
        sent["dimensionFilter"]["filter"]["stringFilter"]["value"],
        "Organic Search"
    );
    assert_eq!(sent["metrics"][2]["name"], "keyEvents");
    assert_eq!(sent["limit"], "5000");

    let transport = FakeTransport::new().reply(
        200,
        json!({ "rows": [{ "dimensionValues": [{ "value": "20261018" }], "metricValues": [{ "value": "75" }, { "value": "40" }, { "value": "2" }, { "value": "0" }] }], "rowCount": 1 })
            .to_string(),
    );
    let daily = analytics::organic(
        &transport,
        "t",
        "properties/222",
        ("2026-10-18", "2026-10-18"),
        Breakdown::Date,
        400,
    )
    .unwrap();
    assert_eq!(daily.rows[0].key, "2026-10-18");
    assert!(!daily.limited);
    assert!(matches!(
        analytics::organic(&transport, "t", "UA-1", ("a", "b"), Breakdown::Date, 1),
        Err(GoogleError::Invalid(_))
    ));
}
