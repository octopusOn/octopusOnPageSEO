use super::{
    AnalysisRequest, Opportunity, OpportunityReport, OpportunityWorkspace, SyncProgress, new_id,
    now_ms,
};
use octopuson_core::table::{Cell, Column, ColumnKind};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::{
    Arc, Mutex, PoisonError,
    atomic::{AtomicBool, Ordering},
};

#[derive(Debug, Clone, Serialize)]
pub struct SyncJob {
    pub id: String,
    pub project_id: String,
    pub kind: String,
    pub created_at_ms: i64,
    pub analysis_id: Option<String>,
    pub status: String,
    pub progress: SyncProgress,
    pub error: Option<String>,
}
struct Snapshot {
    summary: AnalysisSummary,
    rows: Vec<Opportunity>,
}
#[derive(Default)]
struct Runtime {
    jobs: BTreeMap<String, (SyncJob, Arc<AtomicBool>)>,
    snapshots: BTreeMap<String, Arc<Snapshot>>,
}
#[derive(Clone, Default)]
pub struct OpportunityJobs {
    inner: Arc<Mutex<Runtime>>,
}
#[derive(Debug, Clone, Serialize)]
pub struct AnalysisSummary {
    pub id: String,
    pub total: usize,
    pub report: OpportunityReport,
    pub created_at_ms: i64,
}
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct OpportunityPageRequest {
    pub analysis_id: String,
    pub offset: usize,
    pub limit: usize,
    pub search: String,
    pub kind: String,
    pub confidence: String,
}
#[derive(Debug, Serialize)]
pub struct OpportunityPage {
    pub rows: Vec<Opportunity>,
    pub total: usize,
    pub offset: usize,
}
#[derive(Debug, Serialize)]
pub struct OpportunityDetail {
    pub opportunity: Opportunity,
    pub signal_total: usize,
    pub query_total: usize,
}

impl OpportunityJobs {
    pub fn jobs(&self) -> Vec<SyncJob> {
        self.inner
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .jobs
            .values()
            .map(|(job, _)| job.clone())
            .collect()
    }
    pub fn busy(&self) -> bool {
        self.inner
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .jobs
            .values()
            .any(|(job, _)| job.status == "running")
    }
    fn reserve(&self, project_id: &str, kind: &str) -> Result<(String, Arc<AtomicBool>), String> {
        let mut state = self.inner.lock().unwrap_or_else(PoisonError::into_inner);
        if state
            .jobs
            .values()
            .any(|(job, _)| job.project_id == project_id && job.status == "running")
        {
            return Err("This project already has a running operation".into());
        }
        if state
            .jobs
            .values()
            .filter(|(job, _)| job.status == "running")
            .count()
            >= 2
        {
            return Err("At most two opportunity operations can run together".into());
        }
        let id = new_id()?;
        let control = Arc::new(AtomicBool::new(false));
        state
            .jobs
            .retain(|_, (job, _)| job.status == "running" || job.project_id != project_id);
        while state.jobs.len() >= 20 {
            let oldest = state
                .jobs
                .iter()
                .filter(|(_, (job, _))| job.status != "running")
                .min_by_key(|(_, (job, _))| job.created_at_ms)
                .map(|(id, _)| id.clone());
            if let Some(id) = oldest {
                state.jobs.remove(&id);
            } else {
                break;
            }
        }
        state.jobs.insert(
            id.clone(),
            (
                SyncJob {
                    id: id.clone(),
                    project_id: project_id.into(),
                    kind: kind.into(),
                    created_at_ms: now_ms(),
                    analysis_id: None,
                    status: "running".into(),
                    progress: SyncProgress {
                        phase: if kind == "sync" {
                            "starting"
                        } else {
                            "analyzing"
                        }
                        .into(),
                        ..Default::default()
                    },
                    error: None,
                },
                control.clone(),
            ),
        );
        Ok((id, control))
    }
    /// # Errors
    /// Returns an error for an unknown operation identifier.
    pub fn cancel(&self, id: &str) -> Result<(), String> {
        let mut state = self.inner.lock().unwrap_or_else(PoisonError::into_inner);
        let (job, cancel) = state
            .jobs
            .get_mut(id)
            .ok_or("Unknown synchronization job")?;
        cancel.store(true, Ordering::Relaxed);
        if job.status == "running" {
            job.progress.phase = "cancelling".into();
            job.progress.retry_after_seconds = None;
        }
        Ok(())
    }
    /// # Errors
    /// Returns an error if operation limits are reached or a secure identifier cannot be generated.
    pub fn start(
        &self,
        workspace: OpportunityWorkspace,
        request: AnalysisRequest,
    ) -> Result<String, String> {
        let (id, control) = self.reserve(&request.project_id, "sync")?;
        let runtime = self.clone();
        let job_id = id.clone();
        std::thread::spawn(move || {
            let result = workspace.sync_google(&request, &control, |progress| {
                if let Some((job, _)) = runtime
                    .inner
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .jobs
                    .get_mut(&job_id)
                {
                    job.progress = progress;
                    if control.load(Ordering::Relaxed) {
                        job.progress.phase = "cancelling".into();
                        job.progress.retry_after_seconds = None;
                    }
                }
            });
            let mut state = runtime.inner.lock().unwrap_or_else(PoisonError::into_inner);
            if let Some((job, _)) = state.jobs.get_mut(&job_id) {
                job.status = match result {
                    Ok(()) => "completed",
                    Err(_) if control.load(Ordering::Relaxed) => "cancelled",
                    Err(_) => "failed",
                }
                .into();
                job.error = result.err();
            }
        });
        Ok(id)
    }
    /// # Errors
    /// Returns an error for invalid or unavailable observations, or a failed local analysis.
    pub fn analyze(
        &self,
        workspace: &OpportunityWorkspace,
        request: &AnalysisRequest,
    ) -> Result<AnalysisSummary, String> {
        self.analyze_controlled(workspace, request, &|| false)
    }
    fn analyze_controlled(
        &self,
        workspace: &OpportunityWorkspace,
        request: &AnalysisRequest,
        cancel: &dyn Fn() -> bool,
    ) -> Result<AnalysisSummary, String> {
        let mut report = workspace.analyze_with_cancel(request, cancel)?;
        let rows = std::mem::take(&mut report.opportunities);
        let summary = AnalysisSummary {
            id: new_id()?,
            total: rows.len(),
            report,
            created_at_ms: now_ms(),
        };
        let mut state = self.inner.lock().unwrap_or_else(PoisonError::into_inner);
        if cancel() {
            return Err("Operation cancelled".into());
        }
        state
            .snapshots
            .retain(|_, s| s.summary.report.request.project_id != request.project_id);
        while state.snapshots.len() >= 4 {
            if let Some(id) = state
                .snapshots
                .iter()
                .min_by_key(|(_, s)| s.summary.created_at_ms)
                .map(|(id, _)| id.clone())
            {
                state.snapshots.remove(&id);
            }
        }
        state.snapshots.insert(
            summary.id.clone(),
            Arc::new(Snapshot {
                summary: summary.clone(),
                rows,
            }),
        );
        Ok(summary)
    }
    /// # Errors
    /// Returns an error if this project already has a running operation or a secure identifier is unavailable.
    pub fn start_analysis(
        &self,
        workspace: OpportunityWorkspace,
        request: AnalysisRequest,
    ) -> Result<String, String> {
        let (id, control) = self.reserve(&request.project_id, "analysis")?;
        let runtime = self.clone();
        let job_id = id.clone();
        std::thread::spawn(move || {
            let result = runtime
                .analyze_controlled(&workspace, &request, &|| control.load(Ordering::Relaxed));
            let mut state = runtime.inner.lock().unwrap_or_else(PoisonError::into_inner);
            if let Some((job, _)) = state.jobs.get_mut(&job_id) {
                match result {
                    Ok(summary) => {
                        job.status = "completed".into();
                        job.analysis_id = Some(summary.id);
                        job.progress.rows = summary.total;
                    }
                    Err(error) => {
                        job.status = if control.load(Ordering::Relaxed) {
                            "cancelled"
                        } else {
                            "failed"
                        }
                        .into();
                        job.error = Some(error);
                    }
                }
            }
        });
        Ok(id)
    }
    /// # Errors
    /// Returns an error when the analysis snapshot has expired or was replaced.
    pub fn result(&self, id: &str) -> Result<AnalysisSummary, String> {
        Ok(self.snapshot(id)?.summary.clone())
    }
    fn snapshot(&self, id: &str) -> Result<Arc<Snapshot>, String> {
        self.inner
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .snapshots
            .get(id)
            .cloned()
            .ok_or_else(|| "Analysis expired or was replaced; run the local analysis again".into())
    }
    /// # Errors
    /// Returns an error for an expired analysis or unsupported filters.
    pub fn page(&self, request: &OpportunityPageRequest) -> Result<OpportunityPage, String> {
        let snapshot = self.snapshot(&request.analysis_id)?;
        let rows = filtered(&snapshot.rows, request)?;
        Ok(OpportunityPage {
            total: rows.len(),
            offset: request.offset,
            rows: rows
                .into_iter()
                .skip(request.offset)
                .take(request.limit.clamp(1, 250))
                .map(|r| {
                    let mut row = r.clone();
                    row.queries.clear();
                    let mut kinds = std::collections::BTreeSet::new();
                    row.signals
                        .retain(|signal| kinds.insert(signal.kind.clone()));
                    row
                })
                .collect(),
        })
    }
    /// # Errors
    /// Returns an error for an expired analysis or an absent URL.
    pub fn detail(&self, id: &str, url: &str) -> Result<OpportunityDetail, String> {
        let snapshot = self.snapshot(id)?;
        let mut opportunity = snapshot
            .rows
            .iter()
            .find(|o| o.url == url)
            .cloned()
            .ok_or("URL is not in this analysis")?;
        let signal_total = opportunity.signals.len();
        let query_total = opportunity.queries.len();
        opportunity.queries.truncate(300);
        opportunity.signals.truncate(300);
        Ok(OpportunityDetail {
            opportunity,
            signal_total,
            query_total,
        })
    }
    /// # Errors
    /// Returns an error for unsupported filters, an expired snapshot or a failed atomic export.
    pub fn export(&self, request: &OpportunityPageRequest, path: &Path) -> Result<u64, String> {
        let snapshot = self.snapshot(&request.analysis_id)?;
        let rows = filtered(&snapshot.rows, request)?;
        crate::exports::write_rows(path, None, |sink| {
            let columns = [
                ("url", ColumnKind::Text),
                ("signals", ColumnKind::Text),
                ("clicks", ColumnKind::Float),
                ("previous_clicks", ColumnKind::Float),
                ("impressions", ColumnKind::Float),
                ("ctr", ColumnKind::Float),
                ("position", ColumnKind::Float),
                ("click_impact", ColumnKind::Float),
                ("confidence", ColumnKind::Text),
                ("priority", ColumnKind::Float),
                ("priority_basis", ColumnKind::Text),
                ("google_sessions", ColumnKind::Float),
                ("key_events", ColumnKind::Float),
                ("goal_rate", ColumnKind::Float),
                ("purchase_revenue", ColumnKind::Float),
                ("currency", ColumnKind::Text),
            ];
            sink.begin(
                &columns
                    .iter()
                    .map(|(name, kind)| Column::new(*name, *kind))
                    .collect::<Vec<_>>(),
            )
            .map_err(crate::CrawlStoreError::Output)?;
            let optional = |v: Option<f64>| v.map_or(Cell::Null, Cell::Float);
            for row in &rows {
                let kinds: std::collections::BTreeSet<_> =
                    row.signals.iter().map(|s| s.kind.as_str()).collect();
                sink.row(&[
                    Cell::Text(row.url.clone()),
                    Cell::Text(kinds.into_iter().collect::<Vec<_>>().join(", ")),
                    Cell::Float(row.current.clicks),
                    optional(row.previous.as_ref().map(|m| m.clicks)),
                    Cell::Float(row.current.impressions),
                    optional(row.current.ctr),
                    optional(row.current.position),
                    optional(row.impact_clicks),
                    Cell::Text(row.confidence.clone()),
                    Cell::Float(row.priority),
                    Cell::Text(row.priority_basis.clone()),
                    optional(row.business.as_ref().map(|b| b.sessions)),
                    optional(row.business.as_ref().and_then(|b| b.key_events)),
                    optional(row.business.as_ref().and_then(|b| b.goal_rate)),
                    optional(row.business.as_ref().and_then(|b| b.revenue)),
                    row.business
                        .as_ref()
                        .and_then(|b| b.currency.clone())
                        .map_or(Cell::Null, Cell::Text),
                ])
                .map_err(crate::CrawlStoreError::Output)?;
            }
            sink.finish().map_err(crate::CrawlStoreError::Output)?;
            Ok(rows.len() as u64)
        })
        .map_err(|e| e.to_string())
    }
}
fn filtered<'a>(
    rows: &'a [Opportunity],
    request: &OpportunityPageRequest,
) -> Result<Vec<&'a Opportunity>, String> {
    if !matches!(
        request.kind.as_str(),
        "" | "low-ctr" | "near-win" | "traffic-loss"
    ) || !matches!(
        request.confidence.as_str(),
        "" | "observed" | "limited" | "review"
    ) {
        return Err("Unknown opportunity filter".into());
    }
    let search = request.search.to_lowercase();
    Ok(rows
        .iter()
        .filter(|r| {
            (request.kind.is_empty() || r.signals.iter().any(|s| s.kind == request.kind))
                && (request.confidence.is_empty() || r.confidence == request.confidence)
                && (search.is_empty()
                    || r.url.to_lowercase().contains(&search)
                    || r.signals
                        .iter()
                        .filter_map(|s| s.query.as_deref())
                        .any(|s| s.to_lowercase().contains(&search)))
        })
        .collect())
}

#[cfg(test)]
#[path = "tests/runtime.rs"]
mod tests;
