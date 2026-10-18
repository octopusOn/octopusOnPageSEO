use duckdb::{Connection, params};
use seonexus_core::table::Cell;

use crate::{CrawlReader, PageRequest, RemediationRequest};

fn fixture() -> CrawlReader {
    let mut connection = Connection::open_in_memory().unwrap();
    crate::schema::migrate(&mut connection).unwrap();
    let pages = [
        ("tr/services", "tr", "indexable", 100),
        ("tr/services?services_page=1", "tr", "canonicalised", 100),
        ("tr/services?services_page=2", "tr", "canonicalised", 200),
        ("tr/services?utm_source=test", "tr", "canonicalised", 100),
        ("en/services", "en", "indexable", 300),
        ("tr/only-turkish", "tr", "indexable", 400),
        ("tr/unmatched", "tr", "canonicalised", 500),
        ("fr/other", "fr", "indexable", 600),
    ];
    for (id, (path, lang, indexability, hash)) in pages.iter().enumerate() {
        connection.execute("INSERT INTO urls(id,url,internal,crawled_at_ms,status,blocked_by_robots,content_kind,size_bytes,truncated,response_ms,hsts,noindex,nofollow,nosnippet,noarchive,noimageindex,indexability,title_count,canonical,canonical_count,lang,text_hash) VALUES(?,?,true,0,200,false,'html',0,false,0,false,false,false,false,false,false,?,1,?,1,?,?)",
            params![id,format!("https://example.com/{path}"),indexability,format!("https://example.com/{}",if *indexability=="canonicalised" {"tr/services"} else {path}),lang,hash]).unwrap();
    }
    for id in [0, 1, 2, 3, 4] {
        connection.execute("INSERT INTO hreflang VALUES(?,'tr','https://example.com/tr/services'),(?,'en','https://example.com/en/services'),(?,'x-default','https://example.com/tr/services')",params![id,id,id]).unwrap();
    }
    connection.execute_batch("INSERT INTO hreflang VALUES(6,'fr','https://example.com/fr/other'); INSERT INTO audit_findings SELECT rule,url FROM urls CROSS JOIN (VALUES ('hreflang-missing-return'),('hreflang-missing-self')) r(rule) WHERE id IN (1,2,3,6); INSERT INTO audit_findings SELECT 'canonicalised',url FROM urls WHERE id=2;").unwrap();
    CrawlReader::from_connection(connection)
}

fn page(reader: &CrawlReader, tab: &str, issue_id: &str) -> crate::TablePage {
    reader
        .page(&PageRequest {
            tab: tab.into(),
            issue_id: issue_id.into(),
            limit: 100,
            ..Default::default()
        })
        .unwrap()
}

fn field(table: &crate::TablePage, row: usize, name: &str) -> Cell {
    let index = table.columns.iter().position(|c| c.name == name).unwrap();
    table.rows[row][index].clone()
}

#[test]
fn canonical_cluster_resolves_only_proven_absences_in_old_reports_score_and_witnesses() {
    let reader = fixture();
    let score = reader.seo_score().unwrap();
    assert_eq!(score.version, 4);
    assert_eq!(score.canonical_context_findings, 6);
    assert_eq!(score.excluded_findings, 6);
    assert_eq!(
        score
            .deductions
            .iter()
            .find(|d| d.id == "hreflang-missing-return")
            .unwrap()
            .count,
        1
    );
    assert_eq!(page(&reader, "issues", "hreflang-missing-return").total, 1);
    assert_eq!(
        page(&reader, "internal", "hreflang-missing-return").total,
        1
    );
    assert!(
        reader
            .overview()
            .unwrap()
            .issues
            .iter()
            .any(|i| i.id == "hreflang-missing-return" && i.count == 1)
    );
    let detail = reader
        .detail("https://example.com/tr/services?services_page=2")
        .unwrap();
    assert_eq!(detail.findings.total, 1);
    assert_eq!(detail.findings.rows[0][0].to_string(), "canonicalised");
    let remediation = reader
        .remediation(&RemediationRequest {
            issue_id: "hreflang-missing-return".into(),
            limit: 20,
            ..Default::default()
        })
        .unwrap();
    assert_eq!(remediation.urls.total, 1);
    assert_eq!(remediation.evidence.total, 1);
    assert_eq!(
        remediation.evidence.rows[0][1].to_string(),
        "https://example.com/fr/other"
    );
    let rows = page(&reader, "hreflang", "");
    let row = rows
        .rows
        .iter()
        .position(|r| {
            r[0].to_string() == "https://example.com/tr/services?services_page=2"
                && r[1].to_string() == "en"
        })
        .unwrap();
    assert_eq!(field(&rows, row, "return_link"), Cell::Boolean(false));
    assert_eq!(field(&rows, row, "canonical_context"), Cell::Boolean(true));
    assert_eq!(
        field(&rows, row, "pagination_candidate"),
        Cell::Boolean(true)
    );
    assert_eq!(
        field(&rows, row, "canonical_content_equal"),
        Cell::Boolean(false)
    );
    assert_eq!(
        reader
            .scalar("SELECT count(*) FROM audit_findings")
            .unwrap(),
        9,
        "historical evidence stays intact"
    );
}

#[test]
fn invalid_or_unobserved_canonical_targets_and_conflicting_declarations_never_hide_errors() {
    for change in [
        "UPDATE urls SET status=404 WHERE id=0",
        "UPDATE urls SET noindex=true,indexability='noindex' WHERE id=0",
        "UPDATE urls SET title_count=NULL WHERE id=0",
        "UPDATE urls SET truncated=true WHERE id=0",
        "UPDATE urls SET canonical='https://example.com/elsewhere' WHERE id=0",
        "UPDATE urls SET canonical_count=2 WHERE id=1",
        "UPDATE urls SET header_canonical='https://example.com/conflict' WHERE id=1",
        "UPDATE urls SET lang='de' WHERE id=1",
        "INSERT INTO rendering(page,rendered,elapsed_ms,truncated,raw_word_count,raw_link_count) VALUES(1,false,0,false,0,0)",
    ] {
        let reader = fixture();
        reader.connection.execute_batch(change).unwrap();
        assert!(
            page(&reader, "internal", "hreflang-missing-return")
                .rows
                .iter()
                .any(|r| r[0].to_string() == "https://example.com/tr/services?services_page=1"),
            "{change}"
        );
    }
}

#[test]
fn real_missing_returns_and_self_canonical_pagination_stay_actionable() {
    let reader = fixture();
    reader.connection.execute_batch("DELETE FROM hreflang WHERE page=4 AND target='https://example.com/tr/services'; UPDATE urls SET canonical=url,indexability='indexable' WHERE id=1;").unwrap();
    assert_eq!(
        reader.seo_score().unwrap().canonical_context_findings,
        2,
        "only missing-self on proven aliases is resolved"
    );
    let remediation = reader
        .remediation(&RemediationRequest {
            issue_id: "hreflang-missing-return".into(),
            search: "services_page=2".into(),
            limit: 20,
            ..Default::default()
        })
        .unwrap();
    assert_eq!(remediation.urls.total, 1);
    assert_eq!(
        remediation.evidence.total, 1,
        "canonical self rows must not become false missing-return witnesses"
    );
    assert_eq!(
        remediation.evidence.rows[0][1].to_string(),
        "https://example.com/en/services"
    );
    assert_eq!(
        page(&reader, "internal", "hreflang-missing-return").total,
        4
    );
}

#[test]
fn new_audits_do_not_require_translations_or_duplicate_canonical_cluster_absences() {
    let reader = fixture();
    crate::audit::run(&reader.connection).unwrap();
    for issue in ["hreflang-missing-return", "hreflang-missing-self"] {
        let rows = page(&reader, "issues", issue);
        assert_eq!(rows.total, 1, "{issue}");
        assert_eq!(
            rows.rows[0][1].to_string(),
            "https://example.com/tr/unmatched"
        );
        assert_eq!(
            reader
                .scalar(&format!(
                    "SELECT count(*) FROM audit_findings WHERE issue_id='{issue}'"
                ))
                .unwrap(),
            1
        );
    }
    assert_eq!(reader.scalar("SELECT count(*) FROM audit_findings WHERE starts_with(issue_id,'hreflang-') AND url='https://example.com/tr/only-turkish'").unwrap(),0);
    assert!(page(&reader, "internal", "canonicalised").total >= 3);
}

#[test]
fn header_canonical_and_optional_default_context_do_not_hide_broken_alternates() {
    let reader = fixture();
    reader.connection.execute_batch("UPDATE urls SET header_canonical=canonical,canonical=NULL,canonical_count=0 WHERE id IN (0,1); DELETE FROM hreflang WHERE page=1 AND lang='x-default'; INSERT INTO audit_findings VALUES('hreflang-missing-x-default','https://example.com/tr/services?services_page=1');").unwrap();
    assert_eq!(reader.seo_score().unwrap().canonical_context_findings, 7);
    reader
        .connection
        .execute_batch("UPDATE urls SET status=404,indexability='client-error' WHERE id=4")
        .unwrap();
    crate::audit::run(&reader.connection).unwrap();
    let broken = page(&reader, "internal", "hreflang-to-non-200");
    assert!(
        broken
            .rows
            .iter()
            .any(|r| r[0].to_string() == "https://example.com/tr/services?services_page=1")
    );
    assert!(
        reader
            .seo_score()
            .unwrap()
            .deductions
            .iter()
            .any(|d| d.id == "hreflang-to-non-200")
    );
    let rows = page(&reader, "hreflang", "");
    let row = rows
        .rows
        .iter()
        .position(|r| {
            r[0].to_string() == "https://example.com/tr/services?services_page=1"
                && r[1].to_string() == "en"
        })
        .unwrap();
    assert_eq!(field(&rows, row, "target_status"), Cell::Integer(404));
    assert_eq!(
        field(&rows, row, "return_link"),
        Cell::Null,
        "unusable targets are not asserted to have no return"
    );
}
