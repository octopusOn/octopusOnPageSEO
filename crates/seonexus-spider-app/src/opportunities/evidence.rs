use seonexus_core::table::Cell;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct CrawlEvidence {
    pub crawl_id: String,
    pub crawl_name: String,
    pub created_at_ms: i64,
    pub facts: crate::TablePage,
    pub issue_ids: Vec<String>,
}
/// # Errors
/// Returns an error for an unfinished, missing or incompatible archive, or a failed read.
pub fn crawl_evidence(
    workspace: &crate::SpiderWorkspace,
    id: &str,
    url: &str,
) -> Result<CrawlEvidence, String> {
    let meta = workspace.meta(id).map_err(|e| e.to_string())?;
    if meta.status != crate::CrawlStatus::Completed {
        return Err("Choose a completed crawl for technical evidence".into());
    }
    if workspace
        .crawls_dir()
        .join(&meta.id)
        .join("crawl.duckdb.wal")
        .exists()
    {
        return Err("Archive contains a WAL; it is not opened for opportunity evidence".into());
    }
    let reader = workspace
        .library
        .open_archive(id)
        .map_err(|e| e.to_string())?;
    let detail = reader.detail(url).map_err(|e| e.to_string())?;
    let columns = [
        "url",
        "status",
        "title",
        "meta_description",
        "indexability",
        "canonical",
    ];
    let indexes: Vec<_> = detail
        .facts
        .columns
        .iter()
        .enumerate()
        .filter(|(_, c)| columns.contains(&c.name.as_str()))
        .map(|(i, _)| i)
        .collect();
    let facts = crate::TablePage {
        columns: indexes
            .iter()
            .map(|i| detail.facts.columns[*i].clone())
            .collect(),
        rows: detail
            .facts
            .rows
            .iter()
            .map(|r| {
                indexes
                    .iter()
                    .map(|i| r.get(*i).cloned().unwrap_or(Cell::Null))
                    .collect()
            })
            .collect(),
        total: detail.facts.total,
        link_summary: None,
    };
    let issue_ids = detail
        .findings
        .rows
        .iter()
        .filter_map(|r| match r.first() {
            Some(Cell::Text(id)) => Some(id.clone()),
            _ => None,
        })
        .collect();
    Ok(CrawlEvidence {
        crawl_id: meta.id,
        crawl_name: meta.name,
        created_at_ms: meta.created_at_ms,
        facts,
        issue_ids,
    })
}

#[cfg(test)]
#[path = "tests/evidence.rs"]
mod tests;
