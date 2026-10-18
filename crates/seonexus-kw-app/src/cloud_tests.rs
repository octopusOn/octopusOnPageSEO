use seonexus_cloud::{PLACEHOLDER_KEY, PLACEHOLDER_LOGIN, SourceMode, Via};
use seonexus_core::http::testing::FakeTransport;
use seonexus_kw_store::{ProjectSpec, RunStatus};
use seonexus_secrets::{FileBackend, Secrets};
use serde_json::json;

use crate::KeywordWorkspace;
use crate::discover::{DiscoverRequest, DiscoverySource};
use crate::runner::RunControl;

fn workspace() -> (tempfile::TempDir, KeywordWorkspace) {
    let dir = tempfile::tempdir().unwrap();
    let secrets = Secrets::new(
        Box::new(FileBackend::new(dir.path().join("secrets.json"))),
        dir.path().join("secret-bindings.json"),
    )
    .with_environment(|_| None);
    // A closed local port: signing out tries the gateway and must never reach the internet.
    let tokens = seonexus_cloud::TokenStore::in_dir(dir.path(), LOCAL_GATEWAY);
    let workspace =
        KeywordWorkspace::with_secrets(dir.path().to_owned(), secrets).with_tokens(tokens);
    (dir, workspace)
}

const LOCAL_GATEWAY: &str = "http://127.0.0.1:9";

fn catalog() -> String {
    let connector = |id: &str, upstream: &str, routes: serde_json::Value| {
        json!({"id": id, "name": id, "description": "", "upstream": upstream, "routes": routes,
               "status": "active", "price_multiplier": 1.5, "max_body_kb": 64})
    };
    json!({"credit_unit_micros": 1000, "portal_url": "https://my.octopuson.com", "connectors": [
        connector("dataforseo", "https://api.dataforseo.com", json!([{"method": "POST", "path": "/v3/dataforseo_labs/google/*/live"}])),
        connector("serper", "https://google.serper.dev", json!([{"method": "POST", "path": "/search"}])),
        connector("wordstat", "https://searchapi.api.cloud.yandex.net", json!([{"method": "POST", "path": "/v2/wordstat/topRequests"}])),
        connector("yandex-search", "https://searchapi.api.cloud.yandex.net", json!([{"method": "POST", "path": "/v2/web/search"}])),
    ]})
    .to_string()
}

/// Signs the workspace in and loads the catalog from a fake gateway.
fn sign_in(workspace: &KeywordWorkspace) {
    workspace.cloud.tokens.save("oct_test").unwrap();
    workspace.set_cloud_token(Some("oct_test".to_owned()));
    workspace.refresh_catalog(&FakeTransport::new().reply(200, catalog()));
    assert!(workspace.cached_catalog().is_some());
}

#[test]
fn credits_stand_in_for_missing_keys_without_exposing_own_ones() {
    let (_dir, workspace) = workspace();
    let settings = workspace.settings().unwrap();
    assert_eq!(workspace.via(&settings, "serper"), None);
    assert_eq!(workspace.secret("serper", "api_key"), None);

    sign_in(&workspace);
    assert_eq!(workspace.via(&settings, "serper"), Some(Via::Credit));
    assert_eq!(
        workspace.secret("serper", "api_key").as_deref(),
        Some(PLACEHOLDER_KEY)
    );
    assert_eq!(
        workspace.secret("dataforseo", "credentials").as_deref(),
        Some(PLACEHOLDER_LOGIN)
    );
    assert!((workspace.price_factor(&settings, "serper") - 1.5).abs() < f64::EPSILON);
    // Free and Google sources never run on credits.
    assert_ne!(
        workspace.via(&settings, "google-suggest"),
        Some(Via::Credit)
    );
    assert!(workspace.credit_offer("google-suggest").is_none());
    assert!(
        workspace
            .set_source_mode("google-suggest", SourceMode::Credit)
            .is_err()
    );

    workspace
        .set_key("serper", "api_key", "own-serper-key")
        .unwrap();
    let settings = workspace.settings().unwrap();
    assert_eq!(workspace.via(&settings, "serper"), Some(Via::Own));
    assert_eq!(
        workspace.secret("serper", "api_key").as_deref(),
        Some("own-serper-key")
    );
    assert!((workspace.price_factor(&settings, "serper") - 1.0).abs() < f64::EPSILON);

    workspace
        .set_source_mode("serper", SourceMode::Credit)
        .unwrap();
    let settings = workspace.settings().unwrap();
    assert_eq!(workspace.via(&settings, "serper"), Some(Via::Credit));
    assert_eq!(
        workspace.secret("serper", "api_key").as_deref(),
        Some(PLACEHOLDER_KEY)
    );
    let ids: Vec<String> = workspace
        .credit_connectors(&settings)
        .into_iter()
        .map(|connector| connector.id)
        .collect();
    assert!(ids.contains(&"serper".to_owned()));

    workspace
        .set_source_mode("serper", SourceMode::Own)
        .unwrap();
    let settings = workspace.settings().unwrap();
    assert_eq!(workspace.via(&settings, "serper"), Some(Via::Own));
    workspace.remove_key("serper", "api_key").unwrap();
    assert_eq!(
        workspace.via(&workspace.settings().unwrap(), "serper"),
        None
    );

    let providers = workspace.providers().unwrap();
    let wordstat = providers
        .iter()
        .find(|provider| provider.id == "wordstat")
        .unwrap();
    assert!(wordstat.ready);
    assert_eq!(wordstat.via, Some(Via::Credit));
    assert_eq!(wordstat.credit.as_ref().unwrap().connector, "wordstat");

    workspace.sign_out().unwrap();
    assert_eq!(workspace.cloud_token(), None);
    assert_eq!(workspace.secret("serper", "api_key"), None);
}

#[test]
fn runs_on_credits_go_through_the_gateway_and_record_the_charge() {
    let (_dir, workspace) = workspace();
    let id = workspace
        .create_project(&ProjectSpec {
            name: "Kahve".to_owned(),
            country: "tr".to_owned(),
            language: "tr".to_owned(),
            engines: vec!["google".to_owned()],
            site: Some("ornek.com.tr".to_owned()),
            competitors: vec!["rakip.com".to_owned()],
        })
        .unwrap()
        .id;
    sign_in(&workspace);
    let settings = workspace.settings().unwrap();
    let gateway = FakeTransport::new()
        .reply_with_headers(
            200,
            vec![("X-Octopus-Charge-Usd", "0.000240")],
            json!({"results": [{"phrase": "türk kahvesi", "count": "30000"}], "associations": []}).to_string(),
        )
        .reply_with_headers(
            200,
            vec![("X-Octopus-Charge-Usd", "0.018150")],
            json!({"tasks": [{"status_code": 20000, "cost": 0.0121, "result": [{"items": [{
                "keyword_data": {"keyword": "türk kahvesi makinesi", "keyword_info": {"search_volume": 2400}},
                "ranked_serp_element": {"serp_item": {"rank_group": 2, "url": "https://rakip.com/a"}}
            }]}]}]})
            .to_string(),
        );
    let transport = seonexus_cloud::CreditTransport::new(
        gateway,
        workspace.cloud_api(),
        "oct_test",
        super::app_name(),
        workspace.credit_connectors(&settings),
    );
    let request = DiscoverRequest {
        seeds: vec!["türk kahvesi".to_owned()],
        sources: vec![DiscoverySource::Wordstat, DiscoverySource::Competitors],
        ..DiscoverRequest::default()
    };
    let control = RunControl {
        cancelled: &|| false,
        progress: &|_| {},
        sleep: &|_| {},
    };
    let outcome = workspace
        .discover_with(id, &request, &transport, &control)
        .unwrap();
    assert_eq!(outcome.run.status, RunStatus::Done);
    assert!((outcome.run.cost_usd - 0.01839).abs() < 1e-9);

    let sent = transport.inner().requests();
    assert!(
        sent[0]
            .url
            .starts_with("http://127.0.0.1:9/v1/p/wordstat/v2/wordstat/")
    );
    assert!(
        sent[1]
            .url
            .starts_with("http://127.0.0.1:9/v1/p/dataforseo/v3/")
    );
    for request in &sent {
        let authorization = request
            .headers
            .iter()
            .find(|(name, _)| name == "authorization")
            .map(|(_, value)| value.as_str());
        assert_eq!(authorization, Some("Bearer oct_test"));
    }

    let usage = workspace.usage().unwrap();
    let credit: f64 = usage.iter().map(|row| row.credit_usd).sum();
    assert!((credit - 0.01839).abs() < 1e-9);
}

#[test]
fn programs_sharing_the_account_file_see_sign_in_and_sign_out() {
    let (_dir, miner) = workspace();
    let (_other_dir, finder) = workspace();
    let shared = tempfile::tempdir().unwrap();
    let miner = miner.with_tokens(seonexus_cloud::TokenStore::in_dir(
        shared.path(),
        LOCAL_GATEWAY,
    ));
    let finder = finder.with_tokens(seonexus_cloud::TokenStore::in_dir(
        shared.path(),
        LOCAL_GATEWAY,
    ));
    assert_eq!(finder.cloud_token(), None);

    miner.cloud.tokens.save("oct_shared").unwrap();
    miner.set_cloud_token(Some("oct_shared".to_owned()));
    finder.expire_cached_token();
    assert_eq!(finder.cloud_token().as_deref(), Some("oct_shared"));
    finder.refresh_catalog(&FakeTransport::new().reply(200, catalog()));
    assert!(finder.cached_catalog().is_some());

    miner.sign_out().unwrap();
    finder.expire_cached_token();
    assert_eq!(finder.cloud_token(), None);
    // The catalog of the old account is dropped with it.
    assert!(finder.cached_catalog().is_none());
}
