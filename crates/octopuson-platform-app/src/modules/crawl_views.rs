//! Views of an audit's crawl beside its checks: internal links (click depth, pages only the
//! sitemap knows, weakly linked pages and an internal PageRank), hreflang annotations, a page
//! explorer with each page's findings and links, and every finding as a CSV file.
//!
//! The PageRank is computed from the crawl's followed internal links between HTML pages that
//! answered 200 and shown on a 0–100 logarithmic scale; the last few results are kept in
//! memory, as a crawl does not change once finished.

use std::collections::HashMap;
use std::fmt::Write as _;
use std::io;
use std::sync::{Arc, Mutex, PoisonError};

use octopuson_audit::catalog;
use octopuson_core::table::{Cell, Column, TableBuffer, TableSink};
use octopuson_crawl_store::{CrawlReader, CrawlTab, PageRequest};
use octopuson_platform::AuditRow;
use serde::{Deserialize, Serialize};

use super::audit::{lang_of, open, store_error};
use crate::router::{Context, Router};
use crate::{AppError, Workspace};

/// Pages listed in each internal-link list.
const LINK_ROWS: usize = 200;

/// Rounds of the PageRank computation.
const RANK_ROUNDS: usize = 30;

/// How much of a page's rank its links pass on.
const DAMPING: f64 = 0.85;

/// Crawls whose link figures are kept in memory.
const KEPT_CRAWLS: usize = 4;

/// Rows of hreflang annotations listed.
const HREFLANG_ROWS: u32 = 200;

/// Pages of the explorer listed at most at a time.
const EXPLORER_ROWS: u32 = 500;

/// Links into a page listed.
const INLINK_ROWS: usize = 50;

/// Findings written to a CSV file at most.
const CSV_ROWS: u64 = 500_000;

#[derive(Debug, Deserialize)]
struct AuditAsk {
    project_id: i64,
    /// The latest finished audit when absent.
    #[serde(default)]
    audit_id: Option<i64>,
    #[serde(default)]
    lang: Option<String>,
}

/// The audit asked for, or the latest with a finished crawl.
fn audit_of(
    workspace: &Workspace,
    project_id: i64,
    audit_id: Option<i64>,
) -> Result<Option<AuditRow>, AppError> {
    let store = workspace.store()?;
    Ok(match audit_id {
        Some(id) => Some(store.audit(project_id, id)?),
        None => store.audits(project_id, 20)?.into_iter().find(|audit| {
            audit.crawl_id.is_some() && !matches!(audit.status.as_str(), "running" | "failed")
        }),
    })
}

/// The audit and its crawl, when there is a finished one.
fn reader_of(
    workspace: &Workspace,
    project_id: i64,
    audit_id: Option<i64>,
) -> Result<Option<(AuditRow, CrawlReader)>, AppError> {
    let Some(audit) = audit_of(workspace, project_id, audit_id)? else {
        return Ok(None);
    };
    Ok(open(workspace, &audit).map(|reader| (audit, reader)))
}

fn literal(text: &str) -> String {
    format!("'{}'", text.replace('\'', "''"))
}

fn text(cell: Option<&Cell>) -> String {
    match cell {
        None | Some(Cell::Null) => String::new(),
        Some(cell) => cell.to_string(),
    }
}

#[allow(clippy::cast_possible_truncation)] // counts and ids
fn integer(cell: Option<&Cell>) -> Option<i64> {
    match cell {
        Some(Cell::Integer(value)) => Some(*value),
        Some(Cell::Float(value)) => Some(value.round() as i64),
        _ => None,
    }
}

fn rows(reader: &CrawlReader, sql: &str) -> Result<Vec<Vec<Cell>>, AppError> {
    let mut buffer = TableBuffer::default();
    reader
        .query(sql, &mut buffer)
        .map_err(|error| store_error(&error))?;
    Ok(buffer.rows)
}

fn count(reader: &CrawlReader, sql: &str) -> Result<u64, AppError> {
    Ok(rows(reader, sql)?
        .first()
        .and_then(|row| integer(row.first()))
        .and_then(|n| u64::try_from(n).ok())
        .unwrap_or(0))
}

// ----- Internal links ------------------------------------------------------------------------

/// Collects a link list's (source, target) ids without keeping its cells.
#[derive(Default)]
struct Edges(Vec<(u32, u32)>);

impl TableSink for Edges {
    fn begin(&mut self, _: &[Column]) -> io::Result<()> {
        Ok(())
    }

    fn row(&mut self, cells: &[Cell]) -> io::Result<()> {
        let id = |cell: Option<&Cell>| integer(cell).and_then(|value| u32::try_from(value).ok());
        if let (Some(source), Some(target)) = (id(cells.first()), id(cells.get(1))) {
            self.0.push((source, target));
        }
        Ok(())
    }

    fn finish(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// PageRank of `n` pages joined by `edges` (indexes), summing to 1; pages without links share
/// their rank with every page.
#[allow(clippy::cast_precision_loss)] // page counts
pub(crate) fn page_rank(n: usize, edges: &[(usize, usize)], rounds: usize) -> Vec<f64> {
    if n == 0 {
        return Vec::new();
    }
    let mut out = vec![0_u32; n];
    for (source, _) in edges {
        out[*source] += 1;
    }
    let mut rank = vec![1.0 / n as f64; n];
    for _ in 0..rounds {
        let dangling: f64 = (0..n)
            .filter(|page| out[*page] == 0)
            .map(|page| rank[page])
            .sum();
        let base = (1.0 - DAMPING) / n as f64 + DAMPING * dangling / n as f64;
        let mut next = vec![base; n];
        for (source, target) in edges {
            next[*target] += DAMPING * rank[*source] / f64::from(out[*source]);
        }
        rank = next;
    }
    rank
}

/// Ranks on a 0–100 logarithmic scale: the best-linked page 100, the least 0.
pub(crate) fn scaled(rank: &[f64]) -> Vec<f64> {
    let (low, high) = rank
        .iter()
        .fold((f64::MAX, f64::MIN), |(low, high), value| {
            (low.min(*value), high.max(*value))
        });
    if rank.is_empty() || high <= low {
        return vec![100.0; rank.len()];
    }
    let (low, high) = (low.ln(), high.ln());
    rank.iter()
        .map(|value| ((value.ln() - low) / (high - low) * 1000.0).round() / 10.0)
        .collect()
}

/// A page with how it is linked.
#[derive(Debug, Clone, Serialize)]
struct LinkedPage {
    url: String,
    title: String,
    depth: Option<i64>,
    /// Pages linking to it.
    inlinks: i64,
    /// Internal PageRank, 0–100.
    rank: f64,
    indexability: String,
}

/// A page the sitemap lists that no crawled page links to.
#[derive(Debug, Clone, Serialize)]
struct Orphan {
    url: String,
    /// `None` when the crawl did not fetch it.
    status: Option<i64>,
    indexability: String,
}

#[derive(Debug, Clone, Serialize)]
struct LinksReport {
    audit_id: i64,
    /// HTML pages that answered 200.
    pages: u64,
    /// Pages by clicks from the start page (6 = six or more).
    depth: Vec<(i64, u64)>,
    average_depth: Option<f64>,
    /// Pages four or more clicks deep.
    deep: u64,
    /// The crawl read sitemaps.
    sitemaps: bool,
    orphan_total: u64,
    orphans: Vec<Orphan>,
    /// Indexable pages with one linking page or none.
    weak_total: u64,
    weak: Vec<LinkedPage>,
    /// Pages with the most internal PageRank.
    top: Vec<LinkedPage>,
}

static KEPT: Mutex<Vec<(String, Arc<LinksReport>)>> = Mutex::new(Vec::new());

#[allow(clippy::too_many_lines)] // one report from a few queries
fn links_report(reader: &CrawlReader, audit_id: i64) -> Result<LinksReport, AppError> {
    let nodes = rows(
        reader,
        "SELECT u.id, u.url, coalesce(u.title, ''), u.depth, u.indexability, c.unique_inlinks
         FROM urls u JOIN link_counts c USING (id)
         WHERE u.internal AND u.status = 200 AND u.content_kind = 'html' ORDER BY u.id",
    )?;
    let index: HashMap<u32, usize> = nodes
        .iter()
        .enumerate()
        .filter_map(|(at, row)| Some((u32::try_from(integer(row.first())?).ok()?, at)))
        .collect();
    let mut edges = Edges::default();
    reader
        .query(
            "SELECT source, target FROM links
             WHERE kind = 'hyperlink' AND internal AND NOT nofollow AND source <> target
             GROUP BY source, target",
            &mut edges,
        )
        .map_err(|error| store_error(&error))?;
    let joined: Vec<(usize, usize)> = edges
        .0
        .iter()
        .filter_map(|(source, target)| Some((*index.get(source)?, *index.get(target)?)))
        .collect();
    drop(edges);
    let rank = scaled(&page_rank(nodes.len(), &joined, RANK_ROUNDS));
    let pages: Vec<LinkedPage> = nodes
        .iter()
        .zip(&rank)
        .map(|(row, rank)| LinkedPage {
            url: text(row.get(1)),
            title: text(row.get(2)),
            depth: integer(row.get(3)),
            indexability: text(row.get(4)),
            inlinks: integer(row.get(5)).unwrap_or(0),
            rank: *rank,
        })
        .collect();
    let mut depth: Vec<(i64, u64)> = Vec::new();
    for page in &pages {
        let bucket = page.depth.unwrap_or(6).min(6);
        match depth.iter_mut().find(|(level, _)| *level == bucket) {
            Some((_, n)) => *n += 1,
            None => depth.push((bucket, 1)),
        }
    }
    depth.sort_unstable();
    let depths: Vec<i64> = pages.iter().filter_map(|page| page.depth).collect();
    #[allow(clippy::cast_precision_loss)]
    let average_depth = (!depths.is_empty())
        .then(|| (depths.iter().sum::<i64>() as f64 / depths.len() as f64 * 10.0).round() / 10.0);
    let mut weak: Vec<LinkedPage> = pages
        .iter()
        .filter(|page| {
            page.indexability == "indexable" && page.inlinks <= 1 && page.depth != Some(0)
        })
        .cloned()
        .collect();
    weak.sort_by(|a, b| a.rank.total_cmp(&b.rank).then_with(|| a.url.cmp(&b.url)));
    let weak_total = weak.len() as u64;
    weak.truncate(LINK_ROWS);
    let mut top: Vec<LinkedPage> = pages.clone();
    top.sort_by(|a, b| b.rank.total_cmp(&a.rank).then_with(|| a.url.cmp(&b.url)));
    top.truncate(LINK_ROWS);
    let sitemaps = count(reader, "SELECT count(*) FROM sitemap_urls").unwrap_or(0) > 0;
    let orphan_from = "FROM (SELECT DISTINCT coalesce(crawl_url, url) AS url FROM sitemap_urls) s
         LEFT JOIN urls u ON u.url = s.url
         LEFT JOIN link_counts c ON c.id = u.id
         WHERE coalesce(c.unique_inlinks, 0) = 0 AND coalesce(u.depth, 1) > 0";
    let orphan_total = count(reader, &format!("SELECT count(*) {orphan_from}")).unwrap_or(0);
    let orphans = rows(
        reader,
        &format!(
            "SELECT s.url, u.status, coalesce(u.indexability, '') {orphan_from} ORDER BY s.url LIMIT {LINK_ROWS}"
        ),
    )
    .unwrap_or_default()
    .iter()
    .map(|row| Orphan {
        url: text(row.first()),
        status: integer(row.get(1)),
        indexability: text(row.get(2)),
    })
    .collect();
    Ok(LinksReport {
        audit_id,
        pages: pages.len() as u64,
        deep: pages
            .iter()
            .filter(|page| page.depth.is_some_and(|depth| depth >= 4))
            .count() as u64,
        depth,
        average_depth,
        sitemaps,
        orphan_total,
        orphans,
        weak_total,
        weak,
        top,
    })
}

fn audit_links(context: &Context<'_>, ask: AuditAsk) -> Result<Option<LinksReport>, AppError> {
    let Some((audit, reader)) = reader_of(context.workspace, ask.project_id, ask.audit_id)? else {
        return Ok(None);
    };
    let crawl = audit.crawl_id.clone().unwrap_or_default();
    if let Some((_, kept)) = KEPT
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .iter()
        .find(|(id, _)| *id == crawl)
    {
        return Ok(Some(kept.as_ref().clone()));
    }
    let report = Arc::new(links_report(&reader, audit.id)?);
    let mut kept = KEPT.lock().unwrap_or_else(PoisonError::into_inner);
    kept.push((crawl, Arc::clone(&report)));
    if kept.len() > KEPT_CRAWLS {
        kept.remove(0);
    }
    Ok(Some(report.as_ref().clone()))
}

// ----- Hreflang ------------------------------------------------------------------------------

#[derive(Debug, Serialize)]
struct HreflangIssue {
    id: String,
    title: String,
    priority: &'static str,
    count: u64,
}

#[derive(Debug, Serialize)]
struct HreflangReport {
    audit_id: i64,
    /// Pages with hreflang annotations.
    pages: u64,
    /// Languages annotated, with the pages naming each.
    languages: Vec<(String, u64)>,
    issues: Vec<HreflangIssue>,
    total: u64,
    columns: Vec<String>,
    rows: Vec<Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct PagedAsk {
    project_id: i64,
    #[serde(default)]
    audit_id: Option<i64>,
    #[serde(default)]
    offset: u64,
    #[serde(default)]
    lang: Option<String>,
}

fn audit_hreflang(
    context: &Context<'_>,
    ask: PagedAsk,
) -> Result<Option<HreflangReport>, AppError> {
    let Some((audit, reader)) = reader_of(context.workspace, ask.project_id, ask.audit_id)? else {
        return Ok(None);
    };
    let pages = count(&reader, "SELECT count(DISTINCT page) FROM hreflang")?;
    let languages = rows(
        &reader,
        "SELECT lang, count(DISTINCT page) FROM hreflang GROUP BY lang ORDER BY 2 DESC, 1",
    )?
    .iter()
    .map(|row| {
        (
            text(row.first()),
            integer(row.get(1))
                .and_then(|n| u64::try_from(n).ok())
                .unwrap_or(0),
        )
    })
    .collect();
    let overview = reader.overview().map_err(|error| store_error(&error))?;
    let lang = lang_of(ask.lang.as_deref());
    let checks = catalog();
    let issues = checks
        .issues
        .iter()
        .filter(|issue| issue.id.starts_with("hreflang-"))
        .map(|issue| HreflangIssue {
            id: issue.id.clone(),
            title: issue.text(lang).title.clone(),
            priority: issue.priority.as_str(),
            count: overview
                .issues
                .iter()
                .find(|found| found.id == issue.id)
                .map_or(0, |found| found.count),
        })
        .collect();
    let page = reader
        .page(&PageRequest {
            tab: CrawlTab::Hreflang.name().to_owned(),
            offset: ask.offset,
            limit: HREFLANG_ROWS,
            ..PageRequest::default()
        })
        .map_err(|error| store_error(&error))?;
    Ok(Some(HreflangReport {
        audit_id: audit.id,
        pages,
        languages,
        issues,
        total: page.total,
        columns: page
            .columns
            .iter()
            .map(|column| column.name.clone())
            .collect(),
        rows: page
            .rows
            .iter()
            .map(|row| row.iter().map(|cell| text(Some(cell))).collect())
            .collect(),
    }))
}

// ----- Page explorer -------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct PagesAsk {
    project_id: i64,
    #[serde(default)]
    audit_id: Option<i64>,
    #[serde(default)]
    offset: u64,
    #[serde(default)]
    limit: Option<u32>,
    /// A column of the internal tab.
    #[serde(default)]
    sort: String,
    #[serde(default)]
    descending: bool,
    /// Words in the address, title or headings.
    #[serde(default)]
    search: String,
    /// 2, 3, 4 or 5; every page with 0.
    #[serde(default)]
    status_class: u16,
    /// `indexable`, `noindex`, `canonicalised` …; every page when empty.
    #[serde(default)]
    indexability: String,
}

#[derive(Debug, Serialize)]
struct PagesView {
    audit_id: i64,
    total: u64,
    offset: u64,
    columns: Vec<String>,
    rows: Vec<Vec<String>>,
}

fn audit_pages(context: &Context<'_>, ask: PagesAsk) -> Result<Option<PagesView>, AppError> {
    let Some((audit, reader)) = reader_of(context.workspace, ask.project_id, ask.audit_id)? else {
        return Ok(None);
    };
    let page = reader
        .page(&PageRequest {
            tab: CrawlTab::Internal.name().to_owned(),
            search: ask.search.trim().chars().take(200).collect(),
            status_class: if (2..=5).contains(&ask.status_class) {
                ask.status_class
            } else {
                0
            },
            indexability: ask.indexability.trim().to_owned(),
            offset: ask.offset,
            limit: ask.limit.unwrap_or(100).clamp(1, EXPLORER_ROWS),
            sort: ask.sort.trim().to_owned(),
            descending: ask.descending,
            ..PageRequest::default()
        })
        .map_err(|error| store_error(&error))?;
    Ok(Some(PagesView {
        audit_id: audit.id,
        total: page.total,
        offset: ask.offset,
        columns: page
            .columns
            .iter()
            .map(|column| column.name.clone())
            .collect(),
        rows: page
            .rows
            .iter()
            .map(|row| row.iter().map(|cell| text(Some(cell))).collect())
            .collect(),
    }))
}

#[derive(Debug, Deserialize)]
struct PageAsk {
    project_id: i64,
    audit_id: i64,
    url: String,
    #[serde(default)]
    lang: Option<String>,
}

#[derive(Debug, Serialize)]
struct PageFinding {
    id: String,
    title: String,
    priority: &'static str,
    kind: &'static str,
}

#[derive(Debug, Serialize)]
struct Inlink {
    url: String,
    anchor: String,
}

/// One page of a crawl: what was read from it, its findings and who links to it.
#[derive(Debug, Serialize)]
struct PageDetail {
    url: String,
    /// The internal tab's columns and values.
    facts: Vec<(String, String)>,
    findings: Vec<PageFinding>,
    inlinks_total: u64,
    inlinks: Vec<Inlink>,
}

fn audit_page(context: &Context<'_>, ask: PageAsk) -> Result<PageDetail, AppError> {
    let (_, reader) = reader_of(context.workspace, ask.project_id, Some(ask.audit_id))?
        .ok_or_else(|| AppError::Invalid("the audit has no finished crawl".to_owned()))?;
    let url = literal(ask.url.trim());
    let mut facts_buffer = TableBuffer::default();
    reader
        .query(
            &format!("SELECT * FROM tab_internal WHERE address = {url} LIMIT 1"),
            &mut facts_buffer,
        )
        .map_err(|error| store_error(&error))?;
    let row = facts_buffer
        .rows
        .first()
        .ok_or_else(|| AppError::NotFound(format!("page {}", ask.url.trim())))?;
    let facts = facts_buffer
        .columns
        .iter()
        .zip(row)
        .map(|(column, cell)| (column.name.clone(), text(Some(cell))))
        .collect();
    let lang = lang_of(ask.lang.as_deref());
    let checks = catalog();
    let mut findings: Vec<PageFinding> = rows(
        &reader,
        &format!("SELECT DISTINCT issue_id FROM audit_findings WHERE url = {url} ORDER BY 1"),
    )?
    .iter()
    .filter_map(|row| {
        let check = checks.issue(&text(row.first()))?;
        Some(PageFinding {
            id: check.id.clone(),
            title: check.text(lang).title.clone(),
            priority: check.priority.as_str(),
            kind: check.kind.as_str(),
        })
    })
    .collect();
    let order = |priority: &str| match priority {
        "high" => 0,
        "medium" => 1,
        _ => 2,
    };
    findings.sort_by(|a, b| {
        order(a.priority)
            .cmp(&order(b.priority))
            .then_with(|| a.id.cmp(&b.id))
    });
    let from = format!(
        "FROM links l JOIN urls s ON s.id = l.source JOIN urls t ON t.id = l.target
         WHERE t.url = {url} AND l.kind = 'hyperlink' AND l.source <> l.target"
    );
    let inlinks_total = count(&reader, &format!("SELECT count(DISTINCT l.source) {from}"))?;
    let inlinks = rows(
        &reader,
        &format!(
            "SELECT s.url, min(l.text) {from} GROUP BY s.url ORDER BY s.url LIMIT {INLINK_ROWS}"
        ),
    )?
    .iter()
    .map(|row| Inlink {
        url: text(row.first()),
        anchor: text(row.get(1)),
    })
    .collect();
    Ok(PageDetail {
        url: ask.url.trim().to_owned(),
        facts,
        findings,
        inlinks_total,
        inlinks,
    })
}

// ----- Findings as CSV -----------------------------------------------------------------------

/// Writes findings as CSV lines while the crawl streams them.
struct CsvFindings<'a> {
    out: String,
    rows: u64,
    titles: &'a HashMap<String, (String, &'static str)>,
}

fn csv_field(text: &str) -> String {
    if text.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", text.replace('"', "\"\""))
    } else {
        text.to_owned()
    }
}

impl TableSink for CsvFindings<'_> {
    fn begin(&mut self, _: &[Column]) -> io::Result<()> {
        Ok(())
    }

    fn row(&mut self, cells: &[Cell]) -> io::Result<()> {
        if self.rows >= CSV_ROWS {
            return Ok(());
        }
        let id = text(cells.first());
        let (title, priority) = self
            .titles
            .get(&id)
            .map_or((id.as_str(), ""), |(title, priority)| {
                (title.as_str(), *priority)
            });
        let _ = writeln!(
            self.out,
            "{},{},{},{}",
            csv_field(&id),
            csv_field(title),
            priority,
            csv_field(&text(cells.get(1)))
        );
        self.rows += 1;
        Ok(())
    }

    fn finish(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[derive(Debug, Serialize)]
struct FindingsFile {
    name: String,
    csv: String,
    rows: u64,
    /// More findings than a file holds.
    cut: bool,
}

fn audit_findings_csv(context: &Context<'_>, ask: AuditAsk) -> Result<FindingsFile, AppError> {
    let (audit, reader) = reader_of(context.workspace, ask.project_id, ask.audit_id)?
        .ok_or_else(|| AppError::Invalid("the audit has no finished crawl".to_owned()))?;
    let lang = lang_of(ask.lang.as_deref());
    let titles: HashMap<String, (String, &'static str)> = catalog()
        .issues
        .iter()
        .map(|issue| {
            (
                issue.id.clone(),
                (issue.text(lang).title.clone(), issue.priority.as_str()),
            )
        })
        .collect();
    let mut sink = CsvFindings {
        out: String::from("check,title,priority,url\n"),
        rows: 0,
        titles: &titles,
    };
    let total = reader
        .tab(CrawlTab::Issues, &mut sink)
        .map_err(|error| store_error(&error))?;
    Ok(FindingsFile {
        name: format!("audit-{}-findings.csv", audit.id),
        rows: sink.rows,
        cut: total > sink.rows,
        csv: sink.out,
    })
}

/// Adds the module's commands.
pub fn register(router: &mut Router) {
    router.add("audit_links", audit_links);
    router.add("audit_hreflang", audit_hreflang);
    router.add("audit_pages", audit_pages);
    router.add("audit_page", audit_page);
    router.add("audit_findings_csv", audit_findings_csv);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_rank_flows_to_the_most_linked_page() {
        // 0 → 1, 2 → 1, 3 → 1, 1 → 0; 4 is linked by nobody and links nowhere.
        let edges = [(0, 1), (2, 1), (3, 1), (1, 0)];
        let rank = page_rank(5, &edges, RANK_ROUNDS);
        assert!((rank.iter().sum::<f64>() - 1.0).abs() < 1e-9);
        assert!(rank[1] > rank[0] && rank[0] > rank[2]);
        assert!((rank[2] - rank[4]).abs() < 1e-12);
        let score = scaled(&rank);
        assert_eq!(score[1], 100.0);
        assert_eq!(score[4], 0.0);
        assert_eq!(scaled(&[0.5, 0.5]), [100.0, 100.0]);
        assert_eq!(page_rank(0, &[], 3).len(), 0);
    }
}
