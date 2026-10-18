use duckdb::Connection;
use seonexus_core::table::{Cell, TableBuffer};

use crate::{CrawlLibrary, CrawlMeta, CrawlReader, PageRequest, ResponseGroup, TableExportRequest};

fn status_archive(library: &CrawlLibrary) -> CrawlMeta {
    use seonexus_crawl::{
        CrawlConfig, CrawlPlan, CrawlProgress, CrawlSink, CrawlSummary, UrlRecord,
    };
    let plan = CrawlPlan::new(CrawlConfig {
        start_urls: vec!["https://example.com/".into()],
        ..CrawlConfig::default()
    })
    .unwrap();
    let mut writer = library.create(&plan).unwrap();
    let mut id = 0;
    for internal in [true, false] {
        let host = if internal {
            "example.com"
        } else {
            "other.example"
        };
        for status in [199, 200, 499, 500, 599, 600, 999] {
            let mut record =
                UrlRecord::new(id, format!("https://{host}/{status}"), Some(0), internal);
            record.status = Some(status);
            writer.url(&record, &[]).unwrap();
            id += 1;
        }
    }
    writer
        .finish(&CrawlSummary {
            progress: CrawlProgress {
                crawled: id,
                ..CrawlProgress::default()
            },
            stopped: false,
            limit_reached: false,
        })
        .unwrap()
}

fn assert_server_findings(reader: &CrawlReader) {
    let overview = reader.overview().unwrap();
    assert_eq!(
        overview
            .responses
            .iter()
            .find(|r| r.group == ResponseGroup::ServerError)
            .unwrap()
            .count,
        4
    );
    assert_eq!(
        overview
            .responses
            .iter()
            .find(|r| r.group == ResponseGroup::OtherHttp)
            .unwrap()
            .count,
        6
    );
    for (tab, rule, host) in [
        ("internal", "internal-server-error", "example.com"),
        ("external", "external-server-error", "other.example"),
    ] {
        let request = PageRequest {
            tab: tab.into(),
            issue_id: rule.into(),
            sort: "address".into(),
            limit: 1,
            ..PageRequest::default()
        };
        let page = reader.page(&request).unwrap();
        assert_eq!(
            page.total, 2,
            "only 500 through 599 are server errors: {rule}"
        );
        assert_eq!(
            overview
                .issues
                .iter()
                .find(|issue| issue.id == rule)
                .unwrap()
                .count,
            2
        );
        let mut exported = TableBuffer::default();
        assert_eq!(
            reader
                .export_selection(
                    &TableExportRequest {
                        request,
                        columns: vec!["address".into(), "status_code".into()]
                    },
                    &mut exported
                )
                .unwrap(),
            2
        );
        assert_eq!(
            exported.rows,
            vec![
                vec![
                    Cell::Text(format!("https://{host}/500")),
                    Cell::Integer(500)
                ],
                vec![
                    Cell::Text(format!("https://{host}/599")),
                    Cell::Integer(599)
                ],
            ]
        );
    }
}

#[test]
fn server_error_findings_match_5xx_response_counts_and_full_filtered_exports() {
    let dir = tempfile::tempdir().unwrap();
    let library = CrawlLibrary::new(dir.path());
    let meta = status_archive(&library);
    assert_server_findings(&library.open(&meta.id).unwrap());
}

#[test]
fn schema_seven_refreshes_old_server_findings_without_changing_observations_or_metadata() {
    let dir = tempfile::tempdir().unwrap();
    let library = CrawlLibrary::new(dir.path());
    let meta = status_archive(&library);
    let directory = library.path().join(&meta.id);
    let metadata = std::fs::read(directory.join("crawl.json")).unwrap();
    let connection = crate::reader::connect(&directory.join("crawl.duckdb"), false).unwrap();
    connection.execute_batch("DELETE FROM schema_migrations WHERE version>6; UPDATE crawl_info SET value='0' WHERE key='audit_required'; UPDATE urls SET indexability='server-error' WHERE status>=600; DELETE FROM audit_findings WHERE issue_id IN ('internal-server-error','external-server-error'); INSERT INTO audit_findings SELECT CASE WHEN internal THEN 'internal-server-error' ELSE 'external-server-error' END,url FROM urls WHERE status>=500;").unwrap();
    drop(connection);
    let snapshot = |reader: &CrawlReader| {
        let mut table = TableBuffer::default();
        reader
            .query(
                "SELECT * EXCLUDE(indexability) FROM urls ORDER BY id",
                &mut table,
            )
            .unwrap();
        table.rows
    };
    let before = snapshot(&CrawlReader::from_connection(
        crate::reader::connect(&directory.join("crawl.duckdb"), true).unwrap(),
    ));
    let reader = library.open(&meta.id).unwrap();
    assert_server_findings(&reader);
    let corrected: u64 = reader
        .connection
        .query_row(
            "SELECT count(*) FROM urls WHERE indexability='other-http'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(corrected, 6);
    assert_eq!(snapshot(&reader), before);
    assert_eq!(
        std::fs::read(directory.join("crawl.json")).unwrap(),
        metadata
    );
    assert_eq!(library.list().unwrap(), vec![meta]);
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

fn reader() -> CrawlReader {
    let connection = Connection::open_in_memory().unwrap();
    connection.execute_batch("CREATE TABLE urls (url VARCHAR, internal BOOLEAN, status USMALLINT, error VARCHAR, blocked_by_robots BOOLEAN, content_kind VARCHAR, indexability VARCHAR, response_ms UINTEGER,id UINTEGER DEFAULT 0,title_count USMALLINT DEFAULT 0,truncated BOOLEAN DEFAULT false,rel_next VARCHAR,rel_prev VARCHAR);
        INSERT INTO urls (url,internal,status,error,blocked_by_robots,content_kind,indexability,response_ms) VALUES
          ('https://example.com/200', true, 200, NULL, false, 'html', 'indexable', 10),
          ('https://external.example/299', false, 299, 'diagnostic', true, 'unknown', 'no-response', 20),
          ('https://example.com/301', true, 301, 'diagnostic', true, 'unknown', 'redirected', 30),
          ('https://example.com/399', true, 399, NULL, false, 'unknown', 'redirected', 40),
          ('https://example.com/400', true, 400, NULL, false, 'unknown', 'client-error', 50),
          ('https://example.com/499', true, 499, NULL, false, 'unknown', 'client-error', 60),
          ('https://example.com/500', true, 500, NULL, false, 'unknown', 'server-error', 70),
          ('https://example.com/599', true, 599, 'diagnostic', false, 'unknown', 'server-error', 80),
          ('https://example.com/199', true, 199, NULL, false, 'unknown', 'no-response', 90),
          ('https://example.com/600', true, 600, NULL, false, 'unknown', 'no-response', 100),
          ('https://example.com/blocked', true, NULL, 'also-error', true, 'unknown', 'blocked-by-robots', 0),
          ('https://example.com/timeout', true, NULL, 'timeout', false, 'unknown', 'no-response', 0),
          ('https://example.com/unknown', true, NULL, NULL, false, 'unknown', 'no-response', 0),
          ('https://external.example/unknown', false, NULL, NULL, false, 'unknown', 'no-response', 0);
        CREATE VIEW tab_response_codes AS SELECT url AS address, status AS status_code, error, blocked_by_robots, indexability FROM urls;
        CREATE TABLE audit_findings (issue_id VARCHAR, url VARCHAR); CREATE TABLE rendering(page UINTEGER,rendered BOOLEAN,truncated BOOLEAN); CREATE TABLE images(page UINTEGER); CREATE TABLE sitemaps(url VARCHAR); CREATE TABLE links(source UINTEGER,target UINTEGER,target_url VARCHAR,kind VARCHAR,text VARCHAR,internal BOOLEAN,nofollow BOOLEAN,position UINTEGER);").unwrap();
    // Overview scoring also reads the recorded canonical/hreflang context.
    connection.execute_batch("CREATE TABLE hreflang(page UINTEGER,lang VARCHAR,target VARCHAR); CREATE TABLE crawl_info(key VARCHAR,value VARCHAR); ALTER TABLE urls ADD COLUMN canonical VARCHAR; ALTER TABLE urls ADD COLUMN header_canonical VARCHAR; ALTER TABLE urls ADD COLUMN canonical_count USMALLINT; ALTER TABLE urls ADD COLUMN lang VARCHAR; ALTER TABLE urls ADD COLUMN text_hash UBIGINT; ALTER TABLE urls ADD COLUMN noindex BOOLEAN DEFAULT false;").unwrap();
    CrawlReader::from_connection(connection)
}

#[test]
fn response_counts_partition_every_url_and_match_filtered_tables_and_exports() {
    let reader = reader();
    let overview = reader.overview().unwrap();
    assert_eq!((overview.internal, overview.external), (12, 2));
    assert_eq!(
        overview.redirects, 1,
        "automatic navigation count differs from the complete 3xx family"
    );
    assert_eq!(
        overview.responses.iter().map(|row| row.count).sum::<u64>(),
        14
    );
    assert_eq!(
        overview
            .responses
            .iter()
            .map(|row| (row.group, row.count))
            .collect::<Vec<_>>(),
        [
            (ResponseGroup::Success, 2),
            (ResponseGroup::Redirects, 2),
            (ResponseGroup::ClientError, 2),
            (ResponseGroup::ServerError, 2),
            (ResponseGroup::OtherHttp, 2),
            (ResponseGroup::RobotsBlocked, 1),
            (ResponseGroup::RequestError, 1),
            (ResponseGroup::NoResponse, 2),
        ]
    );
    let mut seen = std::collections::HashSet::new();
    for response in overview.responses {
        let request = PageRequest {
            tab: "response-codes".into(),
            response_group: Some(response.group),
            limit: 1,
            offset: 1,
            sort: "address".into(),
            descending: true,
            ..PageRequest::default()
        };
        let page = reader.page(&request).unwrap();
        assert_eq!(page.total, response.count);
        let mut exported = TableBuffer::default();
        assert_eq!(
            reader
                .export_selection(
                    &TableExportRequest {
                        request,
                        columns: vec!["address".into(), "status_code".into()],
                    },
                    &mut exported
                )
                .unwrap(),
            response.count
        );
        if response.count > 1 {
            assert_eq!(page.rows[0][0], exported.rows[1][0]);
        }
        for row in exported.rows {
            assert!(
                seen.insert(row[0].to_string()),
                "no URL belongs to two groups"
            );
        }
    }
    assert_eq!(seen.len(), 14);
}

#[test]
fn response_filters_combine_with_literal_search_and_reject_invalid_requests() {
    let reader = reader();
    let mut request = PageRequest {
        tab: "response-codes".into(),
        response_group: Some(ResponseGroup::NoResponse),
        search: "EXTERNAL".into(),
        ..PageRequest::default()
    };
    let result = reader.page(&request).unwrap();
    assert_eq!(result.total, 1);
    assert_eq!(
        result.rows[0][0],
        Cell::Text("https://external.example/unknown".into())
    );
    request.search = "' OR true --".into();
    assert_eq!(reader.page(&request).unwrap().total, 0);
    request.tab = "internal".into();
    assert!(reader.page(&request).is_err());
    assert!(
        serde_json::from_str::<PageRequest>(
            r#"{"tab":"response-codes","response_group":"' OR true --"}"#
        )
        .is_err()
    );
}

#[test]
fn empty_crawl_has_zero_response_counts_without_inventing_successes() {
    let reader = reader();
    reader.connection.execute_batch("DELETE FROM urls").unwrap();
    let overview = reader.overview().unwrap();
    assert_eq!(overview.responses.len(), 8);
    assert!(overview.responses.iter().all(|row| row.count == 0));
    assert_eq!(overview.average_response_ms, 0.0);
}

#[test]
fn response_filter_export_streams_beyond_a_table_page() {
    let reader = reader();
    reader.connection.execute_batch("INSERT INTO urls (url,internal,status,error,blocked_by_robots,content_kind,indexability,response_ms) SELECT 'https://example.com/error/'||lpad(i::VARCHAR,4,'0'), true, NULL, 'timeout', false, 'unknown', 'no-response', 0 FROM range(1505) t(i)").unwrap();
    let request = PageRequest {
        tab: "response-codes".into(),
        response_group: Some(ResponseGroup::RequestError),
        search: "/error/".into(),
        offset: 1250,
        limit: 250,
        sort: "address".into(),
        ..PageRequest::default()
    };
    let page = reader.page(&request).unwrap();
    assert_eq!((page.total, page.rows.len()), (1505, 250));
    let mut exported = TableBuffer::default();
    assert_eq!(
        reader
            .export_selection(
                &TableExportRequest {
                    request,
                    columns: vec!["address".into(), "error".into()],
                },
                &mut exported
            )
            .unwrap(),
        1505
    );
    assert_eq!(exported.rows[1250][0], page.rows[0][0]);
}
