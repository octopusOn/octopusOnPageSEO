//! Thin Tauri desktop adapter for the OctopusOn Index Checker (`octopuson-finder`).

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use octopuson_finder::{
    ApiConfig, CloudStatus, CreditSource, FinderError, FinderEvent, FinderPageRequest,
    FinderTablePage, FinderWorkspace, RedirectCandidate, ScanComparison, ScanConfig, ScanControl,
    ScanMeta, ScanOverview, ScanPhase, ScanProgress, SourceMode,
};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

static NEXT_JOB_ID: AtomicU64 = AtomicU64::new(1);

/// Live or recently finished scan job exposed to the React UI.
#[derive(Debug, Clone, Serialize)]
pub struct FinderJob {
    /// Job identifier (`job-1`, ...).
    pub id: String,
    /// Target domain.
    pub domain: String,
    /// Status (`running`, `paused`, `completed`, `stopped`, `failed`).
    pub status: String,
    /// Current phase.
    pub phase: ScanPhase,
    /// Live counters.
    pub progress: ScanProgress,
    /// Reserved scan ID on disk.
    pub scan_id: Option<String>,
    /// Final scan metadata once finished.
    pub meta: Option<ScanMeta>,
    /// Error message if failed.
    pub error: Option<String>,
}

struct RunningJob {
    job: FinderJob,
    control: Arc<ScanControl>,
}

/// A one-click sign-in to the OctopusOn SEO account, finished in the browser.
#[derive(Debug, Clone, Serialize)]
pub struct AccountLogin {
    /// `waiting`, `approved`, `cancelled` or `failed`.
    pub status: String,
    /// Why the sign-in failed.
    pub error: Option<String>,
}

struct RunningLogin {
    login: AccountLogin,
    cancel: Arc<AtomicBool>,
}

struct AppState {
    workspace: FinderWorkspace,
    jobs: Mutex<HashMap<String, RunningJob>>,
    login: Mutex<Option<RunningLogin>>,
}

/// Runs account and gateway work off the main thread.
async fn blocking<T: Send + 'static>(
    state: &AppState,
    task: impl FnOnce(&FinderWorkspace) -> Result<T, FinderError> + Send + 'static,
) -> Result<T, String> {
    let workspace = state.workspace.clone();
    tauri::async_runtime::spawn_blocking(move || task(&workspace))
        .await
        .map_err(|error| error.to_string())?
        .map_err(|error| error.to_string())
}

/// Opens `url` in the default browser.
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
#[allow(clippy::needless_pass_by_value)]
fn list_scans(state: State<'_, AppState>) -> Result<Vec<ScanMeta>, String> {
    state
        .workspace
        .store()
        .list_scans()
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn list_jobs(state: State<'_, AppState>) -> Vec<FinderJob> {
    let guard = state
        .jobs
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let mut list: Vec<FinderJob> = guard.values().map(|r| r.job.clone()).collect();
    list.sort_by(|a, b| b.id.cmp(&a.id));
    list
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn start_scan(
    app: AppHandle,
    state: State<'_, AppState>,
    config: ScanConfig,
) -> Result<String, String> {
    let Some(domain) = config.normalized_host() else {
        return Err("Geçerli bir alan adı girin (ör. example.com)".to_owned());
    };
    let seq = NEXT_JOB_ID.fetch_add(1, Ordering::SeqCst);
    let job_id = format!("job-{seq}");
    let control = Arc::new(ScanControl::new());
    let initial = FinderJob {
        id: job_id.clone(),
        domain,
        status: "running".to_owned(),
        phase: ScanPhase::Starting,
        progress: ScanProgress::default(),
        scan_id: None,
        meta: None,
        error: None,
    };

    {
        let mut guard = state
            .jobs
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        guard.insert(
            job_id.clone(),
            RunningJob {
                job: initial.clone(),
                control: Arc::clone(&control),
            },
        );
    }
    let _ = app.emit("finder-job", &initial);

    let workspace = state.workspace.clone();
    let job_id_bg = job_id.clone();
    std::thread::spawn(move || {
        let result = workspace.run_scan(config, &control, |event| {
            let state_ref = app.state::<AppState>();
            let mut guard = state_ref
                .jobs
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if let Some(entry) = guard.get_mut(&job_id_bg) {
                match event {
                    FinderEvent::Phase(phase) => entry.job.phase = phase,
                    FinderEvent::Started { scan_id } => entry.job.scan_id = Some(scan_id),
                    FinderEvent::Progress(progress) => entry.job.progress = progress,
                }
                let snapshot = entry.job.clone();
                drop(guard);
                let _ = app.emit("finder-job", &snapshot);
            }
        });

        let state_ref = app.state::<AppState>();
        let mut guard = state_ref
            .jobs
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(entry) = guard.get_mut(&job_id_bg) {
            match result {
                Ok(meta) => {
                    entry.job.status.clone_from(&meta.status);
                    entry.job.phase = if meta.status == "stopped" {
                        ScanPhase::Stopped
                    } else {
                        ScanPhase::Completed
                    };
                    entry.job.meta = Some(meta);
                }
                Err(err) => {
                    "failed".clone_into(&mut entry.job.status);
                    entry.job.phase = ScanPhase::Failed;
                    entry.job.error = Some(err.to_string());
                }
            }
            let snapshot = entry.job.clone();
            drop(guard);
            let _ = app.emit("finder-job", &snapshot);
        }
    });

    Ok(job_id)
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn control_scan(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    action: String,
) -> Result<(), String> {
    let mut guard = state
        .jobs
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let Some(entry) = guard.get_mut(&id) else {
        return Err(format!("Job `{id}` not found"));
    };
    match action.as_str() {
        "pause" => {
            entry.control.pause();
            "paused".clone_into(&mut entry.job.status);
        }
        "resume" => {
            entry.control.resume();
            "running".clone_into(&mut entry.job.status);
        }
        "stop" => {
            entry.control.stop();
        }
        other => return Err(format!("Unknown action `{other}`")),
    }
    let snapshot = entry.job.clone();
    drop(guard);
    let _ = app.emit("finder-job", &snapshot);
    Ok(())
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn scan_overview(state: State<'_, AppState>, id: String) -> Result<ScanOverview, String> {
    state
        .workspace
        .store()
        .overview(&id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn scan_page(
    state: State<'_, AppState>,
    id: String,
    request: FinderPageRequest,
) -> Result<FinderTablePage, String> {
    state
        .workspace
        .store()
        .query_page(&id, &request)
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn export_scan(
    state: State<'_, AppState>,
    id: String,
    request: FinderPageRequest,
    path: String,
) -> Result<u64, String> {
    state
        .workspace
        .store()
        .export_table(&id, &request, &PathBuf::from(path), None)
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn redirect_rules(
    state: State<'_, AppState>,
    id: String,
    server: String,
) -> Result<String, String> {
    state
        .workspace
        .store()
        .generate_redirect_rules(&id, &server)
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn delete_scan(state: State<'_, AppState>, id: String) -> Result<(), String> {
    state
        .workspace
        .store()
        .delete_scan(&id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn reverify_scan(
    state: State<'_, AppState>,
    id: String,
    url: Option<String>,
) -> Result<ScanOverview, String> {
    state
        .workspace
        .reverify_scan(&id, url.as_deref())
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn compare_scans(
    state: State<'_, AppState>,
    base_id: String,
    target_id: String,
) -> Result<ScanComparison, String> {
    state
        .workspace
        .compare_scans(&base_id, &target_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn redirect_candidates(
    state: State<'_, AppState>,
    id: String,
    url: String,
) -> Result<Vec<RedirectCandidate>, String> {
    state
        .workspace
        .store()
        .redirect_candidates(&id, &url)
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn update_redirect_target(
    state: State<'_, AppState>,
    id: String,
    url: String,
    target: String,
) -> Result<(), String> {
    state
        .workspace
        .store()
        .update_suggested_target(&id, &url, &target)
        .map_err(|e| e.to_string())
}

// ------------------------------------------------------------------ OctopusOn SEO account

#[tauri::command]
async fn cloud_status(state: State<'_, AppState>) -> Result<CloudStatus, String> {
    blocking(&state, FinderWorkspace::cloud_status).await
}

#[tauri::command]
async fn credit_sources(
    state: State<'_, AppState>,
    api: ApiConfig,
) -> Result<Vec<CreditSource>, String> {
    blocking(&state, move |workspace| workspace.credit_sources(&api)).await
}

#[tauri::command]
async fn set_source_mode(
    state: State<'_, AppState>,
    source: String,
    mode: SourceMode,
) -> Result<(), String> {
    blocking(&state, move |workspace| {
        workspace.set_source_mode(&source, mode)
    })
    .await
}

/// Starts a one-click sign-in: the account site opens in the browser and, for a person who
/// is signed in there, comes straight back. `account_login` and the `finder-account` event
/// report how it ended.
#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn start_account_login(app: AppHandle, state: State<'_, AppState>) -> AccountLogin {
    let mut guard = state
        .login
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some(running) = guard
        .as_ref()
        .filter(|running| running.login.status == "waiting")
    {
        return running.login.clone();
    }
    let login = AccountLogin {
        status: "waiting".to_owned(),
        error: None,
    };
    let cancel = Arc::new(AtomicBool::new(false));
    *guard = Some(RunningLogin {
        login: login.clone(),
        cancel: Arc::clone(&cancel),
    });
    drop(guard);
    let workspace = state.workspace.clone();
    std::thread::spawn(move || {
        let result = workspace.connect(&open_browser, &|| cancel.load(Ordering::SeqCst));
        let state = app.state::<AppState>();
        let mut guard = state
            .login
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(running) = guard.as_mut() {
            match result {
                Ok(true) => "approved".clone_into(&mut running.login.status),
                Ok(false) => "cancelled".clone_into(&mut running.login.status),
                Err(error) => {
                    "failed".clone_into(&mut running.login.status);
                    running.login.error = Some(error.to_string());
                }
            }
            let snapshot = running.login.clone();
            drop(guard);
            let _ = app.emit("finder-account", &snapshot);
        }
    });
    login
}

/// The last sign-in, if any.
#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn account_login(state: State<'_, AppState>) -> Option<AccountLogin> {
    state
        .login
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .as_ref()
        .map(|running| running.login.clone())
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn cancel_account_login(state: State<'_, AppState>) {
    if let Some(running) = state
        .login
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .as_ref()
    {
        running.cancel.store(true, Ordering::SeqCst);
    }
}

#[tauri::command]
async fn account_logout(state: State<'_, AppState>) -> Result<(), String> {
    blocking(&state, FinderWorkspace::sign_out).await
}

/// Opens a page of the account site (buying credits, devices, usage) in the browser.
#[tauri::command]
async fn open_account_page(state: State<'_, AppState>, page: String) -> Result<(), String> {
    let path = match page.as_str() {
        "credits" => "/kredi",
        "devices" => "/jetonlar",
        "usage" => "/kullanim",
        _ => "/",
    };
    let status = blocking(&state, FinderWorkspace::cloud_status).await?;
    if open_browser(&format!(
        "{}{path}",
        status.portal_url.trim_end_matches('/')
    )) {
        Ok(())
    } else {
        Err("cannot open the browser".to_owned())
    }
}

/// Starts the Tauri desktop application.
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();
    let directory = std::env::var_os("OCTOPUSSEO_FINDER_DATA_DIR").map(Into::into);
    let workspace = match FinderWorkspace::resolve(directory) {
        Ok(ws) => ws,
        Err(error) => {
            tracing::error!(%error, "cannot open Index Checker workspace");
            std::process::exit(1);
        }
    };
    let state = AppState {
        workspace,
        jobs: Mutex::new(HashMap::new()),
        login: Mutex::new(None),
    };
    let application = tauri::Builder::default()
        .manage(state)
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            list_scans,
            list_jobs,
            start_scan,
            control_scan,
            scan_overview,
            scan_page,
            export_scan,
            redirect_rules,
            delete_scan,
            reverify_scan,
            compare_scans,
            redirect_candidates,
            update_redirect_target,
            cloud_status,
            credit_sources,
            set_source_mode,
            start_account_login,
            account_login,
            cancel_account_login,
            account_logout,
            open_account_page,
        ])
        .build(tauri::generate_context!());

    match application {
        Ok(app) => app.run(|_, _| {}),
        Err(error) => {
            tracing::error!(%error, "application failed");
            std::process::exit(1);
        }
    }
}
