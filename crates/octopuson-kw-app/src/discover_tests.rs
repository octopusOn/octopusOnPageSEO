use std::sync::atomic::{AtomicU32, Ordering};

use octopuson_core::http::testing::FakeTransport;
use octopuson_keywords::Expansion;
use octopuson_kw_store::{KeywordQuery, KeywordSort, ProjectSpec, RunStatus};
use octopuson_secrets::{FileBackend, Secrets};
use serde_json::json;

use super::*;
use crate::enrich::{EnrichRequest, Enrichment};
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

fn project(workspace: &KeywordWorkspace) -> i64 {
    workspace
        .create_project(&ProjectSpec {
            name: "Kahve".to_owned(),
            country: "tr".to_owned(),
            language: "tr".to_owned(),
            engines: vec!["google".to_owned()],
            site: Some("ornek.com.tr".to_owned()),
            competitors: vec!["rakip.com".to_owned()],
        })
        .unwrap()
        .id
}

fn quiet() -> RunControl<'static> {
    RunControl {
        cancelled: &|| false,
        progress: &|_| {},
        sleep: &|_| {},
    }
}

fn labs(items: &serde_json::Value, cost: f64) -> String {
    json!({"tasks": [{"status_code": 20000, "cost": cost, "result": [{"items": items}]}]})
        .to_string()
}

#[test]
fn estimates_requests_cost_and_readiness() {
    let (_dir, workspace) = workspace();
    let id = project(&workspace);
    let request = DiscoverRequest {
        seeds: vec!["kahve".to_owned(), " Kahve ".to_owned(), "çay".to_owned()],
        sources: vec![
            DiscoverySource::Wordstat,
            DiscoverySource::Autocomplete,
            DiscoverySource::Competitors,
        ],
        expansions: vec![Expansion::Questions],
        limit: 1_000,
        ..DiscoverRequest::default()
    };
    let estimate = workspace.estimate_discovery(id, &request).unwrap();
    assert_eq!(estimate.sources.len(), 3);
    assert!(estimate.sources.iter().all(|source| !source.ready));
    assert_eq!(estimate.requests, 0);
    let wordstat = &estimate.sources[0];
    assert_eq!(wordstat.requests, 2);
    assert!(wordstat.reason.as_deref().unwrap().contains("api_key"));

    workspace
        .set_key("dataforseo", "credentials", "me@example.com:pw")
        .unwrap();
    workspace.set_budget("dataforseo", Some(0.05)).unwrap();
    let estimate = workspace.estimate_discovery(id, &request).unwrap();
    let competitors = estimate
        .sources
        .iter()
        .find(|source| source.source == DiscoverySource::Competitors)
        .unwrap();
    assert_eq!(competitors.requests, 1);
    assert!(competitors.reason.as_deref().unwrap().contains("budget"));
    let autocomplete = estimate
        .sources
        .iter()
        .find(|source| source.source == DiscoverySource::Autocomplete)
        .unwrap();
    assert!(autocomplete.requests > 2);
    assert!(autocomplete.ready, "{:?}", autocomplete.reason);
    assert!(matches!(
        workspace.set_key("dataforseo", "credentials", "no-colon"),
        Err(KwAppError::Invalid(_))
    ));
}

/// A Wordstat answer and a competitor's ranking keywords.
fn wordstat_and_competitor_replies() -> FakeTransport {
    FakeTransport::new()
        .reply(
            200,
            json!({"results": [
                {"phrase": "türk kahvesi", "count": "30000"},
                {"phrase": "türk kahvesi fincanı", "count": "4000"},
                {"phrase": "kahve falı", "count": "90000"}
            ], "associations": []})
            .to_string(),
        )
        .reply(
            200,
            labs(
                &json!([{
                    "keyword_data": {
                        "keyword": "türk kahvesi makinesi",
                        "keyword_info": {"search_volume": 2400, "cpc": 0.5},
                        "keyword_properties": {"keyword_difficulty": 31},
                        "search_intent_info": {"main_intent": "commercial"}
                    },
                    "ranked_serp_element": {"serp_item": {"rank_group": 2, "url": "https://rakip.com/a"}}
                }]),
                0.0121,
            ),
        )
}

#[test]
fn discovers_filters_and_records_sources_numbers_and_spending() {
    let (_dir, workspace) = workspace();
    let id = project(&workspace);
    workspace
        .set_key("wordstat", "api_key", "yc-key-1")
        .unwrap();
    workspace
        .set_key("yandex-cloud", "folder_id", "b1g")
        .unwrap();
    workspace
        .set_key("dataforseo", "credentials", "me@example.com:pw")
        .unwrap();
    let transport = wordstat_and_competitor_replies();
    let request = DiscoverRequest {
        seeds: vec!["türk kahvesi".to_owned()],
        sources: vec![DiscoverySource::Wordstat, DiscoverySource::Competitors],
        filter: FilterSpec {
            exclude: vec!["falı".to_owned()],
            ..FilterSpec::default()
        },
        ..DiscoverRequest::default()
    };
    let seen = AtomicU32::new(0);
    let control = RunControl {
        cancelled: &|| false,
        progress: &|progress| {
            seen.store(progress.done, Ordering::SeqCst);
        },
        sleep: &|_| {},
    };
    let outcome = workspace
        .discover_with(id, &request, &transport, &control)
        .unwrap();
    assert_eq!(outcome.run.status, RunStatus::Done);
    assert_eq!(outcome.run.found, 4);
    assert_eq!(outcome.kept, 3);
    assert_eq!(outcome.added.added, 3);
    assert_eq!(seen.load(Ordering::SeqCst), 2);
    assert!((outcome.run.cost_usd - (Wordstat::COST_PER_REQUEST_USD + 0.0121)).abs() < 1e-9);

    let page = workspace
        .keywords(
            id,
            &KeywordQuery {
                sort: KeywordSort::Keyword,
                descending: false,
                ..KeywordQuery::default()
            },
        )
        .unwrap();
    let rows: Vec<_> = page
        .rows
        .iter()
        .map(|row| {
            (
                row.keyword.as_str(),
                row.volume,
                row.sources.join(","),
                row.intent.clone(),
            )
        })
        .collect();
    assert_eq!(
        rows,
        [
            ("türk kahvesi", Some(30_000), "wordstat".to_owned(), None),
            (
                "türk kahvesi fincanı",
                Some(4_000),
                "wordstat".to_owned(),
                None
            ),
            (
                "türk kahvesi makinesi",
                Some(2_400),
                "dataforseo-competitors".to_owned(),
                Some("commercial".to_owned())
            ),
        ]
    );
    let usage = workspace.usage().unwrap();
    assert_eq!(usage.len(), 2);
    let runs = workspace.store().unwrap().runs(Some(id), 1).unwrap();
    assert_eq!(runs[0].status, RunStatus::Done);
}

#[test]
fn stops_a_refused_provider_and_keeps_the_hourly_wordstat_quota() {
    let (_dir, workspace) = workspace();
    let id = project(&workspace);
    workspace
        .set_key("wordstat", "api_key", "yc-key-1")
        .unwrap();
    workspace.set_key("wordstat", "folder_id", "b1g").unwrap();
    let mut settings = workspace.settings().unwrap();
    settings.wordstat_hourly_quota = 1;
    workspace.save_settings(&settings).unwrap();
    let transport = FakeTransport::new().reply(
        200,
        json!({"results": [{"phrase": "kahve", "count": 10}]}).to_string(),
    );
    let request = DiscoverRequest {
        seeds: vec!["kahve".to_owned(), "çay".to_owned()],
        sources: vec![DiscoverySource::Wordstat, DiscoverySource::Autocomplete],
        ..DiscoverRequest::default()
    };
    // The second Wordstat request waits for the quota; the user stops the run meanwhile.
    let waits = AtomicU32::new(0);
    let control = RunControl {
        cancelled: &|| waits.load(Ordering::SeqCst) > 3,
        progress: &|_| {},
        sleep: &|_| {
            waits.fetch_add(1, Ordering::SeqCst);
        },
    };
    let outcome = workspace
        .discover_with(id, &request, &transport, &control)
        .unwrap();
    assert_eq!(outcome.run.status, RunStatus::Cancelled);
    assert_eq!(outcome.added.added, 1);
    assert_eq!(transport.requests().len(), 1);

    // Without DataForSEO credentials autocomplete is reported, not sent.
    let request = DiscoverRequest {
        seeds: vec!["kahve".to_owned()],
        sources: vec![DiscoverySource::Autocomplete],
        ..DiscoverRequest::default()
    };
    let outcome = workspace
        .discover_with(id, &request, &transport, &quiet())
        .unwrap();
    assert_eq!(outcome.run.status, RunStatus::Failed);
    assert_eq!(outcome.run.failures.len(), 1);
    assert_eq!(transport.requests().len(), 1);

    // A refused key stops the provider for the rest of the run.
    workspace
        .set_key("dataforseo", "credentials", "me@example.com:pw")
        .unwrap();
    let transport = FakeTransport::new().reply(401, "nope");
    let request = DiscoverRequest {
        seeds: vec!["kahve".to_owned()],
        sources: vec![DiscoverySource::Autocomplete],
        expansions: vec![Expansion::Commercial],
        ..DiscoverRequest::default()
    };
    let outcome = workspace
        .discover_with(id, &request, &transport, &quiet())
        .unwrap();
    assert_eq!(outcome.run.status, RunStatus::Failed);
    assert_eq!(transport.requests().len(), 1);
}

#[test]
fn enriches_volumes_difficulty_and_intent() {
    let (_dir, workspace) = workspace();
    let id = project(&workspace);
    workspace
        .add_keywords(
            id,
            &["kahve makinesi".to_owned(), "filtre kahve".to_owned()],
            "manual",
            None,
        )
        .unwrap();
    workspace
        .set_key("dataforseo", "credentials", "me@example.com:pw")
        .unwrap();
    let request = EnrichRequest {
        asks: Enrichment::ALL.to_vec(),
        only_missing: true,
        ..EnrichRequest::default()
    };
    let estimate = workspace.estimate_enrichment(id, &request).unwrap();
    assert_eq!(estimate.keywords, [2, 2, 2]);
    assert_eq!(estimate.requests, 3);
    assert_eq!(estimate.reason, None);
    let transport = FakeTransport::new()
        .reply(
            200,
            json!({"tasks": [{"status_code": 20000, "result": [
                {"keyword": "kahve makinesi", "search_volume": 90500, "cpc": 0.8},
                {"keyword": "filtre kahve", "search_volume": 12100}
            ]}]})
            .to_string(),
        )
        .reply(
            200,
            labs(
                &json!([{"keyword": "kahve makinesi", "keyword_difficulty": 55}]),
                0.0,
            ),
        )
        .reply(
            200,
            labs(
                &json!([{"keyword": "filtre kahve", "keyword_intent": {"label": "informational"}}]),
                0.0,
            ),
        );
    let summary = workspace
        .enrich_with(id, &request, &transport, &quiet())
        .unwrap();
    assert_eq!(summary.status, RunStatus::Done);
    let rows = workspace
        .keywords(id, &KeywordQuery::default())
        .unwrap()
        .rows;
    assert_eq!(rows[0].keyword, "kahve makinesi");
    assert_eq!(rows[0].volume, Some(90_500));
    assert_eq!(rows[0].difficulty, Some(55.0));
    assert_eq!(rows[1].intent.as_deref(), Some("informational"));
    // Still missing: one difficulty, and the intent DataForSEO did not return.
    let again = workspace.estimate_enrichment(id, &request).unwrap();
    assert_eq!(again.keywords, [0, 1, 1]);
}
