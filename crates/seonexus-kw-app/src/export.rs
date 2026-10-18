//! Keywords and clusters as CSV, JSON or Excel files.

use std::collections::HashMap;
use std::fs::File;
use std::path::Path;

use seonexus_core::table::{Cell, Column, ColumnKind, TableSink};
use seonexus_kw_store::{KeywordQuery, KeywordRow, MAX_PAGE};
use seonexus_report::ExportFormat;

use crate::{KeywordWorkspace, KwAppError};

fn format_of(path: &Path) -> Result<ExportFormat, KwAppError> {
    ExportFormat::from_path(path).ok_or_else(|| {
        KwAppError::Invalid(format!(
            "`{}`: export to a .csv, .json or .xlsx file",
            path.display()
        ))
    })
}

fn columns(spec: &[(&str, ColumnKind)]) -> Vec<Column> {
    spec.iter()
        .map(|(name, kind)| Column::new(*name, *kind))
        .collect()
}

fn text(value: impl Into<String>) -> Cell {
    Cell::Text(value.into())
}

fn optional_text(value: Option<&str>) -> Cell {
    value.map_or(Cell::Null, |value| Cell::Text(value.to_owned()))
}

fn count(value: Option<u64>) -> Cell {
    value.map_or(Cell::Null, |value| {
        Cell::Integer(i64::try_from(value).unwrap_or(i64::MAX))
    })
}

fn float(value: Option<f64>) -> Cell {
    value.map_or(Cell::Null, Cell::Float)
}

/// Writes rows to `path` in the format its extension names. Returns the number of rows.
fn write(
    path: &Path,
    columns: &[Column],
    rows: impl IntoIterator<Item = Vec<Cell>>,
) -> Result<u64, KwAppError> {
    let format = format_of(path)?;
    let io = KwAppError::io(path);
    let file = File::create(path).map_err(KwAppError::io(path))?;
    let mut sink: Box<dyn TableSink> = format.writer(file);
    let mut written = 0;
    let result = (|| {
        sink.begin(columns)?;
        for row in rows {
            sink.row(&row)?;
            written += 1;
        }
        sink.finish()
    })();
    result.map_err(io)?;
    Ok(written)
}

/// `2026-08:100 2026-09:200`.
fn trend(row: &KeywordRow) -> Cell {
    if row.monthly.is_empty() {
        return Cell::Null;
    }
    text(
        row.monthly
            .iter()
            .map(|month| format!("{}:{}", month.month, month.volume))
            .collect::<Vec<_>>()
            .join(" "),
    )
}

impl KeywordWorkspace {
    /// Every keyword matching `query` (all pages, in its order).
    fn all_keywords(
        &self,
        project_id: i64,
        query: &KeywordQuery,
    ) -> Result<Vec<KeywordRow>, KwAppError> {
        let mut rows = Vec::new();
        let mut page_query = KeywordQuery {
            offset: 0,
            limit: MAX_PAGE,
            ..query.clone()
        };
        loop {
            let page = self.keywords(project_id, &page_query)?;
            let received = page.rows.len() as u64;
            rows.extend(page.rows);
            if received < MAX_PAGE {
                return Ok(rows);
            }
            page_query.offset += MAX_PAGE;
        }
    }

    /// Writes the keywords matching `query` with their numbers. Returns the number of rows.
    ///
    /// # Errors
    /// [`KwAppError::Invalid`] for an unknown file type; store and I/O errors.
    pub fn export_keywords(
        &self,
        project_id: i64,
        query: &KeywordQuery,
        path: &Path,
    ) -> Result<u64, KwAppError> {
        format_of(path)?;
        let rows = self.all_keywords(project_id, query)?;
        let columns = columns(&[
            ("keyword", ColumnKind::Text),
            ("status", ColumnKind::Text),
            ("volume", ColumnKind::Integer),
            ("cpc_usd", ColumnKind::Float),
            ("competition", ColumnKind::Float),
            ("difficulty", ColumnKind::Float),
            ("intent", ColumnKind::Text),
            ("cluster", ColumnKind::Integer),
            ("pillar", ColumnKind::Boolean),
            ("serp", ColumnKind::Boolean),
            ("tags", ColumnKind::Text),
            ("sources", ColumnKind::Text),
            ("trend", ColumnKind::Text),
        ]);
        let cells = rows.iter().map(|row| {
            vec![
                text(row.keyword.clone()),
                text(match row.status {
                    seonexus_kw_store::KeywordStatus::Active => "active",
                    seonexus_kw_store::KeywordStatus::Excluded => "excluded",
                }),
                count(row.volume),
                float(row.cpc_usd),
                float(row.competition),
                float(row.difficulty),
                optional_text(row.intent.as_deref()),
                count(row.cluster.map(|number| u64::from(number) + 1)),
                Cell::Boolean(row.pillar),
                Cell::Boolean(row.serp),
                text(row.tags.join(", ")),
                text(row.sources.join(", ")),
                trend(row),
            ]
        });
        write(path, &columns, cells)
    }

    /// Writes a clustering (the newest when `clustering_id` is absent), one row per keyword
    /// with its cluster's totals and mapping. Returns the number of rows.
    ///
    /// # Errors
    /// [`KwAppError::Invalid`] for an unknown file type or a project without clusterings;
    /// store and I/O errors.
    pub fn export_clusters(
        &self,
        project_id: i64,
        clustering_id: Option<i64>,
        path: &Path,
    ) -> Result<u64, KwAppError> {
        format_of(path)?;
        let (clusters, members) = {
            let store = self.store()?;
            let clustering = match clustering_id {
                Some(id) => store.clustering(id)?,
                None => store.latest_clustering(project_id)?.ok_or_else(|| {
                    KwAppError::Invalid("the project has no clustering yet".to_owned())
                })?,
            };
            if clustering.project_id != project_id {
                return Err(KwAppError::Invalid(format!(
                    "clustering {} belongs to another project",
                    clustering.id
                )));
            }
            (
                store.cluster_records(clustering.id)?,
                store.member_records(clustering.id)?,
            )
        };
        let keywords: HashMap<i64, KeywordRow> = self
            .all_keywords(
                project_id,
                &KeywordQuery {
                    status: None,
                    ..KeywordQuery::default()
                },
            )?
            .into_iter()
            .map(|row| (row.id, row))
            .collect();
        let by_number: HashMap<u32, &seonexus_kw_store::ClusterRecord> = clusters
            .iter()
            .map(|cluster| (cluster.number, cluster))
            .collect();
        let columns = columns(&[
            ("cluster", ColumnKind::Integer),
            ("cluster_label", ColumnKind::Text),
            ("cluster_keywords", ColumnKind::Integer),
            ("cluster_volume", ColumnKind::Integer),
            ("cluster_intent", ColumnKind::Text),
            ("mixed_intent", ColumnKind::Boolean),
            ("cluster_difficulty", ColumnKind::Float),
            ("verified", ColumnKind::Boolean),
            ("target_url", ColumnKind::Text),
            ("competing_urls", ColumnKind::Text),
            ("gap", ColumnKind::Boolean),
            ("competitors", ColumnKind::Text),
            ("keyword", ColumnKind::Text),
            ("pillar", ColumnKind::Boolean),
            ("score", ColumnKind::Float),
            ("shared_results", ColumnKind::Integer),
            ("volume", ColumnKind::Integer),
            ("cpc_usd", ColumnKind::Float),
            ("difficulty", ColumnKind::Float),
            ("intent", ColumnKind::Text),
        ]);
        let cells = members.iter().filter_map(|member| {
            let cluster = by_number.get(&member.number)?;
            let keyword = keywords.get(&member.keyword_id)?;
            Some(vec![
                count(Some(u64::from(cluster.number) + 1)),
                text(cluster.label.clone()),
                count(Some(u64::from(cluster.size))),
                count(Some(cluster.volume)),
                optional_text(cluster.intent.as_deref()),
                Cell::Boolean(cluster.mixed_intent),
                float(cluster.difficulty),
                Cell::Boolean(cluster.verified),
                optional_text(cluster.target_url.as_deref()),
                text(cluster.competing.join(" ")),
                Cell::Boolean(cluster.gap),
                text(
                    cluster
                        .competitors
                        .iter()
                        .map(|competitor| {
                            format!("{} ({})", competitor.domain, competitor.keywords)
                        })
                        .collect::<Vec<_>>()
                        .join(", "),
                ),
                text(keyword.keyword.clone()),
                Cell::Boolean(cluster.pillar_id == member.keyword_id),
                float(Some(member.score)),
                count(member.shared.map(u64::from)),
                count(keyword.volume),
                float(keyword.cpc_usd),
                float(keyword.difficulty),
                optional_text(keyword.intent.as_deref()),
            ])
        });
        write(path, &columns, cells)
    }
}
