//! Finding pairs are observations, never assertions that a check passed or an SEO fix succeeded.

use super::{CrawlComparison, CrawlStatus, CrawlStoreError, TablePage, invalid, literal};
use octopuson_core::table::TableSink;
use serde::{Deserialize, Serialize};

/// One disjoint state for an exact (rule id, stored URL) pair.
#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum FindingChange {
    /// Every finding pair.
    #[default]
    All,
    /// Recorded only now, on a shared URL with a comparable prior observation.
    Introduced,
    /// Recorded only previously, on a shared URL with a comparable current observation.
    NotDetected,
    /// Present in both archives.
    Persisting,
    /// Only now, with the URL present only in the current selected scope.
    NewUrl,
    /// Only previously, with the URL present only in the baseline selected scope.
    MissingUrl,
    /// Missing counterpart observation, changed scope/eligibility or an unknown historical rule.
    Unverified,
}
impl FindingChange {
    fn name(self) -> Option<&'static str> {
        match self {
            Self::All => None,
            Self::Introduced => Some("introduced"),
            Self::NotDetected => Some("not-detected"),
            Self::Persisting => Some("persisting"),
            Self::NewUrl => Some("new-url"),
            Self::MissingUrl => Some("missing-url"),
            Self::Unverified => Some("unverified"),
        }
    }
}

/// Identical filtering for the bounded page and full streaming export.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct FindingComparisonRequest {
    /// Finding state, or all.
    pub change: FindingChange,
    /// Exact rule id, including historical/unknown ids; empty means all.
    pub issue_id: String,
    /// high, medium, low, unknown or empty for all.
    pub priority: String,
    /// Case-insensitive literal URL substring.
    pub search: String,
    /// Findings belonging to stored internal URL records only; false also includes site-file findings.
    pub internal_only: bool,
    /// Rows to skip (ignored by export).
    pub offset: u64,
    /// Page size, clamped to 1–1000 (ignored by export).
    pub limit: u32,
}
impl Default for FindingComparisonRequest {
    fn default() -> Self {
        Self {
            change: FindingChange::All,
            issue_id: String::new(),
            priority: String::new(),
            search: String::new(),
            internal_only: true,
            offset: 0,
            limit: 250,
        }
    }
}

/// Aggregate counts for one rule, or the entire selected scope when id is empty.
#[derive(Debug, Serialize)]
pub struct FindingRuleCount {
    /// Stable rule id, or empty for totals.
    pub id: String,
    /// Current catalog priority, or unknown.
    pub priority: String,
    /// Current catalog category, or unknown.
    pub category: String,
    /// Recorded only in the current archive on comparable shared URLs.
    pub introduced: u64,
    /// No longer recorded on comparable shared URLs; does not imply a passing check.
    pub not_detected: u64,
    /// Recorded in both archives.
    pub persisting: u64,
    /// Findings on only-current URL scope.
    pub new_url: u64,
    /// Findings on only-baseline URL scope.
    pub missing_url: u64,
    /// Differences without a comparable counterpart.
    pub unverified: u64,
    /// Total finding pairs across all six disjoint states.
    pub total: u64,
}

/// Filtered totals plus a bounded per-rule picker for the full selected URL scope.
#[derive(Debug, Serialize)]
pub struct FindingComparisonSummary {
    /// Every pair matching URL/rule/priority filters, across all change states.
    pub counts: FindingRuleCount,
    /// Number of findings stored in the baseline scope.
    pub baseline: u64,
    /// Number of findings stored in the current scope.
    pub current: u64,
    /// First 500 rules by priority/id, including historical rules.
    pub rules: Vec<FindingRuleCount>,
    /// Exact number of distinct rules, even beyond the summary preview.
    pub rule_total: u64,
    /// Scope caveats; all historical comparisons also explain missing audit-version provenance.
    pub warnings: Vec<String>,
}

const COUNTS: &str = "count(*) FILTER(WHERE finding_change='introduced'),count(*) FILTER(WHERE finding_change='not-detected'),count(*) FILTER(WHERE finding_change='persisting'),count(*) FILTER(WHERE finding_change='new-url'),count(*) FILTER(WHERE finding_change='missing-url'),count(*) FILTER(WHERE finding_change='unverified'),count(*)";
const COLUMNS: &str = "address,issue_id,finding_change,comparison_reason,priority,category,before_finding,after_finding,before_status_code,after_status_code,before_indexability,after_indexability,before_error,after_error,before_render_error,after_render_error";
const ORDER: &str = "ORDER BY CASE priority WHEN 'high' THEN 0 WHEN 'medium' THEN 1 WHEN 'low' THEN 2 ELSE 3 END, CASE finding_change WHEN 'introduced' THEN 0 WHEN 'not-detected' THEN 1 WHEN 'unverified' THEN 2 WHEN 'new-url' THEN 3 WHEN 'missing-url' THEN 4 ELSE 5 END,issue_id,address";
// These rules examine HTTP/robots outcomes, not extracted HTML. Other rules use a conservative
// successful indexable HTML observation. This is an absence guard, not a stored pass/evaluation log.
const HTTP_RULES: &[&str] = &[
    "internal-no-response",
    "external-no-response",
    "internal-client-error",
    "external-client-error",
    "internal-server-error",
    "external-server-error",
    "internal-redirect",
    "internal-redirect-temporary",
    "internal-blocked-robots",
];
const HREFLANG_RULES: &[&str] = &[
    "hreflang-invalid-code",
    "hreflang-missing-return",
    "hreflang-to-non-200",
    "hreflang-to-non-indexable",
    "hreflang-missing-self",
    "hreflang-missing-x-default",
    "hreflang-lang-mismatch",
];

impl CrawlComparison {
    fn finding_relation(&self, internal_only: bool) -> String {
        let scope = if internal_only {
            " WHERE EXISTS(SELECT 1 FROM before_urls u WHERE u.url=f.url AND u.internal)"
        } else {
            ""
        };
        let after_scope = scope.replace("before_urls", "after_urls");
        let catalog = octopuson_audit::catalog()
            .issues
            .iter()
            .map(|rule| {
                format!(
                    "({},{},{})",
                    literal(&rule.id),
                    literal(rule.priority.as_str()),
                    literal(&rule.category)
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        let compatible = self.baseline.status == CrawlStatus::Completed
            && self.current.status == CrawlStatus::Completed
            && !self.baseline.limit_reached
            && !self.current.limit_reached
            && self.baseline.mode == octopuson_crawl::CrawlMode::Spider
            && self.current.mode == self.baseline.mode
            && self.baseline.start_urls == self.current.start_urls
            && self.baseline.config == self.current.config;
        let baseline_js =
            self.baseline.config.rendering.mode == octopuson_crawl::RenderMode::Javascript;
        let current_js =
            self.current.config.rendering.mode == octopuson_crawl::RenderMode::Javascript;
        let before_problem = observation_problem("b", baseline_js);
        let after_problem = observation_problem("a", current_js);
        let before_hreflang = hreflang_observation_problem("b", baseline_js);
        let after_hreflang = hreflang_observation_problem("a", current_js);
        let hreflang = HREFLANG_RULES
            .iter()
            .map(|id| literal(id))
            .collect::<Vec<_>>()
            .join(",");
        let http = HTTP_RULES
            .iter()
            .map(|id| literal(id))
            .collect::<Vec<_>>()
            .join(",");
        let join_scope = if internal_only { " AND b.internal" } else { "" };
        let after_join_scope = join_scope.replace("b.internal", "a.internal");
        let fields = ["status_code", "indexability", "error", "render_error"]
            .iter()
            .map(|f| format!("b.{f} AS before_{f},a.{f} AS after_{f}"))
            .collect::<Vec<_>>()
            .join(",");
        format!("(WITH catalog(issue_id,priority,category) AS (VALUES {catalog}), finding_pairs AS (
            SELECT coalesce(b.url,a.url) AS address,coalesce(b.issue_id,a.issue_id) AS issue_id,b.issue_id IS NOT NULL AS before_finding,a.issue_id IS NOT NULL AS after_finding
            FROM (SELECT issue_id,url FROM before_crawl.audit_findings f{scope}) b FULL OUTER JOIN (SELECT issue_id,url FROM after_crawl.audit_findings f{after_scope}) a ON b.issue_id=a.issue_id AND b.url=a.url
        ), observations AS (
            SELECT f.*,coalesce(c.priority,'unknown') AS priority,coalesce(c.category,'unknown') AS category,
            CASE WHEN before_finding AND after_finding THEN ''
                 WHEN b.url IS NULL AND a.url IS NOT NULL AND after_finding THEN 'url-only-current'
                 WHEN a.url IS NULL AND b.url IS NOT NULL AND before_finding THEN 'url-only-baseline'
                 WHEN b.url IS NULL OR a.url IS NULL THEN 'no-url-observation'
                 WHEN b.internal IS DISTINCT FROM a.internal THEN 'classification-changed'
                 WHEN c.issue_id IS NULL THEN 'unknown-rule'
                 WHEN NOT {compatible} THEN 'scope-not-comparable'
                 WHEN starts_with(f.issue_id,'schema-') AND NOT (CASE WHEN after_finding THEN EXISTS(SELECT 1 FROM before_schema_observed s WHERE s.url=f.address) ELSE EXISTS(SELECT 1 FROM after_schema_observed s WHERE s.url=f.address) END) THEN 'schema-check-unobserved'
                 WHEN f.issue_id IN ({hreflang}) AND NOT EXISTS(SELECT 1 FROM before_crawl.crawl_info WHERE key='http_hreflang_observed' AND value='1') THEN 'before-hreflang-unrecorded'
                 WHEN f.issue_id IN ({hreflang}) AND NOT EXISTS(SELECT 1 FROM after_crawl.crawl_info WHERE key='http_hreflang_observed' AND value='1') THEN 'after-hreflang-unrecorded'
                 WHEN f.issue_id='render-noindex-removed' AND (CASE WHEN after_finding THEN b.audit_raw_noindex ELSE a.audit_raw_noindex END) IS NULL THEN CASE WHEN after_finding THEN 'before-audit-baseline-unrecorded' ELSE 'after-audit-baseline-unrecorded' END
                 ELSE CASE WHEN f.issue_id IN ({http}) THEN
                    CASE WHEN after_finding THEN CASE WHEN b.blocked_by_robots='true' OR b.error IS NOT NULL OR b.status_code IS NULL THEN 'before-http-unobserved' ELSE '' END
                         ELSE CASE WHEN a.blocked_by_robots='true' OR a.error IS NOT NULL OR a.status_code IS NULL THEN 'after-http-unobserved' ELSE '' END END
                    WHEN f.issue_id IN ({hreflang}) THEN CASE WHEN after_finding THEN {before_hreflang} ELSE {after_hreflang} END
                    ELSE CASE WHEN after_finding THEN {before_problem} ELSE {after_problem} END END END AS comparison_reason,
            {fields}
            FROM finding_pairs f LEFT JOIN before_urls b ON b.url=f.address{join_scope} LEFT JOIN after_urls a ON a.url=f.address{after_join_scope} LEFT JOIN catalog c ON c.issue_id=f.issue_id
        ) SELECT *, CASE WHEN before_finding AND after_finding THEN 'persisting' WHEN comparison_reason='url-only-current' THEN 'new-url' WHEN comparison_reason='url-only-baseline' THEN 'missing-url' WHEN comparison_reason<>'' THEN 'unverified' WHEN after_finding THEN 'introduced' ELSE 'not-detected' END AS finding_change FROM observations) finding_comparison")
    }

    fn finding_selection(
        &self,
        request: &FindingComparisonRequest,
    ) -> Result<String, CrawlStoreError> {
        if !["", "high", "medium", "low", "unknown"].contains(&request.priority.as_str()) {
            return Err(invalid("unknown finding priority"));
        }
        let mut filters = vec!["true".to_owned()];
        if let Some(change) = request.change.name() {
            filters.push(format!("finding_change={}", literal(change)));
        }
        if !request.issue_id.is_empty() {
            filters.push(format!("issue_id={}", literal(&request.issue_id)));
        }
        if !request.priority.is_empty() {
            filters.push(format!("priority={}", literal(&request.priority)));
        }
        if !request.search.is_empty() {
            filters.push(format!(
                "contains(lower(address),lower({}))",
                literal(&request.search)
            ));
        }
        Ok(format!(
            "FROM {} WHERE {}",
            self.finding_relation(request.internal_only),
            filters.join(" AND ")
        ))
    }

    /// Counts six disjoint finding states and a bounded per-rule breakdown.
    ///
    /// # Errors
    /// Database errors.
    pub fn findings_summary(
        &self,
        internal_only: bool,
    ) -> Result<FindingComparisonSummary, CrawlStoreError> {
        self.filtered_findings_summary(&FindingComparisonRequest {
            internal_only,
            ..FindingComparisonRequest::default()
        })
    }

    /// Counts matching findings across all states, ignoring state and pagination.
    /// Rule-picker counts still cover the full selected URL scope.
    ///
    /// # Errors
    /// Invalid priority or database errors.
    pub fn filtered_findings_summary(
        &self,
        request: &FindingComparisonRequest,
    ) -> Result<FindingComparisonSummary, CrawlStoreError> {
        let from = self.finding_selection(&FindingComparisonRequest {
            change: FindingChange::All,
            ..request.clone()
        })?;
        let scope = format!("FROM {}", self.finding_relation(request.internal_only));
        let counts =
            self.reader
                .connection
                .query_row(&format!("SELECT {COUNTS} {from}"), [], |row| {
                    read_counts(row, String::new(), String::new(), String::new(), 0)
                })?;
        let (baseline,current):(u64,u64)=self.reader.connection.query_row(&format!("SELECT count(*) FILTER(WHERE before_finding),count(*) FILTER(WHERE after_finding) {from}"),[],|row|Ok((row.get(0)?,row.get(1)?)))?;
        let rule_total = self
            .reader
            .scalar(&format!("SELECT count(DISTINCT issue_id) {scope}"))?;
        let mut statement=self.reader.connection.prepare(&format!("SELECT issue_id,priority,category,{COUNTS} {scope} GROUP BY issue_id,priority,category ORDER BY CASE priority WHEN 'high' THEN 0 WHEN 'medium' THEN 1 WHEN 'low' THEN 2 ELSE 3 END,issue_id LIMIT 500"))?;
        let rules = statement
            .query_map([], |row| {
                read_counts(row, row.get(0)?, row.get(1)?, row.get(2)?, 3)
            })?
            .collect::<Result<Vec<_>, _>>()?;
        let mut warnings = self.observation_warnings(request.internal_only)?;
        warnings.push("finding-observations".into());
        Ok(FindingComparisonSummary {
            counts,
            baseline,
            current,
            rules,
            rule_total,
            warnings,
        })
    }

    /// A bounded page of exact (rule, URL) pairs. Scope is applied independently to both archives.
    ///
    /// # Errors
    /// Invalid priority or database errors.
    pub fn findings_page(
        &self,
        request: &FindingComparisonRequest,
    ) -> Result<TablePage, CrawlStoreError> {
        let from = self.finding_selection(request)?;
        let total = self.reader.scalar(&format!("SELECT count(*) {from}"))?;
        self.reader.collect(
            &format!(
                "SELECT {COLUMNS} {from} {ORDER} LIMIT {} OFFSET {}",
                request.limit.clamp(1, 1000),
                request.offset
            ),
            total,
        )
    }

    /// Streams every matching finding pair using the same filters, columns and order as the page.
    ///
    /// # Errors
    /// Invalid priority, database or output errors.
    pub fn export_findings(
        &self,
        request: &FindingComparisonRequest,
        sink: &mut dyn TableSink,
    ) -> Result<u64, CrawlStoreError> {
        self.reader.query(
            &format!(
                "SELECT {COLUMNS} {} {ORDER}",
                self.finding_selection(request)?
            ),
            sink,
        )
    }
}

fn observation_problem(alias: &str, require_js: bool) -> String {
    let prefix = if alias == "b" { "before" } else { "after" };
    format!(
        "CASE WHEN {alias}.blocked_by_robots='true' OR {alias}.error IS NOT NULL OR {alias}.status_code IS NULL THEN '{prefix}-http-unobserved' WHEN {alias}.status_code<>'200' OR {alias}.content<>'html' THEN '{prefix}-not-html-200' WHEN {alias}.indexability<>'indexable' THEN '{prefix}-not-indexable' WHEN {alias}.truncated='true' THEN '{prefix}-incomplete-content' WHEN {alias}.render_error IS NOT NULL OR ({require_js} AND {alias}.rendered IS DISTINCT FROM 'true') THEN '{prefix}-rendering-unverified' ELSE '' END"
    )
}

fn hreflang_observation_problem(alias: &str, require_js: bool) -> String {
    let prefix = if alias == "b" { "before" } else { "after" };
    let html = observation_problem(alias, require_js);
    // PDF annotations come from complete response headers; body truncation and JS are
    // irrelevant to their capture. HTML still needs parsed facts and its existing guards.
    format!(
        "CASE WHEN {alias}.blocked_by_robots='true' OR {alias}.error IS NOT NULL OR {alias}.status_code IS NULL THEN '{prefix}-http-unobserved' WHEN {alias}.status_code<>'200' OR {alias}.content NOT IN ('html','pdf') THEN '{prefix}-not-hreflang-200' WHEN {alias}.indexability<>'indexable' THEN '{prefix}-not-indexable' WHEN {alias}.content='pdf' THEN '' WHEN {alias}.title_count IS NULL THEN '{prefix}-hreflang-html-unparsed' ELSE {html} END"
    )
}
fn read_counts(
    row: &duckdb::Row<'_>,
    id: String,
    priority: String,
    category: String,
    offset: usize,
) -> Result<FindingRuleCount, duckdb::Error> {
    Ok(FindingRuleCount {
        id,
        priority,
        category,
        introduced: row.get(offset)?,
        not_detected: row.get(offset + 1)?,
        persisting: row.get(offset + 2)?,
        new_url: row.get(offset + 3)?,
        missing_url: row.get(offset + 4)?,
        unverified: row.get(offset + 5)?,
        total: row.get(offset + 6)?,
    })
}
