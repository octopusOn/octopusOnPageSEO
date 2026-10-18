use crate::{CrawlReader, DuplicateKind, DuplicateScope, PageRequest, TableExportRequest};
use duckdb::{Connection, params};
use octopuson_core::table::{Cell, TableBuffer};

fn reader() -> CrawlReader {
    let connection = Connection::open_in_memory().unwrap();
    connection.execute_batch("CREATE TABLE urls AS SELECT i::UINTEGER AS id,'https://example.com/'||lpad(i::VARCHAR,4,'0') AS url,i<>1507 AS internal,CASE WHEN i=1508 THEN 404 ELSE 200 END AS status,CASE WHEN i=1509 THEN 'css' ELSE 'html' END AS content_kind,CASE WHEN i=1506 THEN 'noindex' ELSE 'indexable' END AS indexability,CASE WHEN i=1505 THEN 'Other title' ELSE 'Title '' <script>' END AS title,CASE WHEN i%2=0 THEN 'Shared description' ELSE 'Other description' END AS meta_description,'Shared H1' AS h1,'' AS h2,200 AS word_count,CASE WHEN i=1505 THEN 18446744073709551614::UBIGINT ELSE 18446744073709551615::UBIGINT END AS text_hash FROM range(1510) t(i); CREATE VIEW tab_internal AS SELECT url AS address,status AS status_code,indexability,title,meta_description,h1,h2,word_count FROM urls; CREATE TABLE audit_findings AS SELECT 'title-duplicate' AS issue_id,url FROM urls WHERE id<1505;").unwrap();
    connection
        .execute_batch(
            "CREATE TABLE links (source BIGINT,target_url VARCHAR,kind VARCHAR,internal BOOLEAN);",
        )
        .unwrap();
    CrawlReader::from_connection(connection)
}

fn request(kind: DuplicateKind) -> PageRequest {
    PageRequest {
        tab: "internal".into(),
        duplicate_scope: Some(DuplicateScope {
            kind,
            url: "https://example.com/0000".into(),
        }),
        sort: "address".into(),
        limit: 250,
        offset: 1250,
        ..PageRequest::default()
    }
}

#[test]
fn summaries_and_full_group_pages_use_the_existing_audit_eligibility_and_exact_values() {
    let reader = reader();
    let groups = reader.duplicate_groups("https://example.com/0000").unwrap();
    assert_eq!(groups.len(), 4);
    assert_eq!(
        groups.iter().map(|group| group.count).collect::<Vec<_>>(),
        [1505, 753, 1506, 1505]
    );
    assert_eq!(groups[0].value.as_deref(), Some("Title ' <script>"));
    assert!(groups[3].value.is_none());
    for kind in [DuplicateKind::Title, DuplicateKind::Content] {
        let request = request(kind);
        let page = reader.page(&request).unwrap();
        assert_eq!((page.total, page.rows.len()), (1505, 250));
        assert_eq!(
            page.rows[0][0],
            Cell::Text("https://example.com/1250".into())
        );
        let mut out = TableBuffer::default();
        assert_eq!(
            reader
                .export_selection(
                    &TableExportRequest {
                        request,
                        columns: vec!["title".into(), "address".into()]
                    },
                    &mut out
                )
                .unwrap(),
            1505
        );
        assert_eq!(out.columns[0].name, "title");
        assert_eq!(out.rows[1250][1], page.rows[0][0]);
    }
}

#[test]
fn scopes_combine_search_status_issue_sorting_and_indexability_without_losing_the_group() {
    let reader = reader();
    let mut request = request(DuplicateKind::Title);
    request.search = "/1".into();
    request.status_class = 2;
    request.issue_id = "title-duplicate".into();
    request.offset = 0;
    request.limit = 1000;
    request.descending = true;
    let page = reader.page(&request).unwrap();
    assert_eq!(page.total, 505);
    assert_eq!(
        page.rows[0][0],
        Cell::Text("https://example.com/1504".into())
    );
    request.indexability = "noindex".into();
    assert_eq!(reader.page(&request).unwrap().total, 0);
    request.indexability.clear();
    request.status_class = 4;
    assert_eq!(reader.page(&request).unwrap().total, 0);
}

#[test]
fn source_and_field_values_are_literal_and_invalid_scopes_are_rejected() {
    let reader = reader();
    reader
        .connection
        .execute(
            "UPDATE urls SET url=? WHERE id=0",
            params!["https://example.com/?q=' OR true --"],
        )
        .unwrap();
    let url = "https://example.com/?q=' OR true --";
    assert_eq!(reader.duplicate_groups(url).unwrap()[0].count, 1505);
    let mut request = request(DuplicateKind::Title);
    request.duplicate_scope.as_mut().unwrap().url = url.into();
    assert_eq!(reader.page(&request).unwrap().total, 1505);
    request.duplicate_scope.as_mut().unwrap().url = "' OR true --".into();
    assert_eq!(reader.page(&request).unwrap().total, 0);
    assert!(reader.duplicate_groups("' OR true --").unwrap().is_empty());
    for source in [
        "https://example.com/1506",
        "https://example.com/1507",
        "https://example.com/1508",
        "https://example.com/1509",
    ] {
        assert!(reader.duplicate_groups(source).unwrap().is_empty());
        request.duplicate_scope.as_mut().unwrap().url = source.into();
        assert_eq!(reader.page(&request).unwrap().total, 0);
    }
    request.tab = "links".into();
    assert!(reader.page(&request).is_err());
    request.tab = "internal".into();
    request.duplicate_scope.as_mut().unwrap().url.clear();
    assert!(reader.page(&request).is_err());
    assert!(reader.duplicate_groups("").is_err());
    assert!(serde_json::from_str::<PageRequest>(r#"{"tab":"internal","duplicate_scope":{"kind":"title); DROP TABLE urls;--","url":"x"}}"#).is_err());
}

#[test]
fn preview_is_bounded_unicode_and_never_used_as_the_group_key() {
    let reader = reader();
    let prefix = "ş".repeat(512);
    let value = format!("{prefix}original");
    reader
        .connection
        .execute("UPDATE urls SET title=? WHERE id IN (0,1)", params![value])
        .unwrap();
    reader
        .connection
        .execute(
            "UPDATE urls SET title=? WHERE id=2",
            params![format!("{prefix}different")],
        )
        .unwrap();
    let groups = reader.duplicate_groups("https://example.com/0000").unwrap();
    assert_eq!((groups[0].count, groups[0].value_truncated), (2, true));
    assert_eq!(groups[0].value.as_deref(), Some(prefix.as_str()));
    assert_eq!(
        reader.page(&request(DuplicateKind::Title)).unwrap().total,
        2
    );
}
