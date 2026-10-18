use crate::{CrawlReader, LinkDirection, LinkScope, PageRequest, TableExportRequest};
use duckdb::Connection;
use octopuson_core::table::{Cell, TableBuffer};

fn reader() -> CrawlReader {
    let connection = Connection::open_in_memory().unwrap();
    connection.execute_batch("CREATE VIEW tab_internal AS SELECT 'https://example.com/'||lpad(i::VARCHAR,4,'0') AS address, CASE WHEN i%2=0 THEN 200 ELSE 404 END AS status_code, CASE WHEN i%3=0 THEN 'noindex' ELSE 'indexable' END AS indexability, CASE WHEN i%5=0 THEN NULL ELSE 'Title '||i END AS title FROM range(1505) t(i); CREATE TABLE audit_findings AS SELECT 'title-missing' AS issue_id,address AS url FROM tab_internal WHERE title IS NULL;").unwrap();
    connection.execute_batch("CREATE TABLE urls AS SELECT row_number() OVER() AS id,address AS url,true AS internal FROM tab_internal; CREATE TABLE links (source BIGINT,target_url VARCHAR,kind VARCHAR,internal BOOLEAN);").unwrap();
    CrawlReader::from_connection(connection)
}

fn link_reader() -> CrawlReader {
    let connection = Connection::open_in_memory().unwrap();
    connection.execute_batch(r"
CREATE TABLE urls AS SELECT i AS id,'https://example.com/source/'||lpad(i::VARCHAR,4,'0') AS url,200 AS status,true AS internal,NULL::VARCHAR AS error FROM range(1505) t(i)
UNION ALL SELECT 1505,'https://example.com/hub?quote=''&b=1',404,true,NULL
UNION ALL SELECT 1506,'https://example.com/target/200',200,true,NULL
UNION ALL SELECT 1507,'https://example.com/target/404',404,true,NULL;
CREATE TABLE links AS SELECT i AS source,1505 AS target,'https://example.com/hub?quote=''&b=1' AS target_url,'hyperlink' AS kind,'' AS text,false AS nofollow,false AS ugc,false AS sponsored,true AS internal,0 AS position FROM range(1505) t(i)
UNION ALL SELECT 1505,id,url,'hyperlink','',false,false,false,true,id FROM urls WHERE id>1505;
").unwrap();
    CrawlReader::from_connection(connection)
}

#[test]
fn paginates_and_exports_all_exact_incoming_links_beyond_the_inspector_limit() {
    let reader = link_reader();
    let request = PageRequest {
        tab: "links".into(),
        link_scope: Some(LinkScope {
            url: "https://example.com/hub?quote='&b=1".into(),
            direction: LinkDirection::Incoming,
        }),
        offset: 1250,
        limit: 250,
        sort: "source".into(),
        ..PageRequest::default()
    };
    let page = reader.page(&request).unwrap();
    assert_eq!((page.total, page.rows.len()), (1505, 250));
    assert_eq!(
        page.rows[0][0],
        Cell::Text("https://example.com/source/1250".into())
    );
    let mut exported = TableBuffer::default();
    let count = reader
        .export_selection(
            &TableExportRequest {
                request,
                columns: vec!["destination".into(), "source".into()],
            },
            &mut exported,
        )
        .unwrap();
    assert_eq!(count, 1505);
    assert_eq!(exported.rows[1250][1], page.rows[0][0]);
    assert!(
        exported
            .rows
            .iter()
            .all(|row| row[0] == Cell::Text("https://example.com/hub?quote='&b=1".into()))
    );
}

#[test]
fn scopes_link_direction_and_combines_counterpart_search_with_destination_status() {
    let reader = link_reader();
    let mut request = PageRequest {
        tab: "links".into(),
        link_scope: Some(LinkScope {
            url: "https://example.com/hub?quote='&b=1".into(),
            direction: LinkDirection::Outgoing,
        }),
        status_class: 4,
        search: "TARGET/".into(),
        ..PageRequest::default()
    };
    let page = reader.page(&request).unwrap();
    assert_eq!(page.total, 1);
    assert_eq!(
        page.rows[0][1],
        Cell::Text("https://example.com/target/404".into())
    );
    request.link_scope.as_mut().unwrap().direction = LinkDirection::Incoming;
    request.search = "SOURCE/1".into();
    assert_eq!(reader.page(&request).unwrap().total, 505);
    request.search = "hub".into();
    assert_eq!(reader.page(&request).unwrap().total, 0);
    request.link_scope = None;
    request.status_class = 0;
    assert_eq!(reader.page(&request).unwrap().total, 1507);
    request.search = "target/404".into();
    assert_eq!(reader.page(&request).unwrap().total, 1);
}

#[test]
fn validates_link_scopes_and_treats_injection_like_urls_literally() {
    let reader = link_reader();
    let mut request = PageRequest {
        tab: "links".into(),
        link_scope: Some(LinkScope {
            url: "' OR true --".into(),
            direction: LinkDirection::Incoming,
        }),
        ..PageRequest::default()
    };
    assert_eq!(reader.page(&request).unwrap().total, 0);
    request.link_scope.as_mut().unwrap().url.clear();
    assert!(reader.page(&request).is_err());
    request.link_scope.as_mut().unwrap().url = "https://example.com/".into();
    request.tab = "internal".into();
    assert!(reader.page(&request).is_err());
    assert!(
        serde_json::from_str::<PageRequest>(
            r#"{"tab":"links","link_scope":{"url":"https://example.com/","direction":"invalid"}}"#
        )
        .is_err()
    );
}

#[test]
fn streams_all_rows_in_display_order_and_column_order_without_pagination() {
    let reader = reader();
    let request = PageRequest {
        tab: "internal".into(),
        sort: "address".into(),
        descending: true,
        offset: 1000,
        limit: 1,
        ..PageRequest::default()
    };
    let page = reader.page(&request).unwrap();
    assert_eq!((page.total, page.rows.len()), (1505, 1));
    let mut exported = TableBuffer::default();
    let count = reader
        .export_selection(
            &TableExportRequest {
                request,
                columns: vec!["title".into(), "address".into()],
            },
            &mut exported,
        )
        .unwrap();
    assert_eq!(count, 1505);
    assert_eq!(
        exported
            .columns
            .iter()
            .map(|column| column.name.as_str())
            .collect::<Vec<_>>(),
        ["title", "address"]
    );
    assert_eq!(exported.rows[1000][1], page.rows[0][0]);
    assert_eq!(
        exported.rows.first().unwrap()[1],
        Cell::Text("https://example.com/1504".into())
    );
    assert_eq!(
        exported.rows.last().unwrap()[1],
        Cell::Text("https://example.com/0000".into())
    );
    assert_eq!(exported.rows.last().unwrap()[0], Cell::Null);
}

#[test]
fn uses_the_same_combined_filters_and_literals_for_display_and_export() {
    let reader = reader();
    let mut selection = TableExportRequest {
        request: PageRequest {
            tab: "internal".into(),
            search: "EXAMPLE.COM/0".into(),
            status_class: 2,
            indexability: "indexable".into(),
            issue_id: "title-missing".into(),
            sort: "address".into(),
            limit: 1000,
            ..PageRequest::default()
        },
        columns: vec!["address".into(), "title".into()],
    };
    let page = reader.page(&selection.request).unwrap();
    let mut exported = TableBuffer::default();
    let count = reader.export_selection(&selection, &mut exported).unwrap();
    let expected = (0..1000)
        .filter(|i| i % 2 == 0 && i % 3 != 0 && i % 5 == 0)
        .count();
    assert_eq!(usize::try_from(count).unwrap(), expected);
    assert_eq!(count, page.total);
    assert_eq!(
        exported.rows.iter().map(|row| &row[0]).collect::<Vec<_>>(),
        page.rows.iter().map(|row| &row[0]).collect::<Vec<_>>()
    );
    selection.request.search = "' OR true --".into();
    assert_eq!(
        reader
            .export_selection(&selection, &mut TableBuffer::default())
            .unwrap(),
        0
    );
}

#[test]
fn rejects_unknown_and_duplicate_columns_without_running_injected_sql() {
    let reader = reader();
    for columns in [
        vec!["address".into(), "address".into()],
        vec!["address\"; DROP VIEW tab_internal;--".into()],
    ] {
        let selection = TableExportRequest {
            request: PageRequest {
                tab: "internal".into(),
                ..PageRequest::default()
            },
            columns,
        };
        assert!(
            reader
                .export_selection(&selection, &mut TableBuffer::default())
                .is_err()
        );
    }
    assert_eq!(
        reader
            .page(&PageRequest {
                tab: "internal".into(),
                limit: 1,
                ..PageRequest::default()
            })
            .unwrap()
            .total,
        1505
    );
}
