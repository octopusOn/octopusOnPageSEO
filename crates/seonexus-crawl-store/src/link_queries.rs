//! Read-time link evidence, also available to existing archives without a migration.

pub(crate) const LINK_ROWS: &str = "(SELECT s.url AS source,l.target_url AS destination,l.kind,coalesce(n.name,l.text) AS text,l.nofollow,l.ugc,l.sponsored,l.internal,t.status AS destination_status,l.position,s.internal AS source_internal,coalesce(t.internal,l.internal) AS destination_internal,t.error AS destination_error,s.status AS source_status,l.text AS recorded_text,n.visible_text,n.name_source,n.has_svg,n.has_image,n.observation AS name_observation FROM links l JOIN urls s ON s.id=l.source LEFT JOIN urls t ON t.id=l.target LEFT JOIN current_link_names n ON n.source=l.source AND n.position=l.position AND n.target_url=l.target_url ORDER BY l.source,l.position)";

/// Aggregate hyperlink counts once per URL, without rewriting older tab views.
pub(crate) fn url_rows(tab: crate::CrawlTab) -> String {
    let external = if tab == crate::CrawlTab::External {
        ",coalesce(o.external_count,0) AS external_outlinks"
    } else {
        ""
    };
    format!(
        "(SELECT b.*,coalesce(i.internal_count,0) AS internal_inlinks,coalesce(i.external_count,0) AS external_inlinks,coalesce(o.internal_count,0) AS internal_outlinks{external} FROM {} b LEFT JOIN (SELECT l.target_url AS address,count(*) FILTER(WHERE s.internal) AS internal_count,count(*) FILTER(WHERE NOT s.internal) AS external_count FROM links l JOIN urls s ON s.id=l.source WHERE l.kind='hyperlink' GROUP BY l.target_url) i ON i.address=b.address LEFT JOIN (SELECT s.url AS address,count(*) FILTER(WHERE l.internal) AS internal_count,count(*) FILTER(WHERE NOT l.internal) AS external_count FROM links l JOIN urls s ON s.id=l.source WHERE l.kind='hyperlink' GROUP BY s.url) o ON o.address=b.address)",
        tab.view()
    )
}

/// Keep evidence aligned with the corresponding materialized finding's target predicate.
pub(crate) fn evidence_predicate(issue: &str) -> Option<String> {
    let predicate = match issue {
        "links-to-broken" => Some(
            "kind='hyperlink' AND destination_internal AND (destination_status>=400 OR destination_error IS NOT NULL)",
        ),
        "links-to-broken-external" => Some(
            "kind='hyperlink' AND NOT destination_internal AND (destination_status>=400 OR destination_error IS NOT NULL)",
        ),
        "links-to-redirects" => {
            Some("kind='hyperlink' AND destination_status IN (301,302,303,307,308)")
        }
        _ => None,
    }?;
    Some(if issue == "links-to-redirects" {
        format!(
            "{predicate} AND NOT(destination_error IS NULL AND ({}))",
            crate::anchor_context::contact_url("destination", "destination_internal")
        )
    } else {
        predicate.into()
    })
}
