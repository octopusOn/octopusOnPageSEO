//! Versioned, explainable technical health of the observed internal URL sample.

use serde::Serialize;

use crate::{CrawlReader, CrawlStoreError};

/// One rule's share of the capped average deduction, over all scored internal URLs.
#[derive(Debug, Clone, Serialize)]
pub struct ScoreDeduction {
    /// Shared catalog identifier, suitable for opening the corresponding evidence.
    pub id: String,
    /// Distinct internal URLs with this finding.
    pub count: u64,
    /// Points lost, apportioned after the per-URL cap.
    pub points: f64,
}

/// A branch's independent health and its contribution to the overall loss.
#[derive(Debug, Clone, Serialize)]
pub struct ScoreCategory {
    /// Catalog category identifier.
    pub id: String,
    /// Independent per-branch average, not an additive part of the global score.
    pub value: Option<f64>,
    /// Attributed points lost from the global score.
    pub points: f64,
    /// Eligible/evidenced URLs for this branch.
    pub urls: u64,
    /// Distinct URLs with a scored finding in this branch.
    pub affected_urls: u64,
    /// Defined catalog checks; not a claim that every check ran on every URL.
    pub checks: usize,
    /// All contributing checks in descending point order.
    pub deductions: Vec<ScoreDeduction>,
}

/// Original rule observations removed by verified scoring exceptions.
#[derive(Debug, Clone, Serialize)]
pub struct ScoreIgnoredRule {
    /// Shared catalog identifier.
    pub id: String,
    /// Distinct source URL findings omitted from scoring.
    pub count: u64,
}

/// Model v4 preserves historical scoring; opted-in Schema property checks use v5.
#[derive(Debug, Clone, Serialize)]
pub struct SeoScore {
    /// Algorithm revision; the UI must not silently interpret a different model.
    pub version: u8,
    /// 0–100, one decimal; unknown without analyzed HTML or with unknown stored rules.
    pub value: Option<f64>,
    /// Distinct recorded internal URLs, including resources and HTTP failures.
    pub urls: u64,
    /// Internal URLs with at least one recognized finding.
    pub affected_urls: u64,
    /// Internal HTTP 200 HTML pages with observed page facts.
    pub analyzed_html: u64,
    /// Internal URLs with a missing/blocked response, truncated body or failed render.
    pub limited_urls: u64,
    /// Internal finding records whose rule is absent from this binary's catalog.
    pub unknown_findings: u64,
    /// Complete rule contributions in descending order; not rounded before aggregation.
    pub deductions: Vec<ScoreDeduction>,
    /// Known infrastructure addresses excluded from the scored sample.
    pub excluded_urls: u64,
    /// Recorded references to known infrastructure from internal sources.
    pub excluded_links: u64,
    /// Stored findings omitted after URL/reference-specific exception handling.
    pub excluded_findings: u64,
    /// Historical hreflang absences resolved by the verified canonical cluster.
    pub canonical_context_findings: u64,
    /// Historical anchor findings resolved from recorded naming evidence.
    pub anchor_context_findings: u64,
    /// Optional x-default/link-density recommendations, retained without penalties.
    pub advisory_findings: u64,
    /// Expected successful external `WhatsApp` phone-route redirects, not broken links.
    pub contact_redirect_findings: u64,
    /// Historical anchor findings without complete naming observations; not confirmed failures.
    pub unverified_anchor_findings: u64,
    /// Every catalog branch, including branches without sufficient observations.
    pub categories: Vec<ScoreCategory>,
    /// Exact excluded finding counts by rule, separate from scored deductions.
    pub excluded_rules: Vec<ScoreIgnoredRule>,
    /// Present only for crawls explicitly opting into structured-data property validation.
    pub schema_validation: Option<crate::SchemaCoverage>,
}

impl CrawlReader {
    pub(crate) fn seo_score(&self) -> Result<SeoScore, CrawlStoreError> {
        self.prepare_anchors()?;
        let schema_validation = crate::schema_checks::coverage(&self.connection)?;
        let catalog = seonexus_audit::catalog()
            .issues
            .iter()
            .map(|rule| {
                let weight = match rule.priority {
                    seonexus_audit::Priority::High => 20,
                    seonexus_audit::Priority::Medium => 7,
                    seonexus_audit::Priority::Low => 2,
                };
                format!(
                    "('{}',{weight},'{}')",
                    rule.id.replace('\'', "''"),
                    rule.category.replace('\'', "''")
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        let exceptions = crate::score_exclusions::ctes();
        let excluded_target = crate::score_exclusions::system_url("l.target_url");
        let base = format!("WITH catalog(issue_id,weight,category) AS (VALUES {catalog}), {exceptions}, findings AS (
            SELECT f.url,f.issue_id,c.weight,c.category FROM scored_findings f LEFT JOIN catalog c ON c.issue_id=f.issue_id
        ), penalties AS (
            SELECT url,sum(weight) AS raw FROM findings WHERE weight IS NOT NULL GROUP BY url
        )");
        let mut score = self.connection.query_row(&format!("{base}
            SELECT count(*),count(p.url),count(*) FILTER(WHERE o.analyzed_html),count(*) FILTER(WHERE o.limited),
                (SELECT count(*) FROM findings WHERE weight IS NULL),coalesce(avg(least(100,coalesce(p.raw,0))),0),
                (SELECT count(DISTINCT url) FROM all_observed WHERE excluded),
                (SELECT count(*) FROM links l JOIN all_observed s ON s.id=l.source WHERE {excluded_target}),
                (SELECT count(*) FROM original_findings)-(SELECT count(*) FROM scored_findings),
                (SELECT count(*) FROM original_findings f JOIN observed o ON o.url=f.url JOIN hreflang_ignored i ON i.url=f.url AND i.issue_id=f.issue_id),
                (SELECT count(*) FROM original_findings f JOIN observed o ON o.url=f.url JOIN anchor_resolved i ON i.url=f.url AND i.issue_id=f.issue_id),
                (SELECT count(*) FROM original_findings f JOIN observed o ON o.url=f.url WHERE f.issue_id IN ('hreflang-missing-x-default','links-too-many')),
                (SELECT count(*) FROM original_findings f JOIN observed o ON o.url=f.url JOIN contact_resolved i ON i.url=f.url AND i.issue_id=f.issue_id),
                (SELECT count(*) FROM original_findings f JOIN observed o ON o.url=f.url JOIN anchor_unverified i ON i.url=f.url AND i.issue_id=f.issue_id)
            FROM observed o LEFT JOIN penalties p ON p.url=o.url"), [], |row| {
            let urls: u64 = row.get(0)?;
            let analyzed_html: u64 = row.get(2)?;
            let unknown_findings: u64 = row.get(4)?;
            let loss: f64 = row.get(5)?;
            Ok(SeoScore {
                version: if schema_validation.is_some() { 5 } else { 4 },
                value: (urls > 0 && analyzed_html > 0 && unknown_findings == 0)
                    .then(|| ((100.0-loss).clamp(0.0,100.0)*10.0).round()/10.0),
                urls, affected_urls:row.get(1)?, analyzed_html, limited_urls:row.get(3)?, unknown_findings,
                deductions:Vec::new(),
                excluded_urls:row.get(6)?,excluded_links:row.get(7)?,excluded_findings:row.get(8)?,categories:Vec::new(),excluded_rules:Vec::new(),
                canonical_context_findings:row.get(9)?,
                anchor_context_findings:row.get(10)?,advisory_findings:row.get(11)?,
                contact_redirect_findings:row.get(12)?,
                unverified_anchor_findings:row.get(13)?,
                schema_validation: schema_validation.clone(),
            })
        })?;
        if score.urls > 0 {
            let mut statement = self.connection.prepare(&format!("{base}
                SELECT f.issue_id,count(*),sum(f.weight*least(100,p.raw)::DOUBLE/p.raw)/(SELECT count(*) FROM observed) AS points
                FROM findings f JOIN penalties p ON p.url=f.url WHERE f.weight IS NOT NULL
                GROUP BY f.issue_id ORDER BY points DESC,f.issue_id"))?;
            score.deductions = statement
                .query_map([], |row| {
                    Ok(ScoreDeduction {
                        id: row.get(0)?,
                        count: row.get(1)?,
                        points: row.get(2)?,
                    })
                })?
                .collect::<Result<_, _>>()?;
        }
        score.categories = self.score_categories(&base, &score)?;
        let mut statement = self.connection.prepare(&format!("{base} SELECT f.issue_id,count(*) FROM original_findings f WHERE NOT EXISTS(SELECT 1 FROM scored_findings s WHERE s.url=f.url AND s.issue_id=f.issue_id) GROUP BY f.issue_id ORDER BY count(*) DESC,f.issue_id"))?;
        score.excluded_rules = statement
            .query_map([], |row| {
                Ok(ScoreIgnoredRule {
                    id: row.get(0)?,
                    count: row.get(1)?,
                })
            })?
            .collect::<Result<_, _>>()?;
        Ok(score)
    }

    fn score_categories(
        &self,
        base: &str,
        score: &SeoScore,
    ) -> Result<Vec<ScoreCategory>, CrawlStoreError> {
        let coverage: [u64;6] = self.connection.query_row(&format!("{base} SELECT
            count(*) FILTER(WHERE analyzed_html),
            (SELECT count(DISTINCT i.page) FROM images i JOIN all_observed o ON o.id=i.page WHERE NOT o.excluded),
            (SELECT count(DISTINCT r.page) FROM rendering r JOIN all_observed o ON o.id=r.page WHERE NOT o.excluded),
            (SELECT count(DISTINCT u.url) FROM urls u JOIN observed o ON o.url=u.url WHERE u.rel_next IS NOT NULL OR u.rel_prev IS NOT NULL),
            (SELECT count(*) FROM sitemaps),count(*) FROM observed"),[],|row|Ok([row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get(4)?,row.get(5)?]))?;
        let mut statement = self.connection.prepare(&format!("{base} SELECT category,count(*),sum(least(100,raw)) FROM (SELECT url,category,sum(weight) AS raw FROM findings WHERE weight IS NOT NULL GROUP BY url,category) GROUP BY category"))?;
        let losses = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    (row.get::<_, u64>(1)?, row.get::<_, f64>(2)?),
                ))
            })?
            .collect::<Result<std::collections::HashMap<_, _>, _>>();
        drop(statement);
        let losses = losses?;
        Ok(seonexus_audit::catalog()
            .categories
            .iter()
            .map(|category| {
                let rules: Vec<_> = seonexus_audit::catalog()
                    .issues
                    .iter()
                    .filter(|rule| rule.category == category.id)
                    .collect();
                let deductions: Vec<_> = score
                    .deductions
                    .iter()
                    .filter(|row| rules.iter().any(|rule| rule.id == row.id))
                    .cloned()
                    .collect();
                let (affected, loss) = losses.get(&category.id).copied().unwrap_or((0, 0.0));
                let observed = match category.id.as_str() {
                    "response-codes" | "security" | "url" => coverage[5],
                    "images" => coverage[1],
                    "rendering" => coverage[2],
                    "pagination" => coverage[3],
                    "sitemaps" => {
                        if coverage[4] > 0 {
                            coverage[0]
                        } else {
                            0
                        }
                    }
                    "robots" | "ai" => 0,
                    "structured-data" if score.schema_validation.is_some() => score
                        .schema_validation
                        .as_ref()
                        .map_or(0, |c| c.scored_urls),
                    _ => coverage[0],
                }
                .max(affected);
                // URLs have unique UINTEGER IDs; this count is at most 2^32 and exactly representable.
                #[allow(clippy::cast_precision_loss)]
                let denominator = observed as f64;
                ScoreCategory {
                    id: category.id.clone(),
                    value: (observed > 0
                        && score.unknown_findings == 0
                        && !(category.id == "links" && score.unverified_anchor_findings > 0))
                        .then(|| {
                            ((100.0 - loss / denominator).clamp(0.0, 100.0) * 10.0).round() / 10.0
                        }),
                    points: deductions.iter().fold(0.0, |total, row| total + row.points),
                    urls: observed,
                    affected_urls: affected,
                    checks: rules.len(),
                    deductions,
                }
            })
            .collect())
    }
}
