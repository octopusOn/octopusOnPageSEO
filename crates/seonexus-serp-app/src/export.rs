//! Rank reports and analyses as CSV, JSON or Excel files.

use std::fs::File;
use std::path::Path;

use seonexus_core::table::{Cell, Column, ColumnKind, TableSink};
use seonexus_report::ExportFormat;

use crate::{SerpAppError, SerpWorkspace};

fn format_of(path: &Path) -> Result<ExportFormat, SerpAppError> {
    ExportFormat::from_path(path).ok_or_else(|| {
        SerpAppError::InvalidQuery(format!(
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

fn integer(value: Option<impl Into<i64>>) -> Cell {
    value.map_or(Cell::Null, |value| Cell::Integer(value.into()))
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
) -> Result<u64, SerpAppError> {
    let format = format_of(path)?;
    let io = |source| SerpAppError::Io {
        path: path.to_owned(),
        source,
    };
    let mut sink: Box<dyn TableSink> = format.writer(File::create(path).map_err(io)?);
    sink.begin(columns).map_err(io)?;
    let mut written = 0;
    for row in rows {
        sink.row(&row).map_err(io)?;
        written += 1;
    }
    sink.finish().map_err(io)?;
    Ok(written)
}

impl SerpWorkspace {
    /// Writes the latest positions of a project's keywords on every tracker.
    ///
    /// # Errors
    /// [`SerpAppError::InvalidQuery`] for an unknown file type; store and I/O errors.
    pub fn export_rank_report(&self, project_id: i64, path: &Path) -> Result<u64, SerpAppError> {
        format_of(path)?;
        let report = self.rank_report(project_id)?;
        let columns = columns(&[
            ("keyword", ColumnKind::Text),
            ("tags", ColumnKind::Text),
            ("engine", ColumnKind::Text),
            ("country", ColumnKind::Text),
            ("language", ColumnKind::Text),
            ("device", ColumnKind::Text),
            ("position", ColumnKind::Integer),
            ("previous", ColumnKind::Integer),
            ("change", ColumnKind::Integer),
            ("best", ColumnKind::Integer),
            ("url", ColumnKind::Text),
            ("cannibalised", ColumnKind::Boolean),
            ("volume", ColumnKind::Integer),
            ("checked_at", ColumnKind::Text),
            ("error", ColumnKind::Text),
        ]);
        let rows = report.rows.iter().map(|row| {
            let tracker = report
                .project
                .trackers
                .iter()
                .find(|tracker| tracker.id == row.tracker_id);
            vec![
                text(row.keyword.clone()),
                text(row.tags.join(", ")),
                optional_text(tracker.map(|tracker| tracker.engine.as_str())),
                optional_text(tracker.map(|tracker| tracker.country.as_str())),
                optional_text(tracker.map(|tracker| tracker.language.as_str())),
                optional_text(tracker.map(|tracker| tracker.device.as_str())),
                integer(row.position),
                integer(row.previous),
                integer(row.change),
                integer(row.best),
                optional_text(row.url.as_deref()),
                Cell::Boolean(row.cannibalised),
                count(row.volume),
                optional_text(row.checked_at.as_deref()),
                optional_text(row.error.as_deref()),
            ]
        });
        write(path, &columns, rows)
    }

    /// Writes the ranking pages of an analysis with their scores and site facts.
    ///
    /// # Errors
    /// [`SerpAppError::InvalidQuery`] for an unknown file type; store and I/O errors.
    pub fn export_analysis(&self, analysis_id: i64, path: &Path) -> Result<u64, SerpAppError> {
        format_of(path)?;
        let (analysis, snapshot, domains) = self.analysis(analysis_id)?;
        let columns = columns(&[
            ("keyword", ColumnKind::Text),
            ("position", ColumnKind::Integer),
            ("url", ColumnKind::Text),
            ("domain", ColumnKind::Text),
            ("title", ColumnKind::Text),
            ("site_score", ColumnKind::Float),
            ("content_score", ColumnKind::Float),
            ("technical_score", ColumnKind::Float),
            ("strength", ColumnKind::Float),
            ("words", ColumnKind::Integer),
            ("domain_age_years", ColumnKind::Float),
            ("open_page_rank", ColumnKind::Float),
            ("referring_domains", ColumnKind::Integer),
            ("tranco_rank", ColumnKind::Integer),
            ("keyword_difficulty", ColumnKind::Float),
            ("intent", ColumnKind::Text),
        ]);
        let intent = serde_json::to_value(analysis.intent.primary)
            .ok()
            .and_then(|value| value.as_str().map(str::to_owned))
            .unwrap_or_default();
        let rows = snapshot.organic.iter().map(|result| {
            let score = analysis
                .pages
                .iter()
                .find(|page| page.position == result.position);
            let facts = domains.iter().find(|facts| facts.domain == result.domain);
            vec![
                text(snapshot.query.keyword.clone()),
                Cell::Integer(i64::from(result.position)),
                text(result.url.clone()),
                text(result.domain.clone()),
                text(result.title.clone()),
                float(score.and_then(|score| score.site.as_ref()).map(|b| b.score)),
                float(
                    score
                        .and_then(|score| score.content.as_ref())
                        .map(|b| b.score),
                ),
                float(
                    score
                        .and_then(|score| score.technical.as_ref())
                        .map(|b| b.score),
                ),
                float(score.and_then(|score| score.strength)),
                count(score.and_then(|score| u64::try_from(score.words).ok())),
                float(facts.and_then(|facts| facts.age_years)),
                float(facts.and_then(|facts| facts.open_page_rank)),
                count(facts.and_then(|facts| facts.referring_domains)),
                integer(facts.and_then(|facts| facts.tranco_rank)),
                Cell::Float(analysis.difficulty.score),
                text(intent.clone()),
            ]
        });
        write(path, &columns, rows)
    }
}
