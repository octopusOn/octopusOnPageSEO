use std::collections::HashMap;

use seonexus_analytics::periods::{PeriodRow, PeriodSummary, compare};
use seonexus_core::table::TableSink;
use serde::Serialize;

use super::AnalysisError;
use super::collect::{for_each_row, integer, key, with_filter};
use crate::workspace::Workspace;

/// Input of [`compare_periods`].
#[derive(Debug, Clone)]
pub struct PeriodRequest {
    /// Site name.
    pub site: String,
    /// Earlier period as a `date:` value, e.g. `2026-09-01..2026-09-30`.
    pub before: String,
    /// Later period, e.g. `last30d`.
    pub after: String,
    /// Grouping dimension, e.g. `url`, `directory`, `bot` or `status`.
    pub group: String,
    /// Extra filter, e.g. `bot:googlebot* verification:verified`.
    pub filter: Option<String>,
}

/// Result of [`compare_periods`]; the rows went to the sink.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PeriodReport {
    /// Site name.
    pub site: String,
    /// Grouping dimension.
    pub group: String,
    /// Totals and counts per trend.
    pub summary: PeriodSummary,
    /// Rows written.
    pub rows: u64,
}

/// Compares hits per key between two periods (new, lost, up, down).
///
/// # Errors
/// See [`AnalysisError`]; invalid periods or groups are reported as query errors.
pub fn compare_periods(
    workspace: &Workspace,
    request: &PeriodRequest,
    sink: &mut dyn TableSink,
) -> Result<PeriodReport, AnalysisError> {
    let group = request.group.trim();
    let hits = |period: &str| -> Result<HashMap<String, u64>, AnalysisError> {
        let text = with_filter(
            request.filter.as_deref(),
            &format!("date:{} group:{group} metric:hits", period.trim()),
        );
        let mut hits = HashMap::new();
        for_each_row(workspace, &request.site, &text, |cells| {
            let name = key(cells.first()).unwrap_or_else(|| "-".to_owned());
            *hits.entry(name).or_insert(0) += integer(cells.get(1));
        })?;
        Ok(hits)
    };
    let (rows, summary) = compare(&hits(&request.before)?, &hits(&request.after)?);
    sink.begin(&PeriodRow::columns(group))
        .map_err(AnalysisError::Output)?;
    for row in &rows {
        sink.row(&row.cells()).map_err(AnalysisError::Output)?;
    }
    sink.finish().map_err(AnalysisError::Output)?;
    Ok(PeriodReport {
        site: request.site.clone(),
        group: group.to_owned(),
        summary,
        rows: rows.len() as u64,
    })
}
