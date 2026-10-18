// Tauri deserializes command arguments and injects owned State/AppHandle values.
#![allow(clippy::needless_pass_by_value)]

use seonexus_serp_app::cloud::CloudStatus;
use seonexus_serp_app::cloud_api::SourceMode;
use std::path::PathBuf;
use std::sync::Arc;

use seonexus_serp::mapping::ParsedPage;
use seonexus_serp::profile::ProfileOverride;
use seonexus_serp::spec::ProviderSpec;
use seonexus_serp::{Engine, SerpSnapshot, Target};
use seonexus_serp_app::analysis::AnalyzeRequest;
use seonexus_serp_app::browser::ParserInfo;
use seonexus_serp_app::domains::{DomainFacts, DomainOptions, TRANCO_NOTICE};
use seonexus_serp_app::providers::ProviderInfo;
use seonexus_serp_app::rank::{Project, ProjectSpec, RankReport, TrackEstimate};
use seonexus_serp_app::reparse::StoredPage;
use seonexus_serp_app::secrets::SecretState;
use seonexus_serp_app::settings::Settings;
use seonexus_serp_app::store::{
    AnalysisSummary, Dataset, KeywordRow, ProviderUsage, SnapshotFilter, SnapshotSummary,
};
use seonexus_serp_app::{ProviderEstimate, SearchRequest, SerpWorkspace};
use seonexus_serp_score::KeywordAnalysis;
use serde::Serialize;
use tauri::State;

use crate::AppState;
use crate::jobs::{self, Job};

type Result<T> = std::result::Result<T, String>;

/// Normalises a site as typed in the window (`https://www.Örnek.com.tr/`).
fn normalise(target: &mut Target) -> Result<()> {
    *target = Target::parse(&target.value, target.mode).map_err(|e| e.to_string())?;
    Ok(())
}

async fn blocking<T: Send + 'static>(
    state: &State<'_, AppState>,
    work: impl FnOnce(&SerpWorkspace) -> std::result::Result<T, seonexus_serp_app::SerpAppError>
    + Send
    + 'static,
) -> Result<T> {
    let workspace = Arc::clone(&state.workspace);
    tauri::async_runtime::spawn_blocking(move || work(&workspace).map_err(|e| e.to_string()))
        .await
        .map_err(|e| e.to_string())?
}

/// What the window shows about the installation.
#[derive(Debug, Serialize)]
pub(crate) struct AppInfo {
    version: &'static str,
    data_dir: String,
    secret_store: &'static str,
    open_pagerank: SecretState,
    tranco: Option<Dataset>,
    tranco_notice: &'static str,
}

#[tauri::command]
pub(crate) async fn app_info(state: State<'_, AppState>) -> Result<AppInfo> {
    blocking(&state, |workspace| {
        Ok(AppInfo {
            version: env!("CARGO_PKG_VERSION"),
            data_dir: workspace.data_dir().display().to_string(),
            secret_store: workspace.secrets().backend_name(),
            open_pagerank: workspace.open_pagerank_state()?,
            tranco: workspace.store()?.dataset("tranco")?,
            tranco_notice: TRANCO_NOTICE,
        })
    })
    .await
}

#[tauri::command]
pub(crate) async fn get_settings(state: State<'_, AppState>) -> Result<Settings> {
    blocking(&state, SerpWorkspace::settings).await
}

#[tauri::command]
pub(crate) async fn save_settings(state: State<'_, AppState>, settings: Settings) -> Result<()> {
    blocking(&state, move |workspace| workspace.save_settings(&settings)).await
}

#[tauri::command]
pub(crate) async fn list_providers(state: State<'_, AppState>) -> Result<Vec<ProviderInfo>> {
    blocking(&state, SerpWorkspace::providers).await
}

#[tauri::command]
pub(crate) async fn set_secret(
    state: State<'_, AppState>,
    provider: String,
    name: String,
    value: String,
) -> Result<()> {
    let value = value.trim().to_owned();
    if value.is_empty() {
        return Err("the key is empty".to_owned());
    }
    blocking(&state, move |workspace| {
        workspace.set_secret(&provider, &name, &value)
    })
    .await
}

#[tauri::command]
pub(crate) async fn remove_secret(
    state: State<'_, AppState>,
    provider: String,
    name: String,
) -> Result<()> {
    blocking(&state, move |workspace| {
        workspace.remove_secret(&provider, &name)
    })
    .await
}

#[tauri::command]
pub(crate) async fn custom_specs(state: State<'_, AppState>) -> Result<Vec<ProviderSpec>> {
    blocking(&state, SerpWorkspace::custom_specs).await
}

#[tauri::command]
pub(crate) async fn save_spec(state: State<'_, AppState>, spec: ProviderSpec) -> Result<()> {
    blocking(&state, move |workspace| workspace.save_spec(spec)).await
}

#[tauri::command]
pub(crate) async fn remove_spec(state: State<'_, AppState>, id: String) -> Result<()> {
    blocking(&state, move |workspace| workspace.remove_spec(&id)).await
}

#[tauri::command]
pub(crate) async fn import_specs(state: State<'_, AppState>, json: String) -> Result<Vec<String>> {
    blocking(&state, move |workspace| workspace.import_specs(&json)).await
}

#[tauri::command]
pub(crate) async fn export_specs(state: State<'_, AppState>, ids: Vec<String>) -> Result<String> {
    blocking(&state, move |workspace| workspace.export_specs(&ids)).await
}

/// Parses a pasted or stored response with a spec, without sending a request.
#[tauri::command]
pub(crate) fn test_spec(
    spec: ProviderSpec,
    engine: Engine,
    body: String,
    url: Option<String>,
) -> Result<ParsedPage> {
    seonexus_serp_app::providers::test_spec(&spec, engine, &body, url.as_deref())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub(crate) async fn list_parsers(state: State<'_, AppState>) -> Result<Vec<ParserInfo>> {
    blocking(&state, SerpWorkspace::parsers).await
}

#[tauri::command]
pub(crate) async fn save_parser_override(
    state: State<'_, AppState>,
    patch: ProfileOverride,
) -> Result<()> {
    blocking(&state, move |workspace| {
        workspace.save_parser_override(patch)
    })
    .await
}

#[tauri::command]
pub(crate) async fn remove_parser_override(
    state: State<'_, AppState>,
    engine: Engine,
) -> Result<()> {
    blocking(&state, move |workspace| {
        workspace.remove_parser_override(engine)
    })
    .await
}

#[tauri::command]
pub(crate) async fn close_browser(state: State<'_, AppState>) -> Result<()> {
    blocking(&state, |workspace| {
        workspace.close_browser();
        Ok(())
    })
    .await
}

#[tauri::command]
pub(crate) async fn reset_browser_profile(state: State<'_, AppState>) -> Result<()> {
    if jobs::list(&state).iter().any(Job::is_active) {
        return Err("stop the running searches first".to_owned());
    }
    blocking(&state, SerpWorkspace::reset_browser_profile).await
}

#[tauri::command]
pub(crate) async fn estimate_search(
    state: State<'_, AppState>,
    request: SearchRequest,
) -> Result<Vec<ProviderEstimate>> {
    blocking(&state, move |workspace| workspace.estimate(&request)).await
}

#[tauri::command]
pub(crate) fn start_analysis<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    mut request: AnalyzeRequest,
) -> Result<String> {
    if let Some(target) = &mut request.search.target {
        normalise(target)?;
    }
    request.search.query.validate().map_err(|e| e.to_string())?;
    jobs::start_analysis(&app, request)
}

#[tauri::command]
pub(crate) fn start_search<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    mut request: SearchRequest,
) -> Result<String> {
    if let Some(target) = &mut request.target {
        normalise(target)?;
    }
    request.query.validate().map_err(|e| e.to_string())?;
    jobs::start_search(&app, request)
}

#[tauri::command]
pub(crate) async fn start_track<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: State<'_, AppState>,
    project_id: i64,
) -> Result<String> {
    let project = blocking(&state, move |workspace| workspace.project(project_id)).await?;
    jobs::start_track(&app, project.id, project.name)
}

#[tauri::command]
pub(crate) fn list_jobs(state: State<'_, AppState>) -> Vec<Job> {
    jobs::list(&state)
}

#[tauri::command]
pub(crate) fn cancel_job(state: State<'_, AppState>, id: String) -> Result<()> {
    jobs::cancel(&state, &id)
}

#[tauri::command]
pub(crate) fn clear_jobs(state: State<'_, AppState>) {
    jobs::clear_finished(&state);
}

#[tauri::command]
pub(crate) async fn list_analyses(
    state: State<'_, AppState>,
    keyword: Option<String>,
    limit: Option<usize>,
) -> Result<Vec<AnalysisSummary>> {
    blocking(&state, move |workspace| {
        workspace.analyses(keyword.as_deref(), limit.unwrap_or(200))
    })
    .await
}

/// A stored analysis with what it was made from.
#[derive(Debug, Serialize)]
pub(crate) struct AnalysisView {
    id: i64,
    analysis: KeywordAnalysis,
    snapshot: SerpSnapshot,
    domains: Vec<DomainFacts>,
}

#[tauri::command]
pub(crate) async fn get_analysis(state: State<'_, AppState>, id: i64) -> Result<AnalysisView> {
    blocking(&state, move |workspace| {
        let (analysis, snapshot, domains) = workspace.analysis(id)?;
        Ok(AnalysisView {
            id,
            analysis,
            snapshot,
            domains,
        })
    })
    .await
}

#[tauri::command]
pub(crate) async fn rescore_analysis(
    state: State<'_, AppState>,
    id: i64,
) -> Result<KeywordAnalysis> {
    blocking(&state, move |workspace| workspace.rescore(id)).await
}

#[tauri::command]
pub(crate) async fn list_snapshots(
    state: State<'_, AppState>,
    filter: SnapshotFilter,
) -> Result<Vec<SnapshotSummary>> {
    blocking(&state, move |workspace| workspace.snapshots(&filter)).await
}

#[tauri::command]
pub(crate) async fn get_snapshot(state: State<'_, AppState>, id: i64) -> Result<SerpSnapshot> {
    blocking(&state, move |workspace| workspace.snapshot(id)).await
}

#[tauri::command]
pub(crate) async fn delete_snapshot(state: State<'_, AppState>, id: i64) -> Result<()> {
    blocking(&state, move |workspace| workspace.delete_snapshot(id)).await
}

#[tauri::command]
pub(crate) async fn usage(state: State<'_, AppState>) -> Result<Vec<ProviderUsage>> {
    blocking(&state, SerpWorkspace::usage).await
}

#[tauri::command]
pub(crate) async fn domain_facts(
    state: State<'_, AppState>,
    hosts: Vec<String>,
    options: Option<DomainOptions>,
) -> Result<Vec<DomainFacts>> {
    blocking(&state, move |workspace| {
        workspace.domain_facts(&hosts, options.unwrap_or_default(), &|| false)
    })
    .await
}

/// Imports the Tranco list; the window shows the licence notice and asks first.
#[tauri::command]
pub(crate) async fn update_tranco(state: State<'_, AppState>, accepted: bool) -> Result<u64> {
    if !accepted {
        return Err("accept the Tranco data licence first".to_owned());
    }
    blocking(&state, SerpWorkspace::update_tranco).await
}

#[tauri::command]
pub(crate) async fn list_projects(state: State<'_, AppState>) -> Result<Vec<Project>> {
    blocking(&state, SerpWorkspace::projects).await
}

#[tauri::command]
pub(crate) async fn create_project(
    state: State<'_, AppState>,
    mut spec: ProjectSpec,
) -> Result<Project> {
    normalise(&mut spec.target)?;
    blocking(&state, move |workspace| workspace.create_project(&spec)).await
}

#[tauri::command]
pub(crate) async fn update_project(state: State<'_, AppState>, mut project: Project) -> Result<()> {
    normalise(&mut project.target)?;
    blocking(&state, move |workspace| workspace.update_project(&project)).await
}

#[tauri::command]
pub(crate) async fn delete_project(state: State<'_, AppState>, id: i64) -> Result<()> {
    if jobs::list(&state)
        .iter()
        .any(|job| job.is_active() && job.project_id == Some(id))
    {
        return Err("stop the running rank check of this project first".to_owned());
    }
    blocking(&state, move |workspace| workspace.delete_project(id)).await
}

#[tauri::command]
pub(crate) async fn add_keywords(
    state: State<'_, AppState>,
    project_id: i64,
    keywords: Vec<String>,
    tags: Option<Vec<String>>,
) -> Result<usize> {
    blocking(&state, move |workspace| {
        workspace.add_keywords(project_id, &keywords, &tags.unwrap_or_default())
    })
    .await
}

#[tauri::command]
pub(crate) async fn list_keywords(
    state: State<'_, AppState>,
    project_id: i64,
) -> Result<Vec<KeywordRow>> {
    blocking(&state, move |workspace| workspace.keywords(project_id)).await
}

#[tauri::command]
pub(crate) async fn remove_keyword(
    state: State<'_, AppState>,
    project_id: i64,
    keyword_id: i64,
) -> Result<()> {
    blocking(&state, move |workspace| {
        workspace.remove_keyword(project_id, keyword_id)
    })
    .await
}

#[tauri::command]
pub(crate) async fn set_competitors(
    state: State<'_, AppState>,
    project_id: i64,
    domains: Vec<String>,
) -> Result<()> {
    blocking(&state, move |workspace| {
        workspace.set_competitors(project_id, &domains)
    })
    .await
}

#[tauri::command]
pub(crate) async fn estimate_track(
    state: State<'_, AppState>,
    project_id: i64,
) -> Result<TrackEstimate> {
    blocking(&state, move |workspace| {
        workspace.estimate_track(project_id)
    })
    .await
}

#[tauri::command]
pub(crate) async fn rank_report(state: State<'_, AppState>, project_id: i64) -> Result<RankReport> {
    blocking(&state, move |workspace| workspace.rank_report(project_id)).await
}

#[tauri::command]
pub(crate) async fn stored_pages(
    state: State<'_, AppState>,
    snapshot_id: i64,
) -> Result<Vec<StoredPage>> {
    blocking(&state, move |workspace| workspace.stored_pages(snapshot_id)).await
}

/// Reads a stored browser page with the profile in use or the override being edited.
#[tauri::command]
pub(crate) async fn test_parser(
    state: State<'_, AppState>,
    snapshot_id: i64,
    page: Option<u16>,
    patch: Option<ProfileOverride>,
) -> Result<ParsedPage> {
    blocking(&state, move |workspace| {
        workspace.test_parser(snapshot_id, page.unwrap_or(1), patch.as_ref())
    })
    .await
}

#[tauri::command]
pub(crate) async fn reparse_snapshot(
    state: State<'_, AppState>,
    snapshot_id: i64,
) -> Result<SerpSnapshot> {
    blocking(&state, move |workspace| {
        workspace.reparse_snapshot(snapshot_id)
    })
    .await
}

/// Writes a project's rank report to `path` (.csv, .json or .xlsx); returns the rows written.
#[tauri::command]
pub(crate) async fn export_rank_report(
    state: State<'_, AppState>,
    project_id: i64,
    path: PathBuf,
) -> Result<u64> {
    blocking(&state, move |workspace| {
        workspace.export_rank_report(project_id, &path)
    })
    .await
}

/// Writes an analysis's ranking pages to `path` (.csv, .json or .xlsx); returns the rows.
#[tauri::command]
pub(crate) async fn export_analysis(
    state: State<'_, AppState>,
    id: i64,
    path: PathBuf,
) -> Result<u64> {
    blocking(&state, move |workspace| {
        workspace.export_analysis(id, &path)
    })
    .await
}

// ------------------------------------------------------------------ OctopusOn SEO account

/// Opens an address in the default browser.
fn open_browser(url: &str) -> bool {
    let command = if cfg!(target_os = "macos") {
        std::process::Command::new("open").arg(url).status()
    } else if cfg!(windows) {
        std::process::Command::new("rundll32")
            .args(["url.dll,FileProtocolHandler", url])
            .status()
    } else {
        std::process::Command::new("xdg-open").arg(url).status()
    };
    command.is_ok_and(|status| status.success())
}

#[tauri::command]
pub(crate) async fn cloud_status(state: State<'_, AppState>) -> Result<CloudStatus> {
    blocking(&state, SerpWorkspace::cloud_status).await
}

#[tauri::command]
pub(crate) fn start_account_login<R: tauri::Runtime>(app: tauri::AppHandle<R>) -> Result<String> {
    jobs::start_account_login(&app, open_browser)
}

#[tauri::command]
pub(crate) async fn account_logout(state: State<'_, AppState>) -> Result<()> {
    blocking(&state, SerpWorkspace::sign_out).await
}

#[tauri::command]
pub(crate) async fn set_source_mode(
    state: State<'_, AppState>,
    provider: String,
    mode: SourceMode,
) -> Result<()> {
    blocking(&state, move |workspace| {
        workspace.set_source_mode(&provider, mode)
    })
    .await
}

/// Opens a page of the account site (buying credits, devices) in the browser.
#[tauri::command]
pub(crate) async fn open_account_page(state: State<'_, AppState>, page: String) -> Result<()> {
    let path = match page.as_str() {
        "credits" => "/kredi",
        "devices" => "/jetonlar",
        "usage" => "/kullanim",
        _ => "/",
    };
    let status = blocking(&state, SerpWorkspace::cloud_status).await?;
    if open_browser(&format!(
        "{}{path}",
        status.portal_url.trim_end_matches('/')
    )) {
        Ok(())
    } else {
        Err("cannot open the browser".to_owned())
    }
}
