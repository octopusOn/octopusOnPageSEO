use std::sync::Mutex;

use octopuson_core::http::testing::FakeTransport;
use octopuson_core::http::{HttpRequest, HttpResponse, HttpTransport, TransportError};
use serde_json::{Value, json};

use super::*;

/// The shape of DataForSEO's documented `domain_rank_overview` answer.
const OVERVIEW: &str = include_str!("../tests/fixtures/domain_rank_overview.json");

fn market() -> Market {
    Market::new("TR", "tr")
}

fn task_error(code: u64, message: &str) -> String {
    json!({
        "status_code": 20_000,
        "tasks": [{ "status_code": code, "status_message": message, "cost": 0, "result": null }]
    })
    .to_string()
}

#[test]
fn credentials_must_be_login_and_password() {
    let transport = FakeTransport::new();
    for invalid in ["", "login", ":secret", "login:"] {
        assert!(matches!(
            Client::new(&transport, invalid),
            Err(DfsError::Config(_))
        ));
    }
    assert!(Client::new(&transport, " me@example.com:secret ").is_ok());
}

#[test]
fn posts_one_task_with_basic_auth_and_reads_the_envelope() {
    let transport = FakeTransport::new().reply(200, OVERVIEW);
    let client = Client::new(&transport, "me@example.com:secret").unwrap();
    let (overview, answer) =
        labs::domain_overview(&client, "https://www.Example.com/blog", &market()).unwrap();

    let request = &transport.requests()[0];
    assert_eq!(
        request.url,
        "https://api.dataforseo.com/v3/dataforseo_labs/google/domain_rank_overview/live"
    );
    let authorization = request
        .headers
        .iter()
        .find(|(name, _)| name == "Authorization")
        .map(|(_, value)| value.clone())
        .unwrap();
    assert_eq!(
        authorization,
        format!("Basic {}", STANDARD.encode("me@example.com:secret"))
    );
    let body: Value = serde_json::from_slice(request.body.as_deref().unwrap()).unwrap();
    assert_eq!(
        body,
        json!([{ "target": "example.com", "location_code": 2792, "language_code": "tr" }])
    );

    assert!((answer.cost_usd - 0.0101).abs() < 1e-9);
    assert_eq!(answer.charged_usd, None);
    assert_eq!(overview.target, "example.com");
    assert_eq!(overview.organic.keywords, 3052);
    assert_eq!(overview.organic.pos_1, 85);
    assert_eq!(
        overview.organic.pos_21_100,
        410 + 300 + 250 + 220 + 200 + 190 + 180 + 170
    );
    assert!((overview.organic.traffic - 4256.6).abs() < 1e-9);
    assert!((overview.organic.traffic_value_usd - 21310.9).abs() < 1e-9);
    assert_eq!(overview.organic.lost, 126);
    assert_eq!(overview.paid.keywords, 4);
    assert_eq!(overview.paid.pos_21_100, 0);
}

#[test]
fn a_domain_without_data_has_zero_rankings() {
    let empty = json!({
        "status_code": 20_000,
        "tasks": [{ "status_code": 20_000, "cost": 0.0101, "result": [{ "items_count": 0, "items": null }] }]
    });
    let transport = FakeTransport::new().reply(200, empty.to_string());
    let client = Client::new(&transport, "a:b").unwrap();
    let (overview, answer) = labs::domain_overview(&client, "new-site.example", &market()).unwrap();
    assert_eq!(answer.items(), [] as [Value; 0]);
    assert_eq!(overview.organic, labs::RankMetrics::default());
}

#[test]
fn task_status_codes_become_errors() {
    let cases = [
        (40_100, "auth"),
        (40_200, "quota"),
        (40_210, "quota"),
        (40_202, "rate"),
        (40_501, "rejected"),
    ];
    for (code, kind) in cases {
        let transport = FakeTransport::new().reply(200, task_error(code, "No."));
        let client = Client::new(&transport, "a:b").unwrap();
        let error = client.post("x/live", &json!({})).unwrap_err();
        let matched = match kind {
            "auth" => matches!(error, DfsError::Auth(_)),
            "quota" => matches!(error, DfsError::Quota(_)),
            "rate" => matches!(error, DfsError::RateLimited { .. }),
            _ => matches!(error, DfsError::Rejected(_)),
        };
        assert!(matched, "{code} gave {error:?}");
    }
}

#[test]
fn gateway_statuses_become_errors() {
    let reply = |status| FakeTransport::new().reply(status, r#"{"error":"insufficient credits"}"#);
    let transport = reply(402);
    let error = Client::new(&transport, "a:b")
        .unwrap()
        .post("x/live", &json!({}))
        .unwrap_err();
    assert_eq!(error, DfsError::Quota("insufficient credits".to_owned()));

    let transport = reply(503);
    let error = Client::new(&transport, "a:b")
        .unwrap()
        .post("x/live", &json!({}))
        .unwrap_err();
    assert!(matches!(error, DfsError::Unavailable(_)));

    let transport = reply(401);
    let error = Client::new(&transport, "a:b")
        .unwrap()
        .post("x/live", &json!({}))
        .unwrap_err();
    assert!(matches!(error, DfsError::Auth(_)));

    let transport =
        FakeTransport::new().reply_with_headers(429, vec![("Retry-After", "7")], "slow down");
    let error = Client::new(&transport, "a:b")
        .unwrap()
        .post("x/live", &json!({}))
        .unwrap_err();
    assert_eq!(
        error,
        DfsError::RateLimited {
            retry_after: Some(7)
        }
    );
}

#[test]
fn errors_never_show_the_credentials() {
    let transport = FakeTransport::new().reply(400, "bad login me@example.com:secret");
    let error = Client::new(&transport, "me@example.com:secret")
        .unwrap()
        .post("x/live", &json!({}))
        .unwrap_err();
    let text = error.to_string();
    assert!(!text.contains("secret"), "{text}");
    assert!(text.contains("***"));
}

/// A transport that reports a credit charge, like the gateway transport does.
struct Charging {
    inner: FakeTransport,
    charge: Mutex<Option<f64>>,
}

impl HttpTransport for Charging {
    fn send(&self, request: &HttpRequest) -> Result<HttpResponse, TransportError> {
        self.inner.send(request)
    }

    fn take_charge_usd(&self) -> Option<f64> {
        self.charge.lock().unwrap().take()
    }
}

#[test]
fn the_credit_charge_is_what_the_user_spent() {
    let transport = Charging {
        inner: FakeTransport::new().reply(200, OVERVIEW),
        charge: Mutex::new(Some(0.0136)),
    };
    let client = Client::new(&transport, "octopusseo:credit").unwrap();
    let (_, answer) = labs::domain_overview(&client, "example.com", &market()).unwrap();
    assert_eq!(answer.charged_usd, Some(0.0136));
    assert!((answer.spent_usd() - 0.0136).abs() < 1e-9);
}

#[test]
fn unknown_countries_and_empty_domains_are_config_errors() {
    assert!(matches!(
        labs::domain_overview_task("example.com", &Market::new("zz", "en")),
        Err(DfsError::Config(_))
    ));
    assert!(matches!(
        labs::domain_overview_task("  https:// ", &market()),
        Err(DfsError::Config(_))
    ));
}

#[test]
fn targets_are_bare_hosts() {
    assert_eq!(target_domain("https://www.Example.COM/a?b"), "example.com");
    assert_eq!(target_domain("shop.example.com.tr"), "shop.example.com.tr");
    assert_eq!(
        target_domain("http://user@www.example.com:8080/"),
        "example.com"
    );
    assert_eq!(target_domain("example.com."), "example.com");
}

#[test]
fn estimates_follow_the_list_prices() {
    assert!(
        (labs::domain_overview_estimate() - (prices::LABS_TASK + prices::LABS_ROW)).abs() < 1e-12
    );
    assert!((prices::backlinks(1_000) - (0.024 + 0.036)).abs() < 1e-9);
}
