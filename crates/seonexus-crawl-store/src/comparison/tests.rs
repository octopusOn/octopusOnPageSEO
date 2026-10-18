use super::*;
use seonexus_core::table::{Cell, TableBuffer};
use seonexus_crawl::{CrawlConfig, CrawlPlan, CrawlProgress, CrawlSink, CrawlSummary, UrlRecord};

pub(super) fn archive(library: &CrawlLibrary, sql: &str) -> CrawlMeta {
    let plan = CrawlPlan::new(CrawlConfig {
        start_urls: vec!["https://example.com/".into()],
        ..CrawlConfig::default()
    })
    .unwrap();
    let mut writer = library.create(&plan).unwrap();
    for (i, path) in ["same", "edited", "old", "literal?quote='&%_"]
        .iter()
        .enumerate()
    {
        let mut record = UrlRecord::new(
            u32::try_from(i).unwrap(),
            format!("https://example.com/{path}"),
            Some(0),
            true,
        );
        record.status = Some(200);
        writer.url(&record, &[]).unwrap();
    }
    let meta = writer
        .finish(&CrawlSummary {
            progress: CrawlProgress::default(),
            stopped: false,
            limit_reached: false,
        })
        .unwrap();
    let database = library.path().join(&meta.id).join("crawl.duckdb");
    let connection = crate::reader::connect(&database, false).unwrap();
    connection.execute_batch(sql).unwrap();
    meta
}

fn pair(library: &CrawlLibrary) -> (CrawlMeta, CrawlMeta) {
    let baseline = archive(
        library,
        "UPDATE urls SET title='Before',text_hash=18446744073709551614 WHERE id=1; UPDATE urls SET internal=false WHERE id=3;",
    );
    let current = archive(
        library,
        "UPDATE urls SET title='After',status=404,text_hash=18446744073709551615 WHERE id=1; UPDATE urls SET url='https://example.com/new' WHERE id=2; UPDATE urls SET internal=false WHERE id=3; UPDATE urls SET response_ms=999,crawled_at_ms=999;",
    );
    (baseline, current)
}

#[test]
fn compares_membership_fields_and_preserves_exact_large_content_fingerprints() {
    let directory = tempfile::tempdir().unwrap();
    let library = CrawlLibrary::new(directory.path());
    let (baseline, current) = pair(&library);
    let comparison = library.compare(&baseline.id, &current.id).unwrap();
    let summary = comparison.summary(true).unwrap();
    assert_eq!(
        (
            summary.new,
            summary.missing,
            summary.changed,
            summary.unchanged
        ),
        (1, 1, 1, 1)
    );
    assert_eq!(
        summary
            .fields
            .iter()
            .find(|f| f.field == "title")
            .unwrap()
            .count,
        1
    );
    assert_eq!(comparison.summary(false).unwrap().unchanged, 2);
    let detail = comparison
        .detail("https://example.com/edited", true)
        .unwrap();
    let hash = detail
        .rows
        .iter()
        .find(|r| r[0] == Cell::Text("text_hash".into()))
        .unwrap();
    assert_eq!(
        &hash[1..4],
        &[
            Cell::Text("18446744073709551614".into()),
            Cell::Text("18446744073709551615".into()),
            Cell::Boolean(true)
        ]
    );
    assert_eq!(comparison.detail("' OR true --", false).unwrap().total, 0);
    let mut request = ComparisonRequest {
        field: "text_hash".into(),
        ..ComparisonRequest::default()
    };
    assert_eq!(comparison.page(&request).unwrap().total, 1);
    request.field = "title) OR true --".into();
    assert!(comparison.page(&request).is_err());
    assert_eq!(
        library
            .compare(&current.id, &baseline.id)
            .unwrap()
            .summary(true)
            .unwrap()
            .new,
        1
    );
}

#[test]
fn search_is_literal_and_missing_rows_do_not_claim_field_changes() {
    let directory = tempfile::tempdir().unwrap();
    let library = CrawlLibrary::new(directory.path());
    let (baseline, current) = pair(&library);
    let comparison = library.compare(&baseline.id, &current.id).unwrap();
    let request = ComparisonRequest {
        search: "quote='&%_".into(),
        internal_only: false,
        ..ComparisonRequest::default()
    };
    assert_eq!(comparison.page(&request).unwrap().total, 1);
    assert_eq!(
        comparison
            .page(&ComparisonRequest {
                search: "' OR true --".into(),
                ..request
            })
            .unwrap()
            .total,
        0
    );
    let missing = comparison.detail("https://example.com/old", true).unwrap();
    assert!(missing.rows.iter().all(|r| r[3] == Cell::Boolean(false)
        && r[4] == Cell::Boolean(true)
        && r[5] == Cell::Boolean(false)));
    assert_eq!(
        comparison
            .page(&ComparisonRequest {
                filter: ComparisonFilter::Missing,
                field: "title".into(),
                ..ComparisonRequest::default()
            })
            .unwrap()
            .total,
        0
    );
}

#[test]
fn rejects_running_same_invalid_and_pending_archives_without_mutating_sources() {
    let directory = tempfile::tempdir().unwrap();
    let library = CrawlLibrary::new(directory.path());
    let (baseline, current) = pair(&library);
    let source = library.path().join(&baseline.id).join("crawl.duckdb");
    let before = std::fs::read(&source).unwrap();
    assert!(library.compare(&baseline.id, &baseline.id).is_err());
    assert!(library.compare("../escape", &current.id).is_err());
    let mut changed = baseline.clone();
    changed.status = CrawlStatus::Running;
    crate::library::write_meta(
        &library.path().join(&baseline.id).join("crawl.json"),
        &changed,
    )
    .unwrap();
    assert!(library.compare(&baseline.id, &current.id).is_err());
    assert_eq!(std::fs::read(&source).unwrap(), before);
    crate::library::write_meta(
        &library.path().join(&baseline.id).join("crawl.json"),
        &baseline,
    )
    .unwrap();
    let connection = crate::reader::connect(&source, false).unwrap();
    connection
        .execute(
            "UPDATE crawl_info SET value='1' WHERE key='audit_required'",
            [],
        )
        .unwrap();
    drop(connection);
    let pending = std::fs::read(&source).unwrap();
    assert!(library.compare(&baseline.id, &current.id).is_err());
    assert_eq!(std::fs::read(&source).unwrap(), pending);
}

#[test]
fn streams_all_matches_beyond_page_limits_and_reports_partial_scope() {
    let directory = tempfile::tempdir().unwrap();
    let library = CrawlLibrary::new(directory.path());
    let baseline = archive(&library, "UPDATE urls SET title='Previous' WHERE id=1;");
    let mut current = archive(
        &library,
        "UPDATE urls SET title='Current',error='timeout',truncated=true WHERE id=1; INSERT INTO urls SELECT urls.* REPLACE ((id+100+i)::UINTEGER AS id,url||'/'||i AS url) FROM urls CROSS JOIN range(1505) t(i) WHERE id=0;",
    );
    current.status = CrawlStatus::Stopped;
    current.limit_reached = true;
    current.config.rendering.mode = seonexus_crawl::RenderMode::Javascript;
    crate::library::write_meta(
        &library.path().join(&current.id).join("crawl.json"),
        &current,
    )
    .unwrap();
    let before = std::fs::read(library.path().join(&current.id).join("crawl.duckdb")).unwrap();
    let comparison = library.compare(&baseline.id, &current.id).unwrap();
    let request = ComparisonRequest {
        filter: ComparisonFilter::New,
        offset: 1250,
        limit: 250,
        ..ComparisonRequest::default()
    };
    let page = comparison.page(&request).unwrap();
    assert_eq!((page.total, page.rows.len()), (1505, 250));
    let mut output = TableBuffer::default();
    assert_eq!(comparison.export(&request, &mut output).unwrap(), 1505);
    assert_eq!(output.rows[1250][0], page.rows[0][0]);
    assert_eq!(output.columns.len(), 65);
    let summary = comparison.summary(true).unwrap();
    for warning in ["partial", "settings-differ", "incomplete-content"] {
        assert!(summary.warnings.contains(&warning.to_owned()));
    }
    drop(comparison);
    assert_eq!(
        std::fs::read(library.path().join(&current.id).join("crawl.duckdb")).unwrap(),
        before
    );
}

#[test]
fn separates_scope_membership_from_null_empty_and_link_changes() {
    let directory = tempfile::tempdir().unwrap();
    let library = CrawlLibrary::new(directory.path());
    let baseline = archive(&library, "");
    let current = archive(
        &library,
        "UPDATE urls SET title='' WHERE id=0; UPDATE urls SET internal=false WHERE id=1; INSERT INTO links VALUES (0,2,'https://example.com/old','hyperlink','A',false,false,false,true,0);",
    );
    let comparison = library.compare(&baseline.id, &current.id).unwrap();
    let summary = comparison.summary(true).unwrap();
    assert_eq!(
        (
            summary.missing,
            summary.new,
            summary.changed,
            summary.unchanged
        ),
        (1, 0, 2, 1)
    );
    let all = comparison.summary(false).unwrap();
    assert_eq!((all.missing, all.changed, all.unchanged), (0, 2, 2));
    let detail = comparison.detail("https://example.com/same", true).unwrap();
    let title = detail
        .rows
        .iter()
        .find(|row| row[0] == Cell::Text("title".into()))
        .unwrap();
    assert_eq!(
        &title[1..4],
        &[Cell::Null, Cell::Text(String::new()), Cell::Boolean(true)]
    );
    assert_eq!(
        summary
            .fields
            .iter()
            .find(|f| f.field == "inlinks")
            .unwrap()
            .count,
        1
    );
    assert_eq!(
        summary
            .fields
            .iter()
            .find(|f| f.field == "outlinks")
            .unwrap()
            .count,
        1
    );
}

#[test]
fn reports_incompatible_scopes_and_rejects_ambiguous_url_duplicates() {
    let directory = tempfile::tempdir().unwrap();
    let library = CrawlLibrary::new(directory.path());
    let baseline = archive(&library, "");
    let mut current = archive(
        &library,
        "UPDATE urls SET url='https://another.example/'||id;",
    );
    current.start_urls = vec!["https://another.example/".into()];
    current.mode = seonexus_crawl::CrawlMode::List;
    crate::library::write_meta(
        &library.path().join(&current.id).join("crawl.json"),
        &current,
    )
    .unwrap();
    let comparison = library.compare(&baseline.id, &current.id).unwrap();
    let summary = comparison.summary(true).unwrap();
    for warning in ["no-shared-urls", "list-mode", "seeds-differ"] {
        assert!(summary.warnings.contains(&warning.to_owned()));
    }
    assert!(summary.fields.iter().all(|f| f.count == 0));
    drop(comparison);
    let database = library.path().join(&current.id).join("crawl.duckdb");
    let connection = crate::reader::connect(&database, false).unwrap();
    connection
        .execute_batch("UPDATE urls SET url='https://duplicate.example/';")
        .unwrap();
    drop(connection);
    assert!(library.compare(&baseline.id, &current.id).is_err());
}

#[test]
fn allows_simultaneous_readers_and_comparisons_of_the_same_archives() {
    let directory = tempfile::tempdir().unwrap();
    let library = CrawlLibrary::new(directory.path());
    let (baseline, current) = pair(&library);
    let first = library.open(&baseline.id).unwrap();
    let second = library.open(&current.id).unwrap();
    let comparison = library.compare(&baseline.id, &current.id).unwrap();
    let concurrent = library.compare(&baseline.id, &current.id).unwrap();
    assert_eq!(comparison.summary(true).unwrap().changed, 1);
    assert_eq!(
        concurrent
            .page(&ComparisonRequest::default())
            .unwrap()
            .total,
        4
    );
    assert_eq!(first.count(crate::CrawlTab::Internal).unwrap(), 3);
    assert_eq!(second.count(crate::CrawlTab::Internal).unwrap(), 3);
}

#[test]
fn filtered_url_indicators_match_state_pages_without_clearing_search_or_field() {
    let directory = tempfile::tempdir().unwrap();
    let library = CrawlLibrary::new(directory.path());
    let (baseline, current) = pair(&library);
    let comparison = library.compare(&baseline.id, &current.id).unwrap();
    for (search, field) in [
        ("", ""),
        ("EDITED", "title"),
        ("/new", ""),
        ("/new", "title"),
        ("' OR true --", ""),
        ("quote='&%_", ""),
    ] {
        let request = ComparisonRequest {
            search: search.into(),
            field: field.into(),
            filter: ComparisonFilter::Missing,
            offset: 9999,
            limit: 1,
            ..ComparisonRequest::default()
        };
        let summary = comparison.filtered_summary(&request).unwrap();
        let mut total = 0;
        for (filter, expected) in [
            (ComparisonFilter::Changed, summary.changed),
            (ComparisonFilter::Unchanged, summary.unchanged),
            (ComparisonFilter::New, summary.new),
            (ComparisonFilter::Missing, summary.missing),
        ] {
            assert_eq!(
                comparison
                    .page(&ComparisonRequest {
                        filter,
                        offset: 0,
                        ..request.clone()
                    })
                    .unwrap()
                    .total,
                expected,
                "{search}/{field}/{filter:?}"
            );
            total += expected;
        }
        assert_eq!(
            comparison
                .page(&ComparisonRequest {
                    filter: ComparisonFilter::All,
                    offset: 0,
                    ..request
                })
                .unwrap()
                .total,
            total
        );
        assert!(!summary.warnings.contains(&"no-shared-urls".to_string()));
    }
    assert!(
        comparison
            .filtered_summary(&ComparisonRequest {
                field: "title' OR true".into(),
                ..ComparisonRequest::default()
            })
            .is_err()
    );
}
