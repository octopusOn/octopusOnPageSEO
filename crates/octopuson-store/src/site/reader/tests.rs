use octopuson_core::Date;
use octopuson_core::table::{Cell, TableBuffer};
use octopuson_query::parse;
use octopuson_query::sql::{CompileContext, CompiledQuery, Page, Source, compile};

use super::*;
use crate::site::ImportOptions;
use crate::site::tests::{Fixture, import, range};

fn compiled(dsl: &str) -> CompiledQuery {
    let today = Date::from_ymd(2026, 10, 12).unwrap();
    compile(&parse(dsl).unwrap(), CompileContext { today }).unwrap()
}

fn reader(fixture: &Fixture) -> SiteReader {
    SiteReader::open(&fixture.data, SiteId::new(1), StoreSettings::default())
        .unwrap()
        .unwrap()
}

fn run(reader: &SiteReader, dsl: &str, page: Option<Page>) -> TableBuffer {
    let mut buffer = TableBuffer::default();
    reader.run(&compiled(dsl), page, &mut buffer).unwrap();
    buffer
}

/// Fixture with ids 0..100: 80 × 200 and 20 × 404, spanning 2026-10-07 and 2026-10-08.
fn imported() -> Fixture {
    let fixture = Fixture::new();
    let mut store = fixture.open();
    import(
        &mut store,
        "a.log",
        &[&range(0, 100)],
        ImportOptions::default(),
    );
    fixture
}

#[test]
fn open_returns_none_before_any_data() {
    let fixture = Fixture::new();
    let opened = SiteReader::open(&fixture.data, SiteId::new(1), StoreSettings::default());
    assert!(opened.unwrap().is_none());
}

#[test]
fn open_reports_busy_site_while_store_is_open() {
    let fixture = Fixture::new();
    let _store = fixture.open();
    let opened = SiteReader::open(&fixture.data, SiteId::new(1), StoreSettings::default());
    assert!(
        matches!(opened, Err(StoreError::SiteLocked(_))),
        "{opened:?}"
    );
}

#[test]
fn imports_are_refused_while_a_reader_is_open() {
    let fixture = imported();
    let reader = reader(&fixture);
    let opened = SiteStore::open(&fixture.data, SiteId::new(1), StoreSettings::default());
    assert!(matches!(opened, Err(StoreError::SiteLocked(_))));
    drop(reader);
    drop(fixture.open());
}

#[test]
fn raw_query_on_empty_site_returns_no_rows() {
    let fixture = Fixture::new();
    drop(fixture.open());
    let reader = reader(&fixture);
    let query = compiled("status:404");
    assert_eq!(query.source, Source::Hits);

    let mut buffer = TableBuffer::default();
    assert_eq!(reader.run(&query, None, &mut buffer).unwrap(), 0);
    assert_eq!(buffer.columns, query.columns);
    assert_eq!(buffer.rows.len(), 0);
}

#[test]
fn rollup_and_raw_hits_agree() {
    let fixture = imported();
    let reader = reader(&fixture);
    let expected = vec![
        vec![Cell::Integer(200), Cell::Integer(80)],
        vec![Cell::Integer(404), Cell::Integer(20)],
    ];

    assert_ne!(compiled("group:status").source, Source::Hits);
    assert_eq!(compiled("ip:66.249.66.1 group:status").source, Source::Hits);
    let sorted = "sort:status:asc";
    assert_eq!(
        run(&reader, &format!("group:status {sorted}"), None).rows,
        expected
    );
    assert_eq!(
        run(
            &reader,
            &format!("ip:66.249.66.1 group:status {sorted}"),
            None
        )
        .rows,
        expected
    );
}

#[test]
fn day_filter_matches_on_rollup_and_raw_hits() {
    let fixture = imported();
    let reader = reader(&fixture);
    let rollup = run(&reader, "date:2026-10-07 group:day", None);
    let raw = run(&reader, "date:2026-10-07 ip:66.249.66.1 group:day", None);

    assert_eq!(rollup.rows, raw.rows);
    assert_eq!(rollup.rows.len(), 1);
    let day = Date::from_ymd(2026, 10, 7).unwrap();
    assert_eq!(rollup.rows[0][0], Cell::Date(day));
}

#[test]
fn listing_returns_matching_rows_newest_first() {
    let fixture = imported();
    let reader = reader(&fixture);
    let listing = run(&reader, "status:404", None);

    assert_eq!(listing.rows.len(), 20);
    let status = listing
        .columns
        .iter()
        .position(|column| column.name == "status")
        .unwrap();
    assert!(
        listing
            .rows
            .iter()
            .all(|row| row[status] == Cell::Integer(404))
    );
    let timestamps: Vec<_> = listing
        .rows
        .iter()
        .map(|row| match row[0] {
            Cell::Timestamp(ts) => ts,
            ref other => panic!("expected a timestamp, got {other:?}"),
        })
        .collect();
    assert!(timestamps.windows(2).all(|pair| pair[0] >= pair[1]));
}

#[test]
fn pages_respect_offset_size_and_query_limit() {
    let fixture = imported();
    let reader = reader(&fixture);
    let all = run(&reader, "status:200", None);
    let page = run(
        &reader,
        "status:200",
        Some(Page {
            offset: 5,
            size: 10,
        }),
    );
    assert_eq!(page.rows, all.rows[5..15]);

    let limited = run(
        &reader,
        "status:200 limit:12",
        Some(Page {
            offset: 10,
            size: 10,
        }),
    );
    assert_eq!(limited.rows, all.rows[10..12]);
}

#[test]
fn invalid_regex_is_reported_as_database_error() {
    let fixture = imported();
    let reader = reader(&fixture);
    let mut buffer = TableBuffer::default();
    let outcome = reader.run(&compiled(r#"url:~"(""#), None, &mut buffer);
    assert!(matches!(outcome, Err(StoreError::Database(_))));
}

#[test]
fn reports_totals_and_import_history() {
    let fixture = imported();
    let reader = reader(&fixture);
    let totals = reader.totals().unwrap();
    assert_eq!((totals.imports, totals.hits, totals.urls), (1, 100, 7));
    let first = totals.first_request.unwrap();
    let last = totals.last_request.unwrap();
    assert_eq!(Date::of(first), Date::from_ymd(2026, 10, 7).unwrap());
    assert!(first < last);

    let history = reader.import_history().unwrap();
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].path, "a.log");
    assert_eq!(history[0].inserted_rows, 100);
    assert_eq!(history[0].format_spec, "builtin:combined");
}
