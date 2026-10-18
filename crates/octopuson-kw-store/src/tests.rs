use super::*;

const NOW: &str = "2026-10-18T10:00:00Z";

fn store() -> (tempfile::TempDir, KwStore) {
    let dir = tempfile::tempdir().unwrap();
    let store = KwStore::open(dir.path()).unwrap();
    (dir, store)
}

fn spec() -> ProjectSpec {
    ProjectSpec {
        name: "Kahve".to_owned(),
        country: "tr".to_owned(),
        language: "tr".to_owned(),
        engines: vec!["google".to_owned(), "yandex".to_owned()],
        site: Some("ornek.com.tr".to_owned()),
        competitors: vec!["rakip.com".to_owned()],
    }
}

fn keyword(text: &str, key: &str, source: &str) -> NewKeyword {
    NewKeyword {
        keyword: text.to_owned(),
        key: key.to_owned(),
        stems: key.split(' ').map(str::to_owned).collect(),
        source: source.to_owned(),
        seed: Some("kahve".to_owned()),
    }
}

fn metric(keyword_id: i64, source: &str, engine: &str, volume: Option<u64>) -> MetricRecord {
    MetricRecord {
        keyword_id,
        source: source.to_owned(),
        engine: engine.to_owned(),
        country: "tr".to_owned(),
        volume,
        volume_low: None,
        volume_high: None,
        cpc_usd: None,
        competition: None,
        difficulty: None,
        monthly: Vec::new(),
        fetched_at: NOW.to_owned(),
    }
}

/// A project with two keywords, the first with searches.
fn two_machines(store: &mut KwStore) -> (i64, i64, i64) {
    let id = store.create_project(&spec(), NOW).unwrap();
    store
        .add_keywords(
            id,
            &[
                keyword("kahve makinesi", "kahve makinesi", "import"),
                keyword("kahve makineleri", "kahve makineleri", "import"),
            ],
            NOW,
        )
        .unwrap();
    let keys = store.key_index(id).unwrap();
    let (machine, machines) = (keys["kahve makinesi"], keys["kahve makineleri"]);
    store
        .put_metrics(&[metric(machine, "dataforseo", "google", Some(1_000))])
        .unwrap();
    (id, machine, machines)
}

fn machine_cluster(machine: i64) -> ClusterRecord {
    ClusterRecord {
        number: 0,
        pillar_id: machine,
        label: "kahve makinesi".to_owned(),
        size: 2,
        volume: 1_000,
        intent: Some("commercial".to_owned()),
        mixed_intent: false,
        difficulty: None,
        cpc_usd: None,
        verified: false,
        target_url: None,
        competing: Vec::new(),
        gap: true,
        competitors: vec![Competitor {
            domain: "a.com".to_owned(),
            keywords: 1,
        }],
    }
}

fn machine_members(machine: i64, machines: i64) -> [MemberRecord; 2] {
    [
        MemberRecord {
            number: 0,
            keyword_id: machine,
            score: 1.0,
            shared: None,
        },
        MemberRecord {
            number: 0,
            keyword_id: machines,
            score: 0.7,
            shared: None,
        },
    ]
}

#[test]
fn opens_once_and_upgrades_to_the_newest_schema() {
    let dir = tempfile::tempdir().unwrap();
    let store = KwStore::open(dir.path()).unwrap();
    assert!(store.is_current().unwrap());
    assert!(matches!(
        KwStore::open(dir.path()),
        Err(KwStoreError::Busy(_))
    ));
    drop(store);
    let again = KwStore::open(dir.path()).unwrap();
    assert_eq!(again.schema_version().unwrap(), 3);
}

#[test]
fn keeps_projects_with_their_counts() {
    let (_dir, mut store) = store();
    let id = store.create_project(&spec(), NOW).unwrap();
    let project = store.project(id).unwrap();
    assert_eq!(project.spec, spec());
    assert_eq!(project.keywords, 0);
    assert_eq!(project.clusters, None);

    let mut changed = spec();
    changed.name = "Kahve makineleri".to_owned();
    changed.engines = vec!["yandex".to_owned()];
    store.update_project(id, &changed).unwrap();
    assert_eq!(store.projects().unwrap()[0].spec, changed);
    assert_eq!(store.primary_engine(id).unwrap(), "yandex");

    store
        .set_project_settings(id, &serde_json::json!({ "threshold": 0.5 }))
        .unwrap();
    assert_eq!(store.project_settings(id).unwrap()["threshold"], 0.5);

    store
        .add_keywords(id, &[keyword("kahve", "kahve", "import")], NOW)
        .unwrap();
    store.delete_project(id).unwrap();
    assert!(matches!(store.project(id), Err(KwStoreError::NotFound(_))));
    assert!(store.key_index(id).unwrap().is_empty());
    assert!(matches!(
        store.update_project(id, &spec()),
        Err(KwStoreError::NotFound(_))
    ));
}

#[test]
fn merges_variants_and_records_every_source() {
    let (_dir, mut store) = store();
    let id = store.create_project(&spec(), NOW).unwrap();
    let first = store
        .add_keywords(
            id,
            &[
                keyword("Türk kahvesi", "turk kahvesi", "import"),
                keyword("turk kahvesi", "turk kahvesi", "import"),
                keyword("filtre kahve", "filtre kahve", "import"),
            ],
            NOW,
        )
        .unwrap();
    assert_eq!(
        first,
        Added {
            received: 3,
            added: 2,
            known: 0
        }
    );
    let second = store
        .add_keywords(
            id,
            &[
                keyword("TÜRK KAHVESİ", "turk kahvesi", "wordstat"),
                keyword("kahve makinesi", "kahve makinesi", "wordstat"),
            ],
            NOW,
        )
        .unwrap();
    assert_eq!(
        second,
        Added {
            received: 2,
            added: 1,
            known: 1
        }
    );
    let page = store
        .keywords(
            id,
            &KeywordQuery {
                sort: KeywordSort::Added,
                descending: false,
                ..KeywordQuery::default()
            },
        )
        .unwrap();
    assert_eq!(page.total, 3);
    assert_eq!(page.rows[0].keyword, "Türk kahvesi");
    assert_eq!(page.rows[0].sources, ["import", "wordstat"]);
    assert_eq!(page.rows[2].sources, ["wordstat"]);
    assert_eq!(
        store.sources(id).unwrap(),
        [("import".to_owned(), 2), ("wordstat".to_owned(), 2)]
    );
}

#[test]
fn prefers_the_primary_engine_and_the_newest_numbers() {
    let (_dir, mut store) = store();
    let id = store.create_project(&spec(), NOW).unwrap();
    store
        .add_keywords(
            id,
            &[
                keyword("a", "a", "import"),
                keyword("b", "b", "import"),
                keyword("c", "c", "import"),
            ],
            NOW,
        )
        .unwrap();
    let keys = store.key_index(id).unwrap();
    let (a, b, c) = (keys["a"], keys["b"], keys["c"]);
    let mut difficulty = metric(a, "dataforseo-labs", "google", None);
    difficulty.difficulty = Some(42.0);
    let mut trend = metric(b, "wordstat", "yandex", Some(900));
    trend.monthly = vec![MonthlyVolume {
        month: "2026-09".to_owned(),
        volume: 900,
    }];
    store
        .put_metrics(&[
            metric(a, "wordstat", "yandex", Some(5_000)),
            metric(a, "dataforseo", "google", Some(100)),
            difficulty,
            trend,
            metric(c, "dataforseo", "google", Some(1)),
            // A later record for the same keyword and source wins.
            metric(c, "dataforseo", "google", Some(300)),
        ])
        .unwrap();
    let page = store.keywords(id, &KeywordQuery::default()).unwrap();
    let volumes: Vec<_> = page
        .rows
        .iter()
        .map(|row| (row.keyword.as_str(), row.volume))
        .collect();
    assert_eq!(
        volumes,
        [("b", Some(900)), ("c", Some(300)), ("a", Some(100))]
    );
    assert_eq!(page.rows[2].difficulty, Some(42.0));
    assert_eq!(page.rows[0].monthly.len(), 1);

    // Replacing a source's numbers.
    store
        .put_metrics(&[metric(a, "dataforseo", "google", Some(10_000))])
        .unwrap();
    let page = store.keywords(id, &KeywordQuery::default()).unwrap();
    assert_eq!(page.rows[0].keyword, "a");
    assert_eq!(page.rows[0].volume, Some(10_000));

    let filtered = store
        .keywords(
            id,
            &KeywordQuery {
                min_volume: Some(500),
                max_volume: Some(5_000),
                ..KeywordQuery::default()
            },
        )
        .unwrap();
    assert_eq!(filtered.total, 1);
    assert_eq!(filtered.rows[0].keyword, "b");
}

#[test]
fn pages_filter_by_text_tag_status_and_intent() {
    let (_dir, mut store) = store();
    let id = store.create_project(&spec(), NOW).unwrap();
    let words: Vec<NewKeyword> = (0..25)
        .map(|n| keyword(&format!("kahve {n:02}"), &format!("kahve {n:02}"), "import"))
        .collect();
    store.add_keywords(id, &words, NOW).unwrap();
    let keys = store.key_index(id).unwrap();
    let first = keys["kahve 00"];
    let second = keys["kahve 01"];
    store
        .tag(id, &[first, second], &["marka".to_owned()], &[])
        .unwrap();
    store
        .tag(id, &[second], &[], &["marka".to_owned()])
        .unwrap();
    assert_eq!(store.tags(id).unwrap(), [("marka".to_owned(), 1)]);
    assert_eq!(
        store
            .set_status(id, &[second], KeywordStatus::Excluded)
            .unwrap(),
        1
    );
    store
        .put_intents(&[IntentRecord {
            keyword_id: first,
            primary: "transactional".to_owned(),
            secondary: None,
            source: "words".to_owned(),
        }])
        .unwrap();

    let active = store.keywords(id, &KeywordQuery::default()).unwrap();
    assert_eq!(active.total, 24);
    let tagged = store
        .keywords(
            id,
            &KeywordQuery {
                tag: Some("marka".to_owned()),
                ..KeywordQuery::default()
            },
        )
        .unwrap();
    assert_eq!(tagged.rows[0].tags, ["marka"]);
    assert_eq!(tagged.rows[0].intent.as_deref(), Some("transactional"));
    let excluded = store
        .keywords(
            id,
            &KeywordQuery {
                status: Some(KeywordStatus::Excluded),
                ..KeywordQuery::default()
            },
        )
        .unwrap();
    assert_eq!(excluded.rows[0].id, second);
    let searched = store
        .keywords(
            id,
            &KeywordQuery {
                search: Some("e 1".to_owned()),
                sort: KeywordSort::Keyword,
                descending: false,
                limit: 3,
                offset: 3,
                ..KeywordQuery::default()
            },
        )
        .unwrap();
    assert_eq!(searched.total, 10);
    let texts: Vec<_> = searched
        .rows
        .iter()
        .map(|row| row.keyword.as_str())
        .collect();
    assert_eq!(texts, ["kahve 13", "kahve 14", "kahve 15"]);
    let past = store
        .keywords(
            id,
            &KeywordQuery {
                offset: 100,
                ..KeywordQuery::default()
            },
        )
        .unwrap();
    assert_eq!(past.rows, Vec::new());
    assert_eq!(past.total, 24);
}

#[test]
fn feeds_clustering_and_keeps_its_results() {
    let (_dir, mut store) = store();
    let (id, machine, machines) = two_machines(&mut store);
    let serp = SerpRecord {
        keyword_id: machine,
        engine: "google".to_owned(),
        device: "desktop".to_owned(),
        provider: "dataforseo".to_owned(),
        fetched_at: NOW.to_owned(),
        urls: vec!["https://a.com/1".to_owned(), "https://b.com/1".to_owned()],
        site_only: false,
        features: vec!["people-also-ask".to_owned()],
        questions: vec!["Hangi kahve makinesi?".to_owned()],
        related: Vec::new(),
        raw_path: None,
    };
    store.put_serps(std::slice::from_ref(&serp)).unwrap();
    assert_eq!(
        store.serp(machine, "google", "desktop").unwrap().as_ref(),
        Some(&serp)
    );
    assert_eq!(
        store
            .serps(id, &[machine, machines], "google", "desktop")
            .unwrap(),
        std::slice::from_ref(&serp)
    );
    assert_eq!(
        store.serp_ages(id, "google", "desktop").unwrap()[&machine],
        NOW
    );

    let inputs = store.cluster_inputs(id, "google", "desktop").unwrap();
    assert_eq!(inputs.len(), 2);
    assert_eq!(inputs[0].volume, Some(1_000));
    assert_eq!(inputs[0].results.as_deref(), Some(serp.urls.as_slice()));
    assert_eq!(inputs[1].results, None);
    let rows = store.keywords(id, &KeywordQuery::default()).unwrap().rows;
    assert!(rows[0].serp);
    assert!(!rows[1].serp);
}

#[test]
fn keeps_clusterings_members_and_placements() {
    let (_dir, mut store) = store();
    let (id, machine, machines) = two_machines(&mut store);
    let cluster = machine_cluster(machine);
    let members = machine_members(machine, machines);
    let clustering_spec = ClusteringSpec {
        method: "lexical".to_owned(),
        params: serde_json::json!({ "threshold": 0.6 }),
        created_at: NOW.to_owned(),
    };
    let mut last = 0;
    for _ in 0..=KEPT_CLUSTERINGS {
        last = store
            .save_clustering(
                id,
                &clustering_spec,
                std::slice::from_ref(&cluster),
                &members,
            )
            .unwrap();
    }
    let clusterings = store.clusterings(id).unwrap();
    assert_eq!(clusterings.len(), KEPT_CLUSTERINGS as usize);
    assert_eq!(clusterings[0].id, last);
    assert_eq!(store.latest_clustering(id).unwrap().unwrap().clusters, 1);
    assert_eq!(store.project(id).unwrap().clusters, Some(1));

    let page = store
        .clusters(
            last,
            &ClusterQuery {
                search: Some("makineleri".to_owned()),
                ..ClusterQuery::default()
            },
        )
        .unwrap();
    assert_eq!(page.total, 1);
    assert_eq!(page.rows[0], cluster);
    assert_eq!(
        store.cluster_records(last).unwrap(),
        std::slice::from_ref(&cluster)
    );
    assert_eq!(store.member_records(last).unwrap(), members);

    store
        .set_pins(
            id,
            &[Pin {
                keyword_id: machines,
                pillar_id: machine,
            }],
        )
        .unwrap();
    let listed = store.cluster_members(last, 0).unwrap();
    assert!(listed[0].pillar);
    assert_eq!(listed[0].volume, Some(1_000));
    assert!(listed[1].pinned);
    let rows = store.keywords(id, &KeywordQuery::default()).unwrap().rows;
    assert_eq!(rows[0].cluster, Some(0));
    assert!(rows[0].pillar);
    assert!(!rows[1].pillar);

    let split = ClusterRecord {
        number: 1,
        pillar_id: machines,
        label: "kahve makineleri".to_owned(),
        size: 1,
        ..cluster.clone()
    };
    store
        .replace_clusters(
            last,
            &[ClusterRecord { size: 1, ..cluster }, split],
            &[
                members[0].clone(),
                MemberRecord {
                    number: 1,
                    ..members[1].clone()
                },
            ],
        )
        .unwrap();
    assert_eq!(store.clustering(last).unwrap().clusters, 2);
    store.remove_pins(id, &[machines]).unwrap();
    assert_eq!(store.pins(id).unwrap(), Vec::new());
    store.delete_clustering(last).unwrap();
    assert!(matches!(
        store.clustering(last),
        Err(KwStoreError::NotFound(_))
    ));
}

#[test]
fn replaces_site_pages_per_source() {
    let (_dir, mut store) = store();
    let id = store.create_project(&spec(), NOW).unwrap();
    store
        .add_keywords(id, &[keyword("kahve", "kahve", "gsc")], NOW)
        .unwrap();
    let kahve = store.key_index(id).unwrap()["kahve"];
    let page = |url: &str, impressions: f64| SitePageRecord {
        keyword_id: kahve,
        url: url.to_owned(),
        source: "gsc".to_owned(),
        clicks: Some(1.0),
        impressions: Some(impressions),
        position: Some(4.2),
        period: "2026-09-01..2026-09-30".to_owned(),
    };
    store
        .put_site_pages(id, "gsc", &[page("https://ornek.com.tr/a", 10.0)])
        .unwrap();
    store
        .put_site_pages(
            id,
            "gsc",
            &[
                page("https://ornek.com.tr/b", 5.0),
                page("https://ornek.com.tr/c", 50.0),
                page("https://ornek.com.tr/c", 1.0),
            ],
        )
        .unwrap();
    let pages = store.site_pages(id).unwrap();
    let urls: Vec<_> = pages.iter().map(|page| page.url.as_str()).collect();
    assert_eq!(urls, ["https://ornek.com.tr/c", "https://ornek.com.tr/b"]);
}

#[test]
fn counts_runs_requests_and_spending() {
    let (_dir, store) = store();
    let run = store
        .begin_run(RunKind::Discover, None, 10, 0.5, NOW)
        .unwrap();
    for (provider, outcome, cost, engine) in [
        ("wordstat", AttemptOutcome::Success, 0.1, None),
        ("wordstat", AttemptOutcome::Refused, 0.0, None),
        ("dataforseo", AttemptOutcome::Success, 0.25, None),
        ("browser", AttemptOutcome::Success, 0.0, Some("google")),
        ("browser", AttemptOutcome::Failed, 0.0, Some("yandex")),
    ] {
        store
            .record_attempt(&AttemptRecord {
                run_id: Some(run),
                provider: provider.to_owned(),
                attempted_at: NOW.to_owned(),
                outcome,
                requests: 2,
                items: 40,
                cost_usd: cost,
                error: None,
                engine: engine.map(str::to_owned),
                via: None,
            })
            .unwrap();
    }
    store.progress_run(run, 4).unwrap();
    store
        .finish_run(
            run,
            RunStatus::Partial,
            9,
            "2026-10-18T10:05:00Z",
            Some("quota"),
        )
        .unwrap();
    let runs = store.runs(None, 5).unwrap();
    assert_eq!(runs[0].kind, RunKind::Discover);
    assert_eq!(runs[0].status, RunStatus::Partial);
    assert_eq!(runs[0].done, 9);
    assert!((runs[0].cost_usd - 0.35).abs() < 1e-9);

    let usage = store.usage().unwrap();
    assert_eq!(usage[2].provider, "wordstat");
    assert_eq!(usage[2].requests, 4);
    assert_eq!(usage[2].failures, 1);
    assert_eq!(store.requests_on("wordstat", "2026-10-18").unwrap(), 4);
    assert_eq!(
        store
            .engine_requests_on("browser", "google", "2026-10-18")
            .unwrap(),
        2
    );
    assert_eq!(
        store
            .engine_requests_on("browser", "bing", "2026-10-18")
            .unwrap(),
        0
    );
    assert!(store.refused_on("wordstat", "2026-10-18").unwrap());
    assert!(!store.refused_on("dataforseo", "2026-10-18").unwrap());
    assert_eq!(
        store
            .requests_since("wordstat", "2026-10-18T11:00:00Z")
            .unwrap(),
        0
    );
    assert!((store.spent("dataforseo", "2026-10").unwrap() - 0.25).abs() < 1e-9);

    let stale = store.begin_run(RunKind::Serp, None, 1, 0.0, NOW).unwrap();
    assert_eq!(store.close_stale_runs(NOW).unwrap(), 1);
    assert_eq!(store.runs(None, 1).unwrap()[0].id, stale);
    assert_eq!(store.runs(None, 1).unwrap()[0].status, RunStatus::Failed);
}

#[test]
fn imports_fifty_thousand_keywords_in_one_write() {
    let (_dir, mut store) = store();
    let id = store.create_project(&spec(), NOW).unwrap();
    let words: Vec<NewKeyword> = (0..50_000)
        .map(|n| keyword(&format!("kelime {n}"), &format!("kelime {n}"), "import"))
        .collect();
    let started = std::time::Instant::now();
    let added = store.add_keywords(id, &words, NOW).unwrap();
    assert_eq!(added.added, 50_000);
    let page = store
        .keywords(
            id,
            &KeywordQuery {
                limit: MAX_PAGE,
                ..KeywordQuery::default()
            },
        )
        .unwrap();
    assert_eq!(page.total, 50_000);
    assert_eq!(page.rows.len(), 10_000);
    assert!(started.elapsed().as_secs() < 60, "{:?}", started.elapsed());
    assert_eq!(store.add_keywords(id, &words, NOW).unwrap().known, 50_000);
}
