use super::*;
use duckdb::params;
use octopuson_core::table::{Cell, TableBuffer};
use octopuson_crawl::{
    ContentKind, CrawlConfig, CrawlPlan, CrawlSink, CrawlSummary, Indexability, UrlRecord,
};

const BASE: u64 = 0xffff_f123_4567_89ab;

fn fixture(library: &CrawlLibrary) -> CrawlMeta {
    let plan = CrawlPlan::new(CrawlConfig {
        start_urls: vec!["https://example.com/a?q='<script>".into()],
        ..CrawlConfig::default()
    })
    .unwrap();
    let mut writer = library.create(&plan).unwrap();
    for (id, name) in [
        "a?q='<script>",
        "b",
        "exact",
        "short",
        "long",
        "noindex",
        "external",
        "http-error",
        "non-html",
        "raw-truncated",
        "render-failed",
        "dom-truncated",
        "no-fingerprint",
        "exact-only-a",
        "exact-only-b",
    ]
    .into_iter()
    .enumerate()
    {
        let mut record = UrlRecord::new(
            u32::try_from(id).unwrap(),
            format!("https://example.com/{name}"),
            Some(0),
            name != "external",
        );
        record.status = Some(if name == "http-error" { 404 } else { 200 });
        record.content_kind = if name == "non-html" {
            ContentKind::Css
        } else {
            ContentKind::Html
        };
        record.indexability = if name == "noindex" {
            Indexability::Noindex
        } else {
            Indexability::Indexable
        };
        record.truncated = name == "raw-truncated";
        let mut facts = octopuson_html::parse_page(
            "<title>Example &lt;script&gt;</title><p>Body</p>",
            &url::Url::parse(&record.url).unwrap(),
            &["*"],
        );
        facts.word_count = if name == "short" {
            199
        } else if name == "long" {
            314
        } else if name == "b" {
            250
        } else {
            200
        };
        facts.text_hash = if name == "exact" {
            1
        } else if name.starts_with("exact-only") {
            900
        } else if name == "no-fingerprint" {
            0
        } else {
            u64::try_from(id).unwrap() + 1
        };
        facts.simhash = if name.starts_with("exact-only") {
            !BASE
        } else if name == "b" {
            BASE ^ 7
        } else {
            BASE
        };
        record.page = Some(facts);
        writer.url(&record, &[]).unwrap();
    }
    writer
        .finish(&CrawlSummary {
            progress: octopuson_crawl::CrawlProgress::default(),
            stopped: false,
            limit_reached: false,
        })
        .unwrap()
}

fn exclude_failed_captures(connection: &duckdb::Connection) {
    for (id, rendered, truncated) in [(10, false, false), (11, true, true)] {
        connection.execute("INSERT INTO rendering(page,rendered,elapsed_ms,truncated,raw_word_count,raw_link_count) VALUES (?,?,0,?,0,0)", params![id, rendered, truncated]).unwrap();
    }
}

#[test]
fn eligible_pages_get_observed_examples_and_ineligible_or_exact_only_pages_do_not() {
    let dir = tempfile::tempdir().unwrap();
    let library = CrawlLibrary::new(dir.path());
    let meta = fixture(&library);
    let connection =
        crate::reader::connect(&library.path().join(&meta.id).join("crawl.duckdb"), false).unwrap();
    exclude_failed_captures(&connection);
    crate::audit::run(&connection).unwrap();
    drop(connection);
    let reader = library.open(&meta.id).unwrap();
    let page = reader
        .page(&PageRequest {
            tab: "content-matches".into(),
            limit: 100,
            ..PageRequest::default()
        })
        .unwrap();
    assert_eq!(page.total, 3);
    let matching = reader
        .page(&PageRequest {
            tab: "internal".into(),
            issue_id: "content-near-duplicate".into(),
            limit: 100,
            ..PageRequest::default()
        })
        .unwrap();
    assert_eq!(matching.total, 3);
    let example = reader
        .content_match("https://example.com/a?q='<script>")
        .unwrap()
        .unwrap();
    assert_eq!(example.match_url, "https://example.com/b");
    assert_eq!(
        (example.distance, example.words, example.match_words),
        (3, 200, 250)
    );
    assert!(
        reader
            .content_match("https://example.com/noindex")
            .unwrap()
            .is_none()
    );
    assert!(
        reader
            .content_match("https://example.com/' OR 1=1 --")
            .unwrap()
            .is_none()
    );
    assert!(reader.content_match("").is_err());
    // Search matches either endpoint; filtered streaming export preserves projection.
    let request = PageRequest {
        tab: "content-matches".into(),
        search: "/b".into(),
        sort: "address".into(),
        limit: 1,
        ..PageRequest::default()
    };
    assert_eq!(reader.page(&request).unwrap().total, 3);
    let mut exported = TableBuffer::default();
    assert_eq!(
        reader
            .export_selection(
                &TableExportRequest {
                    request,
                    columns: vec![
                        "match_address".into(),
                        "hamming_distance".into(),
                        "address".into()
                    ]
                },
                &mut exported
            )
            .unwrap(),
        3
    );
    assert_eq!(exported.columns[0].name, "match_address");
    assert_eq!(exported.rows[0][1], Cell::Integer(3));
}

#[test]
fn old_crawls_gain_findings_without_changing_records_or_metadata() {
    let dir = tempfile::tempdir().unwrap();
    let library = CrawlLibrary::new(dir.path());
    let meta = fixture(&library);
    let folder = library.path().join(&meta.id);
    let before = std::fs::read(folder.join("crawl.json")).unwrap();
    let connection = crate::reader::connect(&folder.join("crawl.duckdb"), false).unwrap();
    exclude_failed_captures(&connection);
    connection.execute_batch("DELETE FROM audit_findings WHERE issue_id='content-near-duplicate'; DROP VIEW tab_content_matches; DROP TABLE content_matches; DELETE FROM schema_migrations WHERE version>=6;").unwrap();
    drop(connection);
    let reader = library.open(&meta.id).unwrap();
    assert_eq!(reader.count(CrawlTab::ContentMatches).unwrap(), 3);
    assert_eq!(reader.count(CrawlTab::ResponseCodes).unwrap(), 15);
    assert_eq!(std::fs::read(folder.join("crawl.json")).unwrap(), before);
    assert_eq!(
        reader
            .overview()
            .unwrap()
            .issues
            .iter()
            .find(|issue| issue.id == "content-near-duplicate")
            .unwrap()
            .count,
        3
    );
}

#[test]
fn failed_audit_rolls_back_both_examples_and_findings() {
    let dir = tempfile::tempdir().unwrap();
    let library = CrawlLibrary::new(dir.path());
    let meta = fixture(&library);
    let connection =
        crate::reader::connect(&library.path().join(&meta.id).join("crawl.duckdb"), false).unwrap();
    let before: u64 = connection
        .query_row("SELECT count(*) FROM content_matches", [], |row| row.get(0))
        .unwrap();
    let findings: u64 = connection
        .query_row("SELECT count(*) FROM audit_findings", [], |row| row.get(0))
        .unwrap();
    assert!(before > 0);
    connection
        .execute_batch("UPDATE urls SET indexability='noindex'; DROP TABLE site_files;")
        .unwrap();
    assert!(crate::audit::run(&connection).is_err());
    assert_eq!(
        connection
            .query_row("SELECT count(*) FROM content_matches", [], |row| row
                .get::<_, u64>(0))
            .unwrap(),
        before
    );
    assert_eq!(
        connection
            .query_row("SELECT count(*) FROM audit_findings", [], |row| row
                .get::<_, u64>(0))
            .unwrap(),
        findings
    );
}
