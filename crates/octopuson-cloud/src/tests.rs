use octopuson_core::http::testing::FakeTransport;
use octopuson_core::http::{HttpMethod, HttpRequest, HttpTransport};
use octopuson_secrets::FileBackend;

use crate::{
    Catalog, CloudClient, CloudError, Connector, CreditTransport, DevicePoll, Metered, Route,
    SourceMode, TokenStore, Via, api_base, choose, connector_for, matches_pattern,
    sign_in_with_browser,
};

const BASE: &str = "https://api.octopuson.com";

fn connector(id: &str, upstream: &str, routes: &[(&str, &str)]) -> Connector {
    Connector {
        id: id.to_owned(),
        name: id.to_owned(),
        description: String::new(),
        upstream: upstream.to_owned(),
        routes: routes
            .iter()
            .map(|(method, path)| Route {
                method: (*method).to_owned(),
                path: (*path).to_owned(),
            })
            .collect(),
        status: "active".to_owned(),
        price_multiplier: 1.35,
    }
}

fn header<'a>(request: &'a HttpRequest, name: &str) -> Option<&'a str> {
    request
        .headers
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case(name))
        .map(|(_, value)| value.as_str())
}

#[test]
fn chooses_the_own_key_first_in_auto() {
    assert_eq!(choose(SourceMode::Auto, true, true), Some(Via::Own));
    assert_eq!(choose(SourceMode::Auto, false, true), Some(Via::Credit));
    assert_eq!(choose(SourceMode::Auto, false, false), None);
    assert_eq!(choose(SourceMode::Own, false, true), None);
    assert_eq!(choose(SourceMode::Credit, true, true), Some(Via::Credit));
    assert_eq!(choose(SourceMode::Credit, true, false), None);
    assert_eq!(connector_for("dataforseo-ads"), Some("dataforseo"));
    assert_eq!(connector_for("yandex-cloud"), Some("yandex-search"));
    assert_eq!(connector_for("google-suggest"), None);
}

#[test]
fn accepts_https_gateways_and_local_http_only() {
    assert_eq!(api_base(None).as_deref(), Ok(BASE));
    assert_eq!(api_base(Some(" ")).as_deref(), Ok(BASE));
    assert_eq!(
        api_base(Some("http://localhost:8787/")).as_deref(),
        Ok("http://localhost:8787")
    );
    assert!(api_base(Some("http://api.example.com")).is_err());
    assert!(api_base(Some("not a url")).is_err());
}

#[test]
fn matches_route_patterns_like_the_gateway() {
    assert!(matches_pattern(
        "/v3/serp/google/organic/live/advanced",
        "/v3/serp/*/organic/live/*"
    ));
    assert!(!matches_pattern(
        "/v3/serp/google/organic/live",
        "/v3/serp/*/organic/live/*"
    ));
    assert!(matches_pattern(
        "/v3/a/b/task_get/x/1",
        "/v3/**/task_get/**"
    ));
    assert!(matches_pattern(
        "/v2/wordstat/topRequests",
        "/v2/wordstat/top*"
    ));
    assert!(!matches_pattern(
        "/v2/wordstat/dynamics",
        "/v2/wordstat/top*"
    ));
    assert!(matches_pattern("/search.json", "/search.json"));
    assert!(matches_pattern("/a/x-y-z", "/a/x*z"));
    assert!(!matches_pattern("/a/x-y", "/a/x*z"));
}

#[test]
fn signs_in_with_a_device_code() {
    let transport = FakeTransport::new()
        .reply(
            200,
            r#"{"device_code":"dc","user_code":"ABCD-EFGH","verification_uri":"https://my.octopuson.com/cihaz","verification_uri_complete":"https://my.octopuson.com/cihaz?kod=ABCD-EFGH","expires_in":900,"interval":5}"#,
        )
        .reply(400, r#"{"error":"authorization_pending"}"#)
        .reply(400, r#"{"error":"slow_down"}"#)
        .reply(200, r#"{"access_token":"oct_token","token_type":"bearer"}"#)
        .reply(400, r#"{"error":"access_denied"}"#)
        .reply(400, r#"{"error":"expired_token"}"#);
    let client = CloudClient::new(&transport, BASE, "keyword-miner/0.1.0 macOS");
    let sign_in = client.start_sign_in().unwrap();
    assert_eq!(sign_in.user_code, "ABCD-EFGH");
    assert_eq!(client.poll_sign_in("dc"), Ok(DevicePoll::Pending));
    assert_eq!(client.poll_sign_in("dc"), Ok(DevicePoll::SlowDown));
    assert_eq!(
        client.poll_sign_in("dc"),
        Ok(DevicePoll::Approved("oct_token".to_owned()))
    );
    assert_eq!(client.poll_sign_in("dc"), Err(CloudError::Denied));
    assert_eq!(client.poll_sign_in("dc"), Err(CloudError::Expired));

    let requests = transport.requests();
    assert_eq!(requests[0].url, "https://api.octopuson.com/v1/device/code");
    assert_eq!(
        header(&requests[0], "x-octopus-app"),
        Some("keyword-miner/0.1.0 macOS")
    );
    let body: serde_json::Value =
        serde_json::from_slice(requests[1].body.as_ref().unwrap()).unwrap();
    assert_eq!(body["device_code"], "dc");
}

#[test]
fn reads_the_account_and_the_catalog() {
    let transport = FakeTransport::new()
        .reply(
            200,
            r#"{"email":"a@b.c","name":"A","balance_micros":5000000,"held_micros":0,"available_micros":5000000,"credit_unit_micros":1000,"low_balance":false,"portal_url":"https://my.octopuson.com"}"#,
        )
        .reply(
            200,
            r#"{"credit_unit_micros":1000,"portal_url":"https://my.octopuson.com","connectors":[{"id":"serper","name":"Serper","description":"","upstream":"https://google.serper.dev","routes":[{"method":"POST","path":"/search"}],"status":"active","price_multiplier":1.35,"max_body_kb":16}]}"#,
        )
        .reply(401, r#"{"error":"invalid_token"}"#)
        .reply(204, "");
    let client = CloudClient::new(&transport, BASE, "serp-finder");
    let account = client.account("oct_t").unwrap();
    assert!((account.credits() - 5000.0).abs() < f64::EPSILON);
    let catalog = client.catalog(Some("oct_t")).unwrap();
    assert_eq!(catalog.connector("serper").unwrap().routes.len(), 1);
    assert_eq!(client.account("oct_t"), Err(CloudError::SignedOut));
    assert!(client.sign_out("oct_t").is_ok());
    assert_eq!(
        header(&transport.requests()[0], "authorization"),
        Some("Bearer oct_t")
    );
}

#[test]
fn meters_server_work_and_reads_its_prices() {
    let transport = FakeTransport::new()
        .reply(
            200,
            r#"{"credit_unit_micros":1000,"portal_url":"https://my.octopuson.com","connectors":[],"meters":[{"sku":"crawl.page","micros_per_unit":200,"free_per_month":1000}]}"#,
        )
        .reply(
            200,
            r#"{"sku":"crawl.page","units":100,"free_units":40,"charged_micros":12000,"free_left":0,"available_micros":988000,"duplicate":false}"#,
        )
        .reply(402, r#"{"error":"insufficient_credits","message":"x","available_micros":150}"#)
        .reply(401, r#"{"error":"invalid_token"}"#);
    let client = CloudClient::new(&transport, BASE, "geo-seo-platform-web");
    let catalog = client.catalog(None).unwrap();
    assert_eq!(catalog.meters[0].sku, "crawl.page");
    assert_eq!(catalog.meters[0].free_per_month, 1000);
    let Metered::Charged(receipt) = client
        .meter("oct_t", "crawl.page", 100, "audit-1:0")
        .unwrap()
    else {
        panic!("charged");
    };
    assert_eq!((receipt.free_units, receipt.charged_micros), (40, 12_000));
    assert_eq!(
        client
            .meter("oct_t", "crawl.page", 100, "audit-1:1")
            .unwrap(),
        Metered::NoCredits {
            available_micros: 150
        }
    );
    assert_eq!(
        client.meter("oct_t", "crawl.page", 1, "audit-1:2"),
        Err(CloudError::SignedOut)
    );
    let body: serde_json::Value =
        serde_json::from_slice(transport.requests()[1].body.as_deref().unwrap()).unwrap();
    assert_eq!(
        body,
        serde_json::json!({ "sku": "crawl.page", "units": 100, "idempotency_key": "audit-1:0" })
    );
    assert!(transport.requests()[1].url.ends_with("/v1/meter"));
    // An older gateway without meters still gives a catalog.
    let old: Catalog = serde_json::from_str(
        r#"{"credit_unit_micros":1000,"portal_url":"https://my.octopuson.com","connectors":[]}"#,
    )
    .unwrap();
    assert_eq!(old.meters, [] as [crate::MeterPrice; 0]);
}

#[test]
fn routes_credit_sources_through_the_gateway_and_collects_charges() {
    let inner = FakeTransport::new()
        .reply_with_headers(
            200,
            vec![("X-Octopus-Charge-Usd", "0.002700")],
            r#"{"cost":0.002}"#,
        )
        .reply_with_headers(
            200,
            vec![("x-octopus-charge-usd", "0.001350")],
            r#"{"credits":1}"#,
        )
        .reply(200, "direct");
    let transport = CreditTransport::new(
        inner,
        BASE,
        "oct_t",
        "keyword-miner/0.1.0",
        vec![
            connector(
                "dataforseo",
                "https://api.dataforseo.com",
                &[("POST", "/v3/serp/*/organic/live/*")],
            ),
            connector(
                "yandex-search",
                "https://searchapi.api.cloud.yandex.net",
                &[("POST", "/v2/web/search")],
            ),
            connector(
                "wordstat",
                "https://searchapi.api.cloud.yandex.net",
                &[("POST", "/v2/wordstat/topRequests")],
            ),
        ],
    );
    let request = HttpRequest::post(
        "https://api.dataforseo.com/v3/serp/google/organic/live/advanced",
        b"[{}]".to_vec(),
    )
    .header("authorization", "Basic b2N0b3B1c3NlbzpjcmVkaXQ=")
    .header("content-type", "application/json");
    transport.send(&request).unwrap();
    let wordstat = HttpRequest::post(
        "https://searchapi.api.cloud.yandex.net/v2/wordstat/topRequests?x=1",
        b"{}".to_vec(),
    )
    .header("authorization", "Api-Key octopusseo-credit");
    transport.send(&wordstat).unwrap();
    let direct = HttpRequest::get("https://suggestqueries.google.com/complete/search?q=kahve");
    assert_eq!(transport.send(&direct).unwrap().text(), "direct");

    let sent = transport.inner().requests();
    assert_eq!(
        sent[0].url,
        "https://api.octopuson.com/v1/p/dataforseo/v3/serp/google/organic/live/advanced"
    );
    assert_eq!(header(&sent[0], "authorization"), Some("Bearer oct_t"));
    assert_eq!(header(&sent[0], "content-type"), Some("application/json"));
    assert_eq!(
        header(&sent[0], "x-octopus-app"),
        Some("keyword-miner/0.1.0")
    );
    assert_eq!(sent[0].body.as_deref(), Some(b"[{}]".as_slice()));
    assert_eq!(sent[0].method, HttpMethod::Post);
    assert_eq!(
        sent[1].url,
        "https://api.octopuson.com/v1/p/wordstat/v2/wordstat/topRequests?x=1"
    );
    assert_eq!(
        sent[1]
            .headers
            .iter()
            .filter(|(name, _)| name == "authorization")
            .count(),
        1
    );
    assert_eq!(
        sent[2].url,
        "https://suggestqueries.google.com/complete/search?q=kahve"
    );

    let charged = transport.take_charge_usd().unwrap();
    assert!((charged - 0.00405).abs() < 1e-9);
    assert_eq!(transport.take_charge_usd(), None);
}

#[test]
fn keeps_the_token_per_gateway_in_an_owner_only_file() {
    let dir = std::env::temp_dir().join(format!("octopuson-cloud-{}", std::process::id()));
    let shared = dir.join("octopusSEO");
    let store = TokenStore::in_dir(&shared, BASE);
    assert_eq!(store.load(), Ok(None));
    store.save("oct_t").unwrap();
    assert_eq!(store.load(), Ok(Some("oct_t".to_owned())));
    // Another program reading the same folder sees the token at once.
    assert_eq!(
        TokenStore::in_dir(&shared, BASE).load(),
        Ok(Some("oct_t".to_owned()))
    );
    assert_eq!(
        TokenStore::in_dir(&shared, "http://localhost:8787").load(),
        Ok(None)
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode =
            |path: &std::path::Path| std::fs::metadata(path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(&shared), 0o700);
        assert_eq!(mode(&shared.join("account.json")), 0o600);
    }
    store.clear().unwrap();
    assert_eq!(store.load(), Ok(None));
    let file = TokenStore::new(Box::new(FileBackend::new(dir.join("other.json"))), BASE);
    assert_eq!(file.load(), Ok(None));
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn signs_in_with_one_click_through_the_loopback_address() {
    use std::io::{Read, Write};
    use std::net::TcpStream;

    let gateway = FakeTransport::new().reply(
        200,
        r#"{"access_token":"oct_one_click","token_type":"bearer"}"#,
    );
    let client = CloudClient::new(&gateway, BASE, "keyword-miner/0.1.0 macos");
    let seen = std::sync::Mutex::new(String::new());
    // The "browser": the portal sends a signed-in person straight back with a code.
    let browser = |url: &str| {
        seen.lock().unwrap().push_str(url);
        let page = url::Url::parse(url).unwrap();
        let pair = |name: &str| {
            page.query_pairs()
                .find(|(key, _)| key == name)
                .map(|(_, value)| value.into_owned())
                .unwrap()
        };
        let redirect = url::Url::parse(&pair("redirect_uri")).unwrap();
        let state = pair("state");
        let port = redirect.port().unwrap();
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
    let token = sign_in_with_browser(
        &client,
        "https://my.octopuson.com/",
        "keyword-miner/0.1.0 macos",
        &browser,
        &|| false,
        std::time::Duration::from_secs(10),
    )
    .unwrap();
    assert_eq!(token.as_deref(), Some("oct_one_click"));

    let opened = url::Url::parse(&seen.lock().unwrap()).unwrap();
    assert_eq!(opened.path(), "/baglan");
    let pair = |name: &str| {
        opened
            .query_pairs()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.into_owned())
    };
    assert_eq!(pair("client").as_deref(), Some("keyword-miner/0.1.0 macos"));
    assert_eq!(pair("code_challenge_method").as_deref(), Some("S256"));
    let swap = &gateway.requests()[0];
    assert_eq!(swap.url, "https://api.octopuson.com/v1/connect/token");
    let body: serde_json::Value = serde_json::from_slice(swap.body.as_ref().unwrap()).unwrap();
    assert_eq!(body["code"], "the-code");
    let verifier = body["code_verifier"].as_str().unwrap().to_owned();
    // The verifier is the one behind the challenge the browser carried.
    assert_eq!(
        octopuson_oauth::Pkce::from_verifier(verifier).challenge,
        pair("code_challenge").unwrap()
    );

    // No browser: a clear error instead of waiting.
    let none = sign_in_with_browser(
        &client,
        "https://my.octopuson.com",
        "x",
        &|_| false,
        &|| false,
        std::time::Duration::from_secs(1),
    );
    assert!(matches!(none, Err(CloudError::Invalid(_))));
    // Stopped while waiting.
    let stopped = sign_in_with_browser(
        &client,
        "https://my.octopuson.com",
        "x",
        &|_| true,
        &|| true,
        std::time::Duration::from_secs(5),
    );
    assert_eq!(stopped, Ok(None));
}

#[test]
fn the_account_card_reads_the_shared_connection() {
    let dir = std::env::temp_dir().join(format!("octopuson-cloud-session-{}", std::process::id()));
    let session = crate::AccountSession::with_tokens(
        TokenStore::in_dir(&dir, "http://127.0.0.1:9"),
        "seo-spider/0.1.0",
    );
    let offline = FakeTransport::new();
    assert!(!session.status(&offline).signed_in);
    TokenStore::in_dir(&dir, "http://127.0.0.1:9")
        .save("oct_shared")
        .unwrap();
    let gateway = FakeTransport::new()
        .reply(200, r#"{"email":"a@b.c","name":"A","balance_micros":2000000,"held_micros":0,"available_micros":2000000,"credit_unit_micros":1000,"low_balance":false,"portal_url":"http://localhost:3030"}"#)
        .reply(401, r#"{"error":"invalid_token"}"#);
    let status = session.status(&gateway);
    assert!(status.signed_in);
    assert_eq!(status.portal_url, "http://localhost:3030");
    assert!((status.account.unwrap().credits() - 2000.0).abs() < f64::EPSILON);
    // Revoked on the website: forgotten here too.
    assert!(!session.status(&gateway).signed_in);
    assert_eq!(
        TokenStore::in_dir(&dir, "http://127.0.0.1:9").load(),
        Ok(None)
    );
    let catalog = FakeTransport::new().reply(
        200,
        r#"{"credit_unit_micros":1000,"portal_url":"http://localhost:3030/","connectors":[]}"#,
    );
    assert_eq!(
        session.page(&catalog, "credits"),
        "http://localhost:3030/kredi"
    );
    std::fs::remove_dir_all(&dir).unwrap();
}
