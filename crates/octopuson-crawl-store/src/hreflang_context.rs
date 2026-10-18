//! Read-only canonical context; never rewrite observed hreflang or strip parameters.

/// Only analyzed successful HTML and covered HTTP/PDF observations establish absence.
pub(super) const CTES: &str = r"
hreflang_pages AS (
    SELECT * FROM urls WHERE internal AND status=200 AND error IS NULL AND (
        (content_kind='html' AND title_count IS NOT NULL) OR
        (content_kind='pdf' AND EXISTS(SELECT 1 FROM crawl_info WHERE key='http_hreflang_observed' AND value='1'))
    )
), hreflang_aliases AS (
    SELECT s.id,s.url,c.id AS canonical_id,c.url AS canonical_url,
        s.text_hash=c.text_hash AS canonical_content_equal
    FROM hreflang_pages s JOIN hreflang_pages c ON c.url=coalesce(s.canonical,s.header_canonical)
    LEFT JOIN rendering r ON r.page=s.id LEFT JOIN rendering cr ON cr.page=c.id
    WHERE s.content_kind='html' AND c.content_kind='html' AND s.url<>c.url
        AND s.indexability='canonicalised' AND c.indexability='indexable'
        AND NOT s.noindex AND NOT c.noindex AND NOT s.blocked_by_robots AND NOT c.blocked_by_robots
        AND NOT s.truncated AND NOT c.truncated
        AND coalesce(r.rendered AND NOT r.truncated,true) AND coalesce(cr.rendered AND NOT cr.truncated,true)
        AND coalesce(s.canonical_count,0)<=1 AND coalesce(c.canonical_count,0)<=1
        AND (s.canonical IS NULL OR s.header_canonical IS NULL OR s.canonical=s.header_canonical)
        AND (c.canonical IS NULL OR c.header_canonical IS NULL OR c.canonical=c.header_canonical)
        AND coalesce(c.canonical,c.header_canonical)=c.url
        AND coalesce(split_part(lower(s.lang),'-',1),'')<>''
        AND split_part(lower(s.lang),'-',1)=split_part(lower(c.lang),'-',1)
        AND EXISTS(SELECT 1 FROM hreflang h WHERE h.page=s.id)
        AND EXISTS(SELECT 1 FROM hreflang h WHERE h.page=c.id AND h.target=c.url
            AND split_part(lower(h.lang),'-',1)=split_part(lower(c.lang),'-',1))
        AND NOT EXISTS(SELECT 1 FROM hreflang h WHERE h.page=s.id AND NOT EXISTS(
            SELECT 1 FROM hreflang ch WHERE ch.page=c.id AND lower(ch.lang)=lower(h.lang) AND ch.target=h.target
        ))
), hreflang_rows AS (
    SELECT s.id AS source_id,s.url AS page,h.lang,h.target,t.status AS target_status,
        t.indexability AS target_indexability,t.lang AS target_lang,
        coalesce(s.canonical,s.header_canonical) AS source_canonical,
        CASE WHEN t.id IS NOT NULL THEN EXISTS(SELECT 1 FROM hreflang b WHERE b.page=t.id AND b.target=s.url) END AS return_link,
        h.target=s.url AS self_reference,
        a.canonical_content_equal,
        coalesce(s.rel_next IS NOT NULL OR s.rel_prev IS NOT NULL OR
            regexp_matches(s.url,'[?&]([a-zA-Z0-9_-]*[_-])?(page|paged|pagenum|page_num|page_number|offset|start)=[0-9]+(&|$)'),false) AS pagination_candidate,
        a.id IS NOT NULL AND t.id IS NOT NULL AND EXISTS(
            SELECT 1 FROM hreflang b WHERE b.page=t.id AND b.target=a.canonical_url
        ) AS canonical_context
    FROM hreflang h JOIN urls s ON s.id=h.page
    LEFT JOIN hreflang_pages t ON t.url=h.target LEFT JOIN hreflang_aliases a ON a.id=s.id
), hreflang_ignored AS (
    SELECT page AS url,'hreflang-missing-return' AS issue_id FROM hreflang_rows
    WHERE canonical_context AND target<>page AND return_link=false
    GROUP BY page HAVING NOT EXISTS(SELECT 1 FROM hreflang_rows bad
        WHERE bad.page=hreflang_rows.page AND bad.target<>bad.page AND bad.return_link=false AND NOT bad.canonical_context)
    UNION SELECT url,'hreflang-missing-self' FROM hreflang_aliases
    UNION SELECT a.url,'hreflang-missing-x-default' FROM hreflang_aliases a
        WHERE EXISTS(SELECT 1 FROM hreflang h WHERE h.page=a.canonical_id AND lower(h.lang)='x-default')
)";

/// Current interpretation of historical observations, leaving `audit_findings` intact.
pub(super) fn findings() -> String {
    format!(
        "(WITH {CTES}, {} SELECT f.* FROM audit_findings f WHERE NOT EXISTS(SELECT 1 FROM hreflang_ignored i WHERE i.url=f.url AND i.issue_id=f.issue_id) AND NOT EXISTS(SELECT 1 FROM anchor_resolved i WHERE i.url=f.url AND i.issue_id=f.issue_id) AND NOT EXISTS(SELECT 1 FROM contact_resolved i WHERE i.url=f.url AND i.issue_id=f.issue_id) AND NOT EXISTS(SELECT 1 FROM anchor_unverified i WHERE i.url=f.url AND i.issue_id=f.issue_id))",
        crate::anchor_context::ctes()
    )
}

/// Exact URL return and canonical-cluster context are deliberately separate columns.
pub(super) fn rows() -> String {
    format!(
        "(WITH {CTES} SELECT h.page,h.lang,h.target,u.status AS target_status,h.source_canonical,h.return_link,h.canonical_context,h.pagination_candidate,h.canonical_content_equal FROM hreflang_rows h LEFT JOIN urls u ON u.url=h.target ORDER BY h.source_id,h.lang,h.target)"
    )
}
