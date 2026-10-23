use std::path::Path;
use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, Response, StatusCode, header};
use base64::Engine as _;
use http_body_util::BodyExt as _;
use octopuson_core::http::HttpTransport;
use octopuson_core::http::testing::FakeTransport;
use octopuson_secrets::SecretBackend;
use serde_json::{Value, json};
use tower::ServiceExt as _;

use crate::config::{Config, variables};
use crate::sealed::SealedFile;
use crate::{AppState, app};

/// A gateway address nothing listens on: tests never reach a real one.
const LOCAL_GATEWAY: &str = "http://127.0.0.1:9";

const ME: &str = r#"{"user_id":"u_123","email":"a@example.com","name":"A","balance_micros":5000000,"held_micros":0,"available_micros":5000000,"credit_unit_micros":1000,"low_balance":false,"portal_url":"https://my.octopuson.com"}"#;

fn config(dir: &Path, dev: bool) -> Config {
    let key = base64::engine::general_purpose::STANDARD.encode([7_u8; 32]);
    let dir = dir.display().to_string();
    Config::from_lookup(|name| match name {
        variables::DATA_DIR => Some(dir.clone()),
        variables::MASTER_KEY => Some(key.clone()),
        variables::DEV_LOGIN if dev => Some("1".to_owned()),
        variables::PORTAL_URL => Some("https://my.example.test".to_owned()),
        octopuson_cloud::API_VARIABLE => Some(LOCAL_GATEWAY.to_owned()),
        _ => None,
    })
    .unwrap()
}

fn state(dir: &Path, gateway: &Arc<FakeTransport>, dev: bool) -> Arc<AppState> {
    let gateway: Arc<dyn HttpTransport> = gateway.clone();
    Arc::new(AppState::new(config(dir, dev), gateway).unwrap())
}

async fn send(state: &Arc<AppState>, request: Request<Body>) -> Response<Body> {
    app(Arc::clone(state)).oneshot(request).await.unwrap()
}

async fn json_of(response: Response<Body>) -> Value {
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap_or(Value::Null)
}

fn header_of(response: &Response<Body>, name: header::HeaderName) -> Vec<String> {
    response
        .headers()
        .get_all(name)
        .iter()
        .map(|value| value.to_str().unwrap().to_owned())
        .collect()
}

fn get(uri: &str, cookie: Option<&str>) -> Request<Body> {
    let mut request = Request::get(uri);
    if let Some(cookie) = cookie {
        request = request.header(header::COOKIE, cookie);
    }
    request.body(Body::empty()).unwrap()
}

fn post(uri: &str, cookie: &str, csrf: Option<&str>, body: &Value) -> Request<Body> {
    let mut request = Request::post(uri)
        .header(header::COOKIE, cookie)
        .header(header::CONTENT_TYPE, "application/json");
    if let Some(csrf) = csrf {
        request = request.header("x-csrf-token", csrf);
    }
    request.body(Body::from(body.to_string())).unwrap()
}

/// Signs in through the account site (played by the fake gateway); the session cookie.
async fn sign_in(state: &Arc<AppState>) -> String {
    let response = send(state, get("/auth/login?next=/projects", None)).await;
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    let location = header_of(&response, header::LOCATION).remove(0);
    let url = url::Url::parse(&location).unwrap();
    assert_eq!(
        url.origin().ascii_serialization(),
        "https://my.example.test"
    );
    assert_eq!(url.path(), "/baglan");
    let query: std::collections::HashMap<String, String> = url.query_pairs().into_owned().collect();
    assert_eq!(query["client"], "geo-seo-platform-web");
    assert_eq!(query["redirect_uri"], "http://localhost:8796/auth/callback");
    assert_eq!(query["code_challenge_method"], "S256");
    assert_eq!(query["code_challenge"].len(), 43);
    let login_state = query["state"].clone();
    let login_cookie = header_of(&response, header::SET_COOKIE).remove(0);
    assert!(login_cookie.starts_with(&format!("octopuson_login={login_state};")));
    assert!(login_cookie.contains("HttpOnly"));

    let response = send(
        state,
        get(
            &format!("/auth/callback?code=the-code&state={login_state}"),
            Some(&format!("octopuson_login={login_state}")),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert_eq!(header_of(&response, header::LOCATION), ["/projects"]);
    let session = header_of(&response, header::SET_COOKIE)
        .into_iter()
        .find(|cookie| cookie.starts_with("octopuson_session="))
        .unwrap();
    session.split(';').next().unwrap().to_owned()
}

#[tokio::test]
async fn health_and_security_headers() {
    let dir = tempfile::tempdir().unwrap();
    let state = state(dir.path(), &Arc::new(FakeTransport::new()), false);
    let response = send(&state, get("/health", None)).await;
    assert_eq!(response.status(), StatusCode::OK);
    let policy = header_of(&response, header::CONTENT_SECURITY_POLICY).remove(0);
    assert!(policy.contains("frame-ancestors 'none'"));
    // The map's tiles, and nothing else from outside.
    assert!(policy.contains("connect-src 'self' https://tiles.openfreemap.org;"));
    assert_eq!(header_of(&response, header::X_FRAME_OPTIONS), ["DENY"]);
    assert_eq!(
        header_of(&response, header::X_CONTENT_TYPE_OPTIONS),
        ["nosniff"]
    );
}

#[tokio::test]
#[allow(clippy::too_many_lines)] // one sign-in, then the calls it allows, in order
async fn signs_in_through_the_account_site_and_calls_commands() {
    let dir = tempfile::tempdir().unwrap();
    let gateway = Arc::new(
        FakeTransport::new()
            .reply(
                200,
                r#"{"access_token":"oct_web_token","token_type":"Bearer"}"#,
            )
            .reply(200, ME),
    );
    let state = state(dir.path(), &gateway, false);
    assert_eq!(
        send(&state, get("/api/session", None)).await.status(),
        StatusCode::UNAUTHORIZED
    );
    let cookie = sign_in(&state).await;

    let requests = gateway.requests();
    assert_eq!(requests[0].url, format!("{LOCAL_GATEWAY}/v1/connect/token"));
    let body: Value = serde_json::from_slice(requests[0].body.as_deref().unwrap()).unwrap();
    assert_eq!(body["code"], "the-code");
    assert_eq!(body["code_verifier"].as_str().unwrap().len(), 43);
    assert_eq!(requests[1].url, format!("{LOCAL_GATEWAY}/v1/me"));

    // The token is in the account's folder, sealed.
    let sealed = std::fs::read_to_string(dir.path().join("users/u_123/account.sealed")).unwrap();
    assert!(!sealed.contains("oct_web_token"));

    let session = json_of(send(&state, get("/api/session", Some(&cookie))).await).await;
    assert_eq!(session["signed_in"], true);
    assert_eq!(session["email"], "a@example.com");
    let csrf = session["csrf"].as_str().unwrap().to_owned();

    let refused = send(
        &state,
        post("/api/call/list_projects", &cookie, None, &json!({})),
    )
    .await;
    assert_eq!(refused.status(), StatusCode::FORBIDDEN);
    let refused = send(
        &state,
        post(
            "/api/call/list_projects",
            &cookie,
            Some("wrong"),
            &json!({}),
        ),
    )
    .await;
    assert_eq!(refused.status(), StatusCode::FORBIDDEN);

    let listed = send(
        &state,
        post("/api/call/list_projects", &cookie, Some(&csrf), &json!({})),
    )
    .await;
    assert_eq!(listed.status(), StatusCode::OK);
    assert_eq!(json_of(listed).await, json!([]));

    let created = send(
        &state,
        post(
            "/api/call/create_project",
            &cookie,
            Some(&csrf),
            &json!({ "spec": { "domain": "example.com", "country": "tr", "language": "tr" } }),
        ),
    )
    .await;
    assert_eq!(created.status(), StatusCode::OK);
    assert!(dir.path().join("users/u_123/platform.duckdb").exists());

    let info = json_of(
        send(
            &state,
            post("/api/call/app_info", &cookie, Some(&csrf), &json!({})),
        )
        .await,
    )
    .await;
    assert_eq!(info["host"], "web");
    assert_eq!(info.get("data_dir"), None);

    let missing = send(
        &state,
        post(
            "/api/call/get_project",
            &cookie,
            Some(&csrf),
            &json!({ "id": 99 }),
        ),
    )
    .await;
    assert_eq!(missing.status(), StatusCode::NOT_FOUND);
    assert_eq!(json_of(missing).await["code"], "not_found");

    let unsupported = send(
        &state,
        post(
            "/api/call/start_account_login",
            &cookie,
            Some(&csrf),
            &json!({}),
        ),
    )
    .await;
    assert_eq!(unsupported.status(), StatusCode::FORBIDDEN);

    let garbage = Request::post("/api/call/list_projects")
        .header(header::COOKIE, &cookie)
        .header("x-csrf-token", &csrf)
        .body(Body::from("{not json"))
        .unwrap();
    let garbage = send(&state, garbage).await;
    assert_eq!(garbage.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn a_sign_in_from_another_browser_or_twice_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let state = state(dir.path(), &Arc::new(FakeTransport::new()), false);
    let response = send(&state, get("/auth/login", None)).await;
    let location = header_of(&response, header::LOCATION).remove(0);
    let login_state = url::Url::parse(&location)
        .unwrap()
        .query_pairs()
        .find(|(key, _)| key == "state")
        .unwrap()
        .1
        .into_owned();
    let other_browser = send(
        &state,
        get(
            &format!("/auth/callback?code=c&state={login_state}"),
            Some("octopuson_login=other"),
        ),
    )
    .await;
    assert_eq!(other_browser.status(), StatusCode::BAD_REQUEST);
    let unknown = send(
        &state,
        get(
            "/auth/callback?code=c&state=unknown-state",
            Some("octopuson_login=unknown-state"),
        ),
    )
    .await;
    assert_eq!(unknown.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn logging_out_ends_the_session_and_a_revoked_token_ends_them_all() {
    let dir = tempfile::tempdir().unwrap();
    let gateway = Arc::new(
        FakeTransport::new()
            .reply(200, r#"{"access_token":"oct_one","token_type":"Bearer"}"#)
            .reply(200, ME)
            .reply(200, r#"{"access_token":"oct_two","token_type":"Bearer"}"#)
            .reply(200, ME)
            // The second sign-in revokes the first token.
            .reply(204, ""),
    );
    let state = state(dir.path(), &gateway, false);
    let first = sign_in(&state).await;
    let second = sign_in(&state).await;
    assert_eq!(
        gateway.requests()[4].url,
        format!("{LOCAL_GATEWAY}/v1/logout")
    );

    let session = json_of(send(&state, get("/api/session", Some(&first))).await).await;
    let csrf = session["csrf"].as_str().unwrap().to_owned();
    let without_csrf = Request::post("/auth/logout")
        .header(header::COOKIE, &first)
        .body(Body::empty())
        .unwrap();
    assert_eq!(
        send(&state, without_csrf).await.status(),
        StatusCode::FORBIDDEN
    );
    let logout = Request::post("/auth/logout")
        .header(header::COOKIE, &first)
        .header("x-csrf-token", &csrf)
        .body(Body::empty())
        .unwrap();
    assert_eq!(send(&state, logout).await.status(), StatusCode::NO_CONTENT);
    assert_eq!(
        send(&state, get("/api/session", Some(&first)))
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
    // The other browser stays signed in until the token is revoked.
    assert_eq!(
        send(&state, get("/api/session", Some(&second)))
            .await
            .status(),
        StatusCode::OK
    );
    state
        .accounts
        .get("u_123")
        .unwrap()
        .platform
        .workspace()
        .set_account_token(None)
        .unwrap();
    assert_eq!(
        send(&state, get("/api/session", Some(&second)))
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn the_development_sign_in_exists_only_when_asked_for() {
    let dir = tempfile::tempdir().unwrap();
    let off = state(dir.path(), &Arc::new(FakeTransport::new()), false);
    let response = send(&off, get("/auth/dev", None)).await;
    assert_ne!(response.status(), StatusCode::SEE_OTHER);

    let dir = tempfile::tempdir().unwrap();
    let on = state(dir.path(), &Arc::new(FakeTransport::new()), true);
    let response = send(&on, get("/auth/dev?next=https://evil.example", None)).await;
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert_eq!(header_of(&response, header::LOCATION), ["/"]);
    let cookie = header_of(&response, header::SET_COOKIE)
        .into_iter()
        .find(|cookie| cookie.starts_with("octopuson_session="))
        .unwrap();
    let cookie = cookie.split(';').next().unwrap();
    let session = json_of(send(&on, get("/api/session", Some(cookie))).await).await;
    assert_eq!(session["email"], "dev@localhost");
}

#[test]
fn sealed_values_open_only_with_their_key_and_name() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("secrets.sealed");
    let sealed = SealedFile::new(path.clone(), &[1_u8; 32]);
    sealed.set("dataforseo/credentials", "me:secret").unwrap();
    assert_eq!(
        sealed.get("dataforseo/credentials").unwrap().as_deref(),
        Some("me:secret")
    );
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(!text.contains("secret\""));
    assert!(!text.contains("me:secret"));

    let other_key = SealedFile::new(path.clone(), &[2_u8; 32]);
    assert!(other_key.get("dataforseo/credentials").is_err());

    // A value moved to another name does not open.
    let mut values: std::collections::BTreeMap<String, String> =
        serde_json::from_str(&text).unwrap();
    let moved = values.remove("dataforseo/credentials").unwrap();
    values.insert("serper/key".to_owned(), moved);
    std::fs::write(&path, serde_json::to_vec(&values).unwrap()).unwrap();
    assert!(sealed.get("serper/key").is_err());

    sealed.delete("serper/key").unwrap();
    assert_eq!(sealed.get("serper/key").unwrap(), None);
}

#[test]
fn settings_are_checked() {
    let dir = tempfile::tempdir().unwrap();
    let base = |name: &str| match name {
        variables::DATA_DIR => Some(dir.path().display().to_string()),
        octopuson_cloud::API_VARIABLE => Some(LOCAL_GATEWAY.to_owned()),
        _ => None,
    };
    assert!(
        Config::from_lookup(base)
            .unwrap_err()
            .contains(variables::MASTER_KEY)
    );
    let short = |name: &str| match name {
        variables::MASTER_KEY => Some("c2hvcnQ=".to_owned()),
        other => base(other),
    };
    assert!(Config::from_lookup(short).unwrap_err().contains("32 bytes"));
    let plain_http = |name: &str| match name {
        variables::PUBLIC_URL => Some("http://app.example.test".to_owned()),
        variables::DEV_LOGIN => Some("1".to_owned()),
        other => base(other),
    };
    assert!(
        Config::from_lookup(plain_http)
            .unwrap_err()
            .contains("https")
    );
    let open_dev = |name: &str| match name {
        variables::BIND => Some("0.0.0.0:8796".to_owned()),
        variables::DEV_LOGIN => Some("1".to_owned()),
        other => base(other),
    };
    assert!(
        Config::from_lookup(open_dev)
            .unwrap_err()
            .contains("loopback")
    );
    let production = |name: &str| match name {
        variables::PUBLIC_URL => Some("https://app.octopuson.com/".to_owned()),
        variables::MASTER_KEY => Some(base64::engine::general_purpose::STANDARD.encode([9_u8; 32])),
        other => base(other),
    };
    let config = Config::from_lookup(production).unwrap();
    assert!(config.secure());
    assert_eq!(
        config.callback_url(),
        "https://app.octopuson.com/auth/callback"
    );
    assert_eq!(crate::auth::session_cookie(true), "__Host-octopuson");
    assert_eq!(
        config.google_callback_url(),
        "https://app.octopuson.com/auth/google/callback"
    );
    let half_smtp = |name: &str| match name {
        variables::SMTP_URL => Some("smtps://u:p@mail.example.com:465".to_owned()),
        other => production(other),
    };
    assert!(
        Config::from_lookup(half_smtp)
            .unwrap_err()
            .contains("both or neither")
    );
    let half_google = |name: &str| match name {
        variables::GOOGLE_CLIENT_ID => Some("web.apps.googleusercontent.com".to_owned()),
        other => production(other),
    };
    assert!(
        Config::from_lookup(half_google)
            .unwrap_err()
            .contains("both or neither")
    );
}

#[tokio::test]
async fn scheduled_rank_checks_are_noted_and_started_by_the_tick() {
    let dir = tempfile::tempdir().unwrap();
    let state = state(dir.path(), &Arc::new(FakeTransport::new()), true);
    let response = send(&state, get("/auth/dev", None)).await;
    let cookie = header_of(&response, header::SET_COOKIE)
        .into_iter()
        .find(|cookie| cookie.starts_with("octopuson_session="))
        .unwrap();
    let cookie = cookie.split(';').next().unwrap().to_owned();
    let session = json_of(send(&state, get("/api/session", Some(&cookie))).await).await;
    let csrf = session["csrf"].as_str().unwrap().to_owned();
    let call = |command: &str, body: Value| {
        let request = post(&format!("/api/call/{command}"), &cookie, Some(&csrf), &body);
        let state = Arc::clone(&state);
        async move { json_of(send(&state, request).await).await }
    };

    let project = call(
        "create_project",
        json!({ "spec": { "domain": "example.com", "country": "tr", "language": "tr" } }),
    )
    .await;
    let id = project["id"].as_i64().unwrap();
    call(
        "track_keywords",
        json!({ "project_id": id, "keywords": [{ "keyword": "spor ayakkabı" }] }),
    )
    .await;
    let now = crate::unix_now();
    // Checked by hand only: nothing is due.
    assert_eq!(
        state.store.due_accounts(now + 1, 10).unwrap(),
        Vec::<String>::new()
    );
    call(
        "save_rank_settings",
        json!({ "project_id": id, "schedule": "daily", "depth": 10 }),
    )
    .await;
    assert_eq!(
        state.store.due_accounts(now + 1, 10).unwrap(),
        [crate::auth::DEV_USER]
    );

    // The tick starts the run; without a key or credits it fails and says why.
    let worker = Arc::clone(&state);
    let looked = tokio::task::spawn_blocking(move || crate::schedule::tick(&worker, now))
        .await
        .unwrap();
    assert_eq!(looked, 1);
    let platform = Arc::clone(&state.accounts.get(crate::auth::DEV_USER).unwrap().platform);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while platform.jobs().running() > 0 {
        assert!(std::time::Instant::now() < deadline, "the run did not end");
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    let runs = call("rank_runs", json!({ "project_id": id })).await;
    assert_eq!(runs[0]["scheduled"], true);
    assert_eq!(runs[0]["status"], "failed");
    assert!(runs[0]["error"].as_str().unwrap().contains("credits"));
    // While a run may still be posting checks to the queue the account is looked at again a
    // minute later; with none waiting, it is not due again until tomorrow.
    let worker = Arc::clone(&state);
    tokio::task::spawn_blocking(move || crate::schedule::tick(&worker, now + 120))
        .await
        .unwrap();
    assert_eq!(
        state.store.due_accounts(now + 3_600, 10).unwrap(),
        Vec::<String>::new()
    );
    assert_eq!(
        state.store.due_accounts(now + 21 * 3_600, 10).unwrap(),
        [crate::auth::DEV_USER]
    );
}

#[tokio::test]
async fn google_connects_through_the_servers_return_address() {
    let dir = tempfile::tempdir().unwrap();
    let gateway = Arc::new(
        FakeTransport::new()
            .reply(
                200,
                r#"{"access_token":"oct_web_token","token_type":"Bearer"}"#,
            )
            .reply(200, ME),
    );
    let google = Arc::new(FakeTransport::new().reply(
        200,
        r#"{"access_token":"ya29.a","expires_in":3599,"refresh_token":"1//r","scope":"https://www.googleapis.com/auth/webmasters.readonly"}"#,
    ));
    let mut config = config(dir.path(), false);
    config.google = Some(octopuson_oauth::OAuthClient {
        client_id: "web.apps.googleusercontent.com".to_owned(),
        client_secret: "web-secret".to_owned(),
    });
    let gateway_transport: Arc<dyn HttpTransport> = gateway.clone();
    let mut app_state = AppState::new(config, gateway_transport).unwrap();
    let google_transport: Arc<dyn HttpTransport> = google.clone();
    app_state.accounts.use_transport(google_transport);
    let state = Arc::new(app_state);

    let response = send(&state, get("/auth/google/callback?code=c&state=s", None)).await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    let cookie = sign_in(&state).await;
    let session = json_of(send(&state, get("/api/session", Some(&cookie))).await).await;
    let csrf = session["csrf"].as_str().unwrap().to_owned();

    let started = json_of(
        send(
            &state,
            post(
                "/api/call/start_google_login",
                &cookie,
                Some(&csrf),
                &json!({ "analytics": false }),
            ),
        )
        .await,
    )
    .await;
    assert_eq!(started["job_id"], Value::Null);
    let consent = url::Url::parse(started["url"].as_str().unwrap()).unwrap();
    let query: std::collections::HashMap<String, String> =
        consent.query_pairs().into_owned().collect();
    assert_eq!(consent.host_str(), Some("accounts.google.com"));
    assert_eq!(query["client_id"], "web.apps.googleusercontent.com");
    assert_eq!(
        query["redirect_uri"],
        "http://localhost:8796/auth/google/callback"
    );

    let response = send(
        &state,
        get("/auth/google/callback?error=access_denied", Some(&cookie)),
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let response = send(
        &state,
        get("/auth/google/callback?code=c&state=forged", Some(&cookie)),
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let response = send(
        &state,
        get(
            &format!(
                "/auth/google/callback?code=4%2Fweb&state={}",
                query["state"]
            ),
            Some(&cookie),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert_eq!(header_of(&response, header::LOCATION), ["/#/gsc"]);
    let exchange = String::from_utf8(google.requests()[0].body.clone().unwrap()).unwrap();
    assert!(exchange.contains("client_secret=web-secret"));
    assert!(
        exchange.contains("redirect_uri=http%3A%2F%2Flocalhost%3A8796%2Fauth%2Fgoogle%2Fcallback")
    );

    let status = json_of(
        send(
            &state,
            post("/api/call/google_status", &cookie, Some(&csrf), &json!({})),
        )
        .await,
    )
    .await;
    assert_eq!(status["connected"], true);
    assert_eq!(status["search_console"], true);
    let sealed = std::fs::read_to_string(dir.path().join("users/u_123/secrets.sealed")).unwrap();
    assert!(!sealed.contains("1//r"), "the refresh token is sealed");
}

#[tokio::test]
async fn report_links_open_without_signing_in_until_taken_away() {
    let dir = tempfile::tempdir().unwrap();
    let gateway = Arc::new(
        FakeTransport::new()
            .reply(
                200,
                r#"{"access_token":"oct_web_token","token_type":"Bearer"}"#,
            )
            .reply(200, ME),
    );
    let state = state(dir.path(), &gateway, false);
    let cookie = sign_in(&state).await;
    let session = json_of(send(&state, get("/api/session", Some(&cookie))).await).await;
    let csrf = session["csrf"].as_str().unwrap().to_owned();
    let call = |command: &str, body: Value| {
        let state = Arc::clone(&state);
        let request = post(&format!("/api/call/{command}"), &cookie, Some(&csrf), &body);
        async move { json_of(send(&state, request).await).await }
    };
    let project = call(
        "create_project",
        json!({ "spec": { "domain": "ornekmagaza.com", "country": "tr", "language": "tr" } }),
    )
    .await;
    let report = call(
        "save_report",
        json!({ "project_id": project["id"], "name": "Müşteri <raporu>", "language": "tr", "days": 28,
            "widgets": ["summary", "rankings"], "schedule": "manual", "recipients": [] }),
    )
    .await;
    let ids = json!({ "project_id": project["id"], "report_id": report["id"] });
    let setup = call("reports_setup", json!({ "project_id": project["id"] })).await;
    assert_eq!(setup["can_share"], true);
    assert_eq!(setup["can_email"], false, "no SMTP set up");
    assert_eq!(setup["public_url"], "http://localhost:8796");

    let shared = call("share_report", ids.clone()).await;
    let token = shared["share_token"].as_str().unwrap().to_owned();
    let response = send(&state, get(&format!("/r/{token}"), None)).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(header_of(&response, header::CACHE_CONTROL), ["no-store"]);
    let page = String::from_utf8(
        response
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes()
            .to_vec(),
    )
    .unwrap();
    assert!(page.contains("Müşteri &lt;raporu&gt;"));
    assert!(!page.contains("<raporu>"));
    let stored = std::fs::read_to_string(dir.path().join("server.duckdb")).unwrap_or_default();
    assert!(
        !stored.contains(&token),
        "the server keeps only the token's hash"
    );

    for wrong in ["/r/short", "/r/AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"] {
        assert_eq!(
            send(&state, get(wrong, None)).await.status(),
            StatusCode::NOT_FOUND
        );
    }
    call("unshare_report", ids.clone()).await;
    assert_eq!(
        send(&state, get(&format!("/r/{token}"), None))
            .await
            .status(),
        StatusCode::NOT_FOUND
    );
    let refused = call(
        "save_report",
        json!({ "project_id": project["id"], "report_id": report["id"],
        "name": "x", "language": "tr", "days": 28, "widgets": ["summary"], "schedule": "weekly",
        "recipients": ["a@example.com"] }),
    )
    .await;
    assert_eq!(refused["code"], "unsupported");
}

fn mcp_post(token: Option<&str>, origin: Option<&str>, body: &Value) -> Request<Body> {
    let mut request = Request::post("/mcp")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::ACCEPT, "application/json, text/event-stream");
    if let Some(token) = token {
        request = request.header(header::AUTHORIZATION, format!("Bearer {token}"));
    }
    if let Some(origin) = origin {
        request = request.header(header::ORIGIN, origin);
    }
    request.body(Body::from(body.to_string())).unwrap()
}

#[tokio::test]
#[allow(clippy::too_many_lines)] // a token's whole life: made, used, refused, revoked
async fn assistants_use_the_tools_with_a_personal_access_token() {
    let dir = tempfile::tempdir().unwrap();
    let gateway = Arc::new(
        FakeTransport::new()
            .reply(
                200,
                r#"{"access_token":"oct_web_token","token_type":"Bearer"}"#,
            )
            .reply(200, ME),
    );
    let state = state(dir.path(), &gateway, false);
    let cookie = sign_in(&state).await;
    let session = json_of(send(&state, get("/api/session", Some(&cookie))).await).await;
    let csrf = session["csrf"].as_str().unwrap().to_owned();
    let call = |command: &str, body: Value| {
        let state = Arc::clone(&state);
        let request = post(&format!("/api/call/{command}"), &cookie, Some(&csrf), &body);
        async move {
            let response = send(&state, request).await;
            (response.status(), json_of(response).await)
        }
    };
    let (_, project) = call(
        "create_project",
        json!({ "spec": { "domain": "ornekmagaza.com", "country": "tr", "language": "tr" } }),
    )
    .await;
    let (status, _) = call("create_access_token", json!({ "name": "  " })).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, made) = call(
        "create_access_token",
        json!({ "name": " Claude   Desktop " }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let token = made["token"].as_str().unwrap().to_owned();
    assert!(token.starts_with("opt_") && token.len() == 47, "{token}");
    assert_eq!(made["access_token"]["name"], "Claude Desktop");
    let (_, listed) = call("list_access_tokens", json!({})).await;
    assert_eq!(listed.as_array().unwrap().len(), 1);
    assert_eq!(listed[0]["last_used_at"], Value::Null);
    assert!(
        !listed.to_string().contains(&token),
        "the token is shown only once"
    );

    let initialize = json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize",
        "params": { "protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": { "name": "test", "version": "1" } } });
    let response = send(&state, mcp_post(Some(&token), None, &initialize)).await;
    assert_eq!(response.status(), StatusCode::OK);
    let answer = json_of(response).await;
    assert_eq!(answer["result"]["protocolVersion"], "2025-06-18");
    assert_eq!(answer["result"]["serverInfo"]["name"], "octopuson-platform");
    let notification = json!({ "jsonrpc": "2.0", "method": "notifications/initialized" });
    assert_eq!(
        send(&state, mcp_post(Some(&token), None, &notification))
            .await
            .status(),
        StatusCode::ACCEPTED
    );
    let projects = json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/call",
        "params": { "name": "list_projects", "arguments": {} } });
    let answer = json_of(
        send(
            &state,
            mcp_post(Some(&token), Some("http://localhost:8796"), &projects),
        )
        .await,
    )
    .await;
    assert_eq!(answer["result"]["isError"], false, "{answer}");
    assert_eq!(
        answer["result"]["structuredContent"]["projects"][0]["id"],
        project["id"]
    );
    let (_, listed) = call("list_access_tokens", json!({})).await;
    assert!(listed[0]["last_used_at"].is_i64());

    // Other sites' pages, missing or wrong tokens and the session cookie are refused.
    assert_eq!(
        send(
            &state,
            mcp_post(Some(&token), Some("https://evil.example"), &projects)
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
    for wrong in [None, Some("opt_wrong"), Some("oct_web_token")] {
        let response = send(&state, mcp_post(wrong, None, &projects)).await;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert!(header_of(&response, header::WWW_AUTHENTICATE)[0].starts_with("Bearer"));
    }
    let with_cookie = Request::post("/mcp")
        .header(header::COOKIE, &cookie)
        .body(Body::from(projects.to_string()))
        .unwrap();
    assert_eq!(
        send(&state, with_cookie).await.status(),
        StatusCode::UNAUTHORIZED
    );
    let response = send(&state, get("/mcp", None)).await;
    assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
    assert_eq!(header_of(&response, header::ALLOW), ["POST"]);
    let stored = std::fs::read_to_string(dir.path().join("server.duckdb")).unwrap_or_default();
    assert!(
        !stored.contains(&token),
        "the server keeps only the token's hash"
    );

    // Revoking the token, or signing the account out, stops it.
    let id = listed[0]["id"].clone();
    let (status, _) = call("revoke_access_token", json!({ "id": "nothing" })).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = call("revoke_access_token", json!({ "id": id })).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        send(&state, mcp_post(Some(&token), None, &projects))
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
    let (_, made) = call("create_access_token", json!({ "name": "Cursor" })).await;
    let second = made["token"].as_str().unwrap().to_owned();
    assert_eq!(
        send(&state, mcp_post(Some(&second), None, &projects))
            .await
            .status(),
        StatusCode::OK
    );
    state
        .accounts
        .get("u_123")
        .unwrap()
        .platform
        .workspace()
        .set_account_token(None)
        .unwrap();
    assert_eq!(
        send(&state, mcp_post(Some(&second), None, &projects))
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn an_account_has_at_most_ten_access_tokens() {
    let dir = tempfile::tempdir().unwrap();
    let state = state(dir.path(), &Arc::new(FakeTransport::new()), false);
    for n in 0..10 {
        let made = crate::mcp::command(
            &state,
            "u_1",
            "create_access_token",
            &json!({ "name": format!("t{n}") }),
        )
        .unwrap()
        .unwrap();
        assert!(made["token"].is_string());
    }
    let refused = crate::mcp::command(
        &state,
        "u_1",
        "create_access_token",
        &json!({ "name": "one more" }),
    )
    .unwrap()
    .unwrap_err();
    assert_eq!(refused.0, StatusCode::CONFLICT);
    // Another account's tokens are its own.
    assert!(
        crate::mcp::command(
            &state,
            "u_2",
            "create_access_token",
            &json!({ "name": "t" })
        )
        .unwrap()
        .is_ok()
    );
    let listed = crate::mcp::command(&state, "u_2", "list_access_tokens", &json!({}))
        .unwrap()
        .unwrap();
    assert_eq!(listed.as_array().unwrap().len(), 1);
    assert!(crate::mcp::command(&state, "u_1", "list_projects", &json!({})).is_none());
}

fn sync_request(token: Option<&str>, body: Vec<u8>) -> Request<Body> {
    let mut request = Request::post("/api/sync").header(
        header::CONTENT_TYPE,
        octopuson_platform_app::sync::CONTENT_TYPE,
    );
    if let Some(token) = token {
        request = request.header(header::AUTHORIZATION, format!("Bearer {token}"));
    }
    request.body(Body::from(body)).unwrap()
}

fn greeting(changes: Vec<octopuson_platform::SyncChange>) -> Vec<u8> {
    octopuson_platform_app::sync::encode(&octopuson_platform_app::sync::SyncRequest {
        protocol: octopuson_platform_app::sync::PROTOCOL,
        replica: "desktop-0123456789".to_owned(),
        epoch: None,
        cursor: 0,
        changes,
        last: true,
        pull: true,
        replace: false,
        resend: Vec::new(),
        resend_after: 0,
    })
    .unwrap()
}

async fn sync_answer(response: Response<Body>) -> octopuson_platform_app::sync::SyncResponse {
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    octopuson_platform_app::sync::decode(&bytes).unwrap()
}

#[tokio::test]
async fn desktops_sync_with_the_account_token_only() {
    let dir = tempfile::tempdir().unwrap();
    // The gateway refuses the development token (no development sign-in here), then names the
    // account of the desktop's token once; it is remembered after.
    let gateway = Arc::new(
        FakeTransport::new()
            .reply(401, r#"{"error":"unauthorized"}"#)
            .reply(200, ME),
    );
    let state = state(dir.path(), &gateway, false);

    let response = send(&state, sync_request(None, greeting(Vec::new()))).await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    let mut wrong = sync_request(Some("oct_desktop"), greeting(Vec::new()));
    wrong
        .headers_mut()
        .insert(header::CONTENT_TYPE, "application/json".parse().unwrap());
    assert_eq!(
        send(&state, wrong).await.status(),
        StatusCode::UNSUPPORTED_MEDIA_TYPE
    );
    // The development token works only with the development sign-in.
    let response = send(&state, sync_request(Some("oct_dev"), greeting(Vec::new()))).await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    // A desktop's project arrives in the account's workspace.
    let desktop = tempfile::tempdir().unwrap();
    let store = octopuson_platform::PlatformStore::open(desktop.path()).unwrap();
    store
        .create_project(
            &octopuson_platform::ProjectSpec {
                domain: "ornek.com".to_owned(),
                country: "tr".to_owned(),
                language: "tr".to_owned(),
                ..octopuson_platform::ProjectSpec::default()
            },
            "2026-10-22T10:00:00Z",
        )
        .unwrap();
    store.sync_scan(1).unwrap();
    let changes = store.sync_local_changes(0, 0, 1 << 20).unwrap().changes;
    let answer =
        sync_answer(send(&state, sync_request(Some("oct_desktop"), greeting(changes))).await).await;
    assert_eq!(answer.account, "u_123");
    assert_eq!(answer.accepted.len(), 1);
    assert_eq!(answer.projects, 1);
    let account = state.accounts.get("u_123").unwrap();
    let workspace = account.platform.workspace();
    assert_eq!(workspace.store().unwrap().project_count().unwrap(), 1);
    // The desktop's token runs the account's scheduled work until the web is signed in.
    assert_eq!(workspace.account_token().as_deref(), Some("oct_desktop"));
    assert!(crate::sync::borrowed(workspace, "oct_desktop"));
    let again = sync_answer(
        send(
            &state,
            sync_request(Some("oct_desktop"), greeting(Vec::new())),
        )
        .await,
    )
    .await;
    assert_eq!(again.changes.len(), 0);
    assert_eq!(gateway.requests().len(), 2);
}

#[tokio::test]
async fn crawls_travel_with_the_development_account() {
    let dir = tempfile::tempdir().unwrap();
    let state = state(dir.path(), &Arc::new(FakeTransport::new()), true);
    let hello =
        sync_answer(send(&state, sync_request(Some("oct_dev"), greeting(Vec::new()))).await).await;
    assert_eq!(hello.account, crate::auth::DEV_USER);

    let account = state.accounts.get(crate::auth::DEV_USER).unwrap();
    let workspace = account.platform.workspace();
    let audit_uid = {
        let store = workspace.store().unwrap();
        let project = store
            .create_project(
                &octopuson_platform::ProjectSpec {
                    domain: "ornek.com".to_owned(),
                    country: "tr".to_owned(),
                    language: "tr".to_owned(),
                    ..octopuson_platform::ProjectSpec::default()
                },
                "2026-10-22T10:00:00Z",
            )
            .unwrap();
        let audit = store
            .begin_audit(
                project.id,
                false,
                "https://ornek.com/",
                "2026-10-22T10:00:00Z",
            )
            .unwrap();
        store.set_audit_crawl(audit, "20261022-ornek").unwrap();
        store
            .finish_audit(
                audit,
                &octopuson_platform::AuditEnd {
                    status: "done",
                    pages: 1,
                    health: Some(90.0),
                    geo: None,
                    issues: 0,
                    metered_pages: 0,
                    charged_micros: 0,
                    error: None,
                },
                "2026-10-22T10:05:00Z",
            )
            .unwrap();
        store.sync_audit_crawls().unwrap()[0].uid.clone()
    };
    let uri = format!("/api/sync/crawls/{audit_uid}");
    let authorized = |request: axum::http::request::Builder| {
        request.header(header::AUTHORIZATION, "Bearer oct_dev")
    };
    let response = send(
        &state,
        authorized(Request::get(&uri)).body(Body::empty()).unwrap(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    let source = tempfile::tempdir().unwrap();
    std::fs::write(source.path().join("crawl.json"), "{}").unwrap();
    std::fs::write(source.path().join("crawl.duckdb"), vec![7_u8; 5000]).unwrap();
    let packed = octopuson_platform_app::sync::crawls::pack(source.path()).unwrap();
    let response = send(
        &state,
        authorized(Request::post(&uri))
            .body(Body::from(packed))
            .unwrap(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let folder = workspace.data_dir().join("crawls").join("20261022-ornek");
    assert_eq!(
        std::fs::read(folder.join("crawl.duckdb")).unwrap().len(),
        5000
    );

    let response = send(
        &state,
        authorized(Request::get(&uri)).body(Body::empty()).unwrap(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let copy = tempfile::tempdir().unwrap();
    let target = copy.path().join("crawl");
    octopuson_platform_app::sync::crawls::unpack(&bytes, &target).unwrap();
    assert_eq!(
        std::fs::read(target.join("crawl.duckdb")).unwrap().len(),
        5000
    );

    let response = send(
        &state,
        authorized(Request::post("/api/sync/crawls/nope"))
            .body(Body::from(vec![1_u8]))
            .unwrap(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}
