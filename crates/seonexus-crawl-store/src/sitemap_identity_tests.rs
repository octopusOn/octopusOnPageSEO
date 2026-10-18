use seonexus_core::table::{Cell, TableBuffer};
use seonexus_crawl::{
    CrawlConfig, CrawlPlan, CrawlProgress, CrawlSink, CrawlSummary, SitemapEntry, SitemapKind,
    SitemapRecord, UrlRecord,
};

use crate::{CrawlLibrary, CrawlMeta, CrawlTab};

fn archive(library: &CrawlLibrary) -> CrawlMeta {
    let mut config = CrawlConfig {
        start_urls: vec!["https://example.com/".into()],
        ..CrawlConfig::default()
    };
    config.urls.strip_params = vec!["utm_source".into()];
    let plan = CrawlPlan::new(config).unwrap();
    let mut writer = library.create(&plan).unwrap();
    let mut page = UrlRecord::new(0, "https://example.com/orphan".into(), None, true);
    page.status = Some(200);
    page.content_kind = seonexus_crawl::ContentKind::Html;
    page.indexability = seonexus_crawl::Indexability::Indexable;
    writer.url(&page, &[]).unwrap();
    writer
        .sitemap(&SitemapRecord {
            url: "https://example.com/map.xml".into(),
            status: Some(200),
            redirect_url: None,
            redirect_location: None,
            kind: Some(SitemapKind::UrlSet),
            error: None,
            entries: vec![SitemapEntry {
                url: "https://example.com/orphan?utm_source=qa#section".into(),
                lastmod: Some("2026-10-17".into()),
            }],
            children: vec![],
        })
        .unwrap();
    writer
        .finish(&CrawlSummary {
            progress: CrawlProgress {
                crawled: 1,
                ..CrawlProgress::default()
            },
            stopped: false,
            limit_reached: false,
        })
        .unwrap()
}

fn version_seven(library: &CrawlLibrary, id: &str) {
    let connection =
        crate::reader::connect(&library.path().join(id).join("crawl.duckdb"), false).unwrap();
    connection.execute_batch("DROP VIEW tab_sitemap_urls; ALTER TABLE sitemap_urls DROP COLUMN crawl_url; CREATE VIEW tab_sitemap_urls AS SELECT s.url,s.sitemap,s.lastmod,u.status AS status_code,u.indexability,u.depth AS crawl_depth FROM sitemap_urls s LEFT JOIN urls u ON u.url=s.url; DELETE FROM schema_migrations WHERE version>7; UPDATE crawl_info SET value='0' WHERE key='audit_required'; CHECKPOINT;").unwrap();
}

#[test]
fn prepared_version_seven_stays_byte_identical_when_opened_exported_and_compared() {
    let dir = tempfile::tempdir().unwrap();
    let library = CrawlLibrary::new(dir.path());
    let old = archive(&library);
    let current = archive(&library);
    version_seven(&library, &old.id);
    let database = library.path().join(&old.id).join("crawl.duckdb");
    let metadata = library.path().join(&old.id).join("crawl.json");
    let before_db = std::fs::read(&database).unwrap();
    let before_meta = std::fs::read(&metadata).unwrap();
    {
        let reader = library.open(&old.id).unwrap();
        let mut rows = TableBuffer::default();
        reader.tab(CrawlTab::SitemapUrls, &mut rows).unwrap();
        assert_eq!(rows.rows.len(), 1);
        assert!(!rows.columns.iter().any(|column| column.name == "crawl_url"));
        assert_eq!(rows.rows[0][3], Cell::Null);
        assert_eq!(
            library
                .open_archive(&old.id)
                .unwrap()
                .count(CrawlTab::SitemapUrls)
                .unwrap(),
            1
        );
        assert_eq!(
            library
                .compare(&old.id, &current.id)
                .unwrap()
                .summary(true)
                .unwrap()
                .unchanged,
            1
        );
    }
    assert_eq!(std::fs::read(database).unwrap(), before_db);
    assert_eq!(std::fs::read(metadata).unwrap(), before_meta);
}

#[test]
fn a_pending_legacy_audit_upgrades_without_guessing_sitemap_crawl_identities() {
    let dir = tempfile::tempdir().unwrap();
    let library = CrawlLibrary::new(dir.path());
    let meta = archive(&library);
    version_seven(&library, &meta.id);
    let database = library.path().join(&meta.id).join("crawl.duckdb");
    let connection = crate::reader::connect(&database, false).unwrap();
    connection
        .execute(
            "UPDATE crawl_info SET value='1' WHERE key='audit_required'",
            [],
        )
        .unwrap();
    drop(connection);
    let reader = library.open(&meta.id).unwrap();
    let version: i64 = reader
        .connection
        .query_row("SELECT max(version) FROM schema_migrations", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(version, crate::schema::latest_version());
    let mut rows = TableBuffer::default();
    reader.tab(CrawlTab::SitemapUrls, &mut rows).unwrap();
    assert_eq!(
        rows.rows[0][0],
        Cell::Text("https://example.com/orphan?utm_source=qa#section".into())
    );
    assert_eq!(rows.rows[0][1], Cell::Null);
    assert_eq!(rows.rows[0][4], Cell::Null);
}

#[test]
fn prepared_version_eight_sitemap_evidence_stays_byte_identical_with_version_nine() {
    let dir = tempfile::tempdir().unwrap();
    let library = CrawlLibrary::new(dir.path());
    let old = archive(&library);
    let current = archive(&library);
    let database = library.path().join(&old.id).join("crawl.duckdb");
    let metadata = library.path().join(&old.id).join("crawl.json");
    let connection = crate::reader::connect(&database, false).unwrap();
    connection.execute_batch("DROP VIEW tab_sitemaps; DROP TABLE sitemap_redirects; ALTER TABLE sitemaps DROP COLUMN redirect_url; ALTER TABLE sitemaps DROP COLUMN redirect_location; CREATE VIEW tab_sitemaps AS SELECT url,status AS status_code,kind,error,entries,children FROM sitemaps; DELETE FROM schema_migrations WHERE version>8; UPDATE crawl_info SET value='0' WHERE key='audit_required'; CHECKPOINT;").unwrap();
    drop(connection);
    let before_db = std::fs::read(&database).unwrap();
    let before_meta = std::fs::read(&metadata).unwrap();
    {
        let reader = library.open(&old.id).unwrap();
        let mut rows = TableBuffer::default();
        reader.tab(CrawlTab::Sitemaps, &mut rows).unwrap();
        assert_eq!(rows.rows.len(), 1);
        assert!(
            !rows
                .columns
                .iter()
                .any(|column| column.name == "redirect_url")
        );
        assert_eq!(
            library
                .open_archive(&old.id)
                .unwrap()
                .count(CrawlTab::SitemapUrls)
                .unwrap(),
            1
        );
        assert_eq!(
            library
                .compare(&old.id, &current.id)
                .unwrap()
                .summary(true)
                .unwrap()
                .unchanged,
            1
        );
    }
    assert_eq!(std::fs::read(database).unwrap(), before_db);
    assert_eq!(std::fs::read(metadata).unwrap(), before_meta);
}
