//! Bounded, filtered result pages and URL inspection for interactive clients.

use duckdb::OptionalExt;
use seonexus_core::table::{Cell, Column, TableBuffer, TableSink};
use serde::{Deserialize, Serialize};

use crate::{CrawlReader, CrawlStoreError, CrawlTab};

/// Which endpoint of a link is fixed by a URL inspector selection.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LinkDirection {
    /// Links whose destination is the selected URL.
    Incoming,
    /// Links whose source is the selected URL.
    Outgoing,
}

/// Exact URL scope for the links tab, independent of pagination and text search.
#[derive(Debug, Clone, Deserialize)]
pub struct LinkScope {
    /// URL as stored in the crawl.
    pub url: String,
    /// Incoming or outgoing references.
    pub direction: LinkDirection,
}

/// Classification of the counterpart: source for incoming links, destination otherwise.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LinkClass {
    /// An endpoint within this crawl's configured site boundary.
    Internal,
    /// An endpoint outside this crawl's configured site boundary.
    External,
}

/// Full filtered link counts, independent of the returned page limit.
#[derive(Debug, Clone, Serialize)]
pub struct LinkSummary {
    /// Distinct referring URLs.
    pub sources: u64,
    /// Distinct destination URLs.
    pub destinations: u64,
}

/// Internal/external counts of recorded references, including non-hyperlink kinds.
#[derive(Debug, Clone, Serialize)]
pub struct LinkBreakdown {
    /// Internal counterpart references.
    pub internal: u64,
    /// External counterpart references.
    pub external: u64,
}

/// Classification is based on the source for incoming and destination for outgoing links.
#[derive(Debug, Clone, Serialize)]
pub struct LinkCounts {
    /// Incoming references split by the referring page.
    pub incoming: LinkBreakdown,
    /// Outgoing references split by their destination.
    pub outgoing: LinkBreakdown,
}

/// A bounded interactive table request. Filters are escaped and sort columns whitelisted.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct PageRequest {
    /// A built-in tab name.
    pub tab: String,
    /// Case-insensitive literal URL substring.
    pub search: String,
    /// HTTP status class (2–5), or zero for every status.
    pub status_class: u16,
    /// Internal table's indexability filter.
    pub indexability: String,
    /// Limit internal URLs to one audit rule.
    pub issue_id: String,
    /// Rows to skip.
    pub offset: u64,
    /// Rows returned, clamped to 1–1000.
    pub limit: u32,
    /// A column from this tab, or empty for the default order.
    pub sort: String,
    /// Descending sort order.
    pub descending: bool,
    /// Optional exact incoming/outgoing URL scope, supported only by the links tab.
    pub link_scope: Option<LinkScope>,
    /// Optional internal/external counterpart, supported only by the links tab.
    pub link_class: Option<LinkClass>,
    /// Optional exact duplicate group, supported only by the internal table.
    pub duplicate_scope: Option<crate::DuplicateScope>,
    /// Optional disjoint response outcome, supported only by the response-codes tab.
    pub response_group: Option<crate::ResponseGroup>,
}

impl PageRequest {
    fn validate_scopes(&self, tab: CrawlTab) -> Result<(), CrawlStoreError> {
        if self.link_class.is_some() && tab != CrawlTab::Links {
            return Err(CrawlStoreError::InvalidSelection(
                "a link classification requires the links tab".into(),
            ));
        }
        if self.response_group.is_some() && tab != CrawlTab::ResponseCodes {
            return Err(CrawlStoreError::InvalidSelection(
                "a response group requires the response-codes tab".into(),
            ));
        }
        if let Some(scope) = &self.duplicate_scope
            && (tab != CrawlTab::Internal || scope.url.is_empty())
        {
            return Err(CrawlStoreError::InvalidSelection(
                "a duplicate scope requires the internal tab and a nonempty URL".into(),
            ));
        }
        if let Some(scope) = &self.link_scope
            && (tab != CrawlTab::Links || scope.url.is_empty())
        {
            return Err(CrawlStoreError::InvalidSelection(
                "a link scope requires the links tab and a nonempty URL".into(),
            ));
        }
        Ok(())
    }
}

/// All matching rows of a table, with an optional ordered column selection.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct TableExportRequest {
    /// Filters and sorting; pagination is ignored for export.
    pub request: PageRequest,
    /// Column names in output order, or empty for every column.
    pub columns: Vec<String>,
}

struct TableQuery {
    columns: Vec<Column>,
    from: String,
    order: String,
}

/// One bounded result page.
#[derive(Debug, Clone, Serialize)]
pub struct TablePage {
    /// Column schema.
    pub columns: Vec<Column>,
    /// Result cells.
    pub rows: Vec<Vec<Cell>>,
    /// Total matching rows before pagination.
    pub total: u64,
    /// Distinct source/destination counts for the complete filtered link selection.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub link_summary: Option<LinkSummary>,
}

/// Number of URLs affected by a check.
#[derive(Debug, Clone, Serialize)]
pub struct IssueCount {
    /// Shared audit catalog id.
    pub id: String,
    /// Affected URLs.
    pub count: u64,
}

/// Overview of a finished crawl.
#[derive(Debug, Clone, Serialize)]
pub struct CrawlOverview {
    /// Technical health of recorded internal URLs, independent of UI finding filters.
    pub seo_score: crate::SeoScore,
    /// Internal URLs.
    pub internal: u64,
    /// External URLs.
    pub external: u64,
    /// Indexable pages.
    pub indexable: u64,
    /// HTML pages.
    pub html: u64,
    /// robots.txt blocked URLs.
    pub blocked: u64,
    /// Broken URLs and failed requests.
    pub errors: u64,
    /// Automatic Location redirect statuses (301/302/303/307/308).
    pub redirects: u64,
    /// Disjoint response outcomes covering every stored URL exactly once.
    pub responses: Vec<crate::ResponseCount>,
    /// Average time to response headers, in milliseconds.
    pub average_response_ms: f64,
    /// Counts for checks with findings.
    pub issues: Vec<IssueCount>,
}

/// Details for one URL, requested only when the inspector opens.
#[derive(Debug, Clone, Serialize)]
pub struct UrlDetail {
    /// Page facts as one row.
    pub facts: TablePage,
    /// Context-checked findings for this URL, at most 1000 with the full total.
    pub findings: TablePage,
    /// Incoming links, at most 1000.
    pub inlinks: TablePage,
    /// Outgoing links, at most 1000.
    pub outlinks: TablePage,
    /// Full incoming/outgoing internal/external reference counts, without preview limits.
    pub link_counts: LinkCounts,
    /// Saved headers.
    pub headers: TablePage,
    /// Saved source, bounded by the crawl body's size limit.
    pub html: Option<String>,
    /// Original response HTML, before JavaScript, when stored.
    pub raw_html: Option<String>,
    /// Saved optional Schema property report; does not fetch or revalidate a page.
    pub schema_report: Option<serde_json::Value>,
    /// Recorded reason a requested Schema check was unavailable.
    pub schema_error: Option<String>,
}

impl CrawlReader {
    /// A result page; this never collects the full crawl in application memory.
    ///
    /// # Errors
    /// Unknown tabs and database errors.
    pub fn page(&self, request: &PageRequest) -> Result<TablePage, CrawlStoreError> {
        let query = self.table_query(request)?;
        let (total, link_summary) = if CrawlTab::from_name(&request.tab) == Some(CrawlTab::Links) {
            self.connection.query_row(
                &format!(
                    "SELECT count(*),count(DISTINCT source),count(DISTINCT destination) {}",
                    query.from
                ),
                [],
                |row| {
                    Ok((
                        row.get(0)?,
                        Some(LinkSummary {
                            sources: row.get(1)?,
                            destinations: row.get(2)?,
                        }),
                    ))
                },
            )?
        } else {
            (
                self.scalar(&format!("SELECT count(*) {}", query.from))?,
                None,
            )
        };
        let limit = request.limit.clamp(1, 1000);
        let mut page = self.collect(
            &format!(
                "SELECT * {} {} LIMIT {limit} OFFSET {}",
                query.from, query.order, request.offset
            ),
            total,
        )?;
        page.link_summary = link_summary;
        Ok(page)
    }

    /// Streams every filtered row, using the same predicates and sorting as [`Self::page`].
    ///
    /// # Errors
    /// Unknown columns/tabs, duplicate column names, database or sink failures.
    pub fn export_selection(
        &self,
        selection: &TableExportRequest,
        sink: &mut dyn TableSink,
    ) -> Result<u64, CrawlStoreError> {
        let query = self.table_query(&selection.request)?;
        let mut columns = Vec::new();
        for name in &selection.columns {
            if !query.columns.iter().any(|column| column.name == *name) || columns.contains(name) {
                return Err(CrawlStoreError::InvalidSelection(format!(
                    "unknown or duplicate column `{name}`"
                )));
            }
            columns.push(name.clone());
        }
        let projection = if columns.is_empty() {
            "*".into()
        } else {
            columns
                .iter()
                .map(|name| format!("\"{}\"", name.replace('"', "\"\"")))
                .collect::<Vec<_>>()
                .join(",")
        };
        self.query(
            &format!("SELECT {projection} {} {}", query.from, query.order),
            sink,
        )
    }

    fn table_query(&self, request: &PageRequest) -> Result<TableQuery, CrawlStoreError> {
        let tab = CrawlTab::from_name(&request.tab).ok_or_else(|| {
            CrawlStoreError::InvalidSelection(format!("unknown tab `{}`", request.tab))
        })?;
        let view = tab.source();
        request.validate_scopes(tab)?;
        let mut schema = TableBuffer::default();
        self.query(&format!("SELECT * FROM {view} LIMIT 0"), &mut schema)?;
        let has = |name: &str| schema.columns.iter().any(|column| column.name == name);
        let mut filters = vec!["true".to_owned()];
        if let Some(group) = request.response_group {
            filters.push(format!("({})", group.predicate("status_code")));
        }
        if let Some(scope) = &request.duplicate_scope {
            filters.push(scope.predicate(&literal(&scope.url)));
        }
        if let Some(scope) = &request.link_scope {
            let column = match scope.direction {
                LinkDirection::Incoming => "destination",
                LinkDirection::Outgoing => "source",
            };
            filters.push(format!("{column}={}", literal(&scope.url)));
        }
        if let Some(class) = request.link_class {
            let column = if matches!(
                request.link_scope.as_ref().map(|scope| scope.direction),
                Some(LinkDirection::Incoming)
            ) {
                "source_internal"
            } else {
                "destination_internal"
            };
            filters.push(format!("{column}={}", matches!(class, LinkClass::Internal)));
        }
        if tab == CrawlTab::Links
            && let Some(predicate) = crate::link_queries::evidence_predicate(&request.issue_id)
        {
            filters.push(format!("({predicate})"));
        }
        if !request.search.is_empty() {
            let columns = searchable_columns(
                tab,
                request.link_scope.as_ref().map(|scope| scope.direction),
                has,
            );
            if !columns.is_empty() {
                let predicates = columns
                    .iter()
                    .map(|column| {
                        format!(
                            "contains(lower({column}), lower({}))",
                            literal(&request.search)
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(" OR ");
                filters.push(format!("({predicates})"));
            }
        }
        if let Some(column) = ["status_code", "destination_status"]
            .into_iter()
            .find(|column| has(column))
            && (2..=5).contains(&request.status_class)
        {
            let first = request.status_class * 100;
            filters.push(format!("{column} BETWEEN {first} AND {}", first + 99));
        }
        if has("indexability") && !request.indexability.is_empty() {
            filters.push(format!("indexability={}", literal(&request.indexability)));
        }
        if let Some(predicate) = issue_filter(request, tab, has) {
            filters.push(predicate);
        }
        let from = format!("FROM {view} WHERE {}", filters.join(" AND "));
        let sort = if has(&request.sort) {
            format!(
                "ORDER BY \"{}\" {} NULLS LAST",
                request.sort,
                if request.descending { "DESC" } else { "ASC" }
            )
        } else {
            String::new()
        };
        Ok(TableQuery {
            columns: schema.columns,
            from,
            order: sort,
        })
    }

    /// Aggregates counts on disk.
    ///
    /// # Errors
    /// Database errors.
    pub fn overview(&self) -> Result<CrawlOverview, CrawlStoreError> {
        self.prepare_anchors()?;
        let response_counts = crate::ResponseGroup::ALL
            .map(|group| format!("count(*) FILTER(WHERE {})", group.predicate("status")))
            .join(",");
        let sql = format!(
            "SELECT count(*) FILTER(WHERE internal), count(*) FILTER(WHERE NOT internal), count(*) FILTER(WHERE internal AND content_kind='html' AND indexability='indexable'), count(*) FILTER(WHERE internal AND content_kind='html'), count(*) FILTER(WHERE blocked_by_robots AND status IS NULL), count(*) FILTER(WHERE status>=400 OR error IS NOT NULL), count(*) FILTER(WHERE status IN (301,302,303,307,308)), coalesce(avg(response_ms) FILTER(WHERE status IS NOT NULL),0), {response_counts} FROM urls"
        );
        let mut statement = self.connection.prepare(&sql)?;
        let mut overview = statement.query_row([], |r| {
            Ok(CrawlOverview {
                internal: r.get(0)?,
                external: r.get(1)?,
                indexable: r.get(2)?,
                html: r.get(3)?,
                blocked: r.get(4)?,
                errors: r.get(5)?,
                redirects: r.get(6)?,
                responses: crate::ResponseGroup::ALL
                    .into_iter()
                    .enumerate()
                    .map(|(i, group)| {
                        Ok(crate::ResponseCount {
                            group,
                            count: r.get(8 + i)?,
                        })
                    })
                    .collect::<Result<_, duckdb::Error>>()?,
                average_response_ms: r.get(7)?,
                issues: Vec::new(),
                seo_score: crate::SeoScore {
                    schema_validation: None,
                    version: 4,
                    value: None,
                    urls: 0,
                    affected_urls: 0,
                    analyzed_html: 0,
                    limited_urls: 0,
                    unknown_findings: 0,
                    deductions: Vec::new(),
                    excluded_urls: 0,
                    excluded_links: 0,
                    excluded_findings: 0,
                    canonical_context_findings: 0,
                    anchor_context_findings: 0,
                    advisory_findings: 0,
                    contact_redirect_findings: 0,
                    unverified_anchor_findings: 0,
                    categories: Vec::new(),
                    excluded_rules: Vec::new(),
                },
            })
        })?;
        let mut statement = self.connection.prepare(&format!(
            "SELECT issue_id,count(*) FROM {} GROUP BY issue_id ORDER BY count(*) DESC,issue_id",
            crate::hreflang_context::findings()
        ))?;
        overview.issues = statement
            .query_map([], |r| {
                Ok(IssueCount {
                    id: r.get(0)?,
                    count: r.get(1)?,
                })
            })?
            .collect::<Result<_, _>>()?;
        overview.seo_score = self.seo_score()?;
        Ok(overview)
    }

    /// Fetches stored facts, findings, links, headers and source for one exact URL.
    ///
    /// # Errors
    /// Database errors.
    pub fn detail(&self, url: &str) -> Result<UrlDetail, CrawlStoreError> {
        self.prepare_anchors()?;
        let quoted = literal(url);
        let links = crate::link_queries::LINK_ROWS;
        let table = |sql: String| -> Result<TablePage, CrawlStoreError> {
            let total = self.scalar(&format!("SELECT count(*) FROM ({sql}) detail"))?;
            self.collect(&format!("{sql} LIMIT 1000"), total)
        };
        let html = self.connection.query_row(
            &format!(
                "SELECT html FROM html WHERE page=(SELECT id FROM urls WHERE url={quoted} LIMIT 1)"
            ),
            [],
            |r| r.get(0),
        );
        let html = match html {
            Ok(value) => Some(value),
            Err(duckdb::Error::QueryReturnedNoRows) => None,
            Err(error) => return Err(error.into()),
        };
        let raw_html = self.connection.query_row(&format!("SELECT raw_html FROM rendering WHERE page=(SELECT id FROM urls WHERE url={quoted} LIMIT 1)"), [], |r|r.get::<_,Option<String>>(0));
        let raw_html = match raw_html {
            Ok(value) => value,
            Err(duckdb::Error::QueryReturnedNoRows) => None,
            Err(error) => return Err(error.into()),
        };
        Ok(UrlDetail {
            schema_report: crate::schema_checks::report(&self.connection,url)?,
            schema_error: self.connection.query_row("SELECT s.error FROM saved_schema_checks s JOIN urls u ON u.id=s.page WHERE u.url=? LIMIT 1", duckdb::params![url], |r|r.get::<_,Option<String>>(0)).optional()?.flatten(),
            raw_html,
            findings: table(format!(
                "SELECT issue_id FROM {} WHERE url={quoted} ORDER BY issue_id", crate::hreflang_context::findings()
            ))?,
            facts: table(format!(
                "SELECT u.*,r.rendered,r.rendered_url,r.error AS render_error,r.elapsed_ms AS render_time_ms,r.truncated AS render_truncated,r.raw_title,r.raw_word_count,r.raw_link_count,r.rendered_link_count,r.raw_meta_description,r.raw_canonical,r.raw_noindex,r.raw_nofollow FROM urls u LEFT JOIN rendering r ON r.page=u.id WHERE u.url={quoted}"
            ))?,
            inlinks: table(format!(
                "SELECT * FROM {links} WHERE destination={quoted}"
            ))?,
            outlinks: table(format!("SELECT * FROM {links} WHERE source={quoted}"))?,
            link_counts: self.connection.query_row(&format!(
                "SELECT count(*) FILTER(WHERE destination={quoted} AND source_internal),count(*) FILTER(WHERE destination={quoted} AND NOT source_internal),count(*) FILTER(WHERE source={quoted} AND destination_internal),count(*) FILTER(WHERE source={quoted} AND NOT destination_internal) FROM {links} WHERE source={quoted} OR destination={quoted}"
            ), [], |row| Ok(LinkCounts { incoming: LinkBreakdown { internal: row.get(0)?, external: row.get(1)? }, outgoing: LinkBreakdown { internal: row.get(2)?, external: row.get(3)? } }))?,
            headers: table(format!(
                "SELECT name,value FROM response_headers WHERE page=(SELECT id FROM urls WHERE url={quoted} LIMIT 1)"
            ))?,
            html,
        })
    }

    pub(crate) fn scalar(&self, sql: &str) -> Result<u64, CrawlStoreError> {
        self.prepare_anchors()?;
        Ok(self.connection.query_row(sql, [], |r| r.get(0))?)
    }

    pub(crate) fn collect(&self, sql: &str, total: u64) -> Result<TablePage, CrawlStoreError> {
        let mut buffer = TableBuffer::default();
        self.query(sql, &mut buffer)?;
        Ok(TablePage {
            columns: buffer.columns,
            rows: buffer.rows,
            total,
            link_summary: None,
        })
    }
}

fn literal(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

fn searchable_columns(
    tab: CrawlTab,
    direction: Option<LinkDirection>,
    has: impl Fn(&str) -> bool,
) -> Vec<&'static str> {
    match tab {
        CrawlTab::Links => match direction {
            Some(LinkDirection::Incoming) => vec!["source"],
            Some(LinkDirection::Outgoing) => vec!["destination"],
            None => vec!["source", "destination"],
        },
        CrawlTab::ContentMatches => vec!["address", "match_address"],
        CrawlTab::SitemapUrls => ["url", "crawl_url", "sitemap"]
            .into_iter()
            .filter(|name| has(name))
            .collect(),
        CrawlTab::Sitemaps => ["url", "redirect_url", "redirect_location", "final_url"]
            .into_iter()
            .filter(|name| has(name))
            .collect(),
        _ => ["address", "url", "page", "source", "image"]
            .into_iter()
            .find(|name| has(name))
            .into_iter()
            .collect(),
    }
}

fn issue_filter(
    request: &PageRequest,
    tab: CrawlTab,
    has: impl Fn(&str) -> bool,
) -> Option<String> {
    if request.issue_id.is_empty() {
        return None;
    }
    let issue = literal(&request.issue_id);
    if has("issue_id") {
        return Some(format!("issue_id={issue}"));
    }
    let column = ["address", "url", "page", "source"]
        .into_iter()
        .find(|name| has(name))?;
    let column = if tab == CrawlTab::SitemapUrls && has("crawl_url") {
        "coalesce(crawl_url,url)"
    } else {
        column
    };
    let findings = if matches!(
        request.issue_id.as_str(),
        "hreflang-missing-return"
            | "hreflang-missing-self"
            | "hreflang-missing-x-default"
            | "anchor-empty"
            | "anchor-generic"
            | "links-to-redirects"
    ) {
        crate::hreflang_context::findings()
    } else {
        "audit_findings".into()
    };
    Some(format!(
        "{column} IN (SELECT url FROM {findings} WHERE issue_id={issue})"
    ))
}
