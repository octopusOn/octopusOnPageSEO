use duckdb::{Connection, params};
use seonexus_core::table::Cell;

use crate::CrawlReader;

fn reader() -> CrawlReader {
    let mut connection = Connection::open_in_memory().unwrap();
    crate::schema::migrate(&mut connection).unwrap();
    CrawlReader::from_connection(connection)
}

#[test]
fn findings_are_scoped_to_the_exact_url_including_quotes_and_search_metacharacters() {
    let reader = reader();
    let url = "https://example.com/a?quote='&literal=%_";
    for (issue, address) in [
        ("title-missing", url.to_owned()),
        ("render-error", url.to_owned()),
        ("description-missing", format!("{url}/child")),
        ("unknown-from-another-version", "' OR true --".into()),
    ] {
        reader
            .connection
            .execute(
                "INSERT INTO audit_findings VALUES (?,?)",
                params![issue, address],
            )
            .unwrap();
    }
    let detail = reader.detail(url).unwrap();
    assert_eq!(detail.findings.total, 2);
    assert_eq!(detail.findings.columns[0].name, "issue_id");
    assert_eq!(
        detail.findings.rows,
        [
            vec![Cell::Text("render-error".into())],
            vec![Cell::Text("title-missing".into())],
        ]
    );
    assert_eq!(reader.detail("' OR true --").unwrap().findings.total, 1);
    assert_eq!(reader.detail("' OR 1=1 --").unwrap().findings.total, 0);
    assert_eq!(
        reader
            .detail("https://example.com/a")
            .unwrap()
            .findings
            .total,
        0
    );
}

#[test]
fn unknown_urls_have_empty_findings_and_do_not_inherit_site_wide_results() {
    let reader = reader();
    reader
        .connection
        .execute(
            "INSERT INTO audit_findings VALUES ('title-missing','https://example.com/')",
            [],
        )
        .unwrap();
    let detail = reader.detail("https://unknown.example/").unwrap();
    assert_eq!(detail.facts.total, 0);
    assert_eq!(detail.findings.total, 0);
    assert_eq!(detail.findings.rows, Vec::<Vec<Cell>>::new());
    assert_eq!(reader.overview().unwrap().issues.len(), 1);
}

#[test]
fn historical_or_unknown_rules_are_preserved_with_a_bounded_preview_and_full_total() {
    let reader = reader();
    let url = "https://example.com/";
    reader.connection.execute("INSERT INTO audit_findings SELECT 'legacy-'||lpad(i::VARCHAR,4,'0'),? FROM range(1005) t(i)", params![url]).unwrap();
    let detail = reader.detail(url).unwrap();
    assert_eq!(detail.findings.total, 1005);
    assert_eq!(detail.findings.rows.len(), 1000);
    assert_eq!(
        detail.findings.rows.first().unwrap()[0],
        Cell::Text("legacy-0000".into())
    );
    assert_eq!(
        detail.findings.rows.last().unwrap()[0],
        Cell::Text("legacy-0999".into())
    );
}
