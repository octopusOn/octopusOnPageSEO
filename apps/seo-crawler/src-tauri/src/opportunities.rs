#![allow(clippy::needless_pass_by_value)]
use crate::AppState;
use octopuson_spider_app::opportunities::{
    AnalysisRequest, AnalysisSummary, CrawlEvidence, GoogleCatalog, ImportPreview, ImportRequest,
    OAuthStart, OpportunityDetail, OpportunityPage, OpportunityPageRequest, OpportunityProject,
    SyncJob, WorkspaceStatus, crawl_evidence,
};
use tauri::State;

async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(work)
        .await
        .map_err(|e| e.to_string())?
}
#[tauri::command]
pub(crate) async fn opportunity_status(
    state: State<'_, AppState>,
) -> Result<WorkspaceStatus, String> {
    let workspace = state.opportunities.clone();
    blocking(move || workspace.status()).await
}
#[tauri::command]
pub(crate) async fn opportunity_project_save(
    state: State<'_, AppState>,
    project: OpportunityProject,
) -> Result<OpportunityProject, String> {
    let workspace = state.opportunities.clone();
    blocking(move || workspace.save_project(project)).await
}
#[tauri::command]
pub(crate) async fn opportunity_project_remove(
    state: State<'_, AppState>,
    id: String,
) -> Result<(), String> {
    let workspace = state.opportunities.clone();
    blocking(move || workspace.remove_project(&id)).await
}
#[tauri::command]
pub(crate) async fn opportunity_cache_clear(
    state: State<'_, AppState>,
    id: String,
) -> Result<(), String> {
    let workspace = state.opportunities.clone();
    blocking(move || workspace.clear_api_cache(&id)).await
}
#[tauri::command]
pub(crate) async fn opportunity_import_preview(
    state: State<'_, AppState>,
    request: ImportRequest,
) -> Result<ImportPreview, String> {
    let workspace = state.opportunities.clone();
    blocking(move || workspace.preview_import(&request)).await
}
#[tauri::command]
pub(crate) async fn opportunity_import(
    state: State<'_, AppState>,
    request: ImportRequest,
    digest: String,
) -> Result<ImportPreview, String> {
    let workspace = state.opportunities.clone();
    blocking(move || workspace.import(&request, &digest)).await
}
#[tauri::command]
pub(crate) async fn opportunity_google_client_import(
    state: State<'_, AppState>,
    path: String,
) -> Result<(), String> {
    let workspace = state.opportunities.clone();
    blocking(move || workspace.import_google_client(std::path::Path::new(&path))).await
}
#[tauri::command]
pub(crate) async fn opportunity_google_start(
    state: State<'_, AppState>,
) -> Result<OAuthStart, String> {
    let workspace = state.opportunities.clone();
    let started = blocking(move || workspace.begin_google()).await?;
    if let Err(error) = open_browser(&started.url) {
        state.opportunities.cancel_google(&started.id);
        return Err(error);
    }
    Ok(started)
}
#[tauri::command]
pub(crate) fn opportunity_google_cancel(state: State<'_, AppState>, id: String) {
    state.opportunities.cancel_google(&id);
}
#[tauri::command]
pub(crate) async fn opportunity_google_disconnect(
    state: State<'_, AppState>,
    id: String,
) -> Result<(), String> {
    let workspace = state.opportunities.clone();
    blocking(move || workspace.disconnect_google(&id)).await
}
#[tauri::command]
pub(crate) async fn opportunity_google_catalog(
    state: State<'_, AppState>,
    connection: String,
) -> Result<GoogleCatalog, String> {
    let workspace = state.opportunities.clone();
    blocking(move || workspace.google_catalog(&connection)).await
}
#[tauri::command]
pub(crate) fn opportunity_sync(
    state: State<'_, AppState>,
    request: AnalysisRequest,
) -> Result<String, String> {
    state
        .opportunity_jobs
        .start(state.opportunities.clone(), request)
}
#[tauri::command]
pub(crate) fn opportunity_jobs(state: State<'_, AppState>) -> Vec<SyncJob> {
    state.opportunity_jobs.jobs()
}
#[tauri::command]
pub(crate) fn opportunity_cancel(state: State<'_, AppState>, id: String) -> Result<(), String> {
    state.opportunity_jobs.cancel(&id)
}
#[tauri::command]
pub(crate) fn opportunity_analyze(
    state: State<'_, AppState>,
    request: AnalysisRequest,
) -> Result<String, String> {
    state
        .opportunity_jobs
        .start_analysis(state.opportunities.clone(), request)
}
#[tauri::command]
pub(crate) fn opportunity_result(
    state: State<'_, AppState>,
    id: String,
) -> Result<AnalysisSummary, String> {
    state.opportunity_jobs.result(&id)
}
#[tauri::command]
pub(crate) async fn opportunity_page(
    state: State<'_, AppState>,
    request: OpportunityPageRequest,
) -> Result<OpportunityPage, String> {
    let jobs = state.opportunity_jobs.clone();
    blocking(move || jobs.page(&request)).await
}
#[tauri::command]
pub(crate) async fn opportunity_detail(
    state: State<'_, AppState>,
    analysis_id: String,
    url: String,
) -> Result<OpportunityDetail, String> {
    let jobs = state.opportunity_jobs.clone();
    blocking(move || jobs.detail(&analysis_id, &url)).await
}
#[tauri::command]
pub(crate) async fn opportunity_export(
    state: State<'_, AppState>,
    request: OpportunityPageRequest,
    path: String,
) -> Result<u64, String> {
    let jobs = state.opportunity_jobs.clone();
    blocking(move || jobs.export(&request, std::path::Path::new(&path))).await
}
#[tauri::command]
pub(crate) async fn opportunity_crawl_evidence(
    state: State<'_, AppState>,
    id: String,
    url: String,
) -> Result<CrawlEvidence, String> {
    let workspace = state.workspace.clone();
    blocking(move || crawl_evidence(&workspace, &id, &url)).await
}

fn open_browser(url: &str) -> Result<(), String> {
    if !url.starts_with("https://accounts.google.com/o/oauth2/v2/auth?") {
        return Err("Unexpected authorization URL".into());
    }
    #[cfg(target_os = "macos")]
    let result = std::process::Command::new("/usr/bin/open").arg(url).spawn();
    #[cfg(windows)]
    let result = std::process::Command::new("rundll32.exe")
        .args(["url.dll,FileProtocolHandler", url])
        .spawn();
    #[cfg(not(any(target_os = "macos", windows)))]
    let result = std::process::Command::new("xdg-open").arg(url).spawn();
    result
        .map(|_| ())
        .map_err(|_| "Could not open the system browser".into())
}
