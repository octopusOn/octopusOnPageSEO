use super::tests::archive;
use super::*;
use octopuson_core::table::{Cell, TableBuffer};

fn create(library: &CrawlLibrary, sql: &str) -> CrawlMeta {
    archive(
        library,
        &format!(
            "DELETE FROM audit_findings; UPDATE urls SET content_kind='html',indexability='indexable'; {sql}"
        ),
    )
}
fn add(rule: &str, url: &str) -> String {
    format!(
        "INSERT INTO audit_findings VALUES ({},{});",
        literal(rule),
        literal(url)
    )
}
fn counts(summary: &FindingComparisonSummary) -> [u64; 6] {
    let c = &summary.counts;
    [
        c.introduced,
        c.not_detected,
        c.persisting,
        c.new_url,
        c.missing_url,
        c.unverified,
    ]
}

#[test]
fn schema_findings_without_a_counterpart_property_check_are_not_reported_as_resolved() {
    let dir = tempfile::tempdir().unwrap();
    let library = CrawlLibrary::new(dir.path());
    let baseline = create(
        &library,
        &add("schema-required-invalid", "https://example.com/same"),
    );
    let current = create(&library, "");
    let comparison = library.compare(&baseline.id, &current.id).unwrap();
    assert_eq!(
        counts(&comparison.findings_summary(true).unwrap()),
        [0, 0, 0, 0, 0, 1]
    );
}

#[test]
fn partitions_findings_and_reverses_direction_without_claiming_missing_urls_are_fixes() {
    let dir = tempfile::tempdir().unwrap();
    let library = CrawlLibrary::new(dir.path());
    let base = "https://example.com/";
    let baseline = create(
        &library,
        &format!(
            "{}{}{}{}",
            add("title-missing", &format!("{base}same")),
            add("description-missing", &format!("{base}edited")),
            add("h1-missing", &format!("{base}edited")),
            add("canonical-missing", &format!("{base}old"))
        ),
    );
    let current = create(
        &library,
        &format!(
            "UPDATE urls SET url='{base}new' WHERE id=2; {}{}{}{}",
            add("title-missing", &format!("{base}same")),
            add("h1-missing", &format!("{base}edited")),
            add("h2-missing", &format!("{base}edited")),
            add("canonical-missing", &format!("{base}new"))
        ),
    );
    let comparison = library.compare(&baseline.id, &current.id).unwrap();
    let summary = comparison.findings_summary(true).unwrap();
    assert_eq!(counts(&summary), [1, 1, 2, 1, 1, 0]);
    assert_eq!(
        (summary.baseline, summary.current, summary.counts.total),
        (4, 4, 6)
    );
    assert_eq!(summary.rules.len(), 5);
    assert_eq!(summary.rule_total, 5);
    let reverse = library
        .compare(&current.id, &baseline.id)
        .unwrap()
        .findings_summary(true)
        .unwrap();
    assert_eq!(counts(&reverse), [1, 1, 2, 1, 1, 0]);
    let page = comparison
        .findings_page(&FindingComparisonRequest {
            change: FindingChange::NotDetected,
            ..FindingComparisonRequest::default()
        })
        .unwrap();
    assert_eq!(page.total, 1);
    assert_eq!(page.rows[0][1], Cell::Text("description-missing".into()));
}

#[test]
fn changed_html_eligibility_and_failed_or_unknown_observations_are_unverified() {
    let dir = tempfile::tempdir().unwrap();
    let library = CrawlLibrary::new(dir.path());
    let baseline = create(
        &library,
        &format!(
            "{}{}{}{}",
            add("title-missing", "https://example.com/same"),
            add("title-missing", "https://example.com/edited"),
            add("title-missing", "https://example.com/old"),
            add("legacy-'unknown", "https://example.com/literal?quote='&%_")
        ),
    );
    let current = create(
        &library,
        "UPDATE urls SET status=404 WHERE id=0; UPDATE urls SET indexability='noindex',noindex=true WHERE id=1; INSERT INTO rendering(page,rendered,error,elapsed_ms,truncated,raw_word_count,raw_link_count) VALUES (2,false,'timeout',1,false,0,0);",
    );
    let comparison = library.compare(&baseline.id, &current.id).unwrap();
    let summary = comparison.findings_summary(true).unwrap();
    assert_eq!(counts(&summary), [0, 0, 0, 0, 0, 4]);
    let page = comparison
        .findings_page(&FindingComparisonRequest::default())
        .unwrap();
    let reasons = page.rows.iter().map(|r| r[3].clone()).collect::<Vec<_>>();
    for reason in [
        "after-not-html-200",
        "after-not-indexable",
        "after-rendering-unverified",
        "unknown-rule",
    ] {
        assert!(reasons.contains(&Cell::Text(reason.into())));
    }
}

#[test]
fn http_findings_compare_response_outcomes_even_for_non_html_resources() {
    let dir = tempfile::tempdir().unwrap();
    let library = CrawlLibrary::new(dir.path());
    let baseline = create(
        &library,
        &format!(
            "UPDATE urls SET status=404,content_kind='image',indexability='client-error' WHERE id=0; {}",
            add("internal-client-error", "https://example.com/same")
        ),
    );
    let current = create(
        &library,
        &format!(
            "UPDATE urls SET content_kind='image',indexability='non-html' WHERE id=0; UPDATE urls SET status=500,indexability='server-error' WHERE id=1; {}",
            add("internal-server-error", "https://example.com/edited")
        ),
    );
    let summary = library
        .compare(&baseline.id, &current.id)
        .unwrap()
        .findings_summary(true)
        .unwrap();
    assert_eq!(counts(&summary), [1, 1, 0, 0, 0, 0]);
}

#[test]
fn partial_or_changed_crawl_plans_keep_presence_but_do_not_infer_comparable_absence() {
    let dir = tempfile::tempdir().unwrap();
    let library = CrawlLibrary::new(dir.path());
    let baseline = create(
        &library,
        &format!(
            "{}{}",
            add("title-missing", "https://example.com/same"),
            add("description-missing", "https://example.com/edited")
        ),
    );
    let mut current = create(
        &library,
        &add("description-missing", "https://example.com/edited"),
    );
    current.status = CrawlStatus::Stopped;
    current.limit_reached = true;
    crate::library::write_meta(
        &library.path().join(&current.id).join("crawl.json"),
        &current,
    )
    .unwrap();
    let summary = library
        .compare(&baseline.id, &current.id)
        .unwrap()
        .findings_summary(true)
        .unwrap();
    assert_eq!(counts(&summary), [0, 0, 1, 0, 0, 1]);
    current.status = CrawlStatus::Completed;
    current.limit_reached = false;
    current.config.follow.sitemaps = !current.config.follow.sitemaps;
    crate::library::write_meta(
        &library.path().join(&current.id).join("crawl.json"),
        &current,
    )
    .unwrap();
    let summary = library
        .compare(&baseline.id, &current.id)
        .unwrap()
        .findings_summary(true)
        .unwrap();
    assert_eq!(counts(&summary), [0, 0, 1, 0, 0, 1]);
}

#[test]
fn preserves_non_url_findings_unknown_rules_and_literal_inputs_without_source_writes() {
    let dir = tempfile::tempdir().unwrap();
    let library = CrawlLibrary::new(dir.path());
    let url = "https://example.com/literal?quote='&%_";
    let baseline = create(
        &library,
        &format!(
            "{}{}{}",
            add("sitemap-missing", "https://example.com/sitemap.xml"),
            add("llms-txt-missing", "https://example.com/llms.txt"),
            add("unknown-rule", url)
        ),
    );
    let current = create(
        &library,
        &format!(
            "UPDATE urls SET internal=false WHERE id=3; {}{}",
            add("sitemap-missing", "https://example.com/sitemap.xml"),
            add("unknown-rule", url)
        ),
    );
    let database = library.path().join(&baseline.id).join("crawl.duckdb");
    let before = std::fs::read(&database).unwrap();
    let comparison = library.compare(&baseline.id, &current.id).unwrap();
    assert_eq!(
        counts(&comparison.findings_summary(false).unwrap()),
        [0, 0, 2, 0, 0, 1]
    );
    let scope = comparison
        .findings_page(&FindingComparisonRequest {
            internal_only: false,
            search: "quote='&%_".into(),
            ..FindingComparisonRequest::default()
        })
        .unwrap();
    assert_eq!(scope.total, 1);
    assert_eq!(
        comparison
            .findings_page(&FindingComparisonRequest {
                search: "' OR true --".into(),
                internal_only: false,
                ..FindingComparisonRequest::default()
            })
            .unwrap()
            .total,
        0
    );
    assert_eq!(
        comparison
            .findings_page(&FindingComparisonRequest {
                issue_id: "' OR true --".into(),
                ..FindingComparisonRequest::default()
            })
            .unwrap()
            .total,
        0
    );
    assert!(
        comparison
            .findings_page(&FindingComparisonRequest {
                priority: "high' OR true".into(),
                ..FindingComparisonRequest::default()
            })
            .is_err()
    );
    assert_eq!(
        counts(&comparison.findings_summary(true).unwrap()),
        [0, 0, 0, 0, 1, 0]
    );
    drop(comparison);
    assert_eq!(std::fs::read(&database).unwrap(), before);
}

#[test]
fn streams_all_1505_finding_pairs_beyond_page_boundaries_and_caps_rule_previews() {
    let dir = tempfile::tempdir().unwrap();
    let library = CrawlLibrary::new(dir.path());
    let baseline = create(&library, "");
    let current = create(
        &library,
        "INSERT INTO audit_findings SELECT 'historic-'||lpad(i::VARCHAR,4,'0'),'https://example.com/same' FROM range(1505) t(i);",
    );
    let comparison = library.compare(&baseline.id, &current.id).unwrap();
    let request = FindingComparisonRequest {
        change: FindingChange::Unverified,
        priority: "unknown".into(),
        offset: 1250,
        limit: 250,
        ..FindingComparisonRequest::default()
    };
    let page = comparison.findings_page(&request).unwrap();
    assert_eq!((page.total, page.rows.len()), (1505, 250));
    let mut output = TableBuffer::default();
    assert_eq!(
        comparison.export_findings(&request, &mut output).unwrap(),
        1505
    );
    assert_eq!(output.rows[1250], page.rows[0]);
    let summary = comparison.findings_summary(true).unwrap();
    assert_eq!(
        (
            summary.rule_total,
            summary.rules.len(),
            summary.counts.unverified
        ),
        (1505, 500, 1505)
    );
}

#[test]
fn required_js_and_unknown_original_robots_observations_do_not_become_passes() {
    let dir = tempfile::tempdir().unwrap();
    let library = CrawlLibrary::new(dir.path());
    let mut baseline = create(
        &library,
        &format!(
            "{}{}",
            add("title-missing", "https://example.com/same"),
            add("render-noindex-removed", "https://example.com/edited")
        ),
    );
    let mut current = create(
        &library,
        "INSERT INTO rendering(page,rendered,error,elapsed_ms,truncated,raw_word_count,raw_link_count) VALUES(1,true,NULL,1,false,0,0);",
    );
    for meta in [&mut baseline, &mut current] {
        meta.config.rendering.mode = octopuson_crawl::RenderMode::Javascript;
        crate::library::write_meta(&library.path().join(&meta.id).join("crawl.json"), meta)
            .unwrap();
    }
    let comparison = library.compare(&baseline.id, &current.id).unwrap();
    let summary = comparison.findings_summary(true).unwrap();
    assert_eq!(counts(&summary), [0, 0, 0, 0, 0, 2]);
    let rows = comparison
        .findings_page(&FindingComparisonRequest::default())
        .unwrap()
        .rows;
    assert!(
        rows.iter()
            .any(|r| r[3] == Cell::Text("after-rendering-unverified".into()))
    );
    assert!(
        rows.iter()
            .any(|r| r[3] == Cell::Text("after-audit-baseline-unrecorded".into()))
    );
}

#[test]
fn filtered_indicators_match_every_state_page_and_ignore_only_state_and_pagination() {
    let dir = tempfile::tempdir().unwrap();
    let library = CrawlLibrary::new(dir.path());
    let baseline = create(
        &library,
        &format!(
            "{}{}{}{}",
            add("title-missing", "https://example.com/same"),
            add("title-missing", "https://example.com/edited"),
            add("title-missing", "https://example.com/old"),
            add("description-missing", "https://example.com/edited")
        ),
    );
    let current = create(
        &library,
        &format!(
            "UPDATE urls SET url='https://example.com/new' WHERE id=2; {}{}{}",
            add("title-missing", "https://example.com/same"),
            add("title-missing", "https://example.com/new"),
            add("title-missing", "https://example.com/literal?quote='&%_")
        ),
    );
    let comparison = library.compare(&baseline.id, &current.id).unwrap();
    let mut request = FindingComparisonRequest {
        issue_id: "title-missing".into(),
        priority: "high".into(),
        search: "EXAMPLE.COM".into(),
        change: FindingChange::Unverified,
        offset: 9999,
        limit: 1,
        ..FindingComparisonRequest::default()
    };
    let summary = comparison.filtered_findings_summary(&request).unwrap();
    assert_eq!(counts(&summary), [1, 1, 1, 1, 1, 0]);
    assert_eq!(
        (summary.baseline, summary.current, summary.counts.total),
        (3, 3, 5)
    );
    assert_eq!(summary.rule_total, 2); // Picker still allows switching to another rule.
    for (search, priority, issue) in [
        ("EXAMPLE.COM", "high", "title-missing"),
        ("/edited", "high", "title-missing"),
        ("/new", "high", "title-missing"),
        ("quote='&%_", "high", "title-missing"),
        ("", "medium", "title-missing"),
        ("' OR true --", "high", "title-missing"),
        ("", "", "unknown' OR true --"),
    ] {
        request.search = search.into();
        request.priority = priority.into();
        request.issue_id = issue.into();
        let summary = comparison.filtered_findings_summary(&request).unwrap();
        let mut sum = 0;
        for (state, expected) in [
            FindingChange::Introduced,
            FindingChange::NotDetected,
            FindingChange::Persisting,
            FindingChange::NewUrl,
            FindingChange::MissingUrl,
            FindingChange::Unverified,
        ]
        .into_iter()
        .zip(counts(&summary))
        {
            let page = comparison
                .findings_page(&FindingComparisonRequest {
                    change: state,
                    offset: 0,
                    limit: 1000,
                    ..request.clone()
                })
                .unwrap();
            assert_eq!(
                page.total, expected,
                "{search}/{priority}/{issue}/{state:?}"
            );
            sum += expected;
        }
        let all = comparison
            .findings_page(&FindingComparisonRequest {
                change: FindingChange::All,
                offset: 0,
                limit: 1000,
                ..request.clone()
            })
            .unwrap();
        assert_eq!(summary.counts.total, sum);
        assert_eq!(all.total, sum);
        let presence = |index| {
            all.rows
                .iter()
                .filter(|row| row[index] == Cell::Boolean(true))
                .count() as u64
        };
        assert_eq!(
            (summary.baseline, summary.current),
            (presence(6), presence(7))
        );
    }
    request.priority = "bad' OR true".into();
    assert!(comparison.filtered_findings_summary(&request).is_err());
}

fn verify_hreflang_counts(comparison: &CrawlComparison, expected: [u64; 6]) {
    assert_eq!(
        counts(&comparison.findings_summary(true).unwrap()),
        expected
    );
    for (change, count) in [
        FindingChange::Introduced,
        FindingChange::NotDetected,
        FindingChange::Persisting,
        FindingChange::NewUrl,
        FindingChange::MissingUrl,
        FindingChange::Unverified,
    ]
    .into_iter()
    .zip(expected)
    {
        let request = FindingComparisonRequest {
            change,
            limit: 1,
            offset: 0,
            ..FindingComparisonRequest::default()
        };
        assert_eq!(comparison.findings_page(&request).unwrap().total, count);
        let mut output = TableBuffer::default();
        assert_eq!(
            comparison.export_findings(&request, &mut output).unwrap(),
            count
        );
        assert_eq!(output.rows.len() as u64, count);
        assert_eq!(output.columns.len(), 16);
    }
}

#[test]
fn hreflang_comparison_requires_header_coverage_on_both_sides_and_preserves_presence() {
    let dir = tempfile::tempdir().unwrap();
    let library = CrawlLibrary::new(dir.path());
    let baseline = create(
        &library,
        &format!(
            "UPDATE urls SET title_count=0; DELETE FROM crawl_info WHERE key='http_hreflang_observed'; {}{}",
            add("hreflang-invalid-code", "https://example.com/same"),
            add("hreflang-missing-return", "https://example.com/old")
        ),
    );
    let current = create(
        &library,
        &format!(
            "UPDATE urls SET title_count=0; {}{}{}",
            add("hreflang-invalid-code", "https://example.com/same"),
            add("hreflang-missing-self", "https://example.com/edited"),
            add("title-missing", "https://example.com/literal?quote='&%_")
        ),
    );
    let paths = [&baseline, &current]
        .into_iter()
        .flat_map(|meta| {
            [
                library.path().join(&meta.id).join("crawl.duckdb"),
                library.path().join(&meta.id).join("crawl.json"),
            ]
        })
        .collect::<Vec<_>>();
    let bytes = paths
        .iter()
        .map(|path| std::fs::read(path).unwrap())
        .collect::<Vec<_>>();
    for (before, after, prefix, expected) in [
        (&baseline, &current, "before", [1, 0, 1, 0, 0, 2]),
        (&current, &baseline, "after", [0, 1, 1, 0, 0, 2]),
    ] {
        let comparison = library.compare(&before.id, &after.id).unwrap();
        verify_hreflang_counts(&comparison, expected);
        let page = comparison
            .findings_page(&FindingComparisonRequest {
                change: FindingChange::Unverified,
                limit: 100,
                ..FindingComparisonRequest::default()
            })
            .unwrap();
        assert!(
            page.rows
                .iter()
                .all(|row| row[3] == Cell::Text(format!("{prefix}-hreflang-unrecorded")))
        );
    }
    for (path, before) in paths.iter().zip(bytes) {
        assert_eq!(std::fs::read(path).unwrap(), before);
    }
}

#[test]
fn captured_pdf_hreflang_changes_compare_without_html_or_javascript_requirements() {
    let dir = tempfile::tempdir().unwrap();
    let library = CrawlLibrary::new(dir.path());
    let mut baseline = create(
        &library,
        &format!(
            "UPDATE urls SET content_kind='pdf',truncated=true; {}{}",
            add("hreflang-invalid-code", "https://example.com/same"),
            add("hreflang-missing-return", "https://example.com/old")
        ),
    );
    let mut current = create(
        &library,
        &format!(
            "UPDATE urls SET content_kind='pdf',truncated=true; {}{}{}",
            add("hreflang-missing-self", "https://example.com/edited"),
            add("hreflang-missing-return", "https://example.com/old"),
            add("title-missing", "https://example.com/literal?quote='&%_")
        ),
    );
    for meta in [&mut baseline, &mut current] {
        meta.config.rendering.mode = octopuson_crawl::RenderMode::Javascript;
        crate::library::write_meta(&library.path().join(&meta.id).join("crawl.json"), meta)
            .unwrap();
    }
    let comparison = library.compare(&baseline.id, &current.id).unwrap();
    verify_hreflang_counts(&comparison, [1, 1, 1, 0, 0, 1]);
    verify_hreflang_counts(
        &library.compare(&current.id, &baseline.id).unwrap(),
        [1, 1, 1, 0, 0, 1],
    );
    let page = comparison
        .findings_page(&FindingComparisonRequest {
            issue_id: "title-missing".into(),
            ..FindingComparisonRequest::default()
        })
        .unwrap();
    assert_eq!(page.rows[0][3], Cell::Text("before-not-html-200".into()));
}

#[test]
fn hreflang_absence_remains_unknown_for_unparsed_non_indexable_or_failed_content() {
    let dir = tempfile::tempdir().unwrap();
    let library = CrawlLibrary::new(dir.path());
    let sql = ["same", "edited", "old", "literal?quote='&%_"]
        .into_iter()
        .map(|path| {
            add(
                "hreflang-invalid-code",
                &format!("https://example.com/{path}"),
            )
        })
        .collect::<String>();
    let baseline = create(&library, &format!("UPDATE urls SET title_count=0; {sql}"));
    let current = create(
        &library,
        "UPDATE urls SET title_count=0; UPDATE urls SET content_kind='image' WHERE id=0; UPDATE urls SET title_count=NULL WHERE id=1; UPDATE urls SET indexability='noindex',noindex=true WHERE id=2; INSERT INTO rendering(page,rendered,error,elapsed_ms,truncated,raw_word_count,raw_link_count) VALUES(3,false,'timeout',1,false,0,0);",
    );
    let comparison = library.compare(&baseline.id, &current.id).unwrap();
    verify_hreflang_counts(&comparison, [0, 0, 0, 0, 0, 4]);
    let page = comparison
        .findings_page(&FindingComparisonRequest {
            limit: 100,
            ..FindingComparisonRequest::default()
        })
        .unwrap();
    let reasons = page
        .rows
        .iter()
        .map(|row| row[3].to_string())
        .collect::<Vec<_>>();
    for expected in [
        "after-not-hreflang-200",
        "after-hreflang-html-unparsed",
        "after-not-indexable",
        "after-rendering-unverified",
    ] {
        assert!(reasons.contains(&expected.to_owned()), "{expected}");
    }
}
