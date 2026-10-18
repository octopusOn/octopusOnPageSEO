//! Bounded, read-only witnesses for score-based remediation.

use octopuson_core::table::Cell;
use serde::{Deserialize, Serialize};

use crate::{CrawlReader, CrawlStoreError, TablePage};

/// One scored rule's URL list and the selected URL's recorded witnesses.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct RemediationRequest {
    /// Known catalog check.
    pub issue_id: String,
    /// Literal URL substring; never interpreted as a pattern or SQL.
    pub search: String,
    /// URL-list offset.
    pub offset: u64,
    /// URL-list page size, clamped to 1–50.
    pub limit: u64,
    /// Exact URL on this page, or empty to select its first row.
    pub url: String,
    /// Witness offset for the selected URL (50 witnesses per page).
    pub evidence_offset: u64,
}

/// Recorded evidence, deliberately separate from generated solution examples.
#[derive(Debug, Clone, Serialize)]
pub struct RemediationDetail {
    /// Scored internal URLs only, with complete matching count.
    pub urls: TablePage,
    /// Exact selected URL, absent for an empty result.
    pub selected_url: Option<String>,
    /// Original recorded page facts; no network requests or inferred page copy.
    pub facts: TablePage,
    /// Rule-specific witnesses or clearly labeled category context.
    pub evidence: TablePage,
    /// Witness type, used to label the evidence rather than inventing a diagnosis.
    pub evidence_kind: String,
    /// Optional saved property-level Schema witnesses, with paths and correction examples.
    pub schema_report: Option<serde_json::Value>,
}

fn literal(text: &str) -> String {
    format!("'{}'", text.replace('\'', "''"))
}

impl CrawlReader {
    /// Returns only URLs that participate in this rule's score, preserving exclusions.
    ///
    /// # Errors
    /// Unknown checks, invalid selected URLs and database failures.
    pub fn remediation(
        &self,
        request: &RemediationRequest,
    ) -> Result<RemediationDetail, CrawlStoreError> {
        self.prepare_anchors()?;
        let rule = octopuson_audit::catalog()
            .issue(&request.issue_id)
            .ok_or_else(|| CrawlStoreError::InvalidSelection("unknown remediation check".into()))?;
        let base = format!(
            "WITH {}, eligible AS (SELECT url FROM scored_findings WHERE issue_id={} AND contains(lower(url),lower({})))",
            crate::score_exclusions::ctes(),
            literal(&rule.id),
            literal(&request.search)
        );
        let total = self.scalar(&format!("{base} SELECT count(*) FROM eligible"))?;
        let limit = request.limit.clamp(1, 50);
        let urls = self.collect(
            &format!(
                "{base} SELECT url AS address FROM eligible ORDER BY url LIMIT {limit} OFFSET {}",
                request.offset
            ),
            total,
        )?;
        let addresses: Vec<_> = urls
            .rows
            .iter()
            .filter_map(|row| match row.first() {
                Some(Cell::Text(url)) => Some(url.clone()),
                _ => None,
            })
            .collect();
        let selected_url = if request.url.is_empty() {
            addresses.first().cloned()
        } else if addresses.contains(&request.url) {
            Some(request.url.clone())
        } else {
            return Err(CrawlStoreError::InvalidSelection(
                "selected remediation URL is not on this scored result page".into(),
            ));
        };
        let empty = || self.collect("SELECT url AS address FROM urls WHERE false", 0);
        let Some(url) = selected_url.as_ref() else {
            return Ok(RemediationDetail {
                urls,
                selected_url,
                facts: empty()?,
                evidence: empty()?,
                evidence_kind: "facts".into(),
                schema_report: None,
            });
        };
        let quoted = literal(url);
        let facts = self.collect(&format!("WITH {} SELECT u.*,r.rendered,r.error AS render_error,r.elapsed_ms AS render_time_ms,r.raw_title,r.raw_word_count,r.raw_link_count,r.rendered_link_count,u.text_hash=c.text_hash AS canonical_content_equal,coalesce(u.rel_next IS NOT NULL OR u.rel_prev IS NOT NULL OR regexp_matches(u.url,'[?&]([a-zA-Z0-9_-]*[_-])?(page|paged|pagenum|page_num|page_number|offset|start)=[0-9]+(&|$)'),false) AS pagination_candidate,EXISTS(SELECT 1 FROM hreflang_aliases a WHERE a.id=u.id) AS canonical_cluster_verified FROM urls u LEFT JOIN rendering r ON r.page=u.id LEFT JOIN urls c ON c.url=coalesce(u.canonical,u.header_canonical) WHERE u.url={quoted} ORDER BY u.id LIMIT 1",crate::hreflang_context::CTES),1)?;
        let (kind, sql) = witness_sql(&rule.id, &rule.category, &quoted);
        let count = self.scalar(&format!("SELECT count(*) FROM ({sql}) witnesses"))?;
        let evidence = self.collect(
            &format!("{sql} LIMIT 50 OFFSET {}", request.evidence_offset),
            count,
        )?;
        Ok(RemediationDetail {
            schema_report: if rule.category == "structured-data" {
                crate::schema_checks::report(&self.connection, url)?
            } else {
                None
            },
            urls,
            selected_url,
            facts,
            evidence,
            evidence_kind: kind.into(),
        })
    }
}

fn witness_sql(issue: &str, category: &str, url: &str) -> (&'static str, String) {
    let target = crate::score_exclusions::system_url("l.target_url");
    if let Some(predicate) = link_predicate(issue) {
        return (
            "links",
            format!(
                "SELECT l.target_url AS destination,t.status AS destination_status,t.error AS destination_error,l.text,l.nofollow,l.internal AS destination_internal,l.position,l.recorded_text,l.visible_text,l.name_source,l.has_svg,l.has_image,l.observation AS name_observation FROM {} l JOIN urls s ON s.id=l.source LEFT JOIN urls t ON t.id=l.target WHERE s.url={url} AND l.kind='hyperlink' AND NOT ({target}) AND ({predicate}) ORDER BY l.position,l.target_url",
                crate::anchor_context::LINKS
            ),
        );
    }
    if issue.starts_with("hreflang-") {
        let filter = match issue {
            "hreflang-missing-return" => {
                "AND h.target<>h.page AND h.return_link=false AND NOT h.canonical_context"
            }
            "hreflang-to-non-200" => "AND (t.status<>200 OR t.error IS NOT NULL)",
            "hreflang-to-non-indexable" => "AND t.indexability<>'indexable'",
            _ => "",
        };
        return (
            "hreflang",
            format!(
                "WITH {} SELECT h.lang,h.target,t.status AS target_status,t.indexability AS target_indexability,t.lang AS target_lang,h.return_link,h.self_reference,h.source_canonical,h.canonical_context,h.pagination_candidate,h.canonical_content_equal FROM hreflang_rows h LEFT JOIN urls t ON t.url=h.target WHERE h.page={url} {filter} ORDER BY h.lang,h.target",
                crate::hreflang_context::CTES
            ),
        );
    }
    if category == "images" {
        let filter = match issue {
            "image-broken" => "AND (t.status>=400 OR t.error IS NOT NULL)",
            "image-missing-alt" => "AND i.alt IS NULL",
            "image-alt-too-long" => "AND length(i.alt)>100",
            "image-too-large" => "AND t.size_bytes>102400",
            "image-missing-dimensions" => "AND NOT i.has_dimensions",
            _ => "",
        };
        return (
            "images",
            format!(
                "SELECT i.src AS image,i.alt,i.has_dimensions,t.status AS status_code,t.size_bytes FROM images i JOIN urls s ON s.id=i.page LEFT JOIN urls t ON t.url=i.src WHERE s.url={url} {filter} ORDER BY i.position,i.src"
            ),
        );
    }
    if let Some(field) = duplicate_field(issue) {
        return (
            "duplicates",
            format!(
                "SELECT t.url AS address,t.{field} AS matched_value FROM urls s JOIN urls t ON t.{field}=s.{field} WHERE s.url={url} AND t.url<>s.url AND t.internal AND t.content_kind='html' AND t.status=200 AND t.indexability='indexable' ORDER BY t.url"
            ),
        );
    }
    if category == "headings" {
        let filter = if issue == "headings-non-sequential" {
            "AND level>previous_level+1"
        } else {
            ""
        };
        return (
            "headings",
            format!(
                "SELECT position,previous_level,previous_text,level,text FROM (SELECT page,position,level,text,lag(level,1,0) OVER(PARTITION BY page ORDER BY position) AS previous_level,lag(text) OVER(PARTITION BY page ORDER BY position) AS previous_text FROM headings) h JOIN urls s ON s.id=h.page WHERE s.url={url} {filter} ORDER BY position"
            ),
        );
    }
    if category == "performance" || category == "security" {
        return (
            "headers",
            format!(
                "SELECT h.name,h.value FROM response_headers h JOIN urls s ON s.id=h.page WHERE s.url={url} ORDER BY lower(h.name),h.value"
            ),
        );
    }
    if category == "structured-data" {
        return (
            "structured-data",
            format!(
                "SELECT d.format,d.types,d.valid AS json_syntax_valid FROM structured_data d JOIN urls s ON s.id=d.page WHERE s.url={url} ORDER BY d.position"
            ),
        );
    }
    (
        "facts",
        format!(
            "SELECT url AS address,status AS status_code,indexability FROM urls WHERE url={url} ORDER BY id"
        ),
    )
}

fn duplicate_field(issue: &str) -> Option<&'static str> {
    match issue {
        "title-duplicate" => Some("title"),
        "description-duplicate" => Some("meta_description"),
        "h1-duplicate" => Some("h1"),
        "h2-duplicate" => Some("h2"),
        _ => None,
    }
}

fn link_predicate(issue: &str) -> Option<String> {
    match issue {
        "links-to-broken" => Some("l.internal AND (t.status>=400 OR t.error IS NOT NULL)".into()),
        "links-to-broken-external" => {
            Some("NOT l.internal AND (t.status>=400 OR t.error IS NOT NULL)".into())
        }
        "links-to-redirects" => Some(format!(
            "t.status IN (301,302,303,307,308) AND NOT(t.error IS NULL AND ({}))",
            crate::anchor_context::contact_url("l.target_url", "l.internal")
        )),
        "anchor-empty" => Some("trim(l.text)='' AND l.name_source IS NOT NULL".into()),
        "anchor-generic" => Some(format!(
            "({}) AND l.name_source IS NOT NULL",
            crate::score_exclusions::generic_anchor("l.text")
        )),
        "links-internal-nofollow" => Some("l.internal AND l.nofollow".into()),
        "http-links" => {
            Some("starts_with(s.url,'https://') AND starts_with(l.target_url,'http://')".into())
        }
        "links-too-many" => Some("l.internal".into()),
        _ => None,
    }
}
