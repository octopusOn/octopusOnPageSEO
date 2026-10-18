use std::fmt::Write as _;

use octopuson_cloud::{PLACEHOLDER_KEY, SignIn, SourceMode, TokenStore, Via};
use octopuson_core::http::HttpRequest;
use octopuson_core::http::testing::FakeTransport;
use serde_json::json;

use crate::{ApiConfig, FinderStore, FinderWorkspace, ScanConfig, ScanControl, ScanMeta};

/// A closed local port: anything that slips past the fakes fails at once, and nothing reaches
/// the real gateway.
const GATEWAY: &str = "http://127.0.0.1:9";

fn workspace(dir: &std::path::Path) -> FinderWorkspace {
    FinderWorkspace::with_store(FinderStore::new(dir))
        .with_tokens(TokenStore::in_dir(&dir.join("account"), GATEWAY))
}

/// Signs in and loads a catalog with Serper and SerpApi from a fake gateway.
fn sign_in(workspace: &FinderWorkspace) {
    let connector = |id: &str, upstream: &str| {
        json!({"id": id, "name": id, "upstream": upstream, "routes": [{"method": "POST", "path": "/search"}, {"method": "GET", "path": "/search.json"}],
               "status": "active", "price_multiplier": 1.35})
    };
    let catalog = json!({"credit_unit_micros": 1000, "portal_url": "https://my.octopuson.com", "connectors": [
        connector("serper", "https://google.serper.dev"),
        connector("serpapi", "https://serpapi.com"),
        connector("dataforseo", "https://api.dataforseo.com"),
    ]});
    workspace.cloud.tokens.save("oct_test").unwrap();
    workspace.set_cloud_token(Some("oct_test".to_owned()));
    workspace.refresh_catalog(&FakeTransport::new().reply(200, catalog.to_string()));
}

/// A scan that only asks the paid APIs, one page per query, with nothing to verify.
fn config(api: ApiConfig) -> ScanConfig {
    ScanConfig {
        domain: "acme.com".to_owned(),
        engines: Vec::new(),
        pages_per_query: 1,
        api,
        ..ScanConfig::default()
    }
}

fn scan(workspace: &FinderWorkspace, config: ScanConfig, transport: &FakeTransport) -> ScanMeta {
    workspace
        .run_scan_with(config, &ScanControl::new(), transport, |_| {})
        .unwrap()
}

/// Results outside the scanned site, so nothing is verified over the network.
fn serper_page() -> String {
    json!({"organic": [{"title": "Elsewhere", "link": "https://other.example/a", "position": 1}], "credits": 1})
        .to_string()
}

fn serpapi_page() -> String {
    json!({"organic_results": [{"title": "Elsewhere", "link": "https://other.example/b"}]})
        .to_string()
}

fn header<'a>(request: &'a HttpRequest, name: &str) -> Option<&'a str> {
    request
        .headers
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case(name))
        .map(|(_, value)| value.as_str())
}

/// Everything a request carries, to look for a key in it.
fn sent_text(request: &HttpRequest) -> String {
    let mut text = request.url.clone();
    for (name, value) in &request.headers {
        let _ = write!(text, "\n{name}: {value}");
    }
    if let Some(body) = &request.body {
        text.push('\n');
        text.push_str(&String::from_utf8_lossy(body));
    }
    text
}

#[test]
fn sources_without_a_key_run_on_credits_and_the_charges_are_recorded() {
    let dir = tempfile::tempdir().unwrap();
    let workspace = workspace(dir.path());

    // Signed out: nothing to pay with, and the scan asks no API.
    let sources = workspace.credit_sources(&ApiConfig::default()).unwrap();
    let serper = sources.iter().find(|source| source.id == "serper").unwrap();
    assert_eq!(serper.via, None);
    assert!(serper.credit.is_none());
    let idle = FakeTransport::new();
    let meta = scan(&workspace, config(ApiConfig::default()), &idle);
    assert_eq!(idle.requests().len(), 0);
    assert_eq!(meta.credits.requests, 0);

    sign_in(&workspace);
    let sources = workspace.credit_sources(&ApiConfig::default()).unwrap();
    for id in ["serper", "serpapi"] {
        let source = sources.iter().find(|source| source.id == id).unwrap();
        assert_eq!(source.via, Some(Via::Credit), "{id}");
        assert!((source.credit.as_ref().unwrap().price_multiplier - 1.35).abs() < f64::EPSILON);
    }
    for id in ["google-cse", "yandex-xml"] {
        let source = sources.iter().find(|source| source.id == id).unwrap();
        assert!(!source.creditable && source.credit.is_none() && source.via.is_none());
    }

    let gateway = FakeTransport::new()
        .reply_with_headers(
            200,
            vec![("x-octopus-charge-usd", "0.001350")],
            serper_page(),
        )
        .reply_with_headers(
            200,
            vec![("x-octopus-charge-usd", "0.020250")],
            serpapi_page(),
        );
    let meta = scan(&workspace, config(ApiConfig::default()), &gateway);
    let sent = gateway.requests();
    assert_eq!(sent.len(), 2);
    assert_eq!(sent[0].url, "http://127.0.0.1:9/v1/p/serper/search");
    assert_eq!(header(&sent[0], "authorization"), Some("Bearer oct_test"));
    assert_eq!(header(&sent[0], "x-api-key"), None);
    assert!(String::from_utf8_lossy(sent[0].body.as_deref().unwrap()).contains("site:acme.com"));
    assert!(sent[1].url.starts_with(
        "http://127.0.0.1:9/v1/p/serpapi/search.json?engine=google&q=site%3Aacme.com"
    ));
    assert!(sent[1].url.contains(&format!("api_key={PLACEHOLDER_KEY}")));

    assert_eq!(meta.credits.sources, ["serper", "serpapi"]);
    assert_eq!(meta.credits.requests, 2);
    assert_eq!(meta.credits.charged_micros, 21_600);
    assert_eq!(meta.credits.stopped, None);
    // The charges are saved with the scan.
    let saved = workspace.store().list_scans().unwrap();
    let saved = saved.iter().find(|saved| saved.id == meta.id).unwrap();
    assert_eq!(saved.credits, meta.credits);
}

#[test]
fn own_keys_never_reach_the_gateway() {
    let dir = tempfile::tempdir().unwrap();
    let workspace = workspace(dir.path());
    sign_in(&workspace);
    let own = ApiConfig {
        serper_api_key: "own-serper-key".to_owned(),
        serpapi_key: "own-serpapi-key".to_owned(),
        ..ApiConfig::default()
    };

    // Automatic: the own keys win and the requests go straight to the providers.
    let direct = FakeTransport::new()
        .reply(200, serper_page())
        .reply(200, serpapi_page());
    let meta = scan(&workspace, config(own.clone()), &direct);
    let sent = direct.requests();
    assert_eq!(sent[0].url, "https://google.serper.dev/search");
    assert_eq!(header(&sent[0], "x-api-key"), Some("own-serper-key"));
    assert!(sent[1].url.starts_with("https://serpapi.com/search.json?"));
    assert!(sent[1].url.contains("api_key=own-serpapi-key"));
    assert!(
        sent.iter()
            .all(|request| header(request, "authorization").is_none())
    );
    assert_eq!(meta.credits.requests, 0);
    assert_eq!(meta.credits.sources.len(), 0);

    // Credits chosen while an own key is set: the placeholder goes, never the key.
    workspace
        .set_source_mode("serper", SourceMode::Credit)
        .unwrap();
    let sources = workspace.credit_sources(&own).unwrap();
    let serper = sources.iter().find(|source| source.id == "serper").unwrap();
    assert_eq!(
        (serper.mode, serper.via),
        (SourceMode::Credit, Some(Via::Credit))
    );
    let mixed = FakeTransport::new()
        .reply_with_headers(
            200,
            vec![("x-octopus-charge-usd", "0.00135")],
            serper_page(),
        )
        .reply(200, serpapi_page());
    let meta = scan(&workspace, config(own), &mixed);
    let sent = mixed.requests();
    assert_eq!(sent[0].url, "http://127.0.0.1:9/v1/p/serper/search");
    assert!(
        sent.iter()
            .all(|request| !sent_text(request).contains("own-serper-key"))
    );
    assert!(!sent_text(&sent[0]).contains("own-serpapi-key"));
    assert!(sent[1].url.starts_with("https://serpapi.com/search.json?"));
    assert_eq!(meta.credits.sources, ["serper"]);
    assert_eq!(meta.credits.requests, 1);
    assert_eq!(meta.credits.charged_micros, 1350);
}

#[test]
fn modes_are_saved_and_credit_only_sources_wait_for_an_account() {
    let dir = tempfile::tempdir().unwrap();
    let workspace = workspace(dir.path());
    workspace
        .set_source_mode("serpapi", SourceMode::Credit)
        .unwrap();
    workspace
        .set_source_mode("serper", SourceMode::Own)
        .unwrap();
    let saved: serde_json::Value =
        serde_json::from_slice(&std::fs::read(dir.path().join("settings.json")).unwrap()).unwrap();
    assert_eq!(
        saved,
        json!({"credit": {"serpapi": "credit", "serper": "own"}})
    );
    assert!(
        workspace
            .set_source_mode("google-cse", SourceMode::Credit)
            .is_err()
    );

    // Signed out, SerpApi waits for credits even with a key; Serper has no key.
    let api = ApiConfig {
        serpapi_key: "own-serpapi-key".to_owned(),
        ..ApiConfig::default()
    };
    let none = FakeTransport::new();
    let meta = scan(&workspace, config(api), &none);
    assert_eq!(none.requests().len(), 0);
    assert_eq!(meta.credits.requests, 0);

    workspace
        .set_source_mode("serper", SourceMode::Auto)
        .unwrap();
    assert_eq!(
        workspace.settings().unwrap().credit.get("serper").copied(),
        None
    );
}

#[test]
fn a_source_the_gateway_refuses_is_not_asked_again() {
    let dir = tempfile::tempdir().unwrap();
    let workspace = workspace(dir.path());
    sign_in(&workspace);
    workspace
        .set_source_mode("serpapi", SourceMode::Own)
        .unwrap();
    let refused = FakeTransport::new().reply(
        402,
        json!({"error": "insufficient_credits", "message": "Not enough OctopusOn SEO credits."})
            .to_string(),
    );
    let mut two_queries = config(ApiConfig::default());
    two_queries.seed_queries = vec!["blog".to_owned()];
    let meta = scan(&workspace, two_queries, &refused);
    assert_eq!(refused.requests().len(), 1);
    assert_eq!(meta.credits.requests, 1);
    assert_eq!(meta.credits.charged_micros, 0);
    assert!(
        meta.credits
            .stopped
            .as_deref()
            .unwrap()
            .contains("Not enough OctopusOn SEO credits.")
    );
}

#[test]
fn signs_in_reads_the_account_and_forgets_a_revoked_token() {
    let dir = tempfile::tempdir().unwrap();
    let workspace = workspace(dir.path());
    let offline = FakeTransport::new();
    let status = workspace.cloud_status_with(&offline).unwrap();
    assert!(!status.signed_in);
    assert_eq!(offline.requests().len(), 0);

    let sign_in = SignIn {
        device_code: "device".to_owned(),
        user_code: "ABCD-EFGH".to_owned(),
        verification_uri: "https://my.octopuson.com/cihaz".to_owned(),
        verification_uri_complete: "https://my.octopuson.com/cihaz?code=ABCD-EFGH".to_owned(),
        expires_in: 600,
        interval: 1,
    };
    let gateway = FakeTransport::new()
        .reply(400, json!({"error": "authorization_pending"}).to_string())
        .reply(200, json!({"access_token": "oct_new"}).to_string())
        .reply(
            200,
            json!({"credit_unit_micros": 1000, "portal_url": "https://my.octopuson.com", "connectors": []})
                .to_string(),
        );
    assert!(
        workspace
            .finish_sign_in_with(&gateway, &sign_in, &|| false, &|_| {})
            .unwrap()
    );
    assert_eq!(
        TokenStore::in_dir(&dir.path().join("account"), GATEWAY)
            .load()
            .unwrap()
            .as_deref(),
        Some("oct_new")
    );
    let stopped = workspace
        .finish_sign_in_with(&FakeTransport::new(), &sign_in, &|| true, &|_| {})
        .unwrap();
    assert!(!stopped);

    let account = json!({"email": "ajans@example.com", "name": null, "balance_micros": 5_000_000,
        "held_micros": 0, "available_micros": 5_000_000, "credit_unit_micros": 1000,
        "low_balance": false, "portal_url": "https://my.octopuson.com"});
    let status = workspace
        .cloud_status_with(&FakeTransport::new().reply(200, account.to_string()))
        .unwrap();
    assert!(status.signed_in);
    assert!((status.account.unwrap().credits() - 5000.0).abs() < 1e-9);

    let revoked = workspace
        .cloud_status_with(&FakeTransport::new().reply(401, "{}"))
        .unwrap();
    assert!(!revoked.signed_in);
    assert!(workspace.cloud_token().is_none());
}

#[test]
fn connects_with_one_click_through_the_account_site() {
    use std::io::{Read, Write as _};
    use std::net::TcpStream;

    let dir = tempfile::tempdir().unwrap();
    let workspace = workspace(dir.path());
    let catalog = json!({"credit_unit_micros": 1000, "portal_url": "http://localhost:3030/", "connectors": [
        {"id": "serper", "name": "Serper", "upstream": "https://google.serper.dev",
         "routes": [{"method": "POST", "path": "/search"}], "status": "active", "price_multiplier": 1.35},
    ]});
    let gateway = FakeTransport::new()
        .reply(200, catalog.to_string())
        .reply(
            200,
            json!({"access_token": "oct_one_click", "token_type": "bearer"}).to_string(),
        )
        .reply(200, catalog.to_string());
    let opened = std::sync::Mutex::new(String::new());
    // The "browser": the account site sends a signed-in person straight back with a code.
    let browser = |address: &str| {
        address.clone_into(&mut opened.lock().unwrap());
        let page = url::Url::parse(address).unwrap();
        let pair = |name: &str| {
            page.query_pairs()
                .find(|(key, _)| key == name)
                .map(|(_, value)| value.into_owned())
                .unwrap()
        };
        let port = url::Url::parse(&pair("redirect_uri"))
            .unwrap()
            .port()
            .unwrap();
        let state = pair("state");
        std::thread::spawn(move || {
            let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
            write!(
                stream,
                "GET /?code=the-code&state={state} HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n"
            )
            .unwrap();
            let mut answer = String::new();
            let _ = stream.read_to_string(&mut answer);
        });
        true
    };
    assert!(
        workspace
            .connect_with(&gateway, &browser, &|| false)
            .unwrap()
    );

    // The account site named by the gateway was opened for this program.
    let opened = url::Url::parse(&opened.lock().unwrap()).unwrap();
    assert_eq!(
        opened.origin().ascii_serialization(),
        "http://localhost:3030"
    );
    assert_eq!(opened.path(), "/baglan");
    assert!(
        opened
            .query_pairs()
            .any(|(key, value)| key == "client" && value.starts_with("index-404-finder/"))
    );
    let sent = gateway.requests();
    assert_eq!(sent[1].url, "http://127.0.0.1:9/v1/connect/token");
    assert_eq!(
        TokenStore::in_dir(&dir.path().join("account"), GATEWAY)
            .load()
            .unwrap()
            .as_deref(),
        Some("oct_one_click")
    );
    // The catalog came with the connection, so Serper is on offer at once.
    let sources = workspace.credit_sources(&ApiConfig::default()).unwrap();
    let serper = sources.iter().find(|source| source.id == "serper").unwrap();
    assert_eq!(serper.via, Some(Via::Credit));

    // Stopped while the browser is open: not connected, nothing saved.
    let other = tempfile::tempdir().unwrap();
    let stopped = self::workspace(other.path());
    let unused = FakeTransport::new().reply(500, "{}");
    assert!(!stopped.connect_with(&unused, &|_| true, &|| true).unwrap());
    assert!(stopped.cloud_token().is_none());
}
