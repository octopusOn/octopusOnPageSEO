use serde_json::json;

use super::*;

const NOW: &str = "2026-10-21T09:00:00Z";
const LATER: &str = "2026-10-21T10:00:00Z";

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
        owned: Vec::new(),
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
fn queued_checks_wait_with_their_run_and_note_what_the_site_holds() {
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
    let run = store.begin_rank_run(project.id, true, 2, NOW).unwrap();
    let task = |keyword_id: i64, id: &str, next_try_at: i64| RankTaskRow {
        id: 0,
        run_id: run,
        project_id: project.id,
        keyword_id,
        tracker_id: tracker.id,
        engine: "google".to_owned(),
        task_id: id.to_owned(),
        via: "own".to_owned(),
        posted_at: NOW.to_owned(),
        tries: 0,
        next_try_at,
    };
    store
        .queue_rank_tasks(&[
            task(keywords[0].id, "t-1", 100),
            task(keywords[1].id, "t-2", 200),
        ])
        .unwrap();
    store.queue_rank_run(run, 0, 0.0024).unwrap();
    assert_eq!(store.waiting_rank_tasks(project.id).unwrap(), 2);
    assert_eq!(store.next_rank_task_at().unwrap(), Some(100));
    let runs = store.rank_runs(project.id, 1).unwrap();
    assert_eq!((runs[0].status.as_str(), runs[0].waiting), ("running", 2));
    assert!((runs[0].cost_usd - 0.0024).abs() < 1e-12);

    // A run started meanwhile leaves the waiting one running.
    let live = store.begin_rank_run(project.id, false, 2, NOW).unwrap();
    assert_eq!(store.rank_run(run).unwrap().unwrap().status, "running");
    store
        .finish_rank_run(
            live,
            &RunEnd {
                status: "done",
                checked: 2,
                failed: 0,
                cost_usd: 0.0,
                error: None,
            },
            NOW,
        )
        .unwrap();

    let due = store.due_rank_tasks(150, 10).unwrap();
    assert_eq!(due.len(), 1);
    assert_eq!(due[0].task_id, "t-1");
    store.delay_rank_task(due[0].id, 300).unwrap();
    assert_eq!(store.due_rank_tasks(250, 10).unwrap()[0].task_id, "t-2");
    assert_eq!(store.due_rank_tasks(400, 10).unwrap()[1].tries, 1);

    let mut first = check(run, project.id, keywords[0].id, tracker.id, NOW);
    first.owned = vec!["featured_snippet".to_owned(), "local_pack".to_owned()];
    store.add_rank_check(&first).unwrap();
    let mut second = check(run, project.id, keywords[1].id, tracker.id, NOW);
    second.error = Some("not answered".to_owned());
    store.add_rank_check(&second).unwrap();
    for waiting in store.due_rank_tasks(400, 10).unwrap() {
        store.drop_rank_task(waiting.id).unwrap();
    }
    assert_eq!(store.rank_run_tally(run).unwrap(), (1, 1));
    assert_eq!(store.waiting_rank_tasks(project.id).unwrap(), 0);
    let read = store
        .rank_checks(project.id, tracker.id, Some(keywords[0].id), None)
        .unwrap();
    assert_eq!(read[0].features, ["ai_overview"]);
    assert_eq!(read[0].owned, ["featured_snippet", "local_pack"]);

    // Untracked keywords take their waiting checks.
    store
        .queue_rank_tasks(&[task(keywords[1].id, "t-3", 100)])
        .unwrap();
    store
        .untrack_keywords(project.id, &[keywords[1].id])
        .unwrap();
    assert_eq!(store.waiting_rank_tasks(project.id).unwrap(), 0);
}

#[test]
#[allow(clippy::too_many_lines)] // settings, runs, tasks, reviews and ratings in turn
fn local_seo_keeps_its_grid_runs_tasks_reviews_and_ratings() {
    let dir = tempfile::tempdir().unwrap();
    let store = PlatformStore::open(dir.path()).unwrap();
    let project = store.create_project(&spec(), NOW).unwrap();
    assert_eq!(
        store.local_settings(project.id).unwrap(),
        LocalSettings::default()
    );
    let settings = LocalSettings {
        business: Some(json!({ "title": "Örnek Kahve", "lat": 40.99, "lng": 29.03 })),
        keywords: vec![
            " kahve  kadıköy ".to_owned(),
            "Kahve Kadıköy".to_owned(),
            String::new(),
        ],
        grid: 7,
        radius_km: 3.0,
        schedule: "monthly".to_owned(),
        last_run_at: None,
    };
    let saved = store.save_local_settings(project.id, &settings).unwrap();
    assert_eq!(saved.keywords, ["kahve kadıköy"]);
    assert_eq!(saved.business.as_ref().unwrap()["title"], "Örnek Kahve");
    for bad in [
        LocalSettings {
            grid: 4,
            ..settings.clone()
        },
        LocalSettings {
            radius_km: 80.0,
            ..settings.clone()
        },
        LocalSettings {
            schedule: "daily".to_owned(),
            ..settings.clone()
        },
        LocalSettings {
            keywords: (0..21).map(|n| format!("k{n}")).collect(),
            ..settings.clone()
        },
    ] {
        assert!(store.save_local_settings(project.id, &bad).is_err());
    }
    store.mark_local_run(project.id, NOW).unwrap();
    assert_eq!(
        store
            .local_settings(project.id)
            .unwrap()
            .last_run_at
            .as_deref(),
        Some(NOW)
    );
    assert_eq!(store.local_plans().unwrap().len(), 1);

    let run = store
        .begin_local_run(project.id, true, 2, 7, 3.0, (40.99, 29.03), NOW)
        .unwrap();
    let task = |kind: &str, run_id: Option<i64>, id: &str| LocalTaskRow {
        id: 0,
        project_id: project.id,
        kind: kind.to_owned(),
        run_id,
        keyword: Some("kahve".to_owned()),
        row: Some(0),
        col: Some(1),
        lat: Some(41.0),
        lng: Some(29.0),
        task_id: id.to_owned(),
        via: "own".to_owned(),
        posted_at: NOW.to_owned(),
        tries: 0,
        next_try_at: 100,
    };
    store
        .queue_local_tasks(&[task("grid", Some(run), "g-1"), task("reviews", None, "r-1")])
        .unwrap();
    store.queue_local_run(run, 0, 0.0012).unwrap();
    assert_eq!(store.local_runs(project.id, 5).unwrap()[0].waiting, 1);
    assert_eq!(store.waiting_local_tasks(project.id, "reviews").unwrap(), 1);
    assert_eq!(store.next_local_task_at().unwrap(), Some(100));
    // A run started meanwhile leaves the waiting one running.
    store
        .begin_local_run(project.id, false, 1, 7, 3.0, (40.99, 29.03), NOW)
        .unwrap();
    assert_eq!(store.local_run(run).unwrap().unwrap().status, "running");
    let due = store.due_local_tasks(100, 10).unwrap();
    assert_eq!(due.len(), 2);
    store.delay_local_task(due[0].id, 500).unwrap();
    assert_eq!(store.due_local_tasks(200, 10).unwrap().len(), 1);
    store
        .add_local_points(&[LocalPointRow {
            run_id: run,
            project_id: project.id,
            keyword: "kahve".to_owned(),
            row: 0,
            col: 1,
            lat: 41.0,
            lng: 29.0,
            position: Some(3),
            top: json!([{ "position": 1, "title": "Rakip" }]),
            error: None,
            checked_at: NOW.to_owned(),
        }])
        .unwrap();
    for waiting in store.due_local_tasks(1_000, 10).unwrap() {
        store.drop_local_task(waiting.id).unwrap();
    }
    assert_eq!(store.local_run_tally(run).unwrap(), (1, 0));
    let points = store.local_points(run).unwrap();
    assert_eq!((points[0].position, points[0].col), (Some(3), 1));
    assert_eq!(points[0].top[0]["title"], "Rakip");

    let review = |id: &str, answer: Option<&str>| LocalReviewRow {
        review_id: id.to_owned(),
        rating: Some(4),
        text: Some("İyi".to_owned()),
        author: None,
        author_url: None,
        url: None,
        published_at: Some("2026-10-20T10:00:00Z".to_owned()),
        owner_answer: answer.map(str::to_owned),
        answered_at: None,
        photos: 0,
        local_guide: false,
        first_seen: String::new(),
    };
    assert_eq!(
        store
            .save_local_reviews(project.id, &[review("a", None), review("b", None)], NOW)
            .unwrap(),
        2
    );
    // Read again with the business's answer: nothing new, the answer kept.
    assert_eq!(
        store
            .save_local_reviews(project.id, &[review("a", Some("Teşekkürler"))], NOW)
            .unwrap(),
        0
    );
    let reviews = store.local_reviews(project.id, 10).unwrap();
    assert_eq!(reviews.len(), 2);
    assert_eq!(
        reviews
            .iter()
            .find(|row| row.review_id == "a")
            .unwrap()
            .owner_answer
            .as_deref(),
        Some("Teşekkürler")
    );
    assert_eq!(reviews[0].first_seen, NOW);

    store
        .note_local_profile(
            project.id,
            "2026-10-22",
            Some(4.4),
            Some(600),
            Some(&json!({ "title": "Örnek" })),
            NOW,
        )
        .unwrap();
    // Reviews read later that day give a newer rating; the profile stays.
    store
        .note_local_profile(project.id, "2026-10-22", Some(4.5), Some(640), None, NOW)
        .unwrap();
    let days = store.local_profiles(project.id).unwrap();
    assert_eq!(days.len(), 1);
    assert_eq!((days[0].rating, days[0].votes), (Some(4.5), Some(640)));
    assert_eq!(days[0].profile.as_ref().unwrap()["title"], "Örnek");

    store.delete_project(project.id).unwrap();
    assert_eq!(store.local_runs(project.id, 5).unwrap().len(), 0);
    assert_eq!(store.local_reviews(project.id, 5).unwrap().len(), 0);
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
        check_outbound: true,
    };
    store.save_audit_settings(project.id, &weekly).unwrap();
    assert!(store.audit_settings(project.id).unwrap().check_outbound);
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
    assert_eq!(defaults.engines, ["chatgpt", "gemini", "google_ai_mode"]);

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
        compare: "year".to_owned(),
    };
    let saved = store.save_gsc_settings(project.id, &wanted).unwrap();
    assert_eq!(saved.property.as_deref(), Some("sc-domain:example.com"));
    assert_eq!(saved.compare, "year");
    assert_eq!(saved.ga4_property.as_deref(), Some("properties/123456"));
    assert_eq!(saved.last_sync_at, None);
    for bad in [
        GscSettings {
            days: 30,
            ..wanted.clone()
        },
        GscSettings {
            days: 180,
            ..wanted.clone()
        },
        GscSettings {
            compare: "month".to_owned(),
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
        design: "ocean".to_owned(),
        colors: ReportColors {
            accent: Some(" #AA3300 ".to_owned()),
            heading: Some(String::new()),
            ..ReportColors::default()
        },
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
    assert_eq!(report.spec.design, "ocean");
    assert_eq!(
        report.spec.colors,
        ReportColors {
            accent: Some("#aa3300".to_owned()),
            ..ReportColors::default()
        },
        "colours are cleaned, an empty one keeps the design's"
    );
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
        ReportSpec {
            design: "neon".to_owned(),
            ..report_spec()
        },
        ReportSpec {
            colors: ReportColors {
                table: Some("blue".to_owned()),
                ..ReportColors::default()
            },
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

#[test]
fn snapshots_are_kept_listed_and_deleted_with_their_project() {
    let dir = tempfile::tempdir().unwrap();
    let store = PlatformStore::open(dir.path()).unwrap();
    let project = store.create_project(&spec(), NOW).unwrap();
    assert_eq!(
        store.snapshot_settings(project.id).unwrap(),
        SnapshotSettings::default()
    );
    let settings = store
        .save_snapshot_settings(
            project.id,
            &SnapshotSettings {
                schedule: "monthly".to_owned(),
                competitors: vec!["https://www.Rival.com/".to_owned(), "rival.com".to_owned()],
                last_run_at: None,
            },
        )
        .unwrap();
    assert_eq!(settings.competitors, ["rival.com"]);
    assert!(
        store
            .save_snapshot_settings(
                project.id,
                &SnapshotSettings {
                    schedule: "daily".to_owned(),
                    ..SnapshotSettings::default()
                },
            )
            .is_err()
    );
    assert_eq!(store.snapshot_plans().unwrap().len(), 1);

    let first = store
        .begin_snapshot(
            project.id,
            false,
            Some(" Başlangıç "),
            "2026-10-01T09:00:00Z",
        )
        .unwrap();
    store
        .finish_snapshot(
            first,
            &SnapshotEnd {
                status: "done",
                cost_usd: 0.34,
                error: None,
                data: &json!({ "overview": { "traffic": 1240.0 } }),
            },
            "2026-10-01T09:01:00Z",
        )
        .unwrap();
    // A second one left running is interrupted by the next.
    let left = store
        .begin_snapshot(project.id, true, None, "2026-10-15T09:00:00Z")
        .unwrap();
    let last = store
        .begin_snapshot(project.id, true, None, "2026-11-01T09:00:00Z")
        .unwrap();
    let rows = store.snapshots(project.id).unwrap();
    assert_eq!(
        rows.iter().map(|row| row.id).collect::<Vec<_>>(),
        [last, left, first]
    );
    assert_eq!(rows[1].status, "interrupted");
    assert_eq!(rows[2].label.as_deref(), Some("Başlangıç"));
    assert_eq!(rows[2].data, None);
    let kept = store.snapshot(project.id, first).unwrap();
    assert_eq!(kept.data.unwrap()["overview"]["traffic"], 1240.0);
    assert_eq!(
        store
            .snapshot_settings(project.id)
            .unwrap()
            .last_run_at
            .as_deref(),
        Some("2026-11-01T09:00:00Z")
    );
    store.delete_snapshot(project.id, left).unwrap();
    assert!(store.delete_snapshot(project.id, left).is_err());
    store.delete_project(project.id).unwrap();
    assert_eq!(store.snapshots(project.id).unwrap().len(), 0);
}

#[test]
fn metric_points_keep_one_day_each_and_go_with_their_project() {
    let dir = tempfile::tempdir().unwrap();
    let store = PlatformStore::open(dir.path()).unwrap();
    let project = store.create_project(&spec(), NOW).unwrap();
    let point = |day: &str, traffic: f64| MetricPoint {
        target: "example.com".to_owned(),
        kind: "domain".to_owned(),
        market: "tr-tr".to_owned(),
        day: day.to_owned(),
        metrics: json!({ "traffic": traffic }),
        taken_at: format!("{day}T09:00:00Z"),
    };
    store
        .note_metrics(project.id, &point("2026-10-02", 900.0))
        .unwrap();
    store
        .note_metrics(project.id, &point("2026-10-01", 800.0))
        .unwrap();
    // Fetched again the same day: the day's numbers are replaced.
    store
        .note_metrics(project.id, &point("2026-10-02", 950.0))
        .unwrap();
    let points = store
        .metric_points(project.id, "example.com", "domain", "tr-tr")
        .unwrap();
    assert_eq!(
        points
            .iter()
            .map(|point| point.day.as_str())
            .collect::<Vec<_>>(),
        ["2026-10-01", "2026-10-02"]
    );
    assert_eq!(points[1].metrics["traffic"], 950.0);
    // Another market is another history.
    assert_eq!(
        store
            .metric_points(project.id, "example.com", "domain", "de-de")
            .unwrap()
            .len(),
        0
    );
    for wrong in [
        MetricPoint {
            kind: "weather".to_owned(),
            ..point("2026-10-03", 1.0)
        },
        point("3 October", 1.0),
    ] {
        assert!(store.note_metrics(project.id, &wrong).is_err());
    }
    store.delete_project(project.id).unwrap();
    assert_eq!(
        store
            .metric_points(project.id, "example.com", "domain", "tr-tr")
            .unwrap()
            .len(),
        0
    );
}

#[test]
fn ad_copies_gather_their_keywords_and_sightings() {
    let dir = tempfile::tempdir().unwrap();
    let store = PlatformStore::open(dir.path()).unwrap();
    let project = store.create_project(&spec(), NOW).unwrap();
    let seen = |title: &str, keyword: &str| AdSeen {
        title: title.to_owned(),
        description: Some("Ücretsiz kargo.".to_owned()),
        breadcrumb: Some("www.rival.com/kampanya".to_owned()),
        url: Some("https://www.rival.com/kampanya".to_owned()),
        keyword: keyword.to_owned(),
    };
    store
        .note_ads(
            project.id,
            "https://www.Rival.com/",
            &[
                seen("%40 indirim", "spor ayakkabı"),
                seen("%40 indirim", "koşu ayakkabısı"),
                seen(" ", "boş"),
            ],
            "2026-10-01T09:00:00Z",
        )
        .unwrap();
    store
        .note_ads(
            project.id,
            "rival.com",
            &[seen("%40 indirim", "bot"), seen("Yeni sezon", "bot")],
            "2026-10-20T09:00:00Z",
        )
        .unwrap();
    // An older answer kept elsewhere does not move the last sighting back.
    store
        .note_ads(
            project.id,
            "rival.com",
            &[seen("%40 indirim", "terlik")],
            "2026-09-01T09:00:00Z",
        )
        .unwrap();
    let copies = store.ad_copies(project.id, "rival.com").unwrap();
    assert_eq!(copies.len(), 2);
    let sale = copies
        .iter()
        .find(|copy| copy.title == "%40 indirim")
        .unwrap();
    assert_eq!(sale.first_seen, "2026-10-01T09:00:00Z");
    assert_eq!(sale.last_seen, "2026-10-20T09:00:00Z");
    assert_eq!(
        sale.keywords,
        ["bot", "spor ayakkabı", "koşu ayakkabısı", "terlik"]
    );
    store.delete_project(project.id).unwrap();
    assert_eq!(store.ad_copies(project.id, "rival.com").unwrap().len(), 0);
}

#[test]
fn link_decisions_and_prospects_are_kept_per_site() {
    let dir = tempfile::tempdir().unwrap();
    let store = PlatformStore::open(dir.path()).unwrap();
    let project = store.create_project(&spec(), NOW).unwrap();
    store
        .decide_link(
            project.id,
            "https://www.Spam-Site.xyz/",
            Some("disavow"),
            Some("  ağ  "),
            NOW,
        )
        .unwrap();
    store
        .decide_link(
            project.id,
            "spam-site.xyz",
            Some("watch"),
            None,
            "2026-10-24T09:00:00Z",
        )
        .unwrap();
    store
        .decide_link(project.id, "good.com", Some("keep"), None, NOW)
        .unwrap();
    assert!(
        store
            .decide_link(project.id, "good.com", Some("love"), None, NOW)
            .is_err()
    );
    assert!(
        store
            .decide_link(project.id, "localhost", Some("keep"), None, NOW)
            .is_err()
    );
    let decisions = store.link_decisions(project.id).unwrap();
    assert_eq!(decisions.len(), 2);
    assert_eq!(decisions[1].domain, "spam-site.xyz");
    assert_eq!(decisions[1].decision, "watch");
    assert_eq!(decisions[1].note, None);
    store
        .decide_link(project.id, "good.com", None, None, NOW)
        .unwrap();
    assert_eq!(store.link_decisions(project.id).unwrap().len(), 1);

    let mut prospect = LinkProspect {
        domain: "www.blog.example.org".to_owned(),
        url: Some("https://blog.example.org/best-shoes".to_owned()),
        source: "gap".to_owned(),
        status: "prospect".to_owned(),
        note: None,
        added_at: String::new(),
        updated_at: String::new(),
    };
    store.save_prospect(project.id, &prospect, NOW).unwrap();
    prospect.status = "contacted".to_owned();
    prospect.source = "manual".to_owned();
    prospect.url = None;
    prospect.note = Some("editor@example.org".to_owned());
    let saved = store
        .save_prospect(project.id, &prospect, "2026-10-24T09:00:00Z")
        .unwrap();
    assert_eq!(saved.domain, "blog.example.org");
    assert_eq!(saved.source, "gap");
    assert_eq!(saved.status, "contacted");
    assert_eq!(
        saved.url.as_deref(),
        Some("https://blog.example.org/best-shoes")
    );
    assert_eq!(saved.added_at, NOW);
    assert_eq!(saved.updated_at, "2026-10-24T09:00:00Z");
    prospect.status = "maybe".to_owned();
    assert!(store.save_prospect(project.id, &prospect, NOW).is_err());
    assert_eq!(store.prospects(project.id).unwrap().len(), 1);
    store
        .remove_prospect(project.id, "blog.example.org")
        .unwrap();
    assert_eq!(store.prospects(project.id).unwrap().len(), 0);

    store
        .decide_link(project.id, "x.com", Some("keep"), None, NOW)
        .unwrap();
    store.delete_project(project.id).unwrap();
    assert_eq!(store.link_decisions(project.id).unwrap().len(), 0);
}

#[test]
#[allow(clippy::too_many_lines)] // briefs, scores and pages worth checking
fn briefs_keep_their_source_writing_and_one_score_per_page() {
    let dir = tempfile::tempdir().unwrap();
    let store = PlatformStore::open(dir.path()).unwrap();
    let project = store.create_project(&spec(), NOW).unwrap();
    assert!(
        store
            .begin_content_brief(project.id, "  ", ("tr", "tr"), false, NOW)
            .is_err()
    );
    let first = store
        .begin_content_brief(project.id, " spor ayakkabı ", ("tr", "tr"), true, NOW)
        .unwrap();
    let second = store
        .begin_content_brief(project.id, "koşu ayakkabısı", ("tr", "tr"), false, NOW)
        .unwrap();
    assert_eq!(store.content_brief(first).unwrap().status, "interrupted");
    assert_eq!(store.content_brief(first).unwrap().keyword, "spor ayakkabı");
    assert!(store.content_brief_source(second).unwrap().is_none());
    store
        .fail_content_brief(first, "no pages answered", 0.002, NOW)
        .unwrap();
    store
        .finish_content_brief(
            second,
            &json!({ "terms": [] }),
            &json!({ "pages": [] }),
            0.002,
            NOW,
        )
        .unwrap();
    let brief = store.content_brief(second).unwrap();
    assert_eq!(
        (brief.status.as_str(), brief.brief),
        ("done", Some(json!({ "terms": [] })))
    );
    assert_eq!(
        store.content_brief_source(second).unwrap(),
        Some(json!({ "pages": [] }))
    );
    assert!(store.content_brief_source(999).is_err());

    let result = json!({ "missing": [] });
    let page = |score: f64| ScoreToSave {
        kind: "url",
        url: Some("https://ornek.com/kosu"),
        title: None,
        description: None,
        body: None,
        score,
        result: &result,
    };
    let scored = store
        .save_content_score(second, project.id, page(40.0), NOW)
        .unwrap();
    store
        .note_score_suggestions(scored, "Daha uzun yazın")
        .unwrap();
    assert_eq!(
        store
            .save_content_score(second, project.id, page(55.0), LATER)
            .unwrap(),
        scored
    );
    let draft = ScoreToSave {
        kind: "draft",
        url: Some("ignored"),
        title: Some("Koşu ayakkabısı"),
        description: None,
        body: Some("# Koşu"),
        score: 30.0,
        result: &result,
    };
    store
        .save_content_score(second, project.id, draft, NOW)
        .unwrap();
    assert!(
        store
            .save_content_score(
                second,
                project.id,
                ScoreToSave {
                    kind: "pdf",
                    ..draft
                },
                NOW
            )
            .is_err()
    );
    let scores = store.content_scores(second).unwrap();
    assert_eq!(scores.len(), 2);
    assert_eq!(
        (scores[0].kind.as_str(), scores[0].url.as_deref()),
        ("draft", None)
    );
    assert_eq!(scores[1].score, 55.0);
    assert_eq!(scores[1].suggestions, None);
    assert_eq!(
        store.content_page_scores(project.id).unwrap(),
        [(
            "https://ornek.com/kosu".to_owned(),
            "koşu ayakkabısı".to_owned(),
            55.0,
            LATER.to_owned()
        )]
    );
    assert_eq!(store.content_briefs(project.id).unwrap().len(), 2);

    let tracker = store
        .add_rank_tracker(project.id, &google("tr"), NOW)
        .unwrap();
    store
        .track_keywords(
            project.id,
            &[("koşu ayakkabısı".to_owned(), Some(2_400))],
            &[],
            NOW,
        )
        .unwrap();
    let keyword = store.tracked_keywords(project.id).unwrap()[0].id;
    let run = store.begin_rank_run(project.id, false, 1, NOW).unwrap();
    store
        .add_rank_check(&check(run, project.id, keyword, tracker.id, NOW))
        .unwrap();
    let page = |query: &str, page: &str, clicks: f64| QueryPage {
        query: query.to_owned(),
        page: page.to_owned(),
        clicks,
        impressions: clicks * 10.0,
        position: 6.5,
    };
    store
        .replace_gsc_data(
            project.id,
            &GscData {
                search: Vec::new(),
                query_pages: vec![
                    page("spor ayakkabı", "https://example.com/b", 12.0),
                    page("ayakkabı", "https://example.com/b", 30.0),
                    page("koşu", "https://example.com/a", 50.0),
                ],
                traffic: Vec::new(),
            },
        )
        .unwrap();
    let candidates = store.content_candidates(project.id, 10).unwrap();
    assert_eq!(candidates.len(), 2);
    assert_eq!(
        (
            candidates[0].url.as_str(),
            candidates[0].keyword.as_str(),
            candidates[0].source.as_str()
        ),
        ("https://example.com/a", "koşu ayakkabısı", "rank")
    );
    assert_eq!(
        (candidates[0].position, candidates[0].volume),
        (Some(3.0), Some(2_400))
    );
    assert_eq!(
        (candidates[1].keyword.as_str(), candidates[1].clicks),
        ("ayakkabı", Some(30.0))
    );
    store.delete_project(project.id).unwrap();
    assert!(store.content_brief(second).is_err());
    assert_eq!(store.content_scores(second).unwrap().len(), 0);
}

#[test]
fn vitals_keep_one_check_a_day_per_address_and_device() {
    let dir = tempfile::tempdir().unwrap();
    let store = PlatformStore::open(dir.path()).unwrap();
    let project = store.create_project(&spec(), NOW).unwrap();
    let check = |target: &str, scope: &str, device: &str, day: &str, lcp: f64| VitalCheck {
        target: target.to_owned(),
        scope: scope.to_owned(),
        device: device.to_owned(),
        day: day.to_owned(),
        field: Some(json!({ "lcp": { "p75": lcp } })),
        lab: (scope == "page").then(|| json!({ "score": 61 })),
        history: (scope == "origin").then(|| json!({ "dates": ["2026-10-19"] })),
        checked_at: format!("{day}T09:00:00Z"),
    };
    let site = "https://www.example.com";
    store
        .save_vital_check(
            project.id,
            &check(site, "origin", "phone", "2026-10-20", 3000.0),
        )
        .unwrap();
    store
        .save_vital_check(
            project.id,
            &check(site, "origin", "phone", "2026-10-21", 2900.0),
        )
        .unwrap();
    store
        .save_vital_check(
            project.id,
            &check(site, "origin", "phone", "2026-10-21", 2800.0),
        )
        .unwrap();
    store
        .save_vital_check(
            project.id,
            &check(site, "origin", "desktop", "2026-10-21", 1900.0),
        )
        .unwrap();
    store
        .save_vital_check(
            project.id,
            &check(
                "https://www.example.com/a",
                "page",
                "phone",
                "2026-10-21",
                3500.0,
            ),
        )
        .unwrap();
    assert!(
        store
            .save_vital_check(
                project.id,
                &check(site, "domain", "phone", "2026-10-21", 1.0)
            )
            .is_err()
    );
    assert!(
        store
            .save_vital_check(
                project.id,
                &check(site, "origin", "tablet", "2026-10-21", 1.0)
            )
            .is_err()
    );
    assert_eq!(store.vital_checks(project.id, None).unwrap().len(), 4);
    assert_eq!(
        store
            .vital_checks(project.id, Some("2026-10-21"))
            .unwrap()
            .len(),
        3
    );
    let latest = store.latest_vitals(project.id).unwrap();
    assert_eq!(latest.len(), 3);
    assert_eq!(
        (latest[0].scope.as_str(), latest[0].device.as_str()),
        ("page", "phone")
    );
    let phone = latest
        .iter()
        .find(|row| row.scope == "origin" && row.device == "phone")
        .unwrap();
    assert_eq!(phone.field.as_ref().unwrap()["lcp"]["p75"], 2800.0);
    assert_eq!(phone.history.as_ref().unwrap()["dates"][0], "2026-10-19");
    store.delete_project(project.id).unwrap();
    assert_eq!(store.vital_checks(project.id, None).unwrap().len(), 0);
}

#[test]
fn opportunity_decisions_are_kept_until_taken_back_and_spending_is_split_by_module() {
    let dir = tempfile::tempdir().unwrap();
    let store = PlatformStore::open(dir.path()).unwrap();
    let project = store.create_project(&spec(), NOW).unwrap();
    store
        .set_opportunity_state(project.id, "near_top|spor ayakkabı", "done", None, NOW)
        .unwrap();
    store
        .set_opportunity_state(
            project.id,
            "low_ctr|bot",
            "snoozed",
            Some("2026-11-01T00:00:00Z"),
            NOW,
        )
        .unwrap();
    store
        .set_opportunity_state(project.id, "near_top|spor ayakkabı", "ignored", None, LATER)
        .unwrap();
    assert!(
        store
            .set_opportunity_state(project.id, "x", "maybe", None, NOW)
            .is_err()
    );
    assert!(
        store
            .set_opportunity_state(project.id, "x", "snoozed", None, NOW)
            .is_err()
    );
    assert!(
        store
            .set_opportunity_state(project.id, " ", "done", None, NOW)
            .is_err()
    );
    let states = store.opportunity_states(project.id).unwrap();
    assert_eq!(states.len(), 2);
    assert_eq!(
        (states[0].key.as_str(), states[0].state.as_str()),
        ("near_top|spor ayakkabı", "ignored")
    );
    assert_eq!(states[1].until.as_deref(), Some("2026-11-01T00:00:00Z"));
    store
        .set_opportunity_state(project.id, "low_ctr|bot", "open", None, LATER)
        .unwrap();
    assert_eq!(store.opportunity_states(project.id).unwrap().len(), 1);

    for (module, usd) in [("overview", 0.02), ("rank", 0.5), ("overview", 0.03)] {
        store
            .record_spend(&SpendRecord {
                at: NOW.to_owned(),
                project_id: Some(project.id),
                module: module.to_owned(),
                endpoint: "x".to_owned(),
                provider: "dataforseo".to_owned(),
                via: "own".to_owned(),
                requests: 1,
                cost_usd: usd,
            })
            .unwrap();
    }
    let spend = store.project_spend(project.id, &NOW[..7]).unwrap();
    assert_eq!(spend[0].0, "rank");
    assert!((spend[1].1 - 0.05).abs() < 1e-9);
    assert_eq!(store.project_spend(project.id, "2020-01").unwrap().len(), 0);
    store.delete_project(project.id).unwrap();
    assert_eq!(store.opportunity_states(project.id).unwrap().len(), 0);
}

#[test]
#[allow(clippy::too_many_lines)] // settings, alerts, reading and both channels in turn
fn alerts_are_kept_once_read_and_sent_per_channel() {
    let dir = tempfile::tempdir().unwrap();
    let store = PlatformStore::open(dir.path()).unwrap();
    let project = store.create_project(&spec(), NOW).unwrap();
    let defaults = store.alert_settings(project.id).unwrap();
    assert!(defaults.enabled && !defaults.rules.links);
    assert_eq!(defaults.rules.rank_drop, 5);
    let mut settings = defaults.clone();
    settings.email = "daily".to_owned();
    assert!(
        store.save_alert_settings(project.id, &settings).is_err(),
        "e-mails need recipients"
    );
    settings.recipients = vec![" Ajans@Ornek.com ".to_owned(), "ajans@ornek.com".to_owned()];
    settings.rules.gsc_drop = 30.0;
    settings.lang = "en".to_owned();
    let saved = store.save_alert_settings(project.id, &settings).unwrap();
    assert_eq!(saved.recipients, ["ajans@ornek.com"]);
    assert_eq!(saved.rules.gsc_drop, 30.0);
    for bad in [
        AlertSettings {
            email: "hourly".to_owned(),
            ..settings.clone()
        },
        AlertSettings {
            lang: "fr".to_owned(),
            ..settings.clone()
        },
        AlertSettings {
            recipients: vec!["nope".to_owned()],
            ..settings.clone()
        },
        AlertSettings {
            rules: AlertRules {
                health_drop: 150.0,
                ..AlertRules::default()
            },
            ..settings.clone()
        },
    ] {
        assert!(store.save_alert_settings(project.id, &bad).is_err());
    }
    store.note_alert_digest(project.id, NOW).unwrap();
    store.note_alert_evaluation(project.id, LATER).unwrap();
    let plans = store.alert_plans().unwrap();
    assert_eq!(
        (plans[0].1.last_digest_at.as_deref(), plans[0].2.as_deref()),
        (Some(NOW), Some(LATER))
    );

    let alert = |key: &str, at: &str| NewAlert {
        key: key.to_owned(),
        kind: "gsc_drop".to_owned(),
        severity: "bad".to_owned(),
        page: "gsc".to_owned(),
        params: json!({}),
        detail: json!({ "drop": 31.5 }),
        created_at: at.to_owned(),
    };
    assert!(
        store
            .add_alert(project.id, &alert("gsc|2026-10-20", NOW))
            .unwrap()
    );
    assert!(
        !store
            .add_alert(project.id, &alert("gsc|2026-10-20", LATER))
            .unwrap(),
        "kept once"
    );
    assert!(
        store
            .add_alert(project.id, &alert("gsc|2026-10-21", LATER))
            .unwrap()
    );
    let listed = store.alerts(Some(project.id), false, 10).unwrap();
    assert_eq!(listed.len(), 2);
    assert_eq!(listed[0].alert.key, "gsc|2026-10-21");
    assert_eq!(listed[0].alert.detail["drop"], 31.5);
    assert_eq!(store.unread_alerts().unwrap(), [(project.id, 2)]);
    assert_eq!(
        store
            .mark_alerts_read(Some(project.id), Some(&[listed[1].id]), LATER)
            .unwrap(),
        1
    );
    assert_eq!(store.alerts(None, true, 10).unwrap().len(), 1);
    let unsent = store
        .alerts_unsent(Some(project.id), "mailed", NOW)
        .unwrap();
    assert_eq!(unsent.len(), 2);
    store
        .mark_alerts_sent(&[unsent[0].id], "mailed", LATER)
        .unwrap();
    assert_eq!(store.alerts_unsent(None, "mailed", NOW).unwrap().len(), 1);
    assert_eq!(store.alerts_unsent(None, "notified", NOW).unwrap().len(), 2);
    assert_eq!(
        store.alerts_unsent(None, "notified", LATER).unwrap().len(),
        1
    );
    assert_eq!(store.mark_alerts_read(None, None, LATER).unwrap(), 1);
    store.delete_project(project.id).unwrap();
    assert_eq!(store.alerts(None, false, 10).unwrap().len(), 0);
}
