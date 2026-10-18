use duckdb::{Connection, params};

use crate::CrawlReader;

fn reader(internal: u32, external: u32) -> CrawlReader {
    let mut connection = Connection::open_in_memory().unwrap();
    crate::schema::migrate(&mut connection).unwrap();
    for id in 0..internal + external {
        connection.execute("INSERT INTO urls(id,url,internal,crawled_at_ms,status,blocked_by_robots,content_kind,size_bytes,truncated,response_ms,hsts,noindex,nofollow,nosnippet,noarchive,noimageindex,indexability,title_count) VALUES(?,?,?,0,200,false,'html',0,false,0,false,false,false,false,false,false,'indexable',1)", params![id,format!("https://example.com/{id}"),id<internal]).unwrap();
    }
    CrawlReader::from_connection(connection)
}

fn rule(priority: seonexus_audit::Priority) -> &'static str {
    &seonexus_audit::catalog()
        .issues
        .iter()
        .find(|rule| rule.priority == priority)
        .unwrap()
        .id
}

fn finding(reader: &CrawlReader, rule: &str, id: u32) {
    let url: String = reader
        .connection
        .query_row("SELECT url FROM urls WHERE id=?", params![id], |row| {
            row.get(0)
        })
        .unwrap();
    reader
        .connection
        .execute(
            "INSERT OR IGNORE INTO audit_findings VALUES(?,?)",
            params![rule, url],
        )
        .unwrap();
}

fn link(reader: &CrawlReader, source: u32, target: u32, text: &str) {
    let url: String = reader
        .connection
        .query_row("SELECT url FROM urls WHERE id=?", params![target], |row| {
            row.get(0)
        })
        .unwrap();
    let position: u32 = reader
        .connection
        .query_row(
            "SELECT count(*) FROM links WHERE source=?",
            params![source],
            |r| r.get(0),
        )
        .unwrap();
    reader
        .connection
        .execute(
            "INSERT INTO links VALUES(?,?,?,'hyperlink',?,false,false,false,true,?)",
            params![source, target, url, text, position],
        )
        .unwrap();
    reader
        .connection
        .execute(
            "INSERT INTO link_names VALUES(?,?,?,?,?,?,false,false)",
            params![
                source,
                position,
                url,
                text,
                text,
                if text.is_empty() {
                    "missing"
                } else {
                    "content"
                }
            ],
        )
        .unwrap();
}

#[test]
fn cloudflare_exceptions_remove_only_their_own_findings_and_keep_mixed_real_evidence() {
    let reader = reader(5, 0);
    reader.connection.execute_batch("UPDATE urls SET url='https://example.com/cdn-cgi/l/email-protection',status=404,title_count=NULL WHERE id=3; UPDATE urls SET url='https://example.com/cdn-cgi/l/email-protection-guide',status=404,title_count=NULL WHERE id=4;").unwrap();
    link(&reader, 0, 3, "");
    link(&reader, 1, 3, "");
    link(&reader, 1, 4, "Actually broken");
    link(&reader, 2, 3, "click here");
    link(&reader, 2, 0, "");
    link(&reader, 2, 0, "İNCELE");
    for source in 0..3 {
        finding(&reader, "links-to-broken", source);
        finding(&reader, "anchor-empty", source);
    }
    finding(&reader, "anchor-generic", 2);
    finding(&reader, "internal-client-error", 3);
    finding(&reader, "internal-client-error", 4);
    let score = reader.seo_score().unwrap();
    assert_eq!(
        (
            score.urls,
            score.excluded_urls,
            score.excluded_links,
            score.excluded_findings
        ),
        (4, 1, 3, 5)
    );
    assert_eq!(
        score
            .deductions
            .iter()
            .find(|row| row.id == "links-to-broken")
            .unwrap()
            .count,
        1
    );
    assert_eq!(
        score
            .deductions
            .iter()
            .find(|row| row.id == "anchor-empty")
            .unwrap()
            .count,
        1
    );
    assert_eq!(
        score
            .deductions
            .iter()
            .find(|row| row.id == "anchor-generic")
            .unwrap()
            .count,
        1
    );
    assert_eq!(
        score
            .deductions
            .iter()
            .find(|row| row.id == "internal-client-error")
            .unwrap()
            .count,
        1
    );
    assert_eq!(
        score
            .excluded_rules
            .iter()
            .map(|row| row.count)
            .sum::<u64>(),
        5
    );
    assert_eq!(
        reader
            .connection
            .query_row("SELECT count(*) FROM audit_findings", [], |row| row
                .get::<_, u64>(0))
            .unwrap(),
        9
    );
}

#[test]
fn every_branch_has_a_ledger_and_its_impacts_sum_to_the_overall_loss() {
    let reader = reader(3, 0);
    for id in ["title-missing", "description-missing", "h1-missing"] {
        finding(&reader, id, 0);
    }
    finding(&reader, "links-to-broken", 1);
    let score = reader.seo_score().unwrap();
    assert_eq!(score.version, 4);
    assert_eq!(
        score.categories.len(),
        seonexus_audit::catalog().categories.len()
    );
    assert!(
        (score.categories.iter().map(|row| row.points).sum::<f64>()
            - score.deductions.iter().map(|row| row.points).sum::<f64>())
        .abs()
            < 1e-9
    );
    for category in &score.categories {
        assert_eq!(
            category.checks,
            seonexus_audit::catalog()
                .issues
                .iter()
                .filter(|rule| rule.category == category.id)
                .count()
        );
        assert!(
            category
                .value
                .is_none_or(|value| (0.0..=100.0).contains(&value))
        );
    }
    assert_eq!(
        score
            .categories
            .iter()
            .find(|row| row.id == "rendering")
            .unwrap()
            .value,
        None
    );
    assert_eq!(
        score
            .categories
            .iter()
            .find(|row| row.id == "titles")
            .unwrap()
            .deductions[0]
            .id,
        "title-missing"
    );
}

#[test]
fn score_uses_exact_internal_finding_weights_without_external_dilution() {
    let reader = reader(2, 100);
    let high = rule(seonexus_audit::Priority::High);
    let medium = rule(seonexus_audit::Priority::Medium);
    let low = rule(seonexus_audit::Priority::Low);
    finding(&reader, high, 0);
    finding(&reader, high, 0);
    finding(&reader, medium, 0);
    finding(&reader, low, 1);
    finding(&reader, high, 2);
    let score = reader.seo_score().unwrap();
    assert_eq!(score.value, Some(85.5));
    assert_eq!(
        (score.urls, score.affected_urls, score.analyzed_html),
        (2, 2, 2)
    );
    assert_eq!(score.deductions[0].id, high);
    assert_eq!(score.deductions[0].count, 1);
    assert_eq!(score.deductions[0].points, 10.0);
    assert!((score.deductions.iter().map(|d| d.points).sum::<f64>() - 14.5).abs() < 1e-9);
    reader
        .connection
        .execute("DELETE FROM audit_findings WHERE issue_id=?", params![high])
        .unwrap();
    assert_eq!(reader.seo_score().unwrap().value, Some(95.5));
}

#[test]
fn per_url_cap_preserves_bounds_and_contributions_sum_to_the_actual_deduction() {
    let reader = reader(2, 0);
    for rule in seonexus_audit::catalog()
        .issues
        .iter()
        .filter(|rule| rule.priority == seonexus_audit::Priority::High)
        .take(7)
    {
        finding(&reader, &rule.id, 0);
    }
    let score = reader.seo_score().unwrap();
    assert_eq!(score.value, Some(50.0));
    assert!((score.deductions.iter().map(|d| d.points).sum::<f64>() - 50.0).abs() < 1e-9);
    for rule in seonexus_audit::catalog()
        .issues
        .iter()
        .filter(|rule| rule.priority == seonexus_audit::Priority::High)
        .take(7)
    {
        finding(&reader, &rule.id, 1);
    }
    assert_eq!(reader.seo_score().unwrap().value, Some(0.0));
}

#[test]
fn score_distinguishes_clean_unknown_empty_and_unanalyzed_samples() {
    assert_eq!(reader(0, 2).seo_score().unwrap().value, None);
    let reader = reader(1, 0);
    assert_eq!(reader.seo_score().unwrap().value, Some(100.0));
    finding(&reader, "future-unknown-rule", 0);
    assert_eq!(reader.seo_score().unwrap().unknown_findings, 1);
    assert_eq!(reader.seo_score().unwrap().value, None);
    reader
        .connection
        .execute_batch("DELETE FROM audit_findings; UPDATE urls SET title_count=NULL")
        .unwrap();
    assert_eq!(reader.seo_score().unwrap().value, None);
    reader.connection.execute_batch("UPDATE urls SET title_count=1,truncated=true; INSERT INTO rendering(page,rendered,elapsed_ms,truncated,raw_word_count,raw_link_count) VALUES(0,false,0,false,0,0)").unwrap();
    assert_eq!(reader.seo_score().unwrap().limited_urls, 1);
    reader
        .connection
        .execute_batch("UPDATE urls SET error='timeout',truncated=false; DELETE FROM rendering")
        .unwrap();
    assert_eq!(reader.seo_score().unwrap().limited_urls, 1);
}

#[test]
fn remediation_preserves_scored_url_scope_and_exact_non_cloudflare_witnesses() {
    let reader = reader(4, 1);
    reader.connection.execute_batch("UPDATE urls SET url='https://example.com/cdn-cgi/l/email-protection',status=404 WHERE id=2; UPDATE urls SET status=404 WHERE id=3;").unwrap();
    link(&reader, 0, 2, "");
    link(&reader, 1, 2, "");
    link(&reader, 1, 3, "Actual broken target");
    finding(&reader, "links-to-broken", 0);
    finding(&reader, "links-to-broken", 1);
    let mut request = crate::RemediationRequest {
        issue_id: "links-to-broken".into(),
        limit: 20,
        ..Default::default()
    };
    let detail = reader.remediation(&request).unwrap();
    assert_eq!(detail.urls.total, 1);
    assert_eq!(
        detail.selected_url.as_deref(),
        Some("https://example.com/1")
    );
    assert_eq!(detail.evidence.total, 1);
    assert_eq!(
        detail.evidence.rows[0][0].to_string(),
        "https://example.com/3"
    );
    request.url = "https://example.com/0".into();
    assert!(reader.remediation(&request).is_err());
    request.url.clear();
    request.search = "' OR true --".into();
    assert_eq!(reader.remediation(&request).unwrap().urls.total, 0);
    request.issue_id = "unrecognized".into();
    assert!(reader.remediation(&request).is_err());
    assert_eq!(
        reader
            .scalar("SELECT count(*) FROM audit_findings")
            .unwrap(),
        2
    );
}

#[test]
fn remediation_pages_witnesses_and_explains_exact_hreflang_and_heading_failures() {
    let reader = reader(3, 0);
    finding(&reader, "anchor-empty", 0);
    for position in 0..105 {
        reader.connection.execute("INSERT INTO links VALUES(0,1,'https://example.com/1','hyperlink','',false,false,false,true,?)",params![position]).unwrap();
        reader.connection.execute("INSERT INTO link_names VALUES(0,?,'https://example.com/1','','','missing',false,false)",params![position]).unwrap();
    }
    let mut request = crate::RemediationRequest {
        issue_id: "anchor-empty".into(),
        limit: 9999,
        evidence_offset: 50,
        ..Default::default()
    };
    let detail = reader.remediation(&request).unwrap();
    assert_eq!(detail.evidence.total, 105);
    assert_eq!(detail.evidence.rows.len(), 50);
    assert_eq!(detail.evidence.rows[0][6].to_string(), "50");
    reader.connection.execute_batch("INSERT INTO hreflang VALUES(0,'en','https://example.com/1'),(0,'fr','https://example.com/2'),(2,'tr','https://example.com/0'); INSERT INTO headings VALUES(0,0,1,'Main'),(0,1,3,'Skipped H2'),(0,2,2,'Normal');").unwrap();
    finding(&reader, "hreflang-missing-return", 0);
    finding(&reader, "headings-non-sequential", 0);
    request.issue_id = "hreflang-missing-return".into();
    request.evidence_offset = 0;
    let detail = reader.remediation(&request).unwrap();
    assert_eq!(detail.evidence.total, 1);
    assert_eq!(
        detail.evidence.rows[0][1].to_string(),
        "https://example.com/1"
    );
    assert_eq!(detail.evidence.rows[0][5].to_string(), "false");
    request.issue_id = "headings-non-sequential".into();
    let detail = reader.remediation(&request).unwrap();
    assert_eq!(detail.evidence.total, 1);
    assert_eq!(detail.evidence.rows[0][2].to_string(), "Main");
    assert_eq!(detail.evidence.rows[0][4].to_string(), "Skipped H2");
}
