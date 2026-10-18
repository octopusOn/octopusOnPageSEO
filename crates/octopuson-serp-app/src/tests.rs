use octopuson_core::http::testing::FakeTransport;
use octopuson_serp::provider::AttemptOutcome;
use octopuson_serp::spec::ProviderSpec;
use octopuson_serp::{Engine, MatchMode, SerpQuery, Target};
use serde_json::json;

use super::*;
use crate::secrets::{FileBackend, SecretState};
use crate::store::RunStatus;

fn workspace(dir: &Path) -> SerpWorkspace {
    let secrets = Secrets::new(
        Box::new(FileBackend::new(dir.join("secrets.json"))),
        dir.join("secret-bindings.json"),
    )
    .with_environment(|_| None);
    SerpWorkspace::with_secrets(dir.to_owned(), secrets)
}

fn acme(url: &str) -> ProviderSpec {
    serde_json::from_value(json!({
        "id": "acme", "name": "Acme", "engines": ["google"],
        "secrets": [{"name": "token"}],
        "request": {"url": url, "query": {"q": "{keyword}", "key": "{secret:token}"}},
        "response": {"format": "json", "results": {"items": "$.r[*]",
            "fields": {"url": "$.u", "title": "$.t"}}},
        "cost": {"per_request_usd": 0.01}
    }))
    .unwrap()
}

fn page(urls: &[&str]) -> String {
    json!({"r": urls.iter().map(|url| json!({"u": url, "t": url})).collect::<Vec<_>>()}).to_string()
}

fn request(providers: &[&str]) -> SearchRequest {
    SearchRequest {
        query: SerpQuery::new(Engine::Google, "kahve"),
        providers: Some(providers.iter().map(|id| (*id).to_owned()).collect()),
        target: Some(Target::parse("b.example", MatchMode::Domain).unwrap()),
    }
}

#[test]
fn searches_through_a_custom_provider_and_stores_the_snapshot() {
    let dir = tempfile::tempdir().unwrap();
    let workspace = workspace(dir.path());
    workspace
        .save_spec(acme("https://api.acme.example/s"))
        .unwrap();
    workspace.set_secret("acme", "token", "t0ken").unwrap();

    let transport = FakeTransport::new().reply(
        200,
        page(&["https://a.example/", "https://www.b.example/x"]),
    );
    let outcome = workspace
        .search_with(
            &request(&["serper", "nope", "acme"]),
            &transport,
            &|| false,
            &|_| {},
        )
        .unwrap();
    assert_eq!(outcome.snapshot.provider, "acme");
    assert_eq!(outcome.rank.as_ref().map(|rank| rank.position), Some(2));
    let outcomes: Vec<_> = outcome
        .attempts
        .iter()
        .map(|attempt| (attempt.provider.as_str(), attempt.outcome.clone()))
        .collect();
    assert_eq!(
        outcomes,
        [
            ("serper", AttemptOutcome::Skipped),
            ("nope", AttemptOutcome::Skipped),
            ("acme", AttemptOutcome::Success)
        ]
    );
    assert!(transport.requests()[0].url.contains("key=t0ken"));

    let stored = workspace.snapshot(outcome.snapshot_id).unwrap();
    assert_eq!(stored, outcome.snapshot);
    let usage = workspace.usage().unwrap();
    assert!(
        usage
            .iter()
            .any(|row| row.provider == "acme" && (row.cost_usd - 0.01).abs() < 1e-9)
    );
    let raw = workspace
        .store()
        .unwrap()
        .raw_pages(outcome.snapshot_id)
        .unwrap();
    assert!(raw[0].url.contains("key=***"));
}

#[test]
fn skips_providers_over_budget_and_reports_failures() {
    let dir = tempfile::tempdir().unwrap();
    let workspace = workspace(dir.path());
    workspace
        .save_spec(acme("https://api.acme.example/s"))
        .unwrap();
    workspace.set_secret("acme", "token", "t0ken").unwrap();
    let mut settings = workspace.settings().unwrap();
    settings.budgets.insert("acme".to_owned(), 0.005);
    workspace.save_settings(&settings).unwrap();

    let estimates = workspace.estimate(&request(&["acme"])).unwrap();
    assert!(!estimates[0].ready);
    assert!(estimates[0].reason.as_deref().unwrap().contains("budget"));

    match workspace.search_with(
        &request(&["acme"]),
        &FakeTransport::new(),
        &|| false,
        &|_| {},
    ) {
        Err(SerpAppError::Search { attempts, .. }) => {
            assert_eq!(attempts[0].outcome, AttemptOutcome::Skipped);
        }
        other => panic!("expected a search failure, got {other:?}"),
    }

    settings.budgets.clear();
    workspace.save_settings(&settings).unwrap();
    let failing = FakeTransport::new().reply(401, "bad key");
    match workspace.search_with(&request(&["acme"]), &failing, &|| false, &|_| {}) {
        Err(SerpAppError::Search { error, .. }) => assert_eq!(error.code(), "auth"),
        other => panic!("expected an auth failure, got {other:?}"),
    }
    let usage = workspace.usage().unwrap();
    assert!(
        usage
            .iter()
            .any(|row| row.provider == "acme" && row.failures == 1)
    );
}

#[test]
fn moving_a_spec_to_another_host_requires_the_secret_again() {
    let dir = tempfile::tempdir().unwrap();
    let workspace = workspace(dir.path());
    workspace
        .save_spec(acme("https://api.acme.example/s"))
        .unwrap();
    workspace.set_secret("acme", "token", "t0ken").unwrap();
    let state = |workspace: &SerpWorkspace| {
        workspace
            .providers()
            .unwrap()
            .into_iter()
            .find(|provider| provider.id == "acme")
            .unwrap()
            .secrets[0]
            .state
    };
    assert_eq!(state(&workspace), SecretState::Saved);
    workspace
        .save_spec(acme("https://collector.evil.example/s"))
        .unwrap();
    assert_eq!(state(&workspace), SecretState::Rebind);
    let transport = FakeTransport::new();
    assert!(
        workspace
            .search_with(&request(&["acme"]), &transport, &|| false, &|_| {})
            .is_err()
    );
    assert_eq!(transport.requests().len(), 0);
}

#[test]
fn imports_exports_and_protects_builtin_ids() {
    let dir = tempfile::tempdir().unwrap();
    let workspace = workspace(dir.path());
    let exported = {
        workspace
            .save_spec(acme("https://api.acme.example/s"))
            .unwrap();
        workspace.export_specs(&[]).unwrap()
    };
    assert!(!exported.contains("t0ken"));
    workspace.remove_spec("acme").unwrap();
    assert_eq!(workspace.import_specs(&exported).unwrap(), ["acme"]);
    assert_eq!(workspace.custom_specs().unwrap().len(), 1);

    let mut builtin = acme("https://api.acme.example/s");
    builtin.id = "serper".to_owned();
    assert!(workspace.save_spec(builtin).is_err());
    assert!(workspace.import_specs(r#"{"id": "x"}"#).is_err());
    assert!(workspace.set_secret("acme", "nope", "v").is_err());

    let providers = workspace.providers().unwrap();
    for id in ["serper", "serpapi", "dataforseo", "yandex-cloud", "acme"] {
        assert!(
            providers.iter().any(|provider| provider.id == id),
            "{id} is listed"
        );
    }
    assert!(
        providers
            .iter()
            .all(|provider| !provider.ready || ["acme", "browser"].contains(&provider.id.as_str()))
    );
}

mod end_to_end {
    use std::io::{BufRead, BufReader, Write};
    use std::net::TcpListener;
    use std::thread;

    use octopuson_spider_app::CrawlControl;

    use super::*;
    use crate::analysis::{AnalysisEvent, AnalyzeRequest};
    use crate::crawl::PageCrawlOptions;

    const STRONG: &str = "<html><head><title>Türk Kahvesi Nasıl Yapılır? Köpüklü Tarif</title><meta name=description content='Cezvede köpüklü türk kahvesi tarifi, malzemeler ve püf noktaları ile adım adım anlatılıyor.'></head><body><main><h1>Türk Kahvesi Nasıl Yapılır?</h1><p>Türk kahvesi cezvede soğuk su ve ince çekilmiş kahve ile pişirilir. Köpük için ateşi kısık tutun.</p><h2>Malzemeler</h2><ul><li>Cezve</li><li>Kahve</li></ul><h2>Köpük</h2><p>Köpüklü türk kahvesi için kahveyi suyla karıştırın.</p></main></body></html>";
    const MEDIUM: &str = "<html><head><title>Türk kahvesi tarifi</title></head><body><main><h1>Türk kahvesi</h1><p>Cezve ile türk kahvesi pişirmek için kahve ve su gerekir. Köpük önemlidir.</p></main></body></html>";
    const WEAK: &str = "<html><head><title>Kahve makineleri</title></head><body><main><h1>Kahve makineleri</h1><p>Espresso makinesi seçimi.</p></main></body></html>";

    fn serve() -> u16 {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { continue };
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut request = String::new();
                let _ = reader.read_line(&mut request);
                let mut line = String::new();
                while reader.read_line(&mut line).is_ok_and(|read| read > 2) {
                    line.clear();
                }
                let path = request
                    .split_whitespace()
                    .nth(1)
                    .unwrap_or("/")
                    .split('?')
                    .next()
                    .unwrap_or("/")
                    .to_owned();
                let (kind, body) = match path.as_str() {
                    "/serp.json" => (
                        "application/json",
                        json!({"organic": [
                        {"link": format!("http://127.0.0.1:{port}/a"), "title": "A"},
                        {"link": format!("http://127.0.0.1:{port}/b"), "title": "B"},
                        {"link": format!("http://127.0.0.1:{port}/c"), "title": "C"}
                    ], "questions": [{"q": "Türk kahvesi köpüklü nasıl olur?"}]})
                        .to_string(),
                    ),
                    "/a" => ("text/html; charset=utf-8", STRONG.to_owned()),
                    "/b" => ("text/html; charset=utf-8", MEDIUM.to_owned()),
                    "/c" => ("text/html; charset=utf-8", WEAK.to_owned()),
                    _ => ("text/plain", String::new()),
                };
                let status = if body.is_empty() {
                    "404 Not Found"
                } else {
                    "200 OK"
                };
                let _ = write!(
                    stream,
                    "HTTP/1.1 {status}\r\nContent-Type: {kind}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
            }
        });
        port
    }

    #[test]
    fn analyses_a_keyword_from_search_to_scores_and_rescores_it() {
        let port = serve();
        let dir = tempfile::tempdir().unwrap();
        let workspace = workspace(dir.path());
        let spec: ProviderSpec = serde_json::from_value(json!({
            "id": "local", "name": "Local", "engines": ["google"],
            "request": {"url": format!("http://127.0.0.1:{port}/serp.json"), "query": {"q": "{keyword}"}},
            "response": {"format": "json", "results": {"items": "$.organic[*]", "fields": {"url": "$.link", "title": "$.title"}},
                "features": [{"kind": "paa", "items": "$.questions[*]", "fields": {"text": "$.q"}}]}
        }))
        .unwrap();
        workspace.save_spec(spec).unwrap();
        let request = AnalyzeRequest {
            search: SearchRequest {
                query: SerpQuery::new(Engine::Google, "türk kahvesi nasıl yapılır"),
                providers: Some(vec!["local".to_owned()]),
                target: Some(
                    Target::parse(&format!("127.0.0.1:{port}/c"), MatchMode::Prefix).unwrap(),
                ),
            },
            pages: 10,
            crawl: PageCrawlOptions::default(),
            target_url: None,
            volume: false,
            backlinks: false,
        };
        let steps = std::sync::Mutex::new(Vec::new());
        let outcome = workspace
            .analyze(&request, &CrawlControl::new(), &|event| {
                let step = match event {
                    AnalysisEvent::Search { .. } => "search",
                    AnalysisEvent::Crawl { .. } => "crawl",
                    AnalysisEvent::Domains { .. } => "domains",
                    AnalysisEvent::Volume => "volume",
                    AnalysisEvent::Scoring => "scoring",
                };
                steps.lock().unwrap().push(step);
            })
            .unwrap();
        let analysis = &outcome.analysis;
        assert_eq!(analysis.pages.len(), 3);
        let content: Vec<f64> = analysis
            .pages
            .iter()
            .map(|page| page.content.as_ref().unwrap().score)
            .collect();
        assert!(content[0] > content[2], "{content:?}");
        assert!(analysis.difficulty.score > 0.0);
        let target = analysis.target.as_ref().unwrap();
        assert_eq!(target.position, Some(3));
        assert_ne!(target.gaps.len(), 0);
        assert!(steps.lock().unwrap().contains(&"scoring"));

        let listed = workspace.analyses(None, 10).unwrap();
        assert_eq!(listed[0].id, outcome.analysis_id);
        let (stored, snapshot, _) = workspace.analysis(outcome.analysis_id).unwrap();
        assert_eq!(&stored, analysis);
        assert_eq!(snapshot.organic.len(), 3);
        let again = workspace.rescore(outcome.analysis_id).unwrap();
        assert_eq!(again.difficulty.score, analysis.difficulty.score);

        let path = dir.path().join("analysis.json");
        assert_eq!(
            workspace
                .export_analysis(outcome.analysis_id, &path)
                .unwrap(),
            3
        );
        let rows: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(rows[2]["position"], 3);
        assert_eq!(rows[0]["intent"], "informational");
        assert!(rows[0]["content_score"].as_f64().unwrap() > 0.0);
    }
}

mod tracking {
    use std::io::{BufRead, BufReader, Write};
    use std::net::TcpListener;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::thread;

    use super::*;
    use crate::rank::{ProjectSpec, Schedule, TrackerSpec};

    /// A SERP API whose results move the tracked site up one place per check (3 → 2).
    fn serve() -> u16 {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let calls = Arc::new(AtomicUsize::new(0));
        thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { continue };
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut line = String::new();
                let mut request = String::new();
                let _ = reader.read_line(&mut request);
                while reader.read_line(&mut line).is_ok_and(|read| read > 2) {
                    line.clear();
                }
                // Two keywords per check: calls 0–1 are the first check, 2–3 the second.
                let check = calls.fetch_add(1, Ordering::SeqCst) / 2;
                let mut domains = ["rakip.com", "baska.org", "ornek.com.tr"];
                if check > 0 {
                    domains.swap(1, 2);
                }
                let body = json!({"r": domains.iter().map(|domain| json!({"u": format!("https://www.{domain}/x"), "t": domain})).collect::<Vec<_>>()}).to_string();
                let _ = write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
            }
        });
        port
    }

    #[test]
    fn tracks_positions_changes_and_shares() {
        let port = serve();
        let dir = tempfile::tempdir().unwrap();
        let workspace = workspace(dir.path());
        let spec: ProviderSpec = serde_json::from_value(json!({
            "id": "local", "name": "Local", "engines": ["google"],
            "request": {"url": format!("http://127.0.0.1:{port}/serp"), "query": {"q": "{keyword}"}},
            "response": {"format": "json", "results": {"items": "$.r[*]", "fields": {"url": "$.u", "title": "$.t"}}}
        }))
        .unwrap();
        workspace.save_spec(spec).unwrap();
        let project = workspace
            .create_project(&ProjectSpec {
                name: "Örnek".to_owned(),
                target: Target::parse("ornek.com.tr", MatchMode::Domain).unwrap(),
                schedule: Schedule::Daily,
                trackers: vec![TrackerSpec {
                    engine: Engine::Google,
                    country: "tr".to_owned(),
                    language: "tr".to_owned(),
                    device: octopuson_serp::Device::Desktop,
                    depth: 10,
                    location: None,
                }],
                providers: Some(vec!["local".to_owned()]),
            })
            .unwrap();
        assert_eq!(
            workspace
                .add_keywords(
                    project.id,
                    &[
                        "türk kahvesi".to_owned(),
                        "Türk Kahvesi".to_owned(),
                        "kahve".to_owned()
                    ],
                    &[]
                )
                .unwrap(),
            2
        );
        assert_eq!(workspace.due_projects().unwrap(), [project.id]);
        let first = workspace.track(project.id, &|| false, &|_| {}).unwrap();
        assert_eq!((first.checked, first.failed), (2, 0));
        assert_eq!(workspace.due_projects().unwrap().len(), 0);
        let second = workspace.track(project.id, &|| false, &|_| {}).unwrap();
        assert_eq!(second.status, RunStatus::Done);

        let report = workspace.rank_report(project.id).unwrap();
        assert_eq!(report.rows.len(), 2);
        let row = &report.rows[0];
        assert_eq!(row.position, Some(2));
        assert_eq!(row.previous, Some(3));
        assert_eq!(row.change, Some(1));
        assert_eq!(row.best, Some(2));
        assert_eq!(report.visibility.len(), 1);
        assert_eq!(report.visibility[0].top3, 2);
        let own = &report.competitors[0];
        assert!(own.own && own.domain == "ornek.com.tr");
        assert_eq!(own.average_position, Some(2.0));
        let leader = report
            .competitors
            .iter()
            .find(|share| share.domain == "rakip.com")
            .unwrap();
        assert!(leader.visibility > own.visibility);
        workspace.delete_project(project.id).unwrap();
        assert_eq!(workspace.projects().unwrap().len(), 0);
    }

    fn local(port: u16, items: &str) -> ProviderSpec {
        serde_json::from_value(json!({
            "id": "local", "name": "Local", "engines": ["google"],
            "request": {"url": format!("http://127.0.0.1:{port}/serp"), "query": {"q": "{keyword}"}},
            "response": {"format": "json", "results": {"items": items, "fields": {"url": "$.u", "title": "$.t"}}}
        }))
        .unwrap()
    }

    #[test]
    fn reads_stored_pages_again_after_the_mapping_is_fixed() {
        let port = serve();
        let dir = tempfile::tempdir().unwrap();
        let workspace = workspace(dir.path());
        // A mapping that misses the third result, where the tracked site ranks.
        workspace.save_spec(local(port, "$.r[0:2]")).unwrap();
        let project = workspace
            .create_project(&ProjectSpec {
                name: "Örnek".to_owned(),
                target: Target::parse("ornek.com.tr", MatchMode::Domain).unwrap(),
                schedule: Schedule::Manual,
                trackers: vec![TrackerSpec {
                    engine: Engine::Google,
                    country: "tr".to_owned(),
                    language: "tr".to_owned(),
                    device: octopuson_serp::Device::Desktop,
                    depth: 10,
                    location: None,
                }],
                providers: Some(vec!["local".to_owned()]),
            })
            .unwrap();
        workspace
            .add_keywords(
                project.id,
                &["türk kahvesi".to_owned()],
                &["kahve".to_owned()],
            )
            .unwrap();
        workspace.track(project.id, &|| false, &|_| {}).unwrap();
        assert_eq!(
            workspace.rank_report(project.id).unwrap().rows[0].position,
            None
        );

        let snapshot_id = workspace.snapshots(&SnapshotFilter::default()).unwrap()[0].id;
        let pages = workspace.stored_pages(snapshot_id).unwrap();
        assert_eq!(pages.len(), 1);
        assert!(pages[0].body.contains("ornek.com.tr"));
        // Parser profiles read browser pages; API snapshots are read with their spec.
        assert!(workspace.test_parser(snapshot_id, 1, None).is_err());

        workspace.save_spec(local(port, "$.r[*]")).unwrap();
        let fixed = workspace.reparse_snapshot(snapshot_id).unwrap();
        assert_eq!(fixed.organic.len(), 3);
        assert_eq!(workspace.snapshot(snapshot_id).unwrap(), fixed);
        let row = workspace.rank_report(project.id).unwrap().rows[0].clone();
        assert_eq!(row.position, Some(3));
        assert_eq!(row.url.as_deref(), Some("https://www.ornek.com.tr/x"));

        let csv = dir.path().join("ranks.csv");
        assert_eq!(workspace.export_rank_report(project.id, &csv).unwrap(), 1);
        let text = std::fs::read_to_string(&csv).unwrap();
        assert!(text.starts_with("keyword,tags,engine"), "{text}");
        assert!(
            text.contains("türk kahvesi,kahve,google,tr,tr,desktop,3"),
            "{text}"
        );
        let xlsx = dir.path().join("ranks.xlsx");
        assert_eq!(workspace.export_rank_report(project.id, &xlsx).unwrap(), 1);
        assert!(std::fs::metadata(&xlsx).unwrap().len() > 1000);
        assert!(
            workspace
                .export_rank_report(project.id, &dir.path().join("ranks.txt"))
                .is_err()
        );
    }
}
