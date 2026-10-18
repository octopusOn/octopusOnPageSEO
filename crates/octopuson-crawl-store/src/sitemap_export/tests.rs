use super::*;
use duckdb::Connection;

fn reader(count: u64) -> CrawlReader {
    let mut connection = Connection::open_in_memory().unwrap();
    crate::schema::migrate(&mut connection).unwrap();
    connection.execute(&format!("INSERT INTO urls (id,url,depth,internal,crawled_at_ms,status,blocked_by_robots,content_kind,size_bytes,truncated,response_ms,hsts,noindex,nofollow,nosnippet,noarchive,noimageindex,indexability) SELECT i,'https://example.com/page/'||lpad(i::VARCHAR,6,'0'),0,true,0,200,false,'html',0,false,0,false,false,false,false,false,false,'indexable' FROM range({count}) t(i)"),[]).unwrap();
    CrawlReader::from_connection(connection)
}
fn selection() -> SitemapSelection {
    SitemapSelection {
        origin: "https://example.com".into(),
    }
}

#[test]
fn excludes_ineligible_observations_with_disjoint_reasons_and_bounded_preview() {
    let reader = reader(16);
    reader.connection.execute_batch("UPDATE urls SET internal=false WHERE id=0; UPDATE urls SET blocked_by_robots=true,status=NULL WHERE id=1; UPDATE urls SET status=301 WHERE id=2; UPDATE urls SET content_kind='image' WHERE id=3; UPDATE urls SET noindex=true WHERE id=4; INSERT INTO rendering(page,rendered,error,elapsed_ms,truncated,raw_word_count,raw_link_count) VALUES (5,false,'timeout',1,false,0,0); UPDATE urls SET truncated=true WHERE id=6; UPDATE urls SET canonical='https://example.com/other',indexability='canonicalised' WHERE id=7; UPDATE urls SET indexability='unavailable' WHERE id=8; UPDATE urls SET url='javascript:alert(1)' WHERE id=9; UPDATE urls SET url='https://example.com/duplicate' WHERE id IN (10,11); UPDATE urls SET url='https://other.example/page' WHERE id=13; UPDATE urls SET url='https://example.com/product?x=1&y=2',canonical='https://example.com/product?x=1&y=2',header_canonical='https://example.com/product?x=1&y=2' WHERE id=14; INSERT INTO rendering(page,rendered,elapsed_ms,truncated,raw_word_count,raw_link_count,raw_noindex) VALUES (15,true,1,false,0,0,true);").unwrap();
    let plan = reader.sitemap_plan(&selection(), false).unwrap();
    assert_eq!((plan.scanned, plan.included, plan.parts), (16, 2, 1));
    assert_eq!(plan.excluded.iter().map(|v| v.count).sum::<u64>(), 14);
    let count = |reason| {
        plan.excluded
            .iter()
            .find(|v| v.reason == reason)
            .map_or(0, |v| v.count)
    };
    assert_eq!(count(SitemapReason::Noindex), 2);
    assert_eq!(count(SitemapReason::Duplicate), 2);
    for reason in [
        SitemapReason::External,
        SitemapReason::Blocked,
        SitemapReason::Response,
        SitemapReason::NonHtml,
        SitemapReason::Rendering,
        SitemapReason::Truncated,
        SitemapReason::Canonical,
        SitemapReason::NonIndexable,
        SitemapReason::InvalidUrl,
        SitemapReason::OtherOrigin,
    ] {
        assert_eq!(count(reason), 1);
    }
    let mut urls = Vec::new();
    assert_eq!(
        reader
            .sitemap_urls(&selection(), false, |url| {
                urls.push(url.to_owned());
                Ok(())
            })
            .unwrap(),
        2
    );
    assert_eq!(urls, plan.sample);
    assert_eq!(
        plan.xml_bytes,
        (SITEMAP_HEADER.len()
            + SITEMAP_FOOTER.len()
            + urls
                .iter()
                .map(|url| sitemap_url_entry(url).len())
                .sum::<usize>()) as u64
    );
}

#[test]
fn javascript_requires_a_successful_capture_and_rejects_truncated_or_initial_noindex() {
    let reader = reader(5);
    reader.connection.execute_batch("INSERT INTO rendering(page,rendered,error,elapsed_ms,truncated,raw_word_count,raw_link_count,raw_noindex) VALUES (0,true,NULL,1,false,0,0,false),(1,false,'timeout',1,false,0,0,false),(2,true,NULL,1,true,0,0,false),(3,true,NULL,1,false,0,0,true);").unwrap();
    let plan = reader.sitemap_plan(&selection(), true).unwrap();
    assert_eq!(plan.included, 1);
    assert!(plan.sample[0].ends_with("000000"));
    assert_eq!(
        plan.excluded
            .iter()
            .find(|v| v.reason == SitemapReason::Rendering)
            .unwrap()
            .count,
        2
    );
    assert_eq!(
        reader.sitemap_plan(&selection(), false).unwrap().included,
        2
    );
}

#[test]
fn splits_above_fifty_thousand_and_does_not_limit_export_to_the_sample() {
    let reader = reader(50_001);
    let plan = reader.sitemap_plan(&selection(), false).unwrap();
    assert_eq!(
        (plan.included, plan.parts, plan.sample.len()),
        (50_001, 2, 100)
    );
    let mut exported = 0;
    reader
        .sitemap_urls(&selection(), false, |_| {
            exported += 1;
            Ok(())
        })
        .unwrap();
    assert_eq!(exported, 50_001);
    let envelope = (SITEMAP_HEADER.len() + SITEMAP_FOOTER.len()) as u64;
    let mut layout = SitemapLayout::default();
    layout.add(SITEMAP_MAX_BYTES - envelope);
    assert_eq!((layout.parts, layout.total_bytes), (1, SITEMAP_MAX_BYTES));
    layout.add(1);
    assert_eq!(
        (layout.parts, layout.total_bytes),
        (2, SITEMAP_MAX_BYTES + envelope + 1)
    );
}

#[test]
fn keeps_scheme_port_and_subdomain_separate_and_validates_literal_origin() {
    let reader = reader(4);
    reader.connection.execute_batch("UPDATE urls SET url='http://example.com/page' WHERE id=1; UPDATE urls SET url='https://example.com:8443/page' WHERE id=2; UPDATE urls SET url='https://sub.example.com/page' WHERE id=3;").unwrap();
    let plan = reader
        .sitemap_plan(&SitemapSelection::default(), false)
        .unwrap();
    assert_eq!(plan.origins.len(), 4);
    assert_eq!(plan.included, 1);
    assert_eq!(
        reader
            .sitemap_plan(
                &SitemapSelection {
                    origin: "https://empty.example".into()
                },
                false
            )
            .unwrap()
            .included,
        0
    );
    for origin in [
        "' OR true --",
        "https://example.com/",
        "https://example.com/path",
        "https://example.com?x=1",
        "https://user:pass@example.com",
        "ftp://example.com",
    ] {
        assert!(
            reader
                .sitemap_plan(
                    &SitemapSelection {
                        origin: origin.into()
                    },
                    false
                )
                .is_err()
        );
    }
}

#[test]
fn rejects_fragments_credentials_unnormalized_and_overlong_urls_and_escapes_all_xml_entities() {
    assert_eq!(sitemap_escape("&<>\"'"), "&amp;&lt;&gt;&quot;&apos;");
    assert!(valid_url("https://example.com/p?x=1&y=2").is_some());
    for raw in [
        "https://example.com/#x",
        "https://user:pass@example.com/",
        "HTTPS://EXAMPLE.COM/",
        " https://example.com/",
        "https://example.com/é",
        "https://example.com/%xy",
        "https://example.com/%1",
    ] {
        assert!(valid_url(raw).is_none());
    }
    let prefix = "https://example.com/";
    assert!(valid_url(&format!("{prefix}{}", "a".repeat(2047 - prefix.len()))).is_some());
    assert!(valid_url(&format!("{prefix}{}", "a".repeat(2048 - prefix.len()))).is_none());
    let reader = reader(0);
    let plan = reader
        .sitemap_plan(&SitemapSelection::default(), false)
        .unwrap();
    assert_eq!(
        (plan.scanned, plan.included, plan.parts, plan.xml_bytes),
        (0, 0, 0, 0)
    );
    assert!(plan.origins.is_empty());
}

#[test]
fn archive_reader_rejects_running_or_pending_analysis_without_writing_archives() {
    use crate::{CrawlLibrary, CrawlStatus};
    use octopuson_crawl::{CrawlConfig, CrawlPlan, CrawlProgress, CrawlSummary};
    let directory = tempfile::tempdir().unwrap();
    let library = CrawlLibrary::new(directory.path());
    let plan = CrawlPlan::new(CrawlConfig {
        start_urls: vec!["https://example.com/".into()],
        ..CrawlConfig::default()
    })
    .unwrap();
    let writer = library.create(&plan).unwrap();
    let meta = library.list().unwrap().pop().unwrap();
    assert!(library.open_archive(&meta.id).is_err());
    let completed = writer
        .finish(&CrawlSummary {
            progress: CrawlProgress::default(),
            stopped: false,
            limit_reached: false,
        })
        .unwrap();
    assert_eq!(completed.status, CrawlStatus::Completed);
    let database = library.path().join(&meta.id).join("crawl.duckdb");
    let before = std::fs::read(&database).unwrap();
    drop(library.open_archive(&meta.id).unwrap());
    assert_eq!(std::fs::read(&database).unwrap(), before);
    let connection = crate::reader::connect(&database, false).unwrap();
    connection
        .execute_batch("UPDATE crawl_info SET value='1' WHERE key='audit_required';")
        .unwrap();
    drop(connection);
    let pending = std::fs::read(&database).unwrap();
    assert!(library.open_archive(&meta.id).is_err());
    assert_eq!(std::fs::read(&database).unwrap(), pending);
}
