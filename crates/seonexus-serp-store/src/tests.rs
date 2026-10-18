use seonexus_serp::provider::{Attempt, AttemptOutcome, RawPage, SerpFetch};
use seonexus_serp::{
    Engine, Feature, FeatureItem, FeatureKind, Location, Organic, SerpQuery, SerpSnapshot, Sitelink,
};

use super::*;
use crate::DomainFact;

fn snapshot(keyword: &str, fetched_at: &str) -> SerpSnapshot {
    let mut query = SerpQuery::new(Engine::Google, keyword).with_depth(20);
    query.location = Some(Location {
        name: Some("Istanbul".to_owned()),
        ..Location::default()
    });
    SerpSnapshot {
        query,
        provider: "serper".to_owned(),
        provider_rev: Some("serper@v1".to_owned()),
        parser_rev: None,
        fetched_at: fetched_at.to_owned(),
        total_results: Some(1_230_000),
        organic: vec![
            Organic {
                position: 1,
                absolute_position: Some(2),
                page: 1,
                url: "https://www.ornek.com.tr/".to_owned(),
                host: "www.ornek.com.tr".to_owned(),
                domain: "ornek.com.tr".to_owned(),
                title: "Örnek".to_owned(),
                snippet: Some("Açıklama".to_owned()),
                displayed_url: None,
                date: None,
                site_only: false,
                sitelinks: vec![Sitelink {
                    title: "İletişim".to_owned(),
                    url: "https://www.ornek.com.tr/iletisim".to_owned(),
                }],
            },
            Organic {
                position: 2,
                absolute_position: None,
                page: 1,
                url: "https://b.example/".to_owned(),
                host: "b.example".to_owned(),
                domain: "b.example".to_owned(),
                title: "B".to_owned(),
                snippet: None,
                displayed_url: Some("b.example".to_owned()),
                date: Some("3 gün önce".to_owned()),
                site_only: true,
                sitelinks: Vec::new(),
            },
        ],
        features: vec![Feature {
            kind: FeatureKind::PeopleAlsoAsk,
            absolute_position: Some(3),
            page: 1,
            items: vec![FeatureItem {
                text: Some("Ne?".to_owned()),
                url: None,
                domain: None,
            }],
        }],
        warnings: vec!["page 2: timeout".to_owned()],
    }
}

fn fetch(keyword: &str, fetched_at: &str) -> SerpFetch {
    SerpFetch {
        snapshot: snapshot(keyword, fetched_at),
        pages: vec![RawPage {
            page: 1,
            url: "https://google.serper.dev/search".to_owned(),
            status: 200,
            content_type: Some("application/json".to_owned()),
            body: br#"{"organic":[]}"#.repeat(50),
        }],
        requests: 2,
        reported_cost_usd: Some(0.002),
        estimated_cost_usd: 0.002,
    }
}

fn attempts() -> Vec<Attempt> {
    vec![
        Attempt {
            provider: "dataforseo".to_owned(),
            outcome: AttemptOutcome::Failed("quota".to_owned()),
            message: Some("empty".to_owned()),
        },
        Attempt {
            provider: "serper".to_owned(),
            outcome: AttemptOutcome::Success,
            message: None,
        },
    ]
}

#[test]
fn stores_and_reads_back_snapshots_with_raw_pages() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = SerpStore::open(dir.path()).unwrap();
    assert!(store.is_current().unwrap());
    let run = store
        .begin_run(RunKind::Search, None, 1, 0.002, "2026-10-18T10:00:00Z")
        .unwrap();
    let original = fetch("İstanbul Otelleri", "2026-10-18T10:00:01Z");
    let id = store
        .save_fetch(Some(run), None, &original, &attempts())
        .unwrap();
    store
        .finish_run(run, RunStatus::Done, "2026-10-18T10:00:02Z", None)
        .unwrap();

    assert_eq!(store.snapshot(id).unwrap(), original.snapshot);
    let raw = store.raw_pages(id).unwrap();
    assert_eq!(raw, original.pages);
    assert!(dir.path().join("raw/2026-10").is_dir());

    let listed = store
        .snapshots(&SnapshotFilter {
            keyword: Some("istanbul otelleri".to_owned()),
            ..SnapshotFilter::default()
        })
        .unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].organic_count, 2);
    assert_eq!(listed[0].run_id, Some(run));
    let other = store
        .snapshots(&SnapshotFilter {
            engine: Some(Engine::Yandex),
            ..SnapshotFilter::default()
        })
        .unwrap();
    assert_eq!(other.len(), 0);
}

#[test]
fn sums_usage_per_provider_and_month() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = SerpStore::open(dir.path()).unwrap();
    store
        .save_fetch(None, None, &fetch("a", "2026-10-18T10:00:00Z"), &attempts())
        .unwrap();
    store
        .save_fetch(None, None, &fetch("b", "2026-10-19T10:00:00Z"), &attempts())
        .unwrap();
    store
        .record_attempts(None, &attempts()[..1], "2026-09-30T23:00:00Z")
        .unwrap();
    let usage = store.usage().unwrap();
    let serper = usage
        .iter()
        .find(|row| row.provider == "serper" && row.month == "2026-10")
        .unwrap();
    assert_eq!(
        (serper.requests, serper.successes, serper.failures),
        (4, 2, 0)
    );
    assert!((serper.cost_usd - 0.004).abs() < 1e-9);
    let failed = usage
        .iter()
        .find(|row| row.provider == "dataforseo" && row.month == "2026-10")
        .unwrap();
    assert_eq!((failed.successes, failed.failures), (0, 2));
    assert_eq!(usage[0].month, "2026-10");
    assert!((store.spent("serper", "2026-10").unwrap() - 0.004).abs() < 1e-9);
}

#[test]
fn deletes_snapshots_and_their_files() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = SerpStore::open(dir.path()).unwrap();
    let id = store
        .save_fetch(None, None, &fetch("a", "2026-10-18T10:00:00Z"), &attempts())
        .unwrap();
    store.delete_snapshot(id).unwrap();
    assert!(matches!(
        store.snapshot(id),
        Err(SerpStoreError::NotFound(_))
    ));
    assert!(!dir.path().join(format!("raw/2026-10/{id}-1.zst")).exists());
    assert!(matches!(
        store.delete_snapshot(id),
        Err(SerpStoreError::NotFound(_))
    ));
}

#[test]
fn refuses_a_second_writer() {
    let dir = tempfile::tempdir().unwrap();
    let store = SerpStore::open(dir.path()).unwrap();
    assert!(matches!(
        SerpStore::open(dir.path()),
        Err(SerpStoreError::Busy(_))
    ));
    drop(store);
    SerpStore::open(dir.path()).unwrap();
}

#[test]
fn caches_domain_facts_and_imports_tranco() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = SerpStore::open(dir.path()).unwrap();
    let fact = DomainFact {
        domain: "ornek.com.tr".to_owned(),
        signal: "age".to_owned(),
        value: Some(r#"{"created": "2004-02-12"}"#.to_owned()),
        source: Some("whois".to_owned()),
        fetched_at: "2026-10-18T10:00:00Z".to_owned(),
        expires_at: "2027-04-16T10:00:00Z".to_owned(),
        error: None,
    };
    store.put_domain_fact(&fact).unwrap();
    store.put_domain_fact(&fact).unwrap();
    assert_eq!(
        store.domain_fact("ornek.com.tr", "age").unwrap(),
        Some(fact)
    );
    assert_eq!(store.domain_fact("ornek.com.tr", "pagerank").unwrap(), None);

    let csv = dir.path().join("top-1m.csv");
    std::fs::write(&csv, "1,google.com\n2,Facebook.com\n3,ornek.com.tr\n").unwrap();
    assert_eq!(
        store
            .import_tranco(&csv, Some("ABCD"), "2026-10-18T10:00:00Z")
            .unwrap(),
        3
    );
    assert_eq!(store.tranco_rank("facebook.com").unwrap(), Some(2));
    assert_eq!(store.tranco_rank("missing.example").unwrap(), None);
    assert_eq!(
        store.dataset("tranco").unwrap().unwrap().version.as_deref(),
        Some("ABCD")
    );
    assert_eq!(store.schema_version().unwrap(), 6);
}

#[test]
fn saves_and_rescores_analyses() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = SerpStore::open(dir.path()).unwrap();
    let snapshot = store
        .save_fetch(
            None,
            None,
            &fetch("Türk Kahvesi", "2026-10-18T10:00:00Z"),
            &attempts(),
        )
        .unwrap();
    let page = crate::PageScoreRow {
        position: 1,
        url: "https://a.example/".to_owned(),
        domain: "a.example".to_owned(),
        content: Some(70.0),
        technical: Some(90.0),
        site: Some(40.0),
        strength: Some(52.0),
        words: 900,
    };
    let mut record = crate::AnalysisRecord {
        id: 0,
        snapshot_id: snapshot,
        keyword: "Türk Kahvesi".to_owned(),
        engine: Engine::Google,
        country: "tr".to_owned(),
        language: "tr".to_owned(),
        device: "desktop".to_owned(),
        created_at: "2026-10-18T10:01:00Z".to_owned(),
        target_domain: None,
        difficulty: 42.0,
        band: "medium".to_owned(),
        intent: "informational".to_owned(),
        opportunity: None,
        volume: Some(1_000),
        models: "{}".to_owned(),
        result: "{}".to_owned(),
        inputs: "{}".to_owned(),
        domains: None,
        crawl_ids: None,
        pages: vec![page.clone(), page],
    };
    let id = store.save_analysis(&record).unwrap();
    let stored = store.analysis(id).unwrap();
    assert_eq!(stored.pages.len(), 1);
    assert_eq!(stored.difficulty, 42.0);
    record.id = id;
    record.difficulty = 47.5;
    store.update_analysis(&record).unwrap();
    assert_eq!(store.analysis(id).unwrap().difficulty, 47.5);
    let listed = store.analyses(Some("türk kahvesi"), 10).unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(store.analyses(Some("çay"), 10).unwrap().len(), 0);
}

#[test]
fn tracks_projects_keywords_and_rankings() {
    use seonexus_serp::{Device, MatchMode, Target};

    let dir = tempfile::tempdir().unwrap();
    let mut store = SerpStore::open(dir.path()).unwrap();
    let project = crate::ProjectRow {
        id: 0,
        name: "Örnek".to_owned(),
        target: Target::parse("ornek.com.tr", MatchMode::Domain).unwrap(),
        schedule: "daily".to_owned(),
        providers: None,
        created_at: "2026-10-18T10:00:00Z".to_owned(),
        last_run_at: None,
    };
    let id = store.create_project(&project).unwrap();
    let tracker = store
        .add_tracker(&crate::TrackerRow {
            id: 0,
            project_id: id,
            engine: Engine::Google,
            country: "tr".to_owned(),
            language: "tr".to_owned(),
            device: Device::Mobile,
            depth: 20,
            location: None,
        })
        .unwrap();
    let keywords = vec![
        "İstanbul Otel".to_owned(),
        "istanbul  otel".to_owned(),
        "kahve".to_owned(),
    ];
    assert_eq!(
        store
            .add_keywords(
                id,
                &keywords,
                &["otel".to_owned()],
                "tr",
                "2026-10-18T10:00:00Z"
            )
            .unwrap(),
        2
    );
    let rows = store.keywords(id).unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].tags, ["otel"]);
    store
        .add_ranking(&crate::RankingRow {
            run_id: 1,
            project_id: id,
            keyword_id: rows[0].id,
            tracker_id: tracker,
            snapshot_id: None,
            checked_at: "2026-10-18T10:05:00Z".to_owned(),
            position: Some(4),
            url: Some("https://ornek.com.tr/otel".to_owned()),
            positions: vec![4, 9],
            error: None,
        })
        .unwrap();
    assert_eq!(store.rankings(id).unwrap()[0].positions, [4, 9]);
    assert_eq!(store.trackers(id).unwrap()[0].device, Device::Mobile);
    store
        .set_competitors(id, &["Rakip.com".to_owned()])
        .unwrap();
    assert_eq!(store.competitors(id).unwrap(), ["rakip.com"]);
    store.mark_project_run(id, "2026-10-18T10:06:00Z").unwrap();
    assert_eq!(
        store.project(id).unwrap().last_run_at.as_deref(),
        Some("2026-10-18T10:06:00Z")
    );
    store
        .put_volume(
            "kahve",
            Engine::Google,
            "tr",
            Some(74_000),
            "dataforseo-ads",
            "2026-10-18T10:00:00Z",
        )
        .unwrap();
    assert_eq!(
        store
            .volume("kahve", Engine::Google, "tr")
            .unwrap()
            .unwrap()
            .0,
        Some(74_000)
    );
    store.delete_project(id).unwrap();
    assert_eq!(store.projects().unwrap().len(), 0);
    assert_eq!(store.rankings(id).unwrap().len(), 0);
}
