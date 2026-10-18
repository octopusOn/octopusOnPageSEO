//! Read-only scoring exceptions. Saved audit evidence is never rewritten.

pub(super) fn system_url(column: &str) -> String {
    seonexus_crawl::DEFAULT_URL_EXCLUSIONS
        .iter()
        .map(|pattern| format!("regexp_matches({column},'{}')", pattern.replace('\'', "''")))
        .collect::<Vec<_>>()
        .join(" OR ")
}

pub(super) fn generic_anchor(column: &str) -> String {
    let anchors = seonexus_audit::GENERIC_ANCHORS
        .iter()
        .map(|anchor| {
            let folded = anchor
                .to_lowercase()
                .replace('ı', "i")
                .replace('\u{307}', "");
            format!("'{}'", folded.replace('\'', "''"))
        })
        .collect::<Vec<_>>()
        .join(",");
    format!(
        r"trim(regexp_replace(replace(replace(lower(regexp_replace({column},'^[^\p{{L}}\p{{N}}]+|[^\p{{L}}\p{{N}}]+$','','g')),'ı','i'),'̇',''),'[\p{{Z}}\s\x{{0085}}]+',' ','g')) IN ({anchors})"
    )
}

/// Recheck known reference exceptions and proven canonical-cluster absences. Unknown
/// rules and other genuine failures on the same source page stay authoritative.
pub(super) fn ctes() -> String {
    let target = system_url("l.target_url");
    let source = system_url("u.url");
    let generic = generic_anchor("l.text");
    let hreflang = crate::hreflang_context::CTES;
    let anchors = crate::anchor_context::ctes();
    let contact = crate::anchor_context::contact_url("target_url", "internal");
    format!("{hreflang}, {anchors}, all_observed AS (
        SELECT u.id,u.url,u.content_kind='html' AND u.status=200 AND u.title_count IS NOT NULL AS analyzed_html,
            u.status IS NULL OR u.error IS NOT NULL OR u.blocked_by_robots OR u.truncated OR coalesce(NOT r.rendered,false) OR coalesce(r.truncated,false) AS limited,
            ({source}) AS excluded
        FROM urls u LEFT JOIN rendering r ON r.page=u.id WHERE u.internal
    ), observed AS (
        SELECT url,bool_or(analyzed_html) AS analyzed_html,bool_or(limited) AS limited FROM all_observed WHERE NOT excluded GROUP BY url
    ), excluded_sources AS (
        SELECT DISTINCT s.url FROM links l JOIN all_observed s ON s.id=l.source WHERE ({target})
    ), score_links AS (
        SELECT l.*,s.url AS source_url,t.status AS target_status,t.error AS target_error
        FROM effective_links l JOIN all_observed s ON s.id=l.source LEFT JOIN urls t ON t.id=l.target
        WHERE NOT s.excluded AND NOT ({target})
    ), reference_evidence AS (
        SELECT source_url AS url,'links-to-broken' AS issue_id FROM score_links WHERE kind='hyperlink' AND internal AND (target_status>=400 OR target_error IS NOT NULL)
        UNION SELECT source_url,'links-to-broken-external' FROM score_links WHERE kind='hyperlink' AND NOT internal AND (target_status>=400 OR target_error IS NOT NULL)
        UNION SELECT source_url,'links-to-redirects' FROM score_links WHERE kind='hyperlink' AND target_status IN (301,302,303,307,308) AND NOT(target_error IS NULL AND ({contact}))
        UNION SELECT source_url,'anchor-empty' FROM score_links WHERE kind='hyperlink' AND trim(text)=''
        UNION SELECT l.source_url,'anchor-generic' FROM score_links l WHERE l.kind='hyperlink' AND ({generic})
        UNION SELECT source_url,'links-internal-nofollow' FROM score_links WHERE kind='hyperlink' AND internal AND nofollow
        UNION SELECT source_url,'http-links' FROM score_links WHERE kind='hyperlink' AND starts_with(source_url,'https://') AND starts_with(target_url,'http://')
        UNION SELECT source_url,'links-too-many' FROM score_links WHERE kind='hyperlink' AND internal GROUP BY source_url HAVING count(*)>300
    ), original_findings AS (
        SELECT DISTINCT f.url,f.issue_id FROM audit_findings f JOIN all_observed o ON o.url=f.url
    ), scored_findings AS (
        SELECT f.* FROM original_findings f JOIN observed o ON o.url=f.url WHERE
        f.issue_id NOT IN ('hreflang-missing-x-default','links-too-many','schema-recommended','schema-unverified') AND
        NOT (f.issue_id='structured-data-missing' AND EXISTS(SELECT 1 FROM crawl_info WHERE key='schema_validation' AND value='1')) AND
        NOT (f.issue_id='structured-data-invalid' AND EXISTS(SELECT 1 FROM crawl_info WHERE key='schema_validation' AND value='1') AND NOT EXISTS(SELECT 1 FROM saved_schema_checks s JOIN urls u ON u.id=s.page WHERE u.url=f.url AND NOT s.limited AND s.syntax_errors>0)) AND
        NOT EXISTS(SELECT 1 FROM hreflang_ignored i WHERE i.url=f.url AND i.issue_id=f.issue_id)
        AND NOT EXISTS(SELECT 1 FROM anchor_resolved i WHERE i.url=f.url AND i.issue_id=f.issue_id)
        AND NOT EXISTS(SELECT 1 FROM anchor_unverified i WHERE i.url=f.url AND i.issue_id=f.issue_id)
        AND NOT EXISTS(SELECT 1 FROM contact_resolved i WHERE i.url=f.url AND i.issue_id=f.issue_id) AND (
        f.issue_id NOT IN ('links-to-broken','links-to-broken-external','links-to-redirects','anchor-empty','anchor-generic','links-internal-nofollow','http-links','links-too-many')
        OR NOT EXISTS(SELECT 1 FROM excluded_sources s WHERE s.url=f.url)
        OR EXISTS(SELECT 1 FROM reference_evidence e WHERE e.url=f.url AND e.issue_id=f.issue_id))
    )")
}

#[cfg(test)]
mod tests {
    use duckdb::{Connection, params};

    #[test]
    fn generic_text_evidence_matches_the_shared_classifier() {
        let connection = Connection::open_in_memory().unwrap();
        let predicate = super::generic_anchor("l.text");
        for text in [
            "Read more »",
            "  Devamını\u{a0} oku… ",
            "BURAYA TIKLAYIN",
            "İNCELE",
            "",
            "Normal page title",
            "→  Lire la suite  ←",
            "  click\n here ",
        ] {
            let result: bool = connection
                .query_row(
                    &format!("SELECT {predicate} FROM (SELECT ?::VARCHAR AS text) l"),
                    params![text],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(result, seonexus_audit::is_generic_anchor(text), "{text}");
        }
    }
}
