// Tauri deserializes command arguments and injects owned State/AppHandle values.
#![allow(clippy::needless_pass_by_value)]

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, PoisonError};

use seonexus_spider_app::{
    ContentMatch, CrawlActivity, CrawlConfig, CrawlControl, CrawlMeta, CrawlOverview, CrawlPhase,
    CrawlPlan, CrawlProgress, CrawlStorage, CrawlTab, PageRequest, RedirectTrace,
    TableExportRequest, TablePage, UrlDetail,
};
use tauri::{Emitter, Manager, State};

use crate::{AppState, Job, RunningJob};
use seonexus_spider_app::{DuplicateGroup, RemediationDetail, RemediationRequest};

static NEXT: AtomicU64 = AtomicU64::new(1);

type Result<T> = std::result::Result<T, String>;

#[tauri::command]
pub(crate) async fn schema_test(
    request: seonexus_spider_app::SchemaTestRequest,
) -> Result<seonexus_spider_app::SchemaTestResult> {
    blocking(move || seonexus_spider_app::test_schema(&request)).await
}

async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> Result<T> + Send + 'static,
) -> Result<T> {
    tauri::async_runtime::spawn_blocking(work)
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub(crate) async fn list_crawls(state: State<'_, AppState>) -> Result<Vec<CrawlMeta>> {
    let workspace = state.workspace.clone();
    blocking(move || workspace.crawls().map_err(|e| e.to_string())).await
}

#[tauri::command]
pub(crate) async fn crawl_storage(state: State<'_, AppState>) -> Result<Vec<CrawlStorage>> {
    let workspace = state.workspace.clone();
    blocking(move || workspace.crawl_storage().map_err(|e| e.to_string())).await
}

#[tauri::command]
pub(crate) fn list_jobs(state: State<'_, AppState>) -> Vec<Job> {
    let jobs = state.jobs.lock().unwrap_or_else(PoisonError::into_inner);
    let mut sorted: Vec<_> = jobs.values().collect();
    sorted.sort_unstable_by_key(|job| std::cmp::Reverse(job.order));
    sorted.into_iter().map(|job| job.job.clone()).collect()
}

#[tauri::command]
pub(crate) fn start_crawl<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: State<'_, AppState>,
    config: CrawlConfig,
) -> Result<String> {
    CrawlPlan::new(config.clone()).map_err(|e| e.to_string())?;
    let order = NEXT.fetch_add(1, Ordering::Relaxed);
    let id = format!("job-{order}");
    let control = Arc::new(CrawlControl::new());
    let job = Job {
        id: id.clone(),
        url: config.start_urls.first().cloned().unwrap_or_default(),
        status: "running".into(),
        rendering: config.rendering.mode,
        progress: CrawlProgress::default(),
        crawl_id: None,
        phase: CrawlPhase::Starting,
        activity: CrawlActivity::default(),
        revision: 0,
        meta: None,
        error: None,
    };
    state
        .jobs
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .insert(
            id.clone(),
            RunningJob {
                job,
                control: control.clone(),
                order,
            },
        );
    let workspace = state.workspace.clone();
    let job_id = id.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let result = workspace.crawl_observed(config, &control, |event| {
            let state = app.state::<AppState>();
            let mut jobs = state.jobs.lock().unwrap_or_else(PoisonError::into_inner);
            if let Some(running) = jobs.get_mut(&job_id)
                && running.job.observe(event)
            {
                let _ = app.emit("spider-progress", &running.job);
            }
        });
        let state = app.state::<AppState>();
        let mut jobs = state.jobs.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(running) = jobs.get_mut(&job_id) {
            running.job.finish(result);
            let _ = app.emit("spider-progress", &running.job);
        }
        // Retain recent completions for reconnecting clients without growing indefinitely.
        crate::jobs::prune(&mut jobs);
    });
    Ok(id)
}

#[tauri::command]
pub(crate) fn control_crawl<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: State<'_, AppState>,
    job_id: String,
    action: String,
) -> Result<()> {
    let mut jobs = state.jobs.lock().unwrap_or_else(PoisonError::into_inner);
    let running = jobs.get_mut(&job_id).ok_or("Unknown crawl job")?;
    if !running.job.can_control() {
        return Err("Crawl is finalizing or has already finished".into());
    }
    running.job.revision += 1;
    match action.as_str() {
        "pause" => {
            running.control.set_paused(true);
            running.job.status = "paused".into();
        }
        "resume" => {
            running.control.set_paused(false);
            running.job.status = "running".into();
        }
        "stop" => {
            running.control.stop();
            running.job.status = "stopping".into();
        }
        _ => return Err("Unknown crawl action".into()),
    }
    let _ = app.emit("spider-progress", &running.job);
    Ok(())
}

#[tauri::command]
pub(crate) async fn crawl_overview(
    state: State<'_, AppState>,
    id: String,
) -> Result<CrawlOverview> {
    let workspace = state.workspace.clone();
    blocking(move || {
        workspace
            .open(&id)
            .and_then(|r| Ok(r.overview()?))
            .map_err(|e| e.to_string())
    })
    .await
}

#[tauri::command]
pub(crate) async fn crawl_page(
    state: State<'_, AppState>,
    id: String,
    request: PageRequest,
) -> Result<TablePage> {
    let workspace = state.workspace.clone();
    blocking(move || {
        workspace
            .open(&id)
            .and_then(|r| Ok(r.page(&request)?))
            .map_err(|e| e.to_string())
    })
    .await
}

#[tauri::command]
pub(crate) async fn score_remediation(
    state: State<'_, AppState>,
    id: String,
    request: RemediationRequest,
) -> Result<RemediationDetail> {
    let workspace = state.workspace.clone();
    blocking(move || {
        workspace
            .remediation(&id, &request)
            .map_err(|e| e.to_string())
    })
    .await
}

#[tauri::command]
pub(crate) async fn url_detail(
    state: State<'_, AppState>,
    id: String,
    url: String,
) -> Result<UrlDetail> {
    let workspace = state.workspace.clone();
    blocking(move || {
        workspace
            .open(&id)
            .and_then(|r| Ok(r.detail(&url)?))
            .map_err(|e| e.to_string())
    })
    .await
}

#[tauri::command]
pub(crate) async fn url_duplicate_groups(
    state: State<'_, AppState>,
    id: String,
    url: String,
) -> Result<Vec<DuplicateGroup>> {
    let workspace = state.workspace.clone();
    blocking(move || {
        workspace
            .open(&id)
            .and_then(|reader| Ok(reader.duplicate_groups(&url)?))
            .map_err(|e| e.to_string())
    })
    .await
}

#[tauri::command]
pub(crate) async fn url_content_match(
    state: State<'_, AppState>,
    id: String,
    url: String,
) -> Result<Option<ContentMatch>> {
    let workspace = state.workspace.clone();
    blocking(move || {
        workspace
            .open(&id)
            .and_then(|reader| Ok(reader.content_match(&url)?))
            .map_err(|e| e.to_string())
    })
    .await
}

#[tauri::command]
pub(crate) async fn url_redirects(
    state: State<'_, AppState>,
    id: String,
    url: String,
) -> Result<RedirectTrace> {
    let workspace = state.workspace.clone();
    blocking(move || {
        workspace
            .open(&id)
            .and_then(|r| Ok(r.redirect_trace(&url)?))
            .map_err(|e| e.to_string())
    })
    .await
}

#[tauri::command]
pub(crate) async fn export_crawl(
    state: State<'_, AppState>,
    id: String,
    tab: String,
    path: PathBuf,
    selection: Option<TableExportRequest>,
) -> Result<u64> {
    let workspace = state.workspace.clone();
    blocking(move || {
        let tab = CrawlTab::from_name(&tab).ok_or("Unknown export tab")?;
        let result = if let Some(selection) = selection {
            if CrawlTab::from_name(&selection.request.tab) != Some(tab) {
                return Err("Export selection does not match the requested tab".into());
            }
            workspace.export_selection(&id, &selection, &path, None)
        } else {
            workspace.export(&id, tab, &path, None)
        };
        result.map(|r| r.rows).map_err(|e| e.to_string())
    })
    .await
}

#[tauri::command]
pub(crate) async fn delete_crawl(state: State<'_, AppState>, id: String) -> Result<()> {
    let workspace = state.workspace.clone();
    blocking(move || {
        if workspace.meta(&id).map_err(|e| e.to_string())?.status
            == seonexus_spider_app::CrawlStatus::Running
        {
            return Err("Stop the crawl before deleting it".into());
        }
        workspace.delete(&id).map_err(|e| e.to_string())
    })
    .await
}

#[tauri::command]
pub(crate) async fn resource_usage() -> Result<Option<u64>> {
    blocking(|| Ok(seonexus_spider_app::resident_memory_bytes())).await
}

fn comparison_ready(state: &AppState, baseline: &str, current: &str) -> Result<()> {
    let jobs = state.jobs.lock().unwrap_or_else(PoisonError::into_inner);
    if jobs.values().any(|running| {
        running.job.is_active()
            && running
                .job
                .crawl_id
                .as_deref()
                .is_some_and(|id| id == baseline || id == current)
    }) {
        return Err("Finish both crawls before comparing them".into());
    }
    Ok(())
}

fn sitemap_ready(state: &AppState, id: &str) -> Result<()> {
    let jobs = state.jobs.lock().unwrap_or_else(PoisonError::into_inner);
    if jobs
        .values()
        .any(|running| running.job.is_active() && running.job.crawl_id.as_deref() == Some(id))
    {
        return Err("Finish the crawl and its analysis before generating a sitemap".into());
    }
    Ok(())
}

#[tauri::command]
pub(crate) async fn sitemap_plan(
    state: State<'_, AppState>,
    id: String,
    selection: seonexus_spider_app::SitemapSelection,
) -> Result<seonexus_spider_app::SitemapPlan> {
    sitemap_ready(&state, &id)?;
    let workspace = state.workspace.clone();
    blocking(move || {
        workspace
            .sitemap_plan(&id, &selection)
            .map_err(|e| e.to_string())
    })
    .await
}

#[tauri::command]
pub(crate) async fn export_sitemap(
    state: State<'_, AppState>,
    id: String,
    selection: seonexus_spider_app::SitemapSelection,
    path: PathBuf,
    format: seonexus_spider_app::SitemapFormat,
) -> Result<seonexus_spider_app::SitemapExport> {
    sitemap_ready(&state, &id)?;
    let workspace = state.workspace.clone();
    blocking(move || {
        workspace
            .export_sitemap(&id, &selection, &path, format)
            .map_err(|e| e.to_string())
    })
    .await
}

#[tauri::command]
pub(crate) async fn comparison_summary(
    state: State<'_, AppState>,
    baseline: String,
    current: String,
    internal_only: bool,
    request: Option<seonexus_spider_app::ComparisonRequest>,
) -> Result<seonexus_spider_app::ComparisonSummary> {
    comparison_ready(&state, &baseline, &current)?;
    let workspace = state.workspace.clone();
    let mut request = request.unwrap_or_default();
    request.internal_only = internal_only;
    blocking(move || {
        workspace
            .compare(&baseline, &current)
            .and_then(|c| Ok(c.filtered_summary(&request)?))
            .map_err(|e| e.to_string())
    })
    .await
}

#[tauri::command]
pub(crate) async fn comparison_page(
    state: State<'_, AppState>,
    baseline: String,
    current: String,
    request: seonexus_spider_app::ComparisonRequest,
) -> Result<TablePage> {
    comparison_ready(&state, &baseline, &current)?;
    let workspace = state.workspace.clone();
    blocking(move || {
        workspace
            .compare(&baseline, &current)
            .and_then(|c| Ok(c.page(&request)?))
            .map_err(|e| e.to_string())
    })
    .await
}

#[tauri::command]
pub(crate) async fn comparison_detail(
    state: State<'_, AppState>,
    baseline: String,
    current: String,
    url: String,
    internal_only: bool,
) -> Result<TablePage> {
    comparison_ready(&state, &baseline, &current)?;
    let workspace = state.workspace.clone();
    blocking(move || {
        workspace
            .compare(&baseline, &current)
            .and_then(|c| Ok(c.detail(&url, internal_only)?))
            .map_err(|e| e.to_string())
    })
    .await
}

#[tauri::command]
pub(crate) async fn export_comparison(
    state: State<'_, AppState>,
    baseline: String,
    current: String,
    request: seonexus_spider_app::ComparisonRequest,
    path: PathBuf,
) -> Result<u64> {
    comparison_ready(&state, &baseline, &current)?;
    let workspace = state.workspace.clone();
    blocking(move || {
        workspace
            .export_comparison(&baseline, &current, &request, &path)
            .map_err(|e| e.to_string())
    })
    .await
}

#[tauri::command]
pub(crate) async fn finding_comparison_summary(
    state: State<'_, AppState>,
    baseline: String,
    current: String,
    internal_only: bool,
    request: Option<seonexus_spider_app::FindingComparisonRequest>,
) -> Result<seonexus_spider_app::FindingComparisonSummary> {
    comparison_ready(&state, &baseline, &current)?;
    let workspace = state.workspace.clone();
    let mut request = request.unwrap_or_default();
    request.internal_only = internal_only;
    blocking(move || {
        workspace
            .compare(&baseline, &current)
            .and_then(|c| Ok(c.filtered_findings_summary(&request)?))
            .map_err(|e| e.to_string())
    })
    .await
}

#[tauri::command]
pub(crate) async fn finding_comparison_page(
    state: State<'_, AppState>,
    baseline: String,
    current: String,
    request: seonexus_spider_app::FindingComparisonRequest,
) -> Result<TablePage> {
    comparison_ready(&state, &baseline, &current)?;
    let workspace = state.workspace.clone();
    blocking(move || {
        workspace
            .compare(&baseline, &current)
            .and_then(|c| Ok(c.findings_page(&request)?))
            .map_err(|e| e.to_string())
    })
    .await
}

#[tauri::command]
pub(crate) async fn export_finding_comparison(
    state: State<'_, AppState>,
    baseline: String,
    current: String,
    request: seonexus_spider_app::FindingComparisonRequest,
    path: PathBuf,
) -> Result<u64> {
    comparison_ready(&state, &baseline, &current)?;
    let workspace = state.workspace.clone();
    blocking(move || {
        workspace
            .export_finding_comparison(&baseline, &current, &request, &path)
            .map_err(|e| e.to_string())
    })
    .await
}
