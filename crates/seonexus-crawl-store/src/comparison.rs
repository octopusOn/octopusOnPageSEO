//! Read-only, exact-URL comparisons. Joins and exports stay in DuckDB, not UI memory.

use seonexus_core::table::TableSink;
use serde::{Deserialize, Serialize};

use crate::{CrawlLibrary, CrawlMeta, CrawlReader, CrawlStatus, CrawlStoreError, TablePage};

mod findings;
pub use findings::{
    FindingChange, FindingComparisonRequest, FindingComparisonSummary, FindingRuleCount,
};

// Column names are static and used as SQL identifiers only after validation.
const FIELDS: &[&str] = &[
    "status_code",
    "error",
    "blocked_by_robots",
    "content",
    "indexability",
    "redirect_url",
    "title",
    "title_count",
    "meta_description",
    "meta_description_count",
    "h1",
    "h1_count",
    "h2",
    "h2_count",
    "canonical",
    "header_canonical",
    "noindex",
    "nofollow",
    "meta_robots",
    "x_robots_tag",
    "word_count",
    "text_hash",
    "crawl_depth",
    "inlinks",
    "unique_inlinks",
    "outlinks",
    "external_outlinks",
    "rendered",
    "render_error",
    "truncated",
];

/// Exact URL membership or changes to any tracked field.
#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ComparisonFilter {
    /// Every URL in either selected scope.
    #[default]
    All,
    /// Present in both crawls with at least one changed field.
    Changed,
    /// Present only in the current crawl.
    New,
    /// Present only in the baseline; does not imply deletion from the website.
    Missing,
    /// Present in both with equal tracked fields.
    Unchanged,
}

/// Filters shared by the paginated table and the full streaming export.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct ComparisonRequest {
    /// Membership/change filter.
    pub filter: ComparisonFilter,
    /// One tracked field, or empty for all fields.
    pub field: String,
    /// Case-insensitive literal URL substring.
    pub search: String,
    /// Compare only URLs classified as internal in each crawl.
    pub internal_only: bool,
    /// Rows to skip; ignored for exports.
    pub offset: u64,
    /// Page size, clamped to 1–1000; ignored for exports.
    pub limit: u32,
}

impl Default for ComparisonRequest {
    fn default() -> Self {
        Self {
            filter: ComparisonFilter::All,
            field: String::new(),
            search: String::new(),
            internal_only: true,
            offset: 0,
            limit: 250,
        }
    }
}

/// Changes among URLs present in both selected scopes.
#[derive(Debug, Serialize)]
pub struct FieldChanges {
    /// Stable field identifier.
    pub field: String,
    /// Number of matching URLs with a distinct value, including NULL transitions.
    pub count: u64,
}

/// Summary counts follow search/field filters across all membership states.
#[derive(Debug, Serialize)]
pub struct ComparisonSummary {
    /// Only in the current scope.
    pub new: u64,
    /// Only in the baseline scope.
    pub missing: u64,
    /// Shared URL with a changed field.
    pub changed: u64,
    /// Shared URL with identical tracked fields.
    pub unchanged: u64,
    /// Counts for all supported field filters, including zero counts.
    pub fields: Vec<FieldChanges>,
    /// Stable warning codes; clients translate these.
    pub warnings: Vec<String>,
}

/// An ephemeral query context with two read-only attached archives.
pub struct CrawlComparison {
    reader: CrawlReader,
    baseline: CrawlMeta,
    current: CrawlMeta,
}

impl CrawlLibrary {
    /// Opens two finished archives without migrating or writing either database.
    ///
    /// # Errors
    /// Same, running, invalid or incompatible archives; database failures.
    pub fn compare(
        &self,
        baseline: &str,
        current: &str,
    ) -> Result<CrawlComparison, CrawlStoreError> {
        if baseline == current {
            return Err(invalid("choose two different crawls"));
        }
        let baseline = self.meta(baseline)?;
        let current = self.meta(current)?;
        if [baseline.status, current.status].contains(&CrawlStatus::Running) {
            return Err(invalid("finish both crawls before comparing them"));
        }
        let connection = crate::reader::connect(std::path::Path::new(":memory:"), false)?;
        for (alias, meta) in [("before_crawl", &baseline), ("after_crawl", &current)] {
            let database = self.dir(&meta.id)?.join("crawl.duckdb");
            let path = database
                .to_str()
                .ok_or_else(|| invalid("database path is not UTF-8"))?;
            connection
                .execute_batch(&format!("ATTACH {} AS {alias} (READ_ONLY);", literal(path)))?;
            let version: i64 = connection.query_row(
                &format!("SELECT coalesce(max(version),0) FROM {alias}.schema_migrations"),
                [],
                |r| r.get(0),
            )?;
            let pending: Option<String> = connection.query_row(
                &format!("SELECT max(value) FROM {alias}.crawl_info WHERE key='audit_required'"),
                [],
                |r| r.get(0),
            )?;
            if !crate::schema::archive_compatible(version) || pending.as_deref() == Some("1") {
                return Err(invalid(
                    "open each finished crawl once to update its analysis before comparing",
                ));
            }
            // Duplicate stored URLs would make the join ambiguous. Never silently multiply rows.
            let duplicates: u64 = connection.query_row(
                &format!("SELECT count(*)-count(DISTINCT url) FROM {alias}.urls"),
                [],
                |r| r.get(0),
            )?;
            if duplicates != 0 {
                return Err(invalid("a crawl contains duplicate URL records"));
            }
            let schema_exists: bool=connection.query_row("SELECT EXISTS(SELECT 1 FROM information_schema.tables WHERE table_catalog=? AND table_name='schema_checks')",duckdb::params![alias],|r|r.get(0))?;
            let side = if alias == "before_crawl" {
                "before"
            } else {
                "after"
            };
            let checked = if schema_exists {
                format!(
                    "SELECT u.url FROM {alias}.schema_checks s JOIN {alias}.urls u ON u.id=s.page WHERE s.report IS NOT NULL AND NOT s.limited AND s.unchecked_items=0 AND EXISTS(SELECT 1 FROM {alias}.crawl_info WHERE key='schema_validation' AND value='1')"
                )
            } else {
                "SELECT NULL::VARCHAR AS url WHERE false".into()
            };
            connection.execute_batch(&format!(
                "CREATE TEMP VIEW {side}_schema_observed AS {checked}"
            ))?;
        }
        let reader = CrawlReader::from_connection(connection);
        let source = |catalog: &str| {
            let fields = FIELDS
                .iter()
                .map(|name| {
                    let expression = match *name {
                        "status_code" => "u.status",
                        "content" => "u.content_kind",
                        "crawl_depth" => "u.depth",
                        "inlinks" => "c.inlinks",
                        "unique_inlinks" => "c.unique_inlinks",
                        "outlinks" => "c.outlinks",
                        "external_outlinks" => "c.external_outlinks",
                        "rendered" => "r.rendered",
                        "render_error" => "r.error",
                        "truncated" => "(u.truncated OR coalesce(r.truncated,false))",
                        _ => return format!("CAST(u.{name} AS VARCHAR) AS {name}"),
                    };
                    format!("CAST({expression} AS VARCHAR) AS {name}")
                })
                .collect::<Vec<_>>()
                .join(",");
            format!(
                "SELECT u.url,u.internal,{fields},r.raw_noindex AS audit_raw_noindex FROM {catalog}.urls u JOIN {catalog}.link_counts c USING(id) LEFT JOIN {catalog}.rendering r ON r.page=u.id"
            )
        };
        reader.connection.execute_batch(&format!(
            "CREATE TEMP VIEW before_urls AS {}; CREATE TEMP VIEW after_urls AS {};",
            source("before_crawl"),
            source("after_crawl")
        ))?;
        Ok(CrawlComparison {
            reader,
            baseline,
            current,
        })
    }
}

impl CrawlComparison {
    fn relation(internal_only: bool) -> String {
        let scope = if internal_only { " WHERE internal" } else { "" };
        let fields = FIELDS
            .iter()
            .map(|f| format!("b.{f} AS before_{f},a.{f} AS after_{f}"))
            .collect::<Vec<_>>()
            .join(",");
        let changed = FIELDS
            .iter()
            .map(|f| format!("CASE WHEN b.{f} IS DISTINCT FROM a.{f} THEN '{f}' END"))
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "(SELECT coalesce(b.url,a.url) AS address,b.url IS NOT NULL AS in_baseline,a.url IS NOT NULL AS in_current,CASE WHEN b.url IS NULL THEN 'new' WHEN a.url IS NULL THEN 'missing' WHEN ({}) THEN 'changed' ELSE 'unchanged' END AS change,CASE WHEN b.url IS NOT NULL AND a.url IS NOT NULL THEN concat_ws(', ',{changed}) ELSE '' END AS changed_fields,{fields} FROM (SELECT * FROM before_urls{scope}) b FULL OUTER JOIN (SELECT * FROM after_urls{scope}) a ON b.url=a.url) comparison",
            FIELDS
                .iter()
                .map(|f| format!("b.{f} IS DISTINCT FROM a.{f}"))
                .collect::<Vec<_>>()
                .join(" OR ")
        )
    }

    fn selection(request: &ComparisonRequest) -> Result<String, CrawlStoreError> {
        let filter = match request.filter {
            ComparisonFilter::All => "true",
            ComparisonFilter::Changed => "change='changed'",
            ComparisonFilter::New => "change='new'",
            ComparisonFilter::Missing => "change='missing'",
            ComparisonFilter::Unchanged => "change='unchanged'",
        };
        let mut filters = vec![filter.to_owned()];
        if !request.field.is_empty() {
            if !FIELDS.contains(&request.field.as_str()) {
                return Err(invalid("unknown comparison field"));
            }
            filters.push(format!(
                "in_baseline AND in_current AND before_{} IS DISTINCT FROM after_{}",
                request.field, request.field
            ));
        }
        if !request.search.is_empty() {
            filters.push(format!(
                "contains(lower(address),lower({}))",
                literal(&request.search)
            ));
        }
        Ok(format!(
            "FROM {} WHERE {}",
            Self::relation(request.internal_only),
            filters.join(" AND ")
        ))
    }

    /// Membership counts and per-field changes; no URL rows are sent to the UI.
    ///
    /// # Errors
    /// Database errors.
    pub fn summary(&self, internal_only: bool) -> Result<ComparisonSummary, CrawlStoreError> {
        self.filtered_summary(&ComparisonRequest {
            internal_only,
            ..ComparisonRequest::default()
        })
    }

    /// Counts URL/field matches across all membership states, ignoring state and pagination.
    ///
    /// # Errors
    /// Invalid field or database errors.
    pub fn filtered_summary(
        &self,
        request: &ComparisonRequest,
    ) -> Result<ComparisonSummary, CrawlStoreError> {
        let from = Self::selection(&ComparisonRequest {
            filter: ComparisonFilter::All,
            ..request.clone()
        })?;
        let counts = self.reader.collect(&format!("SELECT count(*) FILTER(WHERE change='new'),count(*) FILTER(WHERE change='missing'),count(*) FILTER(WHERE change='changed'),count(*) FILTER(WHERE change='unchanged') {from}"), 1)?;
        let count = |i: usize| match counts.rows[0][i] {
            seonexus_core::table::Cell::Integer(v) => u64::try_from(v).unwrap_or(0),
            _ => 0,
        };
        // Aggregate every field in one pass over the shared URLs.
        let sql = FIELDS.iter().map(|f| format!("count(*) FILTER(WHERE in_baseline AND in_current AND before_{f} IS DISTINCT FROM after_{f})")).collect::<Vec<_>>().join(",");
        let changes = self.reader.collect(&format!("SELECT {sql} {from}"), 1)?;
        let fields = FIELDS
            .iter()
            .enumerate()
            .map(|(i, field)| FieldChanges {
                field: (*field).into(),
                count: match changes.rows[0][i] {
                    seonexus_core::table::Cell::Integer(v) => u64::try_from(v).unwrap_or(0),
                    _ => 0,
                },
            })
            .collect();
        let mut warnings = self.observation_warnings(request.internal_only)?;
        let relation = Self::relation(request.internal_only);
        if self.reader.scalar(&format!("SELECT count(*) FROM (SELECT 1 FROM {relation} WHERE in_baseline AND in_current LIMIT 1)"))? == 0 {
            warnings.push("no-shared-urls".into());
        }
        Ok(ComparisonSummary {
            new: count(0),
            missing: count(1),
            changed: count(2),
            unchanged: count(3),
            fields,
            warnings,
        })
    }

    fn observation_warnings(&self, internal_only: bool) -> Result<Vec<String>, CrawlStoreError> {
        let relation = Self::relation(internal_only);
        let mut warnings = Vec::new();
        if [&self.baseline, &self.current]
            .iter()
            .any(|m| m.status != CrawlStatus::Completed || m.limit_reached)
        {
            warnings.push("partial".into());
        }
        if self.baseline.start_urls != self.current.start_urls {
            warnings.push("seeds-differ".into());
        }
        if self.baseline.config != self.current.config {
            warnings.push("settings-differ".into());
        }
        if [&self.baseline, &self.current]
            .iter()
            .any(|m| m.mode == seonexus_crawl::CrawlMode::List)
        {
            warnings.push("list-mode".into());
        }
        let unreliable = self.reader.scalar(&format!("SELECT count(*) FROM {relation} WHERE before_error IS NOT NULL OR after_error IS NOT NULL OR before_render_error IS NOT NULL OR after_render_error IS NOT NULL OR before_truncated='true' OR after_truncated='true'"))?;
        if unreliable > 0 {
            warnings.push("incomplete-content".into());
        }
        Ok(warnings)
    }

    /// A bounded page, sorted by literal stored URL. Exact URL matching is intentional.
    ///
    /// # Errors
    /// Invalid field or database errors.
    pub fn page(&self, request: &ComparisonRequest) -> Result<TablePage, CrawlStoreError> {
        let from = Self::selection(request)?;
        let total = self.reader.scalar(&format!("SELECT count(*) {from}"))?;
        self.reader.collect(&format!("SELECT address,change,changed_fields,before_status_code,after_status_code,before_indexability,after_indexability,before_title,after_title {from} ORDER BY address LIMIT {} OFFSET {}", request.limit.clamp(1,1000), request.offset), total)
    }

    /// Before/after values for all tracked fields of one exact URL. NULL is not an empty string.
    ///
    /// # Errors
    /// Database errors.
    pub fn detail(&self, url: &str, internal_only: bool) -> Result<TablePage, CrawlStoreError> {
        let relation = Self::relation(internal_only);
        let parts = FIELDS.iter().enumerate().map(|(i,f)| format!("({i},'{f}',before_{f},after_{f},(in_baseline AND in_current AND before_{f} IS DISTINCT FROM after_{f}))")).collect::<Vec<_>>().join(",");
        let total = self.reader.scalar(&format!(
            "SELECT count(*) FROM {relation} WHERE address={}",
            literal(url)
        ))? * FIELDS.len() as u64;
        self.reader.collect(&format!("SELECT field,before_value,after_value,changed,in_baseline,in_current FROM (SELECT * FROM {relation} WHERE address={}) selected CROSS JOIN LATERAL (VALUES {parts}) v(position,field,before_value,after_value,changed) ORDER BY position", literal(url)), total)
    }

    /// Streams every matching row with all before/after fields, ignoring pagination.
    ///
    /// # Errors
    /// Invalid selection, database or output failures.
    pub fn export(
        &self,
        request: &ComparisonRequest,
        sink: &mut dyn TableSink,
    ) -> Result<u64, CrawlStoreError> {
        self.reader.query(
            &format!("SELECT * {} ORDER BY address", Self::selection(request)?),
            sink,
        )
    }
}

fn literal(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}
fn invalid(message: &str) -> CrawlStoreError {
    CrawlStoreError::InvalidSelection(message.into())
}

#[cfg(test)]
mod findings_tests;
#[cfg(test)]
mod tests;
