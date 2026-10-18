use std::collections::HashMap;

use octopuson_analytics::urls::{ComparisonRow, ComparisonSummary, UrlComparison};
use octopuson_core::table::{Cell, Column, ColumnKind, TableSink};
use octopuson_url::{NormalizeOptions, normalize};
use serde::Serialize;

use super::collect::{for_each_row, integer, with_filter};
use super::url_lists::{UrlSource, read_list};
use super::{AnalysisError, DEFAULT_CRAWLER_FILTER};
use crate::workspace::Workspace;

/// Input of [`compare_url_list`].
#[derive(Debug, Clone)]
pub struct UrlListRequest {
    /// Site name.
    pub site: String,
    /// URL lists (crawler exports, sitemaps, plain lists).
    pub sources: Vec<UrlSource>,
    /// Which hits count as "requested"; `None` uses [`DEFAULT_CRAWLER_FILTER`], an empty
    /// string counts every hit.
    pub filter: Option<String>,
    /// How URLs are matched.
    pub normalize: NormalizeOptions,
    /// Only list URLs on this host are compared (others are skipped).
    pub host: Option<String>,
}

/// URLs read from one source.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SourceCount {
    /// File or URL.
    pub source: String,
    /// URLs read.
    pub urls: u64,
}

/// Result of [`compare_url_list`]; the rows went to the sink.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UrlListReport {
    /// Site name.
    pub site: String,
    /// Filter applied to logged hits.
    pub filter: String,
    /// URLs per source.
    pub sources: Vec<SourceCount>,
    /// List entries that are not URLs or belong to another host.
    pub skipped: u64,
    /// Counts per presence.
    pub summary: ComparisonSummary,
    /// Columns of the lists shown next to the log data (crawler export fields, `lastmod`).
    pub list_columns: Vec<String>,
    /// Rows written.
    pub rows: u64,
}

/// Extra list columns per normalized URL, like a spreadsheet `VLOOKUP`: the first non-empty
/// value of every column wins.
#[derive(Debug, Default)]
struct ListData {
    columns: Vec<String>,
    values: HashMap<String, Vec<String>>,
}

impl ListData {
    /// Positions of `names` in the merged columns, adding new ones.
    fn positions(&mut self, names: &[String]) -> Vec<usize> {
        names
            .iter()
            .map(|name| {
                self.columns
                    .iter()
                    .position(|c| c == name)
                    .unwrap_or_else(|| {
                        self.columns.push(name.clone());
                        self.columns.len() - 1
                    })
            })
            .collect()
    }

    fn set(&mut self, key: &str, positions: &[usize], values: &[String]) {
        let width = self.columns.len();
        let row = self.values.entry(key.to_owned()).or_default();
        row.resize(width, String::new());
        for (&position, value) in positions.iter().zip(values) {
            if row[position].is_empty() {
                row[position].clone_from(value);
            }
        }
    }

    /// Output columns with kinds inferred from the values (numbers sort as numbers).
    fn output_columns(&self) -> Vec<Column> {
        let fixed = ComparisonRow::columns();
        (0..self.columns.len())
            .map(|index| {
                let values = self
                    .values
                    .values()
                    .filter_map(|row| row.get(index))
                    .filter(|v| !v.is_empty());
                let kind = infer_kind(values);
                let name = &self.columns[index];
                let clash = fixed.iter().any(|c| c.name.eq_ignore_ascii_case(name));
                let name = if clash {
                    format!("{name} (list)")
                } else {
                    name.clone()
                };
                Column::new(name, kind)
            })
            .collect()
    }

    fn cells(&self, key: &str, columns: &[Column]) -> Vec<Cell> {
        let row = self.values.get(key);
        columns
            .iter()
            .enumerate()
            .map(|(index, column)| {
                let value = row.and_then(|r| r.get(index)).map_or("", String::as_str);
                typed_cell(value, column.kind)
            })
            .collect()
    }
}

fn infer_kind<'a>(values: impl Iterator<Item = &'a String> + Clone) -> ColumnKind {
    let mut values = values.peekable();
    if values.peek().is_none() {
        return ColumnKind::Text;
    }
    let clean = |v: &str| v.trim().replace(',', "");
    if values.clone().all(|v| clean(v).parse::<i64>().is_ok()) {
        ColumnKind::Integer
    } else if values.all(|v| clean(v).parse::<f64>().is_ok()) {
        ColumnKind::Float
    } else {
        ColumnKind::Text
    }
}

fn typed_cell(value: &str, kind: ColumnKind) -> Cell {
    let clean = value.trim().replace(',', "");
    match kind {
        _ if value.trim().is_empty() => Cell::Null,
        ColumnKind::Integer => clean.parse().map_or(Cell::Null, Cell::Integer),
        ColumnKind::Float => clean.parse().map_or(Cell::Null, Cell::Float),
        _ => Cell::Text(value.to_owned()),
    }
}

/// Matches URL lists against logged URLs: listed pages nobody requested (`not-in-logs`) and
/// requested pages missing from the lists (`not-in-list`, e.g. orphans).
///
/// # Errors
/// See [`AnalysisError`].
pub fn compare_url_list(
    workspace: &Workspace,
    request: &UrlListRequest,
    sink: &mut dyn TableSink,
) -> Result<UrlListReport, AnalysisError> {
    let options = request.normalize;
    let host = request.host.as_deref().map(str::to_ascii_lowercase);
    let mut comparison = UrlComparison::default();
    let mut list_data = ListData::default();
    let mut sources = Vec::new();
    let mut skipped = 0;
    for source in &request.sources {
        let list = read_list(source)?;
        if list.urls.is_empty() {
            return Err(AnalysisError::EmptyList(source.to_string()));
        }
        sources.push(SourceCount {
            source: source.to_string(),
            urls: list.urls.len() as u64,
        });
        let positions = list_data.positions(&list.columns);
        for (index, raw) in list.urls.iter().enumerate() {
            match normalize(raw, &options) {
                Some(url) if host.is_none() || url.host.is_none() || url.host == host => {
                    if let Some(values) = list.values.get(index) {
                        list_data.set(&url.key, &positions, values);
                    }
                    comparison.add_listed(url.key);
                }
                _ => skipped += 1,
            }
        }
    }

    let filter = request
        .filter
        .clone()
        .unwrap_or_else(|| DEFAULT_CRAWLER_FILTER.to_owned());
    let text = with_filter(
        Some(&filter),
        "group:url metric:hits,last_seen,status_codes",
    );
    for_each_row(workspace, &request.site, &text, |cells| {
        let Some(Cell::Text(raw)) = cells.first() else {
            return;
        };
        let Some(url) = normalize(raw, &options) else {
            return;
        };
        let last_seen = match cells.get(2) {
            Some(Cell::Timestamp(ts)) => Some(*ts),
            _ => None,
        };
        let statuses = match cells.get(3) {
            Some(Cell::Text(codes)) => codes.split(',').filter_map(|c| c.parse().ok()).collect(),
            _ => Vec::new(),
        };
        comparison.add_logged(url.key, integer(cells.get(1)), last_seen, statuses);
    })?;

    let summary = comparison.summary();
    let rows = comparison.into_rows();
    let extra = list_data.output_columns();
    let mut columns = ComparisonRow::columns();
    columns.extend(extra.iter().cloned());
    sink.begin(&columns).map_err(AnalysisError::Output)?;
    for row in &rows {
        let mut cells = row.cells();
        cells.extend(list_data.cells(&row.url, &extra));
        sink.row(&cells).map_err(AnalysisError::Output)?;
    }
    sink.finish().map_err(AnalysisError::Output)?;
    Ok(UrlListReport {
        site: request.site.clone(),
        filter,
        sources,
        skipped,
        summary,
        list_columns: list_data.columns,
        rows: rows.len() as u64,
    })
}
