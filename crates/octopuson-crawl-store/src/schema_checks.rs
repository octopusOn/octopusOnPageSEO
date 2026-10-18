//! Optional crawl validation, bounded evidence and read-only archive compatibility.
use duckdb::{Connection, params};
use octopuson_crawl::UrlRecord;
use serde::Serialize;

use crate::CrawlStoreError;

const REPORT_LIMIT: usize = 256 * 1024;

/// Coverage of the explicitly enabled structured-data checks, over internal scored URLs.
#[derive(Debug, Clone, Serialize)]
pub struct SchemaCoverage {
    /// Complete documents inspected locally.
    pub checked_urls: u64,
    /// Documents with supported field checks or a confirmed syntax error.
    pub scored_urls: u64,
    /// Documents with confirmed syntax/required-property failures.
    pub error_urls: u64,
    /// Documents with optional recommendations.
    pub warning_urls: u64,
    /// Complete documents without a typed structured-data item; no penalty.
    pub missing_urls: u64,
    /// Unsupported, partial, failed or absent observations; never reported as a pass.
    pub unverified_urls: u64,
}

pub(crate) fn enabled(connection: &Connection) -> Result<bool, CrawlStoreError> {
    Ok(connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM crawl_info WHERE key='schema_validation' AND value='1')",
        [],
        |r| r.get(0),
    )?)
}

pub(crate) fn prepare(connection: &Connection) -> Result<(), CrawlStoreError> {
    let exists: bool = connection.query_row("SELECT EXISTS(SELECT 1 FROM information_schema.tables WHERE table_name='schema_checks' AND table_schema='main' AND table_catalog=current_database())", [], |r|r.get(0))?;
    let source = if exists {
        "SELECT * FROM main.schema_checks"
    } else {
        "SELECT NULL::UINTEGER AS page,NULL::VARCHAR AS report,NULL::VARCHAR AS error,0::UINTEGER AS syntax_errors,0::UINTEGER AS required_errors,0::UINTEGER AS warnings,0::UINTEGER AS checked_items,0::UINTEGER AS unchecked_items,0::UINTEGER AS items,false AS limited WHERE false"
    };
    connection.execute_batch(&format!(
        "CREATE OR REPLACE TEMP VIEW saved_schema_checks AS {source}"
    ))?;
    Ok(())
}

pub(crate) fn append(connection: &Connection, urls: &[UrlRecord]) -> Result<(), CrawlStoreError> {
    if !urls
        .iter()
        .any(|u| u.schema_report.is_some() || u.schema_error.is_some())
    {
        return Ok(());
    }
    let mut rows = connection.appender("schema_checks")?;
    for url in urls
        .iter()
        .filter(|u| u.schema_report.is_some() || u.schema_error.is_some())
    {
        let mut error = url.schema_error.clone();
        let mut saved = url
            .schema_report
            .as_ref()
            .and_then(|r| serde_json::to_string(r).ok());
        if saved.as_ref().is_some_and(|r| r.len() > REPORT_LIMIT) {
            saved = None;
            error = Some("report-limit".into());
        }
        let report = url.schema_report.as_ref().filter(|_| saved.is_some());
        let mut syntax = 0_u32;
        let mut required = 0_u32;
        let mut warnings = 0_u32;
        let mut checked = 0_u32;
        let mut unchecked = 0_u32;
        if let Some(report) = report {
            for item in &report.items {
                if item.profile.is_some() && item.status != "unchecked" {
                    checked += 1;
                } else if item.status == "unchecked" {
                    unchecked += 1;
                }
                for finding in &item.issues {
                    match (finding.severity, finding.code) {
                        ("error", "json-syntax") => syntax += 1,
                        ("error", _) => required += 1,
                        ("warning", _) => warnings += 1,
                        _ => {}
                    }
                }
            }
        }
        rows.append_row(params![
            url.id,
            saved,
            error,
            syntax,
            required,
            warnings,
            checked,
            unchecked,
            u32::try_from(report.map_or(0, |r| r.items.len())).unwrap_or(u32::MAX),
            report.is_none_or(|r| r.limited)
        ])?;
    }
    rows.flush()?;
    Ok(())
}

pub(crate) fn coverage(connection: &Connection) -> Result<Option<SchemaCoverage>, CrawlStoreError> {
    if !enabled(connection)? {
        return Ok(None);
    }
    let excluded = crate::score_exclusions::system_url("u.url");
    Ok(Some(connection.query_row(&format!("SELECT
        count(DISTINCT u.url) FILTER(WHERE s.report IS NOT NULL AND NOT s.limited),
        count(DISTINCT u.url) FILTER(WHERE NOT s.limited AND (s.checked_items>0 OR s.syntax_errors>0 OR s.required_errors>0)),
        count(DISTINCT u.url) FILTER(WHERE NOT s.limited AND (s.syntax_errors>0 OR s.required_errors>0)),
        count(DISTINCT u.url) FILTER(WHERE NOT s.limited AND s.warnings>0),
        count(DISTINCT u.url) FILTER(WHERE s.report IS NOT NULL AND NOT s.limited AND s.items=0),
        count(DISTINCT u.url) FILTER(WHERE s.page IS NULL OR s.error IS NOT NULL OR s.limited OR s.unchecked_items>0)
        FROM urls u LEFT JOIN saved_schema_checks s ON s.page=u.id
        WHERE u.internal AND u.content_kind='html' AND u.status=200 AND NOT ({excluded})"),[],|r|Ok(SchemaCoverage {checked_urls:r.get(0)?,scored_urls:r.get(1)?,error_urls:r.get(2)?,warning_urls:r.get(3)?,missing_urls:r.get(4)?,unverified_urls:r.get(5)?}))?))
}

pub(crate) fn report(
    connection: &Connection,
    url: &str,
) -> Result<Option<serde_json::Value>, CrawlStoreError> {
    let row=connection.query_row("SELECT s.report FROM saved_schema_checks s JOIN urls u ON u.id=s.page WHERE u.url=? ORDER BY u.id LIMIT 1",params![url],|r|r.get::<_,Option<String>>(0));
    match row {
        Ok(Some(raw)) if raw.len() <= REPORT_LIMIT => Ok(serde_json::from_str(&raw).ok()),
        Ok(_) | Err(duckdb::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn oversized_saved_evidence_is_unverified_without_retaining_or_scoring_partial_data() {
        let mut connection = Connection::open_in_memory().unwrap();
        crate::schema::migrate(&mut connection).unwrap();
        let mut report = octopuson_html::schema::validate(
            r#"{"@context":"https://schema.org","@type":"Product"}"#,
            true,
            "https://example.com/",
        )
        .unwrap();
        report.items[0].path = "x".repeat(REPORT_LIMIT);
        let mut record = UrlRecord::new(0, "https://example.com/".into(), Some(0), true);
        record.schema_report = Some(report);
        append(&connection, &[record]).unwrap();
        let saved = connection
            .query_row(
                "SELECT report,error,required_errors,syntax_errors,limited FROM schema_checks",
                [],
                |r| {
                    Ok((
                        r.get::<_, Option<String>>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, u32>(2)?,
                        r.get::<_, u32>(3)?,
                        r.get::<_, bool>(4)?,
                    ))
                },
            )
            .unwrap();
        assert_eq!(saved, (None, "report-limit".into(), 0, 0, true));
    }
    #[test]
    fn legacy_schema_placeholder_is_temporary_and_does_not_enable_optional_scoring() {
        let mut connection = Connection::open_in_memory().unwrap();
        crate::schema::migrate(&mut connection).unwrap();
        connection
            .execute_batch("DROP TABLE schema_checks")
            .unwrap();
        prepare(&connection).unwrap();
        assert!(coverage(&connection).unwrap().is_none());
        let tables: u64 = connection
            .query_row(
                "SELECT count(*) FROM information_schema.tables WHERE table_name='schema_checks'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(tables, 0);
        let rows: u64 = connection
            .query_row("SELECT count(*) FROM saved_schema_checks", [], |r| r.get(0))
            .unwrap();
        assert_eq!(rows, 0);
    }
}
