use seonexus_core::table::{Cell, TableBuffer};
use seonexus_store::StoreError;

use super::*;
use crate::test_support::Fixture;

#[test]
fn aggregates_are_answered_from_rollups() {
    let fixture = Fixture::imported();
    let (outcome, table) = fixture.run("group:status sort:status:asc");
    assert_eq!(outcome.source, "hourly rollup");
    assert_eq!(outcome.rows, 2);
    assert_eq!(
        table.rows,
        [
            [Cell::Integer(200), Cell::Integer(4)],
            [Cell::Integer(404), Cell::Integer(2)]
        ]
    );
}

#[test]
fn raw_filters_and_canonical_text_are_reported() {
    let fixture = Fixture::imported();
    let (outcome, table) = fixture.run("ip:10.0.0.1   status:404");
    assert_eq!(outcome.source, "raw hits");
    assert_eq!(outcome.query, "ip:10.0.0.1 status:404");
    assert_eq!(table.rows.len(), 1);
}

#[test]
fn site_without_data_returns_the_columns_only() {
    let fixture = Fixture::new();
    let (outcome, table) = fixture.run("group:url");
    assert_eq!(outcome.rows, 0);
    assert_eq!(table.columns.len(), 2);
}

#[test]
fn errors_are_reported_before_touching_the_site() {
    let fixture = Fixture::new();
    let mut buffer = TableBuffer::default();
    let request = |site: &str, text: &str| QueryRequest {
        site: site.into(),
        spec: QuerySpec::text(text),
        page: None,
    };
    let parse = run_query(&fixture.workspace, &request("nope", "status:"), &mut buffer);
    assert!(matches!(parse, Err(QueryError::Parse { .. })));
    let invalid = run_query(
        &fixture.workspace,
        &request("nope", "group:url sort:ip"),
        &mut buffer,
    );
    assert!(matches!(invalid, Err(QueryError::Invalid(_))));
    let unknown = run_query(
        &fixture.workspace,
        &request("nope", "group:url"),
        &mut buffer,
    );
    assert!(matches!(
        unknown,
        Err(QueryError::Store(StoreError::SiteNotFound(_)))
    ));
}

#[test]
fn exports_by_extension_and_replaces_atomically() {
    let fixture = Fixture::imported();
    let target = fixture.path("report.csv");
    std::fs::write(&target, "old").unwrap();
    let request = ExportRequest {
        site: "acme.com".into(),
        spec: QuerySpec::Template {
            id: "client-errors".into(),
            refine: None,
        },
        path: target.clone(),
        format: None,
    };
    let report = export_query(&fixture.workspace, &request).unwrap();
    assert_eq!(report.format, "csv");
    assert_eq!(report.outcome.rows, 1);
    let csv = std::fs::read_to_string(&target).unwrap();
    assert!(
        csv.starts_with("url,status,hits,last_seen\r\n/missing,404,2,"),
        "{csv}"
    );
    assert!(!fixture.path("report.csv.partial").exists());

    for name in ["report.json", "report.xlsx"] {
        let request = ExportRequest {
            path: fixture.path(name),
            ..request.clone()
        };
        export_query(&fixture.workspace, &request).unwrap();
        assert!(fixture.path(name).metadata().unwrap().len() > 0);
    }
}

#[test]
fn failed_export_leaves_no_file() {
    let fixture = Fixture::imported();
    let request = ExportRequest {
        site: "missing.com".into(),
        spec: QuerySpec::text("group:url"),
        path: fixture.path("out.json"),
        format: None,
    };
    assert!(export_query(&fixture.workspace, &request).is_err());
    assert!(!fixture.path("out.json").exists());
    assert!(!fixture.path("out.json.partial").exists());

    let unknown = ExportRequest {
        path: fixture.path("out.txt"),
        ..request
    };
    assert!(matches!(
        export_query(&fixture.workspace, &unknown),
        Err(QueryError::File(
            crate::table_file::TableFileError::UnknownFormat(_)
        ))
    ));
}

#[test]
fn bots_are_classified_and_verified_on_import() {
    let fixture = Fixture::imported();
    let (outcome, table) =
        fixture.run("bot:* group:bot,verification sort:bot:asc,verification:asc");
    assert_eq!(outcome.source, "daily URL rollup");
    let text = |s: &str| Cell::Text(s.into());
    assert_eq!(
        table.rows,
        [
            // Outside Google's current ranges and DNS disabled in tests: undecided.
            [
                text("googlebot-desktop"),
                text("unverified"),
                Cell::Integer(1)
            ],
            [
                text("googlebot-desktop"),
                text("verified"),
                Cell::Integer(2)
            ],
            // OpenAI publishes lists only, so a miss is spoofing.
            [text("gptbot"), text("spoofed"), Cell::Integer(1)],
        ]
    );
    // Raw hits look verdicts up per (ip, bot) and agree with the rollup.
    let (raw, listing) = fixture.run("ip:10.0.0.3 verification:spoofed");
    assert_eq!(raw.source, "raw hits");
    assert_eq!(listing.rows.len(), 1);
    let (_, humans) = fixture.run("-bot:* group:ua");
    assert_eq!(humans.rows, [[text("Mozilla/5.0"), Cell::Integer(2)]]);
}

#[test]
fn queries_are_checked_without_a_site() {
    let checked = check_query(&QuerySpec::text("status:404   group:url")).unwrap();
    assert_eq!(checked.query, "status:404 group:url");
    assert_eq!(checked.source, "daily URL rollup");
    assert_eq!(checked.columns.len(), 2);
    assert!(matches!(
        check_query(&QuerySpec::text("group:nope")),
        Err(QueryError::Parse { .. })
    ));
}

#[test]
fn parses_text_and_describes_the_language() {
    let query = parse_query("status:4xx group:url").unwrap();
    assert_eq!(query.group, [Dimension::Url]);
    assert!(parse_query("status:").is_err());
    let schema = query_schema();
    let status = schema.fields.iter().find(|f| f.name == "status").unwrap();
    assert_eq!(status.kind, "integer");
    assert!(schema.dimensions.contains(&"bot"));
    assert!(schema.metrics.contains(&"hits"));
}

/// Browser visits with referers, a stylesheet, a redirect and a later Googlebot crawl.
const REFERER_LOG: &str = r#"10.0.0.5 - - [09/Oct/2026:10:00:00 +0000] "GET /a HTTP/1.1" 200 1000 "https://chatgpt.com/" "Mozilla/5.0"
10.0.0.5 - - [09/Oct/2026:10:00:01 +0000] "GET /style.css HTTP/1.1" 200 2000 "https://www.acme.com/a" "Mozilla/5.0"
10.0.0.6 - - [09/Oct/2026:11:00:00 +0000] "GET /a HTTP/1.1" 200 1000 "https://www.google.com.tr/" "Mozilla/5.0"
10.0.0.7 - - [09/Oct/2026:12:00:00 +0000] "GET /b HTTP/1.1" 301 0 "https://gemini.google.com/app" "Mozilla/5.0"
66.249.66.1 - - [10/Oct/2026:08:00:00 +0000] "GET /missing HTTP/1.1" 410 20 "-" "Googlebot/2.1"
"#;

fn fixture_with_referers() -> Fixture {
    let fixture = Fixture::imported();
    fixture.import("referers.log", REFERER_LOG);
    fixture
}

#[test]
fn every_template_runs_against_real_data() {
    let fixture = fixture_with_referers();
    for template in seonexus_query::template::TEMPLATES {
        let mut buffer = TableBuffer::default();
        let request = QueryRequest {
            site: "acme.com".into(),
            spec: QuerySpec::Template {
                id: template.id.into(),
                refine: None,
            },
            page: None,
        };
        run_query(&fixture.workspace, &request, &mut buffer)
            .unwrap_or_else(|e| panic!("{}: {e}", template.id));
    }
}

#[test]
fn url_metrics_summarise_status_recency_and_bots() {
    let fixture = fixture_with_referers();
    let (outcome, table) = fixture.run(
        "url:/a,/missing group:url metric:hits,last_status,days_since_last,errors,hits_2xx,hits_4xx,inconsistent,googlebot_hits,bot_hits sort:url:asc",
    );
    assert_eq!(outcome.source, "daily URL rollup");
    assert_eq!(
        table.rows,
        [
            [
                Cell::Text("/a".into()),
                Cell::Integer(5),
                Cell::Integer(200),
                Cell::Integer(1),
                Cell::Integer(0),
                Cell::Integer(5),
                Cell::Integer(0),
                Cell::Boolean(false),
                Cell::Integer(2),
                Cell::Integer(3),
            ],
            [
                Cell::Text("/missing".into()),
                Cell::Integer(3),
                Cell::Integer(410),
                Cell::Integer(0),
                Cell::Integer(3),
                Cell::Integer(0),
                Cell::Integer(3),
                Cell::Boolean(true),
                Cell::Integer(2),
                Cell::Integer(2),
            ],
        ]
    );
    // Raw hits agree with the rollup.
    let (raw, raw_table) = fixture.run(
        "url:/a,/missing ip:* group:url metric:hits,last_status,days_since_last,errors,hits_2xx,hits_4xx,inconsistent,googlebot_hits,bot_hits sort:url:asc",
    );
    assert_eq!(raw.source, "raw hits");
    assert_eq!(raw_table.rows, table.rows);
}

#[test]
fn referers_and_content_types_are_classified() {
    let fixture = fixture_with_referers();
    let (_, table) = fixture.run("group:referer_type sort:referer_type:asc");
    let counts: Vec<(String, i64)> = table
        .rows
        .iter()
        .map(|row| match (&row[0], &row[1]) {
            (Cell::Text(kind), Cell::Integer(hits)) => (kind.clone(), *hits),
            other => panic!("{other:?}"),
        })
        .collect();
    assert_eq!(
        counts,
        [
            ("ai".to_owned(), 2),
            ("direct".to_owned(), 7),
            ("internal".to_owned(), 1),
            ("search".to_owned(), 1),
        ]
    );
    let (_, hosts) = fixture.run("referer_type:ai group:referer_host sort:referer_host:asc");
    assert_eq!(
        hosts.rows,
        [
            [Cell::Text("chatgpt.com".into()), Cell::Integer(1)],
            [Cell::Text("gemini.google.com".into()), Cell::Integer(1)],
        ]
    );
    let (outcome, types) =
        fixture.run("group:content_type metric:hits,avg_bytes,co2_mg sort:content_type:asc");
    assert_eq!(outcome.source, "daily URL rollup");
    assert_eq!(types.rows[0][0], Cell::Text("css".into()));
    assert_eq!(types.rows[0][1], Cell::Integer(1));
    assert_eq!(types.rows[0][2], Cell::Float(2000.0));
    let Cell::Float(co2) = types.rows[0][3] else {
        panic!("{:?}", types.rows[0][3]);
    };
    assert!((co2 - 0.716_04).abs() < 1e-9, "{co2}");
    assert_eq!(types.rows[1][0], Cell::Text("html".into()));
    assert_eq!(types.rows[1][1], Cell::Integer(10));
}
