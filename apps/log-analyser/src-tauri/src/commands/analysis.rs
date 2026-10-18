use octopuson_log_app::analysis::{
    self, NormalizeOptions, OverviewRequest, PeriodReport, PeriodRequest, RobotsReport,
    RobotsRequest, SiteCrawlOverview, UrlListReport, UrlListRequest, UrlSource,
};
use octopuson_log_app::table::{Cell, Column, TableBuffer};
use serde::Serialize;
use tauri::State;

use super::blocking;
use crate::error::CommandResult;
use crate::state::AppState;

/// An analysis report together with its result table.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct AnalysisTable<R> {
    report: R,
    columns: Vec<Column>,
    rows: Vec<Vec<Cell>>,
}

impl<R> AnalysisTable<R> {
    fn new(report: R, table: TableBuffer) -> Self {
        Self {
            report,
            columns: table.columns,
            rows: table.rows,
        }
    }
}

#[tauri::command]
pub(crate) async fn crawl_overview(
    state: State<'_, AppState>,
    site: String,
    period: Option<String>,
) -> CommandResult<SiteCrawlOverview> {
    let workspace = state.workspace.clone();
    blocking(move || {
        let request = OverviewRequest { site, period };
        Ok(analysis::crawl_overview(&workspace, &request)?)
    })
    .await
}

#[tauri::command]
pub(crate) async fn compare_urls(
    state: State<'_, AppState>,
    site: String,
    sources: Vec<String>,
    filter: Option<String>,
    normalize: NormalizeOptions,
    host: Option<String>,
) -> CommandResult<AnalysisTable<UrlListReport>> {
    let workspace = state.workspace.clone();
    blocking(move || {
        let request = UrlListRequest {
            site,
            sources: sources.iter().map(|s| UrlSource::parse(s)).collect(),
            filter,
            normalize,
            host: host.filter(|h| !h.trim().is_empty()),
        };
        let mut table = TableBuffer::default();
        let report = analysis::compare_url_list(&workspace, &request, &mut table)?;
        Ok(AnalysisTable::new(report, table))
    })
    .await
}

#[tauri::command]
pub(crate) async fn robots_compliance(
    state: State<'_, AppState>,
    site: String,
    robots: Option<String>,
    filter: Option<String>,
) -> CommandResult<AnalysisTable<RobotsReport>> {
    let workspace = state.workspace.clone();
    blocking(move || {
        let request = RobotsRequest {
            site,
            robots,
            filter,
        };
        let mut table = TableBuffer::default();
        let report = analysis::robots_compliance(&workspace, &request, &mut table)?;
        Ok(AnalysisTable::new(report, table))
    })
    .await
}

#[tauri::command]
pub(crate) async fn compare_periods(
    state: State<'_, AppState>,
    site: String,
    before: String,
    after: String,
    group: String,
    filter: Option<String>,
) -> CommandResult<AnalysisTable<PeriodReport>> {
    let workspace = state.workspace.clone();
    blocking(move || {
        let request = PeriodRequest {
            site,
            before,
            after,
            group,
            filter,
        };
        let mut table = TableBuffer::default();
        let report = analysis::compare_periods(&workspace, &request, &mut table)?;
        Ok(AnalysisTable::new(report, table))
    })
    .await
}
