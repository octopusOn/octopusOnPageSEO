// Tauri deserializes command arguments and injects owned State/AppHandle values.
#![allow(clippy::needless_pass_by_value)]

use std::path::PathBuf;
use std::sync::Arc;

use octopuson_kw_app::browser::BrowserStatus;
use octopuson_kw_app::cloud::CloudStatus;
use octopuson_kw_app::cloud_api::SourceMode;
use octopuson_kw_app::cluster::ClusterRequest;
use octopuson_kw_app::discover::{DiscoverEstimate, DiscoverRequest};
use octopuson_kw_app::enrich::{EnrichEstimate, EnrichRequest};
use octopuson_kw_app::google::{GoogleScopes, GoogleStatus, SearchConsoleRequest};
use octopuson_kw_app::import::{ImportOutcome, ImportPreview};
use octopuson_kw_app::keywords::AddOutcome;
use octopuson_kw_app::secrets::SecretState;
use octopuson_kw_app::serps::{SerpEstimate, SerpRequest};
use octopuson_kw_app::settings::Settings;
use octopuson_kw_app::sources::ProviderInfo;
use octopuson_kw_app::store::{
    ClusterPage, ClusterQuery, ClusteringRow, KeywordPage, KeywordQuery, KeywordStatus, MemberRow,
    ProjectRow, ProjectSpec, ProviderUsage, RunRow, SerpRecord,
};
use octopuson_kw_app::{KeywordWorkspace, KwAppError};
use serde::Serialize;
use tauri::State;

use crate::AppState;
use crate::jobs::{self, Finish, Job, JobKind};

type Result<T> = std::result::Result<T, String>;

async fn blocking<T: Send + 'static>(
    state: &State<'_, AppState>,
    work: impl FnOnce(&KeywordWorkspace) -> std::result::Result<T, KwAppError> + Send + 'static,
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
}

#[tauri::command]
pub(crate) async fn app_info(state: State<'_, AppState>) -> Result<AppInfo> {
    blocking(&state, |workspace| {
        Ok(AppInfo {
            version: env!("CARGO_PKG_VERSION"),
            data_dir: workspace.data_dir().display().to_string(),
            secret_store: workspace.secrets().backend_name(),
        })
    })
    .await
}

#[tauri::command]
pub(crate) async fn get_settings(state: State<'_, AppState>) -> Result<Settings> {
    blocking(&state, KeywordWorkspace::settings).await
}

#[tauri::command]
pub(crate) async fn save_settings(state: State<'_, AppState>, settings: Settings) -> Result<()> {
    blocking(&state, move |workspace| workspace.save_settings(&settings)).await
}

// ------------------------------------------------------------------ Projects

#[tauri::command]
pub(crate) async fn list_projects(state: State<'_, AppState>) -> Result<Vec<ProjectRow>> {
    blocking(&state, KeywordWorkspace::projects).await
}

#[tauri::command]
pub(crate) async fn create_project(
    state: State<'_, AppState>,
    spec: ProjectSpec,
) -> Result<ProjectRow> {
    blocking(&state, move |workspace| workspace.create_project(&spec)).await
}

#[tauri::command]
pub(crate) async fn update_project(
    state: State<'_, AppState>,
    id: i64,
    spec: ProjectSpec,
) -> Result<ProjectRow> {
    blocking(&state, move |workspace| workspace.update_project(id, &spec)).await
}

#[tauri::command]
pub(crate) async fn delete_project(state: State<'_, AppState>, id: i64) -> Result<()> {
    if jobs::list(&state)
        .iter()
        .any(|job| job.is_active() && job.project_id == Some(id))
    {
        return Err("stop the project's running jobs first".to_owned());
    }
    blocking(&state, move |workspace| workspace.delete_project(id)).await
}

#[tauri::command]
pub(crate) async fn list_runs(
    state: State<'_, AppState>,
    project_id: Option<i64>,
) -> Result<Vec<RunRow>> {
    blocking(&state, move |workspace| {
        Ok(workspace.store()?.runs(project_id, 50)?)
    })
    .await
}

// ------------------------------------------------------------------ Keywords

#[tauri::command]
pub(crate) async fn add_keywords(
    state: State<'_, AppState>,
    project_id: i64,
    keywords: Vec<String>,
) -> Result<AddOutcome> {
    blocking(&state, move |workspace| {
        workspace.add_keywords(project_id, &keywords, "manual", None)
    })
    .await
}

#[tauri::command]
pub(crate) async fn preview_import(
    state: State<'_, AppState>,
    path: PathBuf,
) -> Result<ImportPreview> {
    blocking(&state, move |workspace| workspace.preview_import(&path)).await
}

#[tauri::command]
pub(crate) async fn import_file(
    state: State<'_, AppState>,
    project_id: i64,
    path: PathBuf,
) -> Result<ImportOutcome> {
    blocking(&state, move |workspace| workspace.import(project_id, &path)).await
}

#[tauri::command]
pub(crate) async fn list_keywords(
    state: State<'_, AppState>,
    project_id: i64,
    query: KeywordQuery,
) -> Result<KeywordPage> {
    blocking(&state, move |workspace| {
        workspace.keywords(project_id, &query)
    })
    .await
}

#[tauri::command]
pub(crate) async fn set_status(
    state: State<'_, AppState>,
    project_id: i64,
    ids: Vec<i64>,
    status: KeywordStatus,
) -> Result<u64> {
    blocking(&state, move |workspace| {
        workspace.set_status(project_id, &ids, status)
    })
    .await
}

#[tauri::command]
pub(crate) async fn tag_keywords(
    state: State<'_, AppState>,
    project_id: i64,
    ids: Vec<i64>,
    add: Vec<String>,
    remove: Vec<String>,
) -> Result<()> {
    blocking(&state, move |workspace| {
        workspace.tag(project_id, &ids, &add, &remove)
    })
    .await
}

/// Tags and sources of a project's keywords with their counts.
#[derive(Debug, Serialize)]
pub(crate) struct Facets {
    tags: Vec<(String, u64)>,
    sources: Vec<(String, u64)>,
}

#[tauri::command]
pub(crate) async fn keyword_facets(state: State<'_, AppState>, project_id: i64) -> Result<Facets> {
    blocking(&state, move |workspace| {
        let store = workspace.store()?;
        Ok(Facets {
            tags: store.tags(project_id)?,
            sources: store.sources(project_id)?,
        })
    })
    .await
}

#[tauri::command]
pub(crate) async fn keyword_serp(
    state: State<'_, AppState>,
    keyword_id: i64,
    engine: String,
    device: String,
) -> Result<Option<SerpRecord>> {
    blocking(&state, move |workspace| {
        Ok(workspace.store()?.serp(keyword_id, &engine, &device)?)
    })
    .await
}

// ------------------------------------------------------------------ Paid runs

fn project_name(state: &AppState, project_id: i64) -> std::result::Result<String, String> {
    state
        .workspace
        .project(project_id)
        .map(|project| project.spec.name)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) async fn estimate_discovery(
    state: State<'_, AppState>,
    project_id: i64,
    request: DiscoverRequest,
) -> Result<DiscoverEstimate> {
    blocking(&state, move |workspace| {
        workspace.estimate_discovery(project_id, &request)
    })
    .await
}

#[tauri::command]
pub(crate) fn start_discovery<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: State<'_, AppState>,
    project_id: i64,
    request: DiscoverRequest,
) -> Result<String> {
    let job = Job::new(
        JobKind::Discover,
        project_name(&state, project_id)?,
        Some(project_id),
    );
    jobs::spawn(&app, job, move |workspace, control| {
        Ok(Finish::Run(
            workspace.discover(project_id, &request, control)?.run,
        ))
    })
}

#[tauri::command]
pub(crate) async fn estimate_enrichment(
    state: State<'_, AppState>,
    project_id: i64,
    request: EnrichRequest,
) -> Result<EnrichEstimate> {
    blocking(&state, move |workspace| {
        workspace.estimate_enrichment(project_id, &request)
    })
    .await
}

#[tauri::command]
pub(crate) fn start_enrichment<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: State<'_, AppState>,
    project_id: i64,
    request: EnrichRequest,
) -> Result<String> {
    let job = Job::new(
        JobKind::Enrich,
        project_name(&state, project_id)?,
        Some(project_id),
    );
    jobs::spawn(&app, job, move |workspace, control| {
        Ok(Finish::Run(
            workspace.enrich(project_id, &request, control)?,
        ))
    })
}

#[tauri::command]
pub(crate) async fn estimate_serps(
    state: State<'_, AppState>,
    project_id: i64,
    request: SerpRequest,
) -> Result<SerpEstimate> {
    blocking(&state, move |workspace| {
        workspace.estimate_serps(project_id, &request)
    })
    .await
}

#[tauri::command]
pub(crate) fn start_serps<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: State<'_, AppState>,
    project_id: i64,
    request: SerpRequest,
) -> Result<String> {
    if let Some(id) = jobs::running_for(&state, JobKind::Serp, Some(project_id)) {
        return Ok(id);
    }
    let job = Job::new(
        JobKind::Serp,
        project_name(&state, project_id)?,
        Some(project_id),
    );
    jobs::spawn(&app, job, move |workspace, control| {
        Ok(Finish::Run(
            workspace.fetch_serps(project_id, &request, control)?.run,
        ))
    })
}

// ------------------------------------------------------------------ Clusters

#[tauri::command]
pub(crate) fn start_cluster<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: State<'_, AppState>,
    project_id: i64,
    request: ClusterRequest,
) -> Result<String> {
    if let Some(id) = jobs::running_for(&state, JobKind::Cluster, Some(project_id)) {
        return Ok(id);
    }
    let mut job = Job::new(
        JobKind::Cluster,
        project_name(&state, project_id)?,
        Some(project_id),
    );
    request.method.as_str().clone_into(&mut job.step);
    jobs::spawn(&app, job, move |workspace, _control| {
        Ok(Finish::Done(
            workspace.cluster(project_id, &request)?.clustering.id,
        ))
    })
}

#[tauri::command]
pub(crate) async fn list_clusterings(
    state: State<'_, AppState>,
    project_id: i64,
) -> Result<Vec<ClusteringRow>> {
    blocking(&state, move |workspace| {
        Ok(workspace.store()?.clusterings(project_id)?)
    })
    .await
}

#[tauri::command]
pub(crate) async fn list_clusters(
    state: State<'_, AppState>,
    clustering_id: i64,
    query: ClusterQuery,
) -> Result<ClusterPage> {
    blocking(&state, move |workspace| {
        Ok(workspace.store()?.clusters(clustering_id, &query)?)
    })
    .await
}

#[tauri::command]
pub(crate) async fn cluster_members(
    state: State<'_, AppState>,
    clustering_id: i64,
    number: u32,
) -> Result<Vec<MemberRow>> {
    blocking(&state, move |workspace| {
        Ok(workspace.store()?.cluster_members(clustering_id, number)?)
    })
    .await
}

#[tauri::command]
pub(crate) async fn place_keywords(
    state: State<'_, AppState>,
    project_id: i64,
    keyword_ids: Vec<i64>,
    pillar_id: i64,
) -> Result<ClusteringRow> {
    blocking(&state, move |workspace| {
        workspace.place_keywords(project_id, &keyword_ids, pillar_id)
    })
    .await
}

#[tauri::command]
pub(crate) async fn unplace_keywords(
    state: State<'_, AppState>,
    project_id: i64,
    keyword_ids: Vec<i64>,
) -> Result<()> {
    blocking(&state, move |workspace| {
        workspace.unplace_keywords(project_id, &keyword_ids)
    })
    .await
}

#[tauri::command]
pub(crate) async fn remap(state: State<'_, AppState>, project_id: i64) -> Result<ClusteringRow> {
    blocking(&state, move |workspace| {
        let project = workspace.project(project_id)?;
        let engine = project.spec.engines.first().cloned().unwrap_or_default();
        workspace.refresh_serp_pages(project_id, &engine, "desktop")?;
        workspace.remap(project_id)
    })
    .await
}

// ------------------------------------------------------------------ Sources

#[tauri::command]
pub(crate) async fn list_providers(state: State<'_, AppState>) -> Result<Vec<ProviderInfo>> {
    blocking(&state, KeywordWorkspace::providers).await
}

#[tauri::command]
pub(crate) async fn set_key(
    state: State<'_, AppState>,
    provider: String,
    name: String,
    value: String,
) -> Result<()> {
    blocking(&state, move |workspace| {
        workspace.set_key(&provider, &name, &value)
    })
    .await
}

#[tauri::command]
pub(crate) async fn remove_key(
    state: State<'_, AppState>,
    provider: String,
    name: String,
) -> Result<()> {
    blocking(&state, move |workspace| {
        workspace.remove_key(&provider, &name)
    })
    .await
}

#[tauri::command]
pub(crate) async fn set_budget(
    state: State<'_, AppState>,
    provider: String,
    budget_usd: Option<f64>,
) -> Result<()> {
    blocking(&state, move |workspace| {
        workspace.set_budget(&provider, budget_usd)
    })
    .await
}

#[tauri::command]
pub(crate) async fn set_enabled(
    state: State<'_, AppState>,
    provider: String,
    enabled: bool,
) -> Result<()> {
    blocking(&state, move |workspace| {
        workspace.set_enabled(&provider, enabled)
    })
    .await
}

#[tauri::command]
pub(crate) async fn usage(state: State<'_, AppState>) -> Result<Vec<ProviderUsage>> {
    blocking(&state, KeywordWorkspace::usage).await
}

// ------------------------------------------------------------------ Google

#[tauri::command]
pub(crate) async fn google_status(state: State<'_, AppState>) -> Result<GoogleStatus> {
    blocking(&state, KeywordWorkspace::google_status).await
}

#[tauri::command]
pub(crate) async fn set_google_client(
    state: State<'_, AppState>,
    client_id: String,
    client_secret: String,
) -> Result<()> {
    blocking(&state, move |workspace| {
        workspace.set_google_client(&client_id, &client_secret)
    })
    .await
}

#[tauri::command]
pub(crate) async fn set_ads_account(
    state: State<'_, AppState>,
    customer_id: Option<String>,
    login_customer_id: Option<String>,
) -> Result<()> {
    blocking(&state, move |workspace| {
        workspace.set_ads_account(customer_id.as_deref(), login_customer_id.as_deref())
    })
    .await
}

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
pub(crate) fn start_google_login<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: State<'_, AppState>,
    scopes: GoogleScopes,
) -> Result<String> {
    if let Some(id) = jobs::running_for(&state, JobKind::GoogleLogin, None) {
        return Ok(id);
    }
    let mut job = Job::new(JobKind::GoogleLogin, "Google".to_owned(), None);
    "browser".clone_into(&mut job.step);
    jobs::spawn(&app, job, move |workspace, control| {
        workspace.google_login(scopes, &open_browser, control.cancelled)?;
        Ok(Finish::Done(0))
    })
}

#[tauri::command]
pub(crate) async fn google_logout(state: State<'_, AppState>) -> Result<()> {
    blocking(&state, KeywordWorkspace::google_logout).await
}

#[tauri::command]
pub(crate) async fn search_console_properties(
    state: State<'_, AppState>,
) -> Result<Vec<octopuson_kw_app::google::Property>> {
    blocking(&state, KeywordWorkspace::search_console_properties).await
}

#[tauri::command]
pub(crate) fn start_search_console<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: State<'_, AppState>,
    project_id: i64,
    request: SearchConsoleRequest,
) -> Result<String> {
    let mut job = Job::new(
        JobKind::SearchConsole,
        project_name(&state, project_id)?,
        Some(project_id),
    );
    "search-console".clone_into(&mut job.step);
    jobs::spawn(&app, job, move |workspace, control| {
        let outcome = workspace.import_search_console(project_id, &request, control.cancelled)?;
        Ok(Finish::Done(outcome.run_id))
    })
}

/// Where the attended browser mode stands.
#[tauri::command]
pub(crate) async fn browser_status(state: State<'_, AppState>) -> Result<BrowserStatus> {
    blocking(&state, KeywordWorkspace::browser_status).await
}

/// Closes the browser window and deletes its profile (cookies, consent choices).
#[tauri::command]
pub(crate) async fn reset_browser_profile(state: State<'_, AppState>) -> Result<()> {
    if jobs::list(&state)
        .iter()
        .any(|job| job.is_active() && job.kind == JobKind::Serp)
    {
        return Err("stop the result-page runs first".to_owned());
    }
    blocking(&state, KeywordWorkspace::reset_browser_profile).await
}

/// The state of one secret, for the settings of the free suggestion endpoints.
#[tauri::command]
pub(crate) async fn secret_state(
    state: State<'_, AppState>,
    owner: String,
    name: String,
) -> Result<SecretState> {
    blocking(&state, move |workspace| {
        let origin = octopuson_kw_app::sources::PROVIDERS
            .iter()
            .find(|provider| provider.secrets_of == owner)
            .map(|provider| provider.origin)
            .unwrap_or_default();
        workspace
            .secrets()
            .state(&owner, &name, &[origin])
            .map_err(|error| KwAppError::Secrets(error.to_string()))
    })
    .await
}

// ------------------------------------------------------------------ Exports and jobs

#[tauri::command]
pub(crate) async fn export_keywords(
    state: State<'_, AppState>,
    project_id: i64,
    query: KeywordQuery,
    path: PathBuf,
) -> Result<u64> {
    blocking(&state, move |workspace| {
        workspace.export_keywords(project_id, &query, &path)
    })
    .await
}

#[tauri::command]
pub(crate) async fn export_clusters(
    state: State<'_, AppState>,
    project_id: i64,
    clustering_id: Option<i64>,
    path: PathBuf,
) -> Result<u64> {
    blocking(&state, move |workspace| {
        workspace.export_clusters(project_id, clustering_id, &path)
    })
    .await
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

// ------------------------------------------------------------------ OctopusOn SEO account

#[tauri::command]
pub(crate) async fn cloud_status(state: State<'_, AppState>) -> Result<CloudStatus> {
    blocking(&state, KeywordWorkspace::cloud_status).await
}

/// Signs in to the OctopusOn SEO account with one click: the account site opens in the browser
/// and comes straight back for a person who is signed in there.
#[tauri::command]
pub(crate) fn start_account_login<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: State<'_, AppState>,
) -> Result<String> {
    if let Some(id) = jobs::running_for(&state, JobKind::AccountLogin, None) {
        return Ok(id);
    }
    let mut job = Job::new(JobKind::AccountLogin, "OctopusOn SEO".to_owned(), None);
    "browser".clone_into(&mut job.step);
    jobs::spawn(&app, job, move |workspace, control| {
        workspace.connect(&open_browser, control)?;
        Ok(Finish::Done(0))
    })
}

#[tauri::command]
pub(crate) async fn account_logout(state: State<'_, AppState>) -> Result<()> {
    blocking(&state, KeywordWorkspace::sign_out).await
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
    let status = blocking(&state, KeywordWorkspace::cloud_status).await?;
    if open_browser(&format!(
        "{}{path}",
        status.portal_url.trim_end_matches('/')
    )) {
        Ok(())
    } else {
        Err("cannot open the browser".to_owned())
    }
}
