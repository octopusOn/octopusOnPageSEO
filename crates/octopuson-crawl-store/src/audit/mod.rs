//! Materialized audit findings. Rules run once after the crawl flushes its last batch.

mod near_duplicates;
mod redirects;
mod rules;
mod sitemap_redirects;

use duckdb::{Connection, params};
use octopuson_audit::{ai_crawler_access, is_generic_anchor, is_valid_hreflang, pixel_width};

use crate::CrawlStoreError;

pub(crate) fn run(connection: &Connection) -> Result<(), CrawlStoreError> {
    connection.execute_batch("BEGIN TRANSACTION")?;
    let result = evaluate(connection).and_then(|()| {
        connection.execute(
            "INSERT OR REPLACE INTO crawl_info VALUES ('audit_required','0')",
            [],
        )?;
        Ok(())
    });
    match result {
        Ok(()) => {
            connection.execute_batch("COMMIT")?;
            Ok(())
        }
        Err(error) => {
            let _ = connection.execute_batch("ROLLBACK");
            Err(error)
        }
    }
}

fn evaluate(connection: &Connection) -> Result<(), CrawlStoreError> {
    crate::anchor_context::prepare(connection)?;
    crate::schema_checks::prepare(connection)?;
    connection.execute_batch(&format!(
        "CREATE OR REPLACE TEMP VIEW named_links AS SELECT * FROM {}",
        crate::anchor_context::LINKS
    ))?;
    connection.execute_batch("DELETE FROM audit_findings; CREATE OR REPLACE TEMP VIEW audit_pages AS SELECT * FROM urls WHERE internal AND content_kind='html' AND status=200; CREATE OR REPLACE TEMP VIEW audit_indexable AS SELECT * FROM audit_pages WHERE indexability='indexable';")?;
    connection.execute_batch("CREATE OR REPLACE TEMP VIEW audit_hreflang_pages AS SELECT * FROM audit_pages WHERE title_count IS NOT NULL UNION ALL SELECT * FROM urls WHERE internal AND content_kind='pdf' AND status=200 AND EXISTS (SELECT 1 FROM crawl_info WHERE key='http_hreflang_observed' AND value='1')")?;
    connection.execute_batch(&format!(
        "CREATE OR REPLACE TEMP TABLE audit_context_ignored AS WITH {},{} SELECT * FROM hreflang_ignored UNION ALL SELECT * FROM anchor_resolved UNION ALL SELECT * FROM contact_resolved UNION ALL SELECT * FROM anchor_unverified",
        crate::hreflang_context::CTES,crate::anchor_context::ctes()
    ))?;
    near_duplicates::build(connection)?;
    for &(id, query) in rules::RULES {
        let context = if matches!(
            id,
            "hreflang-missing-return"
                | "hreflang-missing-self"
                | "hreflang-missing-x-default"
                | "anchor-empty"
                | "anchor-generic"
                | "links-to-redirects"
        ) {
            format!(
                " WHERE NOT EXISTS(SELECT 1 FROM audit_context_ignored i WHERE i.url=findings.url AND i.issue_id='{id}')"
            )
        } else {
            String::new()
        };
        connection.execute(
            &format!("INSERT OR IGNORE INTO audit_findings SELECT ?, url FROM ({query}) findings{context}"),
            params![id],
        )?;
    }
    let mut statement = connection.prepare("SELECT url,title,meta_description FROM audit_pages")?;
    let mut rows = statement.query([])?;
    while let Some(row) = rows.next()? {
        let url: String = row.get(0)?;
        let title: Option<String> = row.get(1)?;
        let description: Option<String> = row.get(2)?;
        if title.is_some_and(|s| pixel_width(&s, 20.0) > 561) {
            insert(connection, "title-too-wide", &url)?;
        }
        if description.is_some_and(|s| pixel_width(&s, 14.0) > 985) {
            insert(connection, "description-too-wide", &url)?;
        }
    }
    check_strings(
        connection,
        "SELECT p.url,h.lang FROM hreflang h JOIN urls p ON p.id=h.page WHERE p.internal",
        "hreflang-invalid-code",
        |s| !is_valid_hreflang(s),
    )?;
    check_strings(
        connection,
        "SELECT p.url,l.text FROM named_links l JOIN urls p ON p.id=l.source WHERE l.kind='hyperlink'",
        "anchor-generic",
        is_generic_anchor,
    )?;
    let mut statement =
        connection.prepare("SELECT url,text FROM robots_files WHERE http_status=200")?;
    let mut rows = statement.query([])?;
    while let Some(row) = rows.next()? {
        let url: String = row.get(0)?;
        let text: String = row.get(1)?;
        for access in ai_crawler_access(&text) {
            let rule = match (access.crawler.purpose.as_str(), access.allowed) {
                ("search", false) => Some("ai-search-blocked"),
                ("assistant", false) => Some("ai-assistant-blocked"),
                ("training", true) => Some("ai-training-allowed"),
                _ => None,
            };
            if let Some(rule) = rule {
                insert(connection, rule, &url)?;
            }
        }
    }
    redirects::check(connection)?;
    sitemap_redirects::check(connection)?;
    Ok(())
}

fn insert(connection: &Connection, rule: &str, url: &str) -> Result<(), CrawlStoreError> {
    connection.execute(
        "INSERT OR IGNORE INTO audit_findings VALUES (?,?)",
        params![rule, url],
    )?;
    Ok(())
}

fn check_strings(
    connection: &Connection,
    sql: &str,
    rule: &str,
    predicate: impl Fn(&str) -> bool,
) -> Result<(), CrawlStoreError> {
    let mut statement = connection.prepare(sql)?;
    let mut rows = statement.query([])?;
    while let Some(row) = rows.next()? {
        let url: String = row.get(0)?;
        let text: String = row.get(1)?;
        if predicate(&text) {
            insert(connection, rule, &url)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_query_executes_and_references_a_catalog_rule() {
        let mut connection = Connection::open_in_memory().unwrap();
        crate::schema::migrate(&mut connection).unwrap();
        for &(id, _) in rules::RULES {
            assert!(octopuson_audit::catalog().issue(id).is_some(), "{id}");
        }
        run(&connection).unwrap();
    }

    #[test]
    fn pdf_absence_checks_require_observed_header_coverage_without_changing_html_checks() {
        let mut connection = Connection::open_in_memory().unwrap();
        crate::schema::migrate(&mut connection).unwrap();
        for (id, path, kind, internal) in [
            (1, "pdf-source", "pdf", true),
            (2, "pdf-target", "pdf", true),
            (3, "html-source", "html", true),
            (4, "html-target", "html", true),
            (5, "external-source", "html", false),
            (6, "broken", "html", true),
            (7, "status-only-html", "html", true),
            (8, "checked-source", "html", true),
        ] {
            connection.execute(
                "INSERT INTO urls(id,url,internal,crawled_at_ms,status,blocked_by_robots,content_kind,size_bytes,truncated,response_ms,hsts,noindex,nofollow,nosnippet,noarchive,noimageindex,indexability,title_count) VALUES (?, ?, ?,0,?,false,?,0,false,0,false,false,false,false,false,false,?,?)",
                params![id, format!("https://example.com/{path}"), internal, if id==6 {404} else {200}, kind, if id==6 {"client-error"} else {"indexable"}, if id==7 {None} else {Some(0)}],
            ).unwrap();
        }
        connection.execute_batch("INSERT INTO hreflang VALUES (1,'en','https://example.com/pdf-target'), (3,'en','https://example.com/html-target'), (5,'en_BAD','https://example.com/html-target'), (5,'en','https://example.com/broken'), (8,'en','https://example.com/checked-source'), (8,'x-default','https://example.com/checked-source'), (8,'fr','https://example.com/status-only-html')").unwrap();
        let affected = |issue: &str| -> Vec<String> {
            connection
                .prepare("SELECT url FROM audit_findings WHERE issue_id=? ORDER BY url")
                .unwrap()
                .query_map(params![issue], |row| row.get(0))
                .unwrap()
                .collect::<Result<Vec<_>, _>>()
                .unwrap()
        };
        for covered in [false, true, false] {
            connection
                .execute(
                    "DELETE FROM crawl_info WHERE key='http_hreflang_observed'",
                    [],
                )
                .unwrap();
            if covered {
                connection
                    .execute(
                        "INSERT INTO crawl_info VALUES ('http_hreflang_observed','1')",
                        [],
                    )
                    .unwrap();
            }
            run(&connection).unwrap();
            for issue in [
                "hreflang-missing-return",
                "hreflang-missing-self",
                "hreflang-missing-x-default",
            ] {
                assert_eq!(
                    affected(issue),
                    if covered {
                        vec![
                            "https://example.com/html-source",
                            "https://example.com/pdf-source",
                        ]
                    } else {
                        vec!["https://example.com/html-source"]
                    },
                    "{issue}"
                );
            }
            let marker: bool = connection.query_row(
                "SELECT EXISTS (SELECT 1 FROM crawl_info WHERE key='http_hreflang_observed' AND value='1')", [], |row| row.get(0)
            ).unwrap();
            assert_eq!(
                marker, covered,
                "auditing must not fabricate historical coverage"
            );
            let external: u64 = connection.query_row("SELECT count(*) FROM audit_findings WHERE url='https://example.com/external-source' AND starts_with(issue_id,'hreflang-')", [], |row| row.get(0)).unwrap();
            assert_eq!(
                external, 0,
                "external status checks must not create site hreflang findings"
            );
        }
    }
}
