use serde_json::json;

use super::*;

const NOW: &str = "2026-10-21T09:00:00Z";

fn spec() -> ProjectSpec {
    ProjectSpec {
        name: String::new(),
        domain: "https://www.Example.com/tr/".to_owned(),
        country: "TR".to_owned(),
        language: "tr".to_owned(),
        competitors: vec![
            "rival.com".to_owned(),
            "https://rival.com/x".to_owned(),
            "example.com".to_owned(),
            " ".to_owned(),
            "other.com.tr".to_owned(),
        ],
        brands: vec![
            "Example".to_owned(),
            "example".to_owned(),
            "Örnek".to_owned(),
        ],
    }
}

#[test]
fn opens_upgrades_and_locks_the_database() {
    let dir = tempfile::tempdir().unwrap();
    let store = PlatformStore::open(dir.path()).unwrap();
    assert_eq!(store.schema_version().unwrap(), schema::latest());
    assert!(matches!(
        PlatformStore::open(dir.path()),
        Err(PlatformStoreError::Busy(_))
    ));
    drop(store);
    let again = PlatformStore::open(dir.path()).unwrap();
    assert_eq!(again.schema_version().unwrap(), schema::latest());
}

#[test]
fn projects_are_cleaned_saved_and_listed() {
    let dir = tempfile::tempdir().unwrap();
    let store = PlatformStore::open(dir.path()).unwrap();
    let created = store.create_project(&spec(), NOW).unwrap();
    assert_eq!(created.name, "example.com");
    assert_eq!(created.domain, "example.com");
    assert_eq!(created.country, "tr");
    assert_eq!(created.competitors, ["rival.com", "other.com.tr"]);
    assert_eq!(created.brands, ["Example", "Örnek"]);
    assert_eq!(created.created_at, NOW);

    let mut renamed = spec();
    renamed.name = "  Örnek Mağaza ".to_owned();
    renamed.competitors.clear();
    let updated = store
        .update_project(created.id, &renamed, "2026-10-22T09:00:00Z")
        .unwrap();
    assert_eq!(updated.name, "Örnek Mağaza");
    assert_eq!(updated.competitors, [] as [String; 0]);
    assert_eq!(updated.created_at, NOW);
    assert_eq!(updated.updated_at, "2026-10-22T09:00:00Z");

    let second = store
        .create_project(
            &ProjectSpec {
                domain: "another.org".to_owned(),
                country: "us".to_owned(),
                language: "en".to_owned(),
                ..ProjectSpec::default()
            },
            NOW,
        )
        .unwrap();
    let names: Vec<String> = store
        .projects()
        .unwrap()
        .into_iter()
        .map(|p| p.name)
        .collect();
    assert_eq!(names, ["another.org", "Örnek Mağaza"]);

    store.delete_project(second.id).unwrap();
    assert!(matches!(
        store.project(second.id),
        Err(PlatformStoreError::NotFound(_))
    ));
    assert!(matches!(
        store.delete_project(second.id),
        Err(PlatformStoreError::NotFound(_))
    ));
}

#[test]
fn unusable_projects_are_refused_by_field() {
    type Change = fn(&mut ProjectSpec);
    let cases: [(&str, Change); 6] = [
        ("domain", |spec| spec.domain = "not a domain".to_owned()),
        ("domain", |spec| spec.domain = "localhost".to_owned()),
        ("country", |spec| spec.country = "TUR".to_owned()),
        ("language", |spec| spec.language = "t".to_owned()),
        ("competitors", |spec| {
            spec.competitors = vec!["bad domain".to_owned()];
        }),
        ("competitors", |spec| {
            spec.competitors = (0..11).map(|n| format!("rival{n}.com")).collect();
        }),
    ];
    for (field, change) in cases {
        let mut spec = spec();
        change(&mut spec);
        match spec.normalized() {
            Err(PlatformStoreError::Invalid(message)) => {
                assert!(message.starts_with(field), "{message}");
            }
            other => panic!("{field}: {other:?}"),
        }
    }
}

#[test]
fn domains_are_reduced_to_the_host() {
    assert_eq!(
        normalize_domain("HTTPS://WWW.Example.COM:443/a?b#c"),
        "example.com"
    );
    assert_eq!(
        normalize_domain("blog.example.com.tr"),
        "blog.example.com.tr"
    );
    assert_eq!(normalize_domain("www.türkçe.com"), "türkçe.com");
    assert_eq!(normalize_domain("   "), "");
}

#[test]
fn cache_keys_ignore_field_order() {
    let one = cache_key("labs/x", &json!({ "a": 1, "b": { "c": 2, "d": [1, 2] } }));
    let two = cache_key("labs/x", &json!({ "b": { "d": [1, 2], "c": 2 }, "a": 1 }));
    assert_eq!(one, two);
    assert_ne!(
        one,
        cache_key("labs/y", &json!({ "a": 1, "b": { "c": 2, "d": [1, 2] } }))
    );
    assert_ne!(
        one,
        cache_key("labs/x", &json!({ "a": 1, "b": { "c": 2, "d": [2, 1] } }))
    );
}

#[test]
fn answers_are_kept_replaced_and_dropped_when_long_stale() {
    let dir = tempfile::tempdir().unwrap();
    let store = PlatformStore::open(dir.path()).unwrap();
    let project = store.create_project(&spec(), NOW).unwrap();
    let key = cache_key("labs/overview", &json!({ "target": "example.com" }));
    assert_eq!(store.cached(&key).unwrap(), None);

    let entry = CacheEntry {
        body: json!([{ "items": [] }]),
        fetched_at: NOW.to_owned(),
        expires_at: 1_000 + CacheClass::Domain.ttl_seconds(),
        cost_usd: 0.0101,
        via: "credit".to_owned(),
    };
    store
        .keep(
            &key,
            "labs/overview",
            CacheClass::Domain,
            Some(project.id),
            &entry,
        )
        .unwrap();
    let found = store.cached(&key).unwrap().unwrap();
    assert_eq!(found, entry);
    assert!(found.is_fresh(1_000));
    assert!(!found.is_fresh(1_000 + CacheClass::Domain.ttl_seconds()));

    let newer = CacheEntry {
        cost_usd: 0.02,
        ..entry.clone()
    };
    store
        .keep(
            &key,
            "labs/overview",
            CacheClass::Domain,
            Some(project.id),
            &newer,
        )
        .unwrap();
    assert!((store.cached(&key).unwrap().unwrap().cost_usd - 0.02).abs() < 1e-12);

    assert_eq!(store.drop_stale(entry.expires_at + 10, 60).unwrap(), 0);
    assert_eq!(store.drop_stale(entry.expires_at + 61, 60).unwrap(), 1);
    assert_eq!(store.cached(&key).unwrap(), None);
}

#[test]
fn deleting_a_project_drops_its_cache() {
    let dir = tempfile::tempdir().unwrap();
    let store = PlatformStore::open(dir.path()).unwrap();
    let project = store.create_project(&spec(), NOW).unwrap();
    let entry = CacheEntry {
        body: json!([]),
        fetched_at: NOW.to_owned(),
        expires_at: i64::MAX,
        cost_usd: 0.0,
        via: "own".to_owned(),
    };
    store
        .keep(
            "k",
            "labs/overview",
            CacheClass::Domain,
            Some(project.id),
            &entry,
        )
        .unwrap();
    store.delete_project(project.id).unwrap();
    assert_eq!(store.cached("k").unwrap(), None);
}

#[test]
fn spending_adds_up_by_month_provider_and_payment() {
    let dir = tempfile::tempdir().unwrap();
    let store = PlatformStore::open(dir.path()).unwrap();
    let record = |at: &str, via: &str, cost: f64| SpendRecord {
        at: at.to_owned(),
        project_id: Some(1),
        module: "overview".to_owned(),
        endpoint: "labs/overview".to_owned(),
        provider: "dataforseo".to_owned(),
        via: via.to_owned(),
        requests: 1,
        cost_usd: cost,
    };
    store
        .record_spend(&record("2026-09-30T23:59:59Z", "own", 0.5))
        .unwrap();
    store
        .record_spend(&record("2026-10-01T00:00:00Z", "credit", 0.25))
        .unwrap();
    store
        .record_spend(&record("2026-10-20T10:00:00Z", "credit", 0.25))
        .unwrap();
    store
        .record_spend(&record("2026-10-20T11:00:00Z", "own", 0.1))
        .unwrap();

    let usage = store.usage().unwrap();
    assert_eq!(usage.len(), 3);
    assert_eq!(usage[0].month, "2026-10");
    assert_eq!(usage[0].via, "credit");
    assert_eq!(usage[0].requests, 2);
    assert!((usage[0].cost_usd - 0.5).abs() < 1e-12);
    assert_eq!(usage[2].month, "2026-09");
    assert!((store.spent_in("dataforseo", "2026-10").unwrap() - 0.6).abs() < 1e-12);
    assert!(store.spent_in("serper", "2026-10").unwrap().abs() < 1e-12);
}

#[test]
fn keyword_lists_keep_keywords_with_their_numbers() {
    let dir = tempfile::tempdir().unwrap();
    let store = PlatformStore::open(dir.path()).unwrap();
    let project = store.create_project(&spec(), NOW).unwrap();
    assert!(matches!(
        store.create_keyword_list(project.id, "  ", NOW),
        Err(PlatformStoreError::Invalid(_))
    ));
    assert!(matches!(
        store.create_keyword_list(999, "x", NOW),
        Err(PlatformStoreError::NotFound(_))
    ));
    let list = store
        .create_keyword_list(project.id, " Ayakkabı ", NOW)
        .unwrap();
    assert_eq!(list.name, "Ayakkabı");
    assert_eq!(list.keywords, 0);

    let count = store
        .add_to_keyword_list(
            list.id,
            &[
                ("spor ayakkabı".to_owned(), json!({ "volume": 2900 })),
                ("bot".to_owned(), json!({ "volume": 14800 })),
                (" ".to_owned(), json!({})),
            ],
            NOW,
        )
        .unwrap();
    assert_eq!(count, 2);
    // Adding again replaces the numbers, not the keyword.
    let count = store
        .add_to_keyword_list(
            list.id,
            &[("bot".to_owned(), json!({ "volume": 15000 }))],
            "2026-10-22T09:00:00Z",
        )
        .unwrap();
    assert_eq!(count, 2);
    let keywords = store.list_keywords(list.id).unwrap();
    let bot = keywords
        .iter()
        .find(|entry| entry.keyword == "bot")
        .unwrap();
    assert_eq!(bot.data["volume"], 15000);
    assert_eq!(bot.added_at, NOW);

    let renamed = store.rename_keyword_list(list.id, "Botlar").unwrap();
    assert_eq!(renamed.name, "Botlar");
    assert_eq!(store.keyword_lists(project.id).unwrap().len(), 1);
    assert_eq!(
        store
            .remove_from_keyword_list(list.id, &["bot".to_owned()])
            .unwrap(),
        1
    );

    // Deleting the project removes its lists.
    store.delete_project(project.id).unwrap();
    assert!(matches!(
        store.keyword_list(list.id),
        Err(PlatformStoreError::NotFound(_))
    ));
}

fn google(country: &str) -> TrackerSpec {
    TrackerSpec {
        engine: "Google".to_owned(),
        country: country.to_owned(),
        language: "tr".to_owned(),
        device: "desktop".to_owned(),
        location: None,
    }
}

fn check(run_id: i64, project_id: i64, keyword_id: i64, tracker_id: i64, at: &str) -> RankCheckRow {
    RankCheckRow {
        run_id,
        project_id,
        keyword_id,
        tracker_id,
        checked_at: at.to_owned(),
        position: Some(3),
        url: Some("https://example.com/a".to_owned()),
        pages: vec![(3, "https://example.com/a".to_owned())],
        features: vec!["ai_overview".to_owned()],
        ai_overview: true,
        ai_cited: false,
        snippet: false,
        results: vec![CheckResult {
            position: 1,
            domain: "rival.com".to_owned(),
            url: "https://rival.com/".to_owned(),
            title: None,
        }],
        error: None,
    }
}

#[test]
fn rank_settings_start_manual_and_keep_the_last_run() {
    let dir = tempfile::tempdir().unwrap();
    let store = PlatformStore::open(dir.path()).unwrap();
    let project = store.create_project(&spec(), NOW).unwrap();
    let defaults = store.rank_settings(project.id).unwrap();
    assert_eq!(defaults.schedule, "manual");
    assert_eq!(defaults.depth, 20);
    assert_eq!(store.rank_plans().unwrap(), [] as [RankPlan; 0]);

    store.mark_rank_run(project.id, NOW).unwrap();
    let daily = RankSettings {
        schedule: "daily".to_owned(),
        depth: 50,
        ai_overview: false,
        last_run_at: None,
    };
    let saved = store.save_rank_settings(project.id, &daily).unwrap();
    assert_eq!(saved.schedule, "daily");
    assert_eq!(saved.last_run_at.as_deref(), Some(NOW));
    let wrong = RankSettings {
        schedule: "hourly".to_owned(),
        ..daily.clone()
    };
    assert!(matches!(
        store.save_rank_settings(project.id, &wrong),
        Err(PlatformStoreError::Invalid(_))
    ));
    let deep = RankSettings {
        depth: 200,
        ..daily
    };
    assert!(store.save_rank_settings(project.id, &deep).is_err());
    let plans = store.rank_plans().unwrap();
    assert_eq!(plans.len(), 1);
    assert_eq!((plans[0].keywords, plans[0].trackers), (0, 0));
}

#[test]
fn trackers_and_keywords_are_cleaned_and_deduplicated() {
    let dir = tempfile::tempdir().unwrap();
    let store = PlatformStore::open(dir.path()).unwrap();
    let project = store.create_project(&spec(), NOW).unwrap();
    let tracker = store
        .add_rank_tracker(project.id, &google("TR"), NOW)
        .unwrap();
    assert_eq!(
        (tracker.engine.as_str(), tracker.country.as_str()),
        ("google", "tr")
    );
    assert!(matches!(
        store.add_rank_tracker(project.id, &google("tr"), NOW),
        Err(PlatformStoreError::Invalid(_))
    ));
    let mut city = google("tr");
    city.location = Some(" Istanbul,Istanbul,Turkey ".to_owned());
    let istanbul = store.add_rank_tracker(project.id, &city, NOW).unwrap();
    assert_eq!(
        istanbul.location.as_deref(),
        Some("Istanbul,Istanbul,Turkey")
    );
    let mut wrong = google("tr");
    wrong.engine = "duckduckgo".to_owned();
    assert!(store.add_rank_tracker(project.id, &wrong, NOW).is_err());

    let added = store
        .track_keywords(
            project.id,
            &[
                ("spor  ayakkabı".to_owned(), None),
                ("bot".to_owned(), Some(14_800)),
                ("spor ayakkabı".to_owned(), Some(2_900)),
                ("  ".to_owned(), None),
            ],
            &["marka".to_owned(), " marka ".to_owned()],
            NOW,
        )
        .unwrap();
    assert_eq!(added, 2);
    // Again: no new keyword, a new tag, a volume it lacked kept as known.
    let added = store
        .track_keywords(
            project.id,
            &[
                ("bot".to_owned(), Some(1)),
                ("spor ayakkabı".to_owned(), None),
            ],
            &["kış".to_owned()],
            NOW,
        )
        .unwrap();
    assert_eq!(added, 0);
    let keywords = store.tracked_keywords(project.id).unwrap();
    assert_eq!(keywords[0].keyword, "spor ayakkabı");
    assert_eq!(keywords[0].volume, Some(2_900));
    assert_eq!(keywords[0].tags, ["marka", "kış"]);
    assert_eq!(keywords[1].volume, Some(14_800));

    store
        .set_keyword_tags(project.id, &[keywords[1].id], &["sezon".to_owned()])
        .unwrap();
    store
        .set_keyword_volumes(project.id, &[("bot".to_owned(), Some(15_000))])
        .unwrap();
    let keywords = store.tracked_keywords(project.id).unwrap();
    assert_eq!(keywords[1].tags, ["sezon"]);
    assert_eq!(keywords[1].volume, Some(15_000));
    assert!(
        store
            .track_keywords(project.id, &[("x".repeat(201), None)], &[], NOW)
            .is_err()
    );
    let plans = store.rank_plans().unwrap();
    assert_eq!(plans, [] as [RankPlan; 0], "no settings saved yet");
    store.mark_rank_run(project.id, NOW).unwrap();
    let plans = store.rank_plans().unwrap();
    assert_eq!((plans[0].keywords, plans[0].trackers), (2, 2));
    assert_eq!(
        store
            .untrack_keywords(project.id, &[keywords[0].id])
            .unwrap(),
        1
    );
}

#[test]
fn runs_and_checks_are_kept_by_tracker_and_go_with_the_project() {
    let dir = tempfile::tempdir().unwrap();
    let store = PlatformStore::open(dir.path()).unwrap();
    let project = store.create_project(&spec(), NOW).unwrap();
    let tracker = store
        .add_rank_tracker(project.id, &google("tr"), NOW)
        .unwrap();
    store
        .track_keywords(
            project.id,
            &[("bot".to_owned(), None), ("çizme".to_owned(), None)],
            &[],
            NOW,
        )
        .unwrap();
    let keywords = store.tracked_keywords(project.id).unwrap();

    let stale = store.begin_rank_run(project.id, true, 2, NOW).unwrap();
    let run = store
        .begin_rank_run(project.id, false, 2, "2026-10-22T09:00:00Z")
        .unwrap();
    store
        .add_rank_check(&check(
            run,
            project.id,
            keywords[0].id,
            tracker.id,
            "2026-10-22T09:00:01Z",
        ))
        .unwrap();
    let mut failed = check(
        run,
        project.id,
        keywords[1].id,
        tracker.id,
        "2026-10-22T09:00:02Z",
    );
    failed.position = None;
    failed.error = Some("quota".to_owned());
    store.add_rank_check(&failed).unwrap();
    store
        .finish_rank_run(
            run,
            &RunEnd {
                status: "partial",
                checked: 1,
                failed: 1,
                cost_usd: 0.004,
                error: None,
            },
            "2026-10-22T09:00:03Z",
        )
        .unwrap();
    let runs = store.rank_runs(project.id, 10).unwrap();
    assert_eq!(runs[0].id, run);
    assert_eq!(runs[0].status, "partial");
    assert!(!runs[0].scheduled);
    assert_eq!(runs[1].id, stale);
    assert_eq!(runs[1].status, "interrupted");

    let checks = store
        .rank_checks(project.id, tracker.id, None, None)
        .unwrap();
    assert_eq!(checks.len(), 2);
    assert_eq!(checks[0].pages, [(3, "https://example.com/a".to_owned())]);
    assert_eq!(checks[0].results[0].domain, "rival.com");
    assert_eq!(checks[1].error.as_deref(), Some("quota"));
    let one = store
        .rank_checks(project.id, tracker.id, Some(keywords[1].id), None)
        .unwrap();
    assert_eq!(one.len(), 1);
    assert_eq!(
        store
            .rank_checks(project.id, tracker.id, None, Some("2026-10-23"))
            .unwrap(),
        [] as [RankCheckRow; 0]
    );

    // A removed tracker takes its checks; a deleted project takes everything.
    store.remove_rank_tracker(project.id, tracker.id).unwrap();
    assert_eq!(
        store
            .rank_checks(project.id, tracker.id, None, None)
            .unwrap(),
        [] as [RankCheckRow; 0]
    );
    store.delete_project(project.id).unwrap();
    assert_eq!(
        store.tracked_keywords(project.id).unwrap(),
        [] as [TrackedKeyword; 0]
    );
    assert_eq!(
        store.rank_runs(project.id, 10).unwrap(),
        [] as [RankRunRow; 0]
    );
}

#[test]
fn audits_keep_settings_runs_and_their_crawls() {
    let dir = tempfile::tempdir().unwrap();
    let store = PlatformStore::open(dir.path()).unwrap();
    let project = store.create_project(&spec(), NOW).unwrap();
    assert_eq!(
        store.audit_settings(project.id).unwrap(),
        AuditSettings::default()
    );
    let weekly = AuditSettings {
        schedule: "weekly".to_owned(),
        max_pages: 2000,
        last_run_at: None,
    };
    store.save_audit_settings(project.id, &weekly).unwrap();
    assert!(
        store
            .save_audit_settings(
                project.id,
                &AuditSettings {
                    schedule: "daily".to_owned(),
                    ..weekly.clone()
                }
            )
            .is_err()
    );

    let stale = store
        .begin_audit(project.id, true, "https://example.com/", NOW)
        .unwrap();
    let audit = store
        .begin_audit(
            project.id,
            false,
            "https://example.com/",
            "2026-10-22T09:00:00Z",
        )
        .unwrap();
    assert_eq!(
        store
            .audit_settings(project.id)
            .unwrap()
            .last_run_at
            .as_deref(),
        Some("2026-10-22T09:00:00Z")
    );
    store
        .set_audit_crawl(audit, "20261022-090000-example-com")
        .unwrap();
    store
        .finish_audit(
            audit,
            &AuditEnd {
                status: "done",
                pages: 120,
                health: Some(87.5),
                geo: Some(61.0),
                issues: 42,
                metered_pages: 120,
                charged_micros: 0,
                error: None,
            },
            "2026-10-22T09:05:00Z",
        )
        .unwrap();
    let audits = store.audits(project.id, 10).unwrap();
    assert_eq!(audits[0].id, audit);
    assert_eq!(audits[0].status, "done");
    assert_eq!(audits[0].health, Some(87.5));
    assert_eq!(audits[1].id, stale);
    assert_eq!(audits[1].status, "interrupted");
    assert_eq!(store.audit_plans().unwrap().len(), 1);
    assert_eq!(
        store.audit_crawls(project.id).unwrap(),
        ["20261022-090000-example-com"]
    );
    assert_eq!(
        store.delete_audit(project.id, audit).unwrap().as_deref(),
        Some("20261022-090000-example-com")
    );
    assert!(matches!(
        store.audit(project.id, audit),
        Err(PlatformStoreError::NotFound(_))
    ));
    store.delete_project(project.id).unwrap();
    assert_eq!(store.audits(project.id, 10).unwrap(), [] as [AuditRow; 0]);
    assert_eq!(store.audit_plans().unwrap(), []);
}

#[test]
#[allow(clippy::too_many_lines)] // settings, questions, a run and its answer
fn ai_visibility_keeps_settings_questions_runs_and_answers() {
    let dir = tempfile::tempdir().unwrap();
    let store = PlatformStore::open(dir.path()).unwrap();
    let project = store.create_project(&spec(), NOW).unwrap();
    let defaults = store.ai_settings(project.id).unwrap();
    assert_eq!(defaults.schedule, "manual");
    assert_eq!(defaults.engines.len(), AI_ENGINES.len());

    let mut settings = AiSettings {
        schedule: "weekly".to_owned(),
        engines: vec![
            "claude".to_owned(),
            "chatgpt".to_owned(),
            "claude".to_owned(),
        ],
        models: [("claude".to_owned(), " claude-sonnet-5-5 ".to_owned())]
            .into_iter()
            .collect(),
        rivals: vec![
            Rival {
                domain: "https://www.Rival.com/".to_owned(),
                names: vec![" Rival ".to_owned(), String::new()],
            },
            Rival {
                domain: "rival.com".to_owned(),
                names: Vec::new(),
            },
        ],
        last_run_at: None,
    };
    let saved = store.save_ai_settings(project.id, &settings).unwrap();
    assert_eq!(saved.engines, ["claude", "chatgpt"]);
    assert_eq!(saved.models["claude"], "claude-sonnet-5-5");
    assert_eq!(
        saved.rivals,
        [Rival {
            domain: "rival.com".to_owned(),
            names: vec!["Rival".to_owned()]
        }]
    );
    settings.engines = vec!["bard".to_owned()];
    assert!(store.save_ai_settings(project.id, &settings).is_err());

    let added = store
        .add_ai_prompts(
            project.id,
            &[
                "En iyi  spor ayakkabı markası hangisi?".to_owned(),
                "En iyi spor ayakkabı markası hangisi?".to_owned(),
                " ".to_owned(),
            ],
            &["marka".to_owned()],
            NOW,
        )
        .unwrap();
    assert_eq!(added, 1);
    assert!(
        store
            .add_ai_prompts(project.id, &["x".repeat(501)], &[], NOW)
            .is_err()
    );
    let prompts = store.ai_prompts(project.id).unwrap();
    assert_eq!(prompts[0].tags, ["marka"]);
    let plans = store.ai_plans().unwrap();
    assert_eq!((plans[0].1.schedule.as_str(), plans[0].2), ("weekly", 1));

    let run = store.begin_ai_run(project.id, false, 2, NOW).unwrap();
    assert_eq!(
        store
            .ai_settings(project.id)
            .unwrap()
            .last_run_at
            .as_deref(),
        Some(NOW)
    );
    let answer = AiAnswerRow {
        run_id: run,
        project_id: project.id,
        prompt_id: prompts[0].id,
        engine: "claude".to_owned(),
        via: "own".to_owned(),
        model: Some("claude-opus-5-5".to_owned()),
        checked_at: NOW.to_owned(),
        text: "ü".repeat(25_000),
        mentioned: true,
        position: Some(2),
        cited: true,
        sentiment: Some(0.5),
        brands: vec![AnswerBrand {
            key: "example.com".to_owned(),
            own: true,
            position: Some(2),
            cited: true,
            sentiment: Some(0.5),
        }],
        citations: vec![AnswerSource {
            url: "https://example.com/a".to_owned(),
            title: None,
            domain: "example.com".to_owned(),
        }],
        error: None,
    };
    store.add_ai_answer(&answer).unwrap();
    store
        .finish_ai_run(
            run,
            &RunEnd {
                status: "done",
                checked: 1,
                failed: 0,
                cost_usd: 0.05,
                error: None,
            },
            NOW,
        )
        .unwrap();
    let answers = store.ai_answers(project.id, None, None, None).unwrap();
    assert_eq!(answers[0].text.chars().count(), 20_000);
    assert_eq!(answers[0].brands, answer.brands);
    assert_eq!(answers[0].citations, answer.citations);
    assert_eq!(
        store
            .ai_answers(project.id, None, Some(prompts[0].id), Some("gemini"))
            .unwrap(),
        [] as [AiAnswerRow; 0]
    );
    assert_eq!(store.ai_runs(project.id, 5).unwrap()[0].status, "done");
    assert_eq!(
        store
            .remove_ai_prompts(project.id, &[prompts[0].id])
            .unwrap(),
        0
    );
    assert_eq!(
        store.ai_answers(project.id, None, None, None).unwrap(),
        [] as [AiAnswerRow; 0]
    );
    store.delete_project(project.id).unwrap();
    assert_eq!(store.ai_runs(project.id, 5).unwrap(), [] as [AiRunRow; 0]);
}

fn search(key: &str, clicks: f64) -> SearchRow {
    SearchRow {
        key: key.to_owned(),
        clicks,
        impressions: clicks * 20.0,
        position: 4.5,
    }
}

#[test]
#[allow(clippy::too_many_lines)] // settings, a sync and its numbers, the next one replacing them
fn search_console_settings_syncs_and_numbers_replace_each_other() {
    let dir = tempfile::tempdir().unwrap();
    let store = PlatformStore::open(dir.path()).unwrap();
    let project = store.create_project(&spec(), NOW).unwrap();
    let defaults = store.gsc_settings(project.id).unwrap();
    assert_eq!((defaults.days, defaults.schedule.as_str()), (28, "weekly"));
    assert_eq!(defaults.property, None);
    assert_eq!(store.gsc_plans().unwrap(), []);

    let wanted = GscSettings {
        property: Some(" sc-domain:example.com ".to_owned()),
        ga4_property: Some("123456".to_owned()),
        ga4_name: Some("Example".to_owned()),
        days: 90,
        schedule: "daily".to_owned(),
        last_sync_at: Some("ignored".to_owned()),
    };
    let saved = store.save_gsc_settings(project.id, &wanted).unwrap();
    assert_eq!(saved.property.as_deref(), Some("sc-domain:example.com"));
    assert_eq!(saved.ga4_property.as_deref(), Some("properties/123456"));
    assert_eq!(saved.last_sync_at, None);
    for bad in [
        GscSettings {
            days: 30,
            ..wanted.clone()
        },
        GscSettings {
            schedule: "hourly".to_owned(),
            ..wanted.clone()
        },
        GscSettings {
            property: Some("example.com".to_owned()),
            ..wanted.clone()
        },
        GscSettings {
            ga4_property: Some("UA-1".to_owned()),
            ..wanted.clone()
        },
    ] {
        assert!(matches!(
            store.save_gsc_settings(project.id, &bad),
            Err(PlatformStoreError::Invalid(_))
        ));
    }
    assert_eq!(store.gsc_plans().unwrap()[0].0, project.id);

    let first = store.begin_gsc_sync(project.id, false, NOW).unwrap();
    let data = GscData {
        search: vec![
            (SearchKind::Date, Period::Days, search("2026-10-02", 3.0)),
            (SearchKind::Date, Period::Days, search("2026-10-01", 5.0)),
            (
                SearchKind::Query,
                Period::Now,
                search("spor ayakkabı", 40.0),
            ),
            (SearchKind::Query, Period::Now, search("sneaker", 60.0)),
            (SearchKind::Query, Period::Before, search("sneaker", 20.0)),
        ],
        query_pages: vec![QueryPage {
            query: "sneaker".to_owned(),
            page: "https://example.com/sneaker".to_owned(),
            clicks: 60.0,
            impressions: 900.0,
            position: 3.1,
        }],
        traffic: vec![(
            TrafficKind::Landing,
            Period::Now,
            TrafficRow {
                key: "/sneaker".to_owned(),
                sessions: 80.0,
                engaged_sessions: 50.0,
                key_events: 4.0,
                revenue: 120.0,
            },
        )],
    };
    assert_eq!(data.rows(), 7);
    store.replace_gsc_data(project.id, &data).unwrap();
    store
        .finish_gsc_sync(
            first,
            &GscSyncEnd {
                status: "done",
                period: Some(("2026-09-21", "2026-10-18")),
                rows: data.rows(),
                currency: Some("TRY"),
                limited: true,
                error: None,
            },
            NOW,
        )
        .unwrap();
    let days = store
        .gsc_rows(project.id, SearchKind::Date, Period::Days, 100)
        .unwrap();
    assert_eq!(days[0].key, "2026-10-01", "days in order");
    let queries = store
        .gsc_rows(project.id, SearchKind::Query, Period::Now, 100)
        .unwrap();
    assert_eq!(queries[0].key, "sneaker", "most clicks first");
    assert_eq!(
        store
            .gsc_rows(project.id, SearchKind::Query, Period::Before, 100)
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        store
            .gsc_query_pages(project.id, Some("sneaker"), 10)
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        store
            .gsc_query_pages(project.id, Some("yok"), 10)
            .unwrap()
            .len(),
        0
    );
    assert_eq!(
        store
            .ga4_rows(project.id, TrafficKind::Landing, Period::Now, 10)
            .unwrap()[0]
            .key_events,
        4.0
    );
    let syncs = store.gsc_syncs(project.id, 5).unwrap();
    assert_eq!(syncs[0].status, "done");
    assert_eq!(syncs[0].period_end.as_deref(), Some("2026-10-18"));
    assert_eq!((syncs[0].rows, syncs[0].limited), (7, true));
    assert_eq!(
        store
            .gsc_settings(project.id)
            .unwrap()
            .last_sync_at
            .as_deref(),
        Some(NOW)
    );

    // The next sync replaces every number; a sync left running is interrupted.
    store
        .begin_gsc_sync(project.id, true, "2026-10-22T09:00:00Z")
        .unwrap();
    store
        .begin_gsc_sync(project.id, true, "2026-10-22T09:05:00Z")
        .unwrap();
    store
        .replace_gsc_data(
            project.id,
            &GscData {
                search: vec![(SearchKind::Query, Period::Now, search("bot", 1.0))],
                ..GscData::default()
            },
        )
        .unwrap();
    assert_eq!(
        store
            .gsc_rows(project.id, SearchKind::Query, Period::Now, 100)
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        store
            .gsc_rows(project.id, SearchKind::Date, Period::Days, 100)
            .unwrap()
            .len(),
        0
    );
    assert_eq!(
        store.gsc_syncs(project.id, 5).unwrap()[1].status,
        "interrupted"
    );

    store.delete_project(project.id).unwrap();
    assert_eq!(
        store.gsc_syncs(project.id, 5).unwrap(),
        [] as [GscSyncRow; 0]
    );
    assert_eq!(
        store
            .gsc_rows(project.id, SearchKind::Query, Period::Now, 100)
            .unwrap()
            .len(),
        0
    );
}

fn report_spec() -> ReportSpec {
    ReportSpec {
        name: "  Aylık   rapor ".to_owned(),
        language: "tr".to_owned(),
        days: 28,
        widgets: vec![
            "summary".to_owned(),
            "rankings".to_owned(),
            "summary".to_owned(),
        ],
        schedule: "monthly".to_owned(),
        recipients: vec![
            " Musteri@Example.com ".to_owned(),
            "musteri@example.com".to_owned(),
            String::new(),
        ],
    }
}

#[test]
#[allow(clippy::too_many_lines)] // branding, a report, its link and deliveries
fn reports_keep_branding_definitions_links_and_deliveries() {
    let dir = tempfile::tempdir().unwrap();
    let store = PlatformStore::open(dir.path()).unwrap();
    let project = store.create_project(&spec(), NOW).unwrap();
    assert_eq!(store.report_branding().unwrap(), Branding::default());
    let branding = store
        .save_report_branding(&Branding {
            company: Some(" Ajans ".to_owned()),
            logo: Some("data:image/png;base64,iVBORw0KGgo=".to_owned()),
            accent: Some("#1E6FD9".to_owned()),
            website: Some("https://ajans.example".to_owned()),
            footer: None,
            hide_octopuson: true,
        })
        .unwrap();
    assert_eq!(branding.company.as_deref(), Some("Ajans"));
    assert_eq!(branding.accent.as_deref(), Some("#1e6fd9"));
    for bad in [
        Branding {
            logo: Some("https://evil.example/logo.png".to_owned()),
            ..Branding::default()
        },
        Branding {
            logo: Some("data:image/png;base64,<script>".to_owned()),
            ..Branding::default()
        },
        Branding {
            accent: Some("red".to_owned()),
            ..Branding::default()
        },
        Branding {
            website: Some("javascript:alert(1)".to_owned()),
            ..Branding::default()
        },
    ] {
        assert!(matches!(
            store.save_report_branding(&bad),
            Err(PlatformStoreError::Invalid(_))
        ));
    }

    let report = store
        .save_report(project.id, None, &report_spec(), NOW)
        .unwrap();
    assert_eq!(report.spec.name, "Aylık rapor");
    assert_eq!(report.spec.widgets, ["summary", "rankings"]);
    assert_eq!(report.spec.recipients, ["musteri@example.com"]);
    for bad in [
        ReportSpec {
            widgets: vec!["weather".to_owned()],
            ..report_spec()
        },
        ReportSpec {
            widgets: Vec::new(),
            ..report_spec()
        },
        ReportSpec {
            days: 30,
            ..report_spec()
        },
        ReportSpec {
            language: "fr".to_owned(),
            ..report_spec()
        },
        ReportSpec {
            recipients: vec!["a@b.com, c@d.com".to_owned()],
            ..report_spec()
        },
        ReportSpec {
            recipients: vec!["x@example.com\r\nBcc: y@example.com".to_owned()],
            ..report_spec()
        },
        ReportSpec {
            recipients: (0..6).map(|n| format!("p{n}@example.com")).collect(),
            ..report_spec()
        },
    ] {
        assert!(matches!(
            store.save_report(project.id, None, &bad, NOW),
            Err(PlatformStoreError::Invalid(_))
        ));
    }
    let changed = store
        .save_report(
            project.id,
            Some(report.id),
            &ReportSpec {
                schedule: "weekly".to_owned(),
                ..report_spec()
            },
            "2026-10-22T09:00:00Z",
        )
        .unwrap();
    assert_eq!(changed.spec.schedule, "weekly");
    assert_eq!(changed.created_at, NOW);
    assert_eq!(store.reports(project.id).unwrap().len(), 1);
    assert_eq!(store.report_plans().unwrap().len(), 1);

    let shared = store
        .set_report_share(project.id, report.id, Some("tok-123"))
        .unwrap();
    assert_eq!(shared.share_token.as_deref(), Some("tok-123"));
    assert_eq!(
        store.open_shared_report("tok-123").unwrap().unwrap().id,
        report.id
    );
    assert_eq!(store.report(project.id, report.id).unwrap().share_views, 1);
    assert_eq!(store.open_shared_report("other").unwrap(), None);
    assert_eq!(store.open_shared_report("").unwrap(), None);
    store.set_report_share(project.id, report.id, None).unwrap();
    assert_eq!(store.open_shared_report("tok-123").unwrap(), None);

    let delivered = store
        .add_report_delivery(&changed, true, None, "2026-10-23T09:00:00Z")
        .unwrap();
    assert_eq!(
        (delivered.status.as_str(), delivered.recipients),
        ("sent", 1)
    );
    store
        .add_report_delivery(
            &changed,
            false,
            Some("SMTP refused"),
            "2026-10-24T09:00:00Z",
        )
        .unwrap();
    let deliveries = store.report_deliveries(report.id, 5).unwrap();
    assert_eq!(deliveries[0].status, "failed");
    assert_eq!(
        store
            .report(project.id, report.id)
            .unwrap()
            .last_sent_at
            .as_deref(),
        Some("2026-10-24T09:00:00Z")
    );
    store.delete_report(project.id, report.id).unwrap();
    assert!(matches!(
        store.report(project.id, report.id),
        Err(PlatformStoreError::NotFound(_))
    ));
    assert_eq!(
        store.report_deliveries(report.id, 5).unwrap(),
        [] as [DeliveryRow; 0]
    );
    store
        .save_report(project.id, None, &report_spec(), NOW)
        .unwrap();
    store.delete_project(project.id).unwrap();
    assert_eq!(store.report_plans().unwrap(), [] as [ReportRow; 0]);
}
