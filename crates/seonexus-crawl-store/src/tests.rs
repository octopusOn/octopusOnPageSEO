use std::collections::HashMap;

use seonexus_core::table::{Cell, TableBuffer};
use seonexus_crawl::{
    CrawlConfig, CrawlControl, CrawlPlan, Crawler, FetchError, FetchRequest, FetchResponse, Fetcher,
};

use super::*;

struct Site(HashMap<&'static str, FetchResponse>);

impl Fetcher for Site {
    fn fetch(&self, request: &FetchRequest<'_>) -> Result<FetchResponse, FetchError> {
        Ok(self
            .0
            .get(request.url)
            .cloned()
            .unwrap_or_else(|| FetchResponse::new(404, &[("Content-Type", "text/html")], "")))
    }
}

fn site() -> Site {
    let html = |body: &str| FetchResponse::new(200, &[("Content-Type", "text/html")], body);
    Site(HashMap::from([
        (
            "https://example.com/",
            html(
                r#"<html lang="tr"><head><title>Ana sayfa</title>
                <meta name="description" content="Açıklama">
                <link rel="alternate" hreflang="en" href="/en/">
                </head><body><h1>Merhaba</h1><h2>Bir</h2><h2>İki</h2>
                <script type="application/ld+json">{"@type":"Organization"}</script>
                <a href="/a">A</a><a href="/a">A again</a><a href="https://other.org/">O</a>
                <img src="/i.png"></body></html>"#,
            ),
        ),
        (
            "https://example.com/a",
            html("<title>A</title><a href='/'>home</a>"),
        ),
        ("https://example.com/en/", html("<title>EN</title>")),
        (
            "https://example.com/i.png",
            FetchResponse::new(200, &[("Content-Type", "image/png")], "png"),
        ),
        ("https://other.org/", html("")),
        (
            "https://example.com/sitemap.xml",
            FetchResponse::new(
                200,
                &[],
                "<urlset><url><loc>https://example.com/a</loc></url></urlset>",
            ),
        ),
    ]))
}

fn table(reader: &CrawlReader, tab: CrawlTab) -> TableBuffer {
    let mut buffer = TableBuffer::default();
    reader.tab(tab, &mut buffer).unwrap();
    buffer
}

fn redirect_fixture(library: &CrawlLibrary) -> CrawlMeta {
    use seonexus_crawl::{CrawlProgress, CrawlSink, CrawlSummary, UrlRecord};
    let plan = CrawlPlan::new(CrawlConfig {
        start_urls: vec!["https://example.com/start".into()],
        ..CrawlConfig::default()
    })
    .unwrap();
    let mut writer = library.create(&plan).unwrap();
    let base = "https://example.com/";
    let records = [
        ("start", 301, Some("middle")),
        ("middle", 302, Some("missing")),
        ("missing", 404, None),
        ("loop-a", 301, Some("loop-b")),
        ("loop-b", 308, Some("loop-a")),
        ("tail", 302, Some("loop-a")),
        ("unknown", 301, Some("not-fetched")),
        ("ok", 200, None),
    ];
    for (i, &(url, status, target)) in records.iter().enumerate() {
        let mut record = UrlRecord::new(
            u32::try_from(i).unwrap(),
            format!("{base}{url}"),
            Some(0),
            true,
        );
        record.status = Some(status);
        record.redirect_url = target.map(|target| format!("{base}{target}"));
        writer.url(&record, &[]).unwrap();
    }
    writer
        .finish(&CrawlSummary {
            progress: CrawlProgress {
                crawled: 8,
                ..CrawlProgress::default()
            },
            stopped: false,
            limit_reached: false,
        })
        .unwrap()
}

#[test]
fn upgrades_redirect_findings_through_the_final_target_without_changing_records() {
    let dir = tempfile::tempdir().unwrap();
    let library = CrawlLibrary::new(dir.path());
    let meta = redirect_fixture(&library);
    let base = "https://example.com/";
    let database = library.path().join(&meta.id).join("crawl.duckdb");
    let connection = crate::reader::connect(&database, false).unwrap();
    legacy_rendering_schema(&connection);
    // Recreate the previous rule and schema marker to exercise a saved-crawl upgrade.
    connection.execute_batch("DELETE FROM audit_findings WHERE issue_id='redirect-to-error'; INSERT INTO audit_findings SELECT 'redirect-to-error',s.url FROM urls s JOIN urls t ON s.redirect_url=t.url WHERE t.status>=400 OR t.error IS NOT NULL; DROP TABLE site_files;").unwrap();
    drop(connection);
    assert!(
        library.open(&meta.id).is_err(),
        "a failed audit is reported"
    );
    let connection = crate::reader::connect(&database, false).unwrap();
    let pending: String = connection
        .query_row(
            "SELECT value FROM crawl_info WHERE key='audit_required'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(
        pending, "1",
        "the completed migration must not suppress a retry"
    );
    let old_findings: u64 = connection
        .query_row(
            "SELECT count(*) FROM audit_findings WHERE issue_id='redirect-to-error'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(
        old_findings, 1,
        "failed evaluation preserves the previous findings"
    );
    connection.execute_batch("CREATE TABLE site_files (url VARCHAR PRIMARY KEY, status USMALLINT, content_type VARCHAR, text VARCHAR, error VARCHAR);").unwrap();
    drop(connection);
    let reader = library.open(&meta.id).unwrap();
    let findings = |rule: &str| {
        reader
            .page(&PageRequest {
                tab: "internal".into(),
                issue_id: rule.into(),
                sort: "address".into(),
                limit: 100,
                ..PageRequest::default()
            })
            .unwrap()
    };
    let errors = findings("redirect-to-error");
    assert_eq!(errors.total, 2);
    assert_eq!(
        errors
            .rows
            .iter()
            .map(|row| row[0].clone())
            .collect::<Vec<_>>(),
        [
            Cell::Text(format!("{base}middle")),
            Cell::Text(format!("{base}start"))
        ]
    );
    assert_eq!(findings("redirect-loop").total, 3);
    assert_eq!(reader.count(CrawlTab::Internal).unwrap(), 8);
    assert_eq!(
        reader
            .redirect_trace(&format!("{base}start"))
            .unwrap()
            .outcome,
        RedirectOutcome::HttpError
    );
    assert_eq!(library.list().unwrap()[0], meta);
    let version: i64 = reader
        .connection
        .query_row("SELECT max(version) FROM schema_migrations", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(version, crate::schema::latest_version());
    let pending: String = reader
        .connection
        .query_row(
            "SELECT value FROM crawl_info WHERE key='audit_required'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(pending, "0");
}

fn legacy_rendering_schema(connection: &duckdb::Connection) {
    connection.execute_batch("CREATE OR REPLACE VIEW tab_rendering AS SELECT u.url AS address,u.status AS status_code,r.rendered,r.rendered_url,r.error AS render_error,r.elapsed_ms AS render_time_ms,r.truncated AS render_truncated,r.raw_title,u.title AS rendered_title,r.raw_word_count,u.word_count AS rendered_word_count,r.raw_link_count,r.rendered_link_count FROM rendering r JOIN urls u ON u.id=r.page ORDER BY u.id;
        ALTER TABLE rendering DROP COLUMN raw_meta_description;
        ALTER TABLE rendering DROP COLUMN raw_canonical;
        ALTER TABLE rendering DROP COLUMN raw_noindex;
        ALTER TABLE rendering DROP COLUMN raw_nofollow;
        DROP VIEW tab_content_matches;
        DROP TABLE content_matches;
        DELETE FROM schema_migrations WHERE version>=5;
        INSERT INTO rendering VALUES (7,true,'https://example.com/ok',NULL,15,false,'Legacy title',12,3,5,NULL);").unwrap();
}

#[test]
fn legacy_rendering_keeps_unknown_metadata_distinct_from_false() {
    let dir = tempfile::tempdir().unwrap();
    let library = CrawlLibrary::new(dir.path());
    let meta = redirect_fixture(&library);
    let connection =
        crate::reader::connect(&library.path().join(&meta.id).join("crawl.duckdb"), false).unwrap();
    legacy_rendering_schema(&connection);
    drop(connection);
    let reader = library.open(&meta.id).unwrap();
    let rendering = table(&reader, CrawlTab::Rendering);
    assert_eq!(
        column(&rendering, "raw_title"),
        [&Cell::Text("Legacy title".into())]
    );
    assert_eq!(column(&rendering, "raw_noindex"), [&Cell::Null]);
    assert_eq!(column(&rendering, "raw_meta_description"), [&Cell::Null]);
    assert!(
        !reader
            .overview()
            .unwrap()
            .issues
            .iter()
            .any(|issue| issue.id == "render-noindex-removed")
    );
}

fn column<'a>(table: &'a TableBuffer, name: &str) -> Vec<&'a Cell> {
    let index = table.columns.iter().position(|c| c.name == name).unwrap();
    table.rows.iter().map(|row| &row[index]).collect()
}

#[test]
fn stores_a_crawl_and_reads_its_tabs() {
    let dir = tempfile::tempdir().unwrap();
    let library = CrawlLibrary::new(dir.path());
    let config = CrawlConfig {
        start_urls: vec![
            "https://www.example.com/".into(),
            "https://example.com/".into(),
        ],
        ..CrawlConfig::default()
    };
    let plan = CrawlPlan::new(config).unwrap();
    let mut writer = library.create(&plan).unwrap();
    assert_eq!(writer.meta().name, "example.com");
    let summary = Crawler::new(&plan, &site())
        .run(&mut writer, &CrawlControl::new())
        .unwrap();
    let live = writer.reader().unwrap();
    assert_eq!(
        live.count(CrawlTab::Internal).unwrap(),
        0,
        "records are buffered"
    );
    writer.flush().unwrap();
    assert!(live.count(CrawlTab::Internal).unwrap() > 0);
    drop(live);
    let meta = writer.finish(&summary).unwrap();
    assert_eq!(meta.status, CrawlStatus::Completed);

    let listed = library.list().unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].progress.crawled, summary.progress.crawled);

    let reader = library.open(&meta.id).unwrap();
    let internal = table(&reader, CrawlTab::Internal);
    let addresses = column(&internal, "address");
    assert!(addresses.contains(&&Cell::Text("https://example.com/".into())));
    let home = internal
        .rows
        .iter()
        .find(|row| row[0] == Cell::Text("https://example.com/".into()))
        .unwrap();
    let get = |name: &str| {
        let index = internal
            .columns
            .iter()
            .position(|c| c.name == name)
            .unwrap();
        home[index].clone()
    };
    assert_eq!(get("title"), Cell::Text("Ana sayfa".into()));
    assert_eq!(get("title_length"), Cell::Integer(9));
    assert_eq!(get("h2_count"), Cell::Integer(2));
    assert_eq!(get("indexability"), Cell::Text("indexable".into()));
    assert_eq!(get("outlinks"), Cell::Integer(2));
    assert_eq!(get("unique_outlinks"), Cell::Integer(1));
    assert_eq!(get("external_outlinks"), Cell::Integer(1));
    assert_eq!(get("crawl_depth"), Cell::Integer(0));
    assert!(matches!(get("crawled_at"), Cell::Timestamp(_)));

    assert_eq!(reader.count(CrawlTab::External).unwrap(), 1);
    let images = table(&reader, CrawlTab::Images);
    assert_eq!(column(&images, "missing_alt"), [&Cell::Boolean(true)]);
    assert_eq!(column(&images, "status_code"), [&Cell::Integer(200)]);
    assert_eq!(reader.count(CrawlTab::Headings).unwrap(), 3);
    assert_eq!(reader.count(CrawlTab::Hreflang).unwrap(), 1);
    let structured = table(&reader, CrawlTab::StructuredData);
    assert_eq!(
        column(&structured, "types"),
        [&Cell::Text("Organization".into())]
    );
    assert_eq!(
        reader.count(CrawlTab::Robots).unwrap(),
        2,
        "one per start origin"
    );
    assert_eq!(
        reader.count(CrawlTab::Sitemaps).unwrap(),
        2,
        "/sitemap.xml of each origin"
    );
    assert_eq!(reader.count(CrawlTab::SitemapUrls).unwrap(), 1);
    assert!(reader.count(CrawlTab::Links).unwrap() >= 5);

    let mut custom = TableBuffer::default();
    reader
        .query(
            "SELECT count(*) AS n FROM urls WHERE status = 404",
            &mut custom,
        )
        .unwrap();
    assert_eq!(custom.rows, [vec![Cell::Integer(1)]]);

    assert_interactive_views(&reader);

    drop(reader);
    assert_legacy_upgrade(&library, &meta.id);
    library.delete(&meta.id).unwrap();
    assert_eq!(library.list().unwrap().len(), 0);
    assert!(matches!(
        library.open("../etc"),
        Err(CrawlStoreError::NotFound(_))
    ));
}

#[test]
fn every_tab_name_round_trips() {
    for tab in CrawlTab::ALL {
        assert_eq!(CrawlTab::from_name(tab.name()), Some(tab));
    }
}

fn assert_legacy_upgrade(library: &CrawlLibrary, id: &str) {
    let path = library.path().join(id).join("crawl.duckdb");
    let connection = crate::reader::connect(&path, false).unwrap();
    connection.execute_batch("DROP VIEW tab_internal; ALTER VIEW tab_internal_base RENAME TO tab_internal; DROP VIEW tab_rendering; DROP TABLE rendering; DROP VIEW tab_issues; DROP TABLE audit_findings; DROP TABLE site_files; DROP VIEW tab_content_matches; DROP TABLE content_matches; DELETE FROM schema_migrations WHERE version>1;").unwrap();
    drop(connection);
    let upgraded = library.open(id).unwrap();
    assert!(upgraded.count(CrawlTab::Issues).unwrap() > 0);
    assert_eq!(upgraded.overview().unwrap().indexable, 3);
}

#[test]
fn concurrent_crawls_reserve_distinct_directories() {
    let dir = tempfile::tempdir().unwrap();
    let library = CrawlLibrary::new(dir.path());
    let plan = CrawlPlan::new(CrawlConfig {
        start_urls: vec!["https://example.com/".into()],
        ..CrawlConfig::default()
    })
    .unwrap();
    let ids = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..3)
            .map(|_| {
                scope.spawn(|| {
                    let writer = library.create(&plan).unwrap();
                    writer.meta().id.clone()
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .collect::<std::collections::HashSet<_>>()
    });
    assert_eq!(ids.len(), 3);
}

fn assert_interactive_views(reader: &CrawlReader) {
    let overview = reader.overview().unwrap();
    assert_eq!(
        overview.indexable, 3,
        "images and external HTML are not page totals"
    );
    assert_eq!(
        overview
            .issues
            .iter()
            .find(|issue| issue.id == "h1-missing")
            .unwrap()
            .count,
        2
    );
    assert!(
        !overview
            .issues
            .iter()
            .any(|issue| issue.id == "title-missing"),
        "404 responses must not trigger missing titles"
    );
    assert!(
        overview
            .issues
            .iter()
            .any(|issue| issue.id == "llms-txt-missing")
    );
    let filtered = reader
        .page(&PageRequest {
            tab: "internal".into(),
            issue_id: "h1-missing".into(),
            limit: 1,
            ..PageRequest::default()
        })
        .unwrap();
    assert_eq!((filtered.total, filtered.rows.len()), (2, 1));
    let malicious = reader
        .page(&PageRequest {
            tab: "internal".into(),
            search: "' OR true --".into(),
            sort: "address; DROP TABLE urls;--".into(),
            limit: 1000,
            ..PageRequest::default()
        })
        .unwrap();
    assert_eq!(malicious.total, 0);
    let detail = reader.detail("https://example.com/").unwrap();
    assert_eq!(detail.facts.rows.len(), 1);
    assert!(detail.findings.total > 0);
    let expected: u64 = reader
        .connection
        .query_row(
            "SELECT count(*) FROM audit_findings WHERE url='https://example.com/'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(detail.findings.total, expected);
    assert!(detail.outlinks.total >= 3);
    assert!(detail.html.is_none());
    assert_eq!(detail.headers.total, 0);
}

fn hreflang_site() -> Site {
    let html = |body: &str| FetchResponse::new(200, &[("Content-Type", "text/html")], body);
    Site(HashMap::from([
        (
            "https://example.com/unreturned",
            html("<link rel='alternate' hreflang='en' href='/target'>"),
        ),
        (
            "https://example.com/http-error",
            html("<link rel='alternate' hreflang='en' href='/broken'>"),
        ),
        (
            "https://example.com/redirected",
            html("<link rel='alternate' hreflang='en' href='/move'>"),
        ),
        (
            "https://example.com/non-html",
            html("<link rel='alternate' hreflang='en' href='/feed.xml'>"),
        ),
        (
            "https://example.com/external",
            html("<link rel='alternate' hreflang='en' href='https://other.org/en'>"),
        ),
        (
            "https://example.com/valid",
            html("<link rel='alternate' hreflang='en' href='/returned'>"),
        ),
        (
            "https://example.com/blocked",
            html("<link rel='alternate' hreflang='en' href='/private'>"),
        ),
        (
            "https://example.com/target",
            html("<title>No return annotation</title>"),
        ),
        (
            "https://example.com/returned",
            html("<link rel='alternate' hreflang='tr' href='/valid'>"),
        ),
        (
            "https://other.org/en",
            html("<link rel='alternate' hreflang='tr' href='https://example.com/external'>"),
        ),
        (
            "https://example.com/move",
            FetchResponse::new(301, &[("Location", "/target")], ""),
        ),
        (
            "https://example.com/feed.xml",
            FetchResponse::new(200, &[("Content-Type", "application/xml")], "<feed/>"),
        ),
        (
            "https://example.com/robots.txt",
            FetchResponse::new(
                200,
                &[("Content-Type", "text/plain")],
                "User-agent: *\nDisallow: /private\n",
            ),
        ),
    ]))
}

#[test]
fn hreflang_return_requires_an_analyzed_html_target() {
    let mut config = CrawlConfig {
        start_urls: [
            "unreturned",
            "http-error",
            "redirected",
            "non-html",
            "external",
            "valid",
            "blocked",
        ]
        .map(|path| format!("https://example.com/{path}"))
        .to_vec(),
        ..CrawlConfig::default()
    };
    config.follow.sitemaps = false;
    let plan = CrawlPlan::new(config).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let library = CrawlLibrary::new(dir.path());
    let mut writer = library.create(&plan).unwrap();
    let summary = Crawler::new(&plan, &hreflang_site())
        .run(&mut writer, &CrawlControl::new())
        .unwrap();
    let meta = writer.finish(&summary).unwrap();
    let reader = library.open(&meta.id).unwrap();
    let missing = reader
        .page(&PageRequest {
            tab: "internal".into(),
            issue_id: "hreflang-missing-return".into(),
            limit: 1000,
            ..PageRequest::default()
        })
        .unwrap();
    let address = missing
        .columns
        .iter()
        .position(|column| column.name == "address")
        .unwrap();
    assert_eq!(
        missing
            .rows
            .iter()
            .map(|row| row[address].clone())
            .collect::<Vec<_>>(),
        [Cell::Text("https://example.com/unreturned".into())],
        "a status check or an unusable response cannot prove a missing return annotation"
    );
    let status_errors = reader
        .page(&PageRequest {
            tab: "internal".into(),
            issue_id: "hreflang-to-non-200".into(),
            limit: 1000,
            ..PageRequest::default()
        })
        .unwrap();
    assert_eq!(
        status_errors.total, 2,
        "404 and redirect targets remain separate findings"
    );
}
