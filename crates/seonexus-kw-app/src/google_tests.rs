use std::io::{Read, Write};
use std::net::TcpStream;

use seonexus_core::http::testing::FakeTransport;
use seonexus_kw_store::{KeywordQuery, ProjectSpec, RunStatus};
use seonexus_secrets::{FileBackend, Secrets};
use serde_json::json;

use super::*;
use crate::discover::{DiscoverRequest, DiscoverySource};
use crate::runner::RunControl;

fn workspace() -> (tempfile::TempDir, KeywordWorkspace) {
    let dir = tempfile::tempdir().unwrap();
    let secrets = Secrets::new(
        Box::new(FileBackend::new(dir.path().join("secrets.json"))),
        dir.path().join("secret-bindings.json"),
    )
    .with_environment(|_| None);
    let workspace = KeywordWorkspace::with_secrets(dir.path().to_owned(), secrets);
    (dir, workspace)
}

fn connected() -> (tempfile::TempDir, KeywordWorkspace, i64) {
    let (dir, workspace) = workspace();
    workspace
        .set_google_client("123-abc.apps.googleusercontent.com", "desktop-secret")
        .unwrap();
    workspace
        .secrets()
        .set(
            GOOGLE,
            "refresh_token",
            "1//saved",
            &[seonexus_oauth::TOKEN_ORIGIN],
        )
        .unwrap();
    let id = workspace
        .create_project(&ProjectSpec {
            name: "Örnek".to_owned(),
            country: "tr".to_owned(),
            language: "tr".to_owned(),
            engines: vec!["google".to_owned()],
            site: Some("ornek.com.tr".to_owned()),
            competitors: Vec::new(),
        })
        .unwrap()
        .id;
    (dir, workspace, id)
}

const TOKEN: &str = r#"{"access_token":"ya29.fresh","expires_in":3599}"#;

/// Plays the browser: follows the consent address straight to the redirect.
fn browser(url: &str) -> bool {
    let url = url::Url::parse(url).unwrap();
    let pair = |name: &str| {
        url.query_pairs()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.into_owned())
            .unwrap()
    };
    let redirect = pair("redirect_uri");
    let state = pair("state");
    std::thread::spawn(move || {
        let mut stream = TcpStream::connect(redirect.trim_start_matches("http://")).unwrap();
        write!(
            stream,
            "GET /?state={state}&code=4%2Fcode HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n"
        )
        .unwrap();
        let mut answer = String::new();
        stream.read_to_string(&mut answer).unwrap();
    });
    true
}

#[test]
fn signs_in_with_the_users_client_and_keeps_the_refresh_token() {
    let (_dir, workspace) = workspace();
    assert!(matches!(
        workspace.google_login_with(
            GoogleScopes::default(),
            &FakeTransport::new(),
            &browser,
            &|| false
        ),
        Err(KwAppError::Invalid(_))
    ));
    assert!(matches!(
        workspace.set_google_client("not-a-client", "x"),
        Err(KwAppError::InvalidSettings(_))
    ));
    workspace
        .set_google_client("123-abc.apps.googleusercontent.com", "desktop-secret")
        .unwrap();
    let transport = FakeTransport::new().reply(
        200,
        json!({
            "access_token": "ya29.a", "expires_in": 3599, "refresh_token": "1//new",
            "scope": "https://www.googleapis.com/auth/webmasters.readonly"
        })
        .to_string(),
    );
    let status = workspace
        .google_login_with(
            GoogleScopes {
                search_console: true,
                ads: true,
            },
            &transport,
            &browser,
            &|| false,
        )
        .unwrap();
    assert!(status.connected && status.client_secret);
    // Ads was asked for but not granted.
    assert_eq!(status.scopes, [scopes::SEARCH_CONSOLE]);
    assert_eq!(
        workspace.secret(GOOGLE, "refresh_token").as_deref(),
        Some("1//new")
    );

    let revoke = FakeTransport::new().reply(200, "");
    workspace.google_logout_with(&revoke).unwrap();
    assert!(!workspace.google_status().unwrap().connected);
    assert!(
        revoke.requests()[0]
            .url
            .starts_with(seonexus_oauth::REVOKE_URL)
    );
}

#[test]
fn imports_search_console_queries_with_their_pages() {
    let (_dir, workspace, id) = connected();
    let transport = FakeTransport::new()
        .reply(200, TOKEN)
        .reply(
            200,
            json!({"rows": [
                {"keys": ["türk kahvesi", "https://ornek.com.tr/kahve"], "clicks": 10, "impressions": 300, "position": 4.0},
                {"keys": ["Türk Kahvesi", "https://ornek.com.tr/kahve"], "clicks": 2, "impressions": 100, "position": 8.0},
                {"keys": ["türk kahvesi", "https://ornek.com.tr/blog"], "clicks": 0, "impressions": 20, "position": 30.0},
                {"keys": ["kahve fincanı", "https://ornek.com.tr/fincan"], "clicks": 1, "impressions": 50, "position": 6.0}
            ]})
            .to_string(),
        );
    let request = SearchConsoleRequest {
        site_url: "sc-domain:ornek.com.tr".to_owned(),
        ..SearchConsoleRequest::default()
    };
    let outcome = workspace
        .import_search_console_with(id, &request, &transport, &|| false)
        .unwrap();
    assert_eq!(outcome.rows, 4);
    assert_eq!(outcome.added.added, 2);
    assert_eq!(outcome.pages, 3);
    let store = workspace.store().unwrap();
    let pages = store.site_pages(id).unwrap();
    let main = pages
        .iter()
        .find(|page| page.url == "https://ornek.com.tr/kahve")
        .unwrap();
    assert_eq!(main.impressions, Some(400.0));
    assert!((main.position.unwrap() - 5.0).abs() < 1e-9);
    assert_eq!(
        store.project_settings(id).unwrap()["gsc_site"],
        "sc-domain:ornek.com.tr"
    );
    let body: serde_json::Value =
        serde_json::from_slice(transport.requests()[1].body.as_deref().unwrap()).unwrap();
    assert_eq!(body["dimensions"], json!(["query", "page"]));
    let refresh = String::from_utf8(transport.requests()[0].body.clone().unwrap()).unwrap();
    assert!(refresh.contains("refresh_token=1%2F%2Fsaved"));
    drop(store);
    let rows = workspace
        .keywords(id, &KeywordQuery::default())
        .unwrap()
        .rows;
    assert!(rows.iter().all(|row| row.sources == ["gsc"]));
}

#[test]
fn a_revoked_grant_is_forgotten() {
    let (_dir, workspace, id) = connected();
    let transport = FakeTransport::new().reply(400, r#"{"error":"invalid_grant"}"#);
    let request = SearchConsoleRequest {
        site_url: "sc-domain:ornek.com.tr".to_owned(),
        ..SearchConsoleRequest::default()
    };
    assert!(
        workspace
            .import_search_console_with(id, &request, &transport, &|| false)
            .is_err()
    );
    assert!(!workspace.google_status().unwrap().connected);
}

#[test]
fn discovers_with_the_users_google_ads_account() {
    let (_dir, workspace, id) = connected();
    let quiet = RunControl {
        cancelled: &|| false,
        progress: &|_| {},
        sleep: &|_| {},
    };
    let request = DiscoverRequest {
        seeds: vec!["türk kahvesi".to_owned()],
        sources: vec![DiscoverySource::GoogleAds],
        ..DiscoverRequest::default()
    };
    // Without a customer id the source is reported, after the token refresh is skipped.
    let outcome = workspace
        .discover_with(id, &request, &FakeTransport::new(), &quiet)
        .unwrap();
    assert_eq!(outcome.run.status, RunStatus::Failed);

    workspace
        .set_ads_account(Some("123-456-7890"), None)
        .unwrap();
    let transport = FakeTransport::new().reply(200, TOKEN).reply(
        200,
        json!({"results": [{"text": "türk kahvesi makinesi", "keywordIdeaMetrics": {"avgMonthlySearches": "8100"}}]})
            .to_string(),
    );
    let outcome = workspace
        .discover_with(id, &request, &transport, &quiet)
        .unwrap();
    assert_eq!(outcome.run.status, RunStatus::Done);
    assert_eq!(outcome.added.added, 1);
    let rows = workspace
        .keywords(id, &KeywordQuery::default())
        .unwrap()
        .rows;
    assert_eq!(rows[0].volume, Some(8_100));
    assert_eq!(rows[0].sources, ["google-ads-ideas"]);
    assert!(
        transport.requests()[1]
            .headers
            .contains(&("Authorization".to_owned(), "Bearer ya29.fresh".to_owned()))
    );
}
