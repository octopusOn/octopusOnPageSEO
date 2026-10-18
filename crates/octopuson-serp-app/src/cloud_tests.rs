use octopuson_cloud::{CreditTransport, PLACEHOLDER_KEY, SourceMode, Via};
use octopuson_core::http::HttpTransport;
use octopuson_core::http::testing::FakeTransport;
use octopuson_serp::{Engine, SerpQuery};
use serde_json::json;

use crate::SearchRequest;
use crate::secrets::{FileBackend, Secrets};
use crate::{SerpAppError, SerpWorkspace};

fn workspace(dir: &std::path::Path) -> SerpWorkspace {
    let secrets = Secrets::new(
        Box::new(FileBackend::new(dir.join("secrets.json"))),
        dir.join("secret-bindings.json"),
    )
    .with_environment(|_| None);
    SerpWorkspace::with_secrets(dir.to_owned(), secrets)
}

/// Signs in and loads a catalog with Serper and DataForSEO from a fake gateway.
fn sign_in(workspace: &SerpWorkspace) {
    let connector = |id: &str, upstream: &str| {
        json!({"id": id, "name": id, "upstream": upstream, "routes": [{"method": "POST", "path": "/**"}],
               "status": "active", "price_multiplier": 1.35})
    };
    let catalog = json!({"credit_unit_micros": 1000, "portal_url": "https://my.octopuson.com", "connectors": [
        connector("serper", "https://google.serper.dev"),
        connector("dataforseo", "https://api.dataforseo.com"),
    ]});
    workspace.cloud.tokens.save("oct_test").unwrap();
    workspace.set_cloud_token(Some("oct_test".to_owned()));
    workspace.refresh_catalog(&FakeTransport::new().reply(200, catalog.to_string()));
}

fn request() -> SearchRequest {
    SearchRequest {
        query: SerpQuery::new(Engine::Google, "kahve"),
        providers: Some(vec!["serper".to_owned()]),
        target: None,
    }
}

#[test]
fn a_source_without_a_key_runs_on_credits_and_records_the_charge() {
    let dir = tempfile::tempdir().unwrap();
    let workspace = workspace(dir.path());
    match workspace.search_with(&request(), &FakeTransport::new(), &|| false, &|_| {}) {
        Err(SerpAppError::Search { attempts, .. }) => {
            assert!(attempts[0].message.as_deref().unwrap().contains("api_key"));
        }
        other => panic!("expected a skipped provider, got {other:?}"),
    }

    sign_in(&workspace);
    let estimate = workspace.estimate(&request()).unwrap();
    assert!(estimate[0].ready);
    let gateway = FakeTransport::new().reply_with_headers(
        200,
        vec![("x-octopus-charge-usd", "0.001350")],
        json!({"organic": [{"title": "A", "link": "https://a.example/", "position": 1}], "credits": 1}).to_string(),
    );
    let settings = workspace.settings().unwrap();
    let transport = CreditTransport::new(
        gateway,
        workspace.cloud_api(),
        "oct_test",
        super::app_name(),
        workspace.credit_connectors(&settings).unwrap(),
    );
    let outcome = workspace
        .search_with(&request(), &transport, &|| false, &|_| {})
        .unwrap();
    assert!((outcome.cost_usd - 0.00135).abs() < 1e-9);
    let sent = &transport.inner().requests()[0];
    assert_eq!(sent.url, "https://api.octopuson.com/v1/p/serper/search");
    assert!(
        sent.headers
            .iter()
            .all(|(name, value)| name != "x-api-key" && !value.contains(PLACEHOLDER_KEY))
    );
    assert_eq!(transport.take_charge_usd(), None);

    let usage = workspace.store().unwrap().usage().unwrap();
    let serper = usage.iter().find(|row| row.provider == "serper").unwrap();
    assert!((serper.credit_usd - 0.00135).abs() < 1e-9);

    // An own key wins in automatic mode, and credits can be forced.
    workspace
        .set_secret("serper", "api_key", "own-key")
        .unwrap();
    let providers = workspace.providers().unwrap();
    let info = providers.iter().find(|info| info.id == "serper").unwrap();
    assert_eq!(info.via, Some(Via::Own));
    assert!((info.credit.as_ref().unwrap().price_multiplier - 1.35).abs() < f64::EPSILON);
    workspace
        .set_source_mode("serper", SourceMode::Credit)
        .unwrap();
    let settings = workspace.settings().unwrap();
    assert!(
        workspace
            .credit_connectors(&settings)
            .unwrap()
            .iter()
            .any(|connector| connector.id == "serper")
    );
    assert_eq!(
        workspace
            .source_secret(
                &settings,
                "dataforseo",
                "credentials",
                "https://api.dataforseo.com"
            )
            .as_deref(),
        Some(octopuson_cloud::PLACEHOLDER_LOGIN)
    );
    assert!(
        workspace
            .set_source_mode("open-pagerank", SourceMode::Credit)
            .is_err()
    );
}
