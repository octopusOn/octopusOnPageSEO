//! Thin desktop adapters for the shared SEO Keyword Research use cases.

mod commands;
mod jobs;
#[cfg(target_os = "macos")]
mod menu;

use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};

use seonexus_kw_app::KeywordWorkspace;
use tauri::{Emitter, Manager};

use crate::jobs::RunningJob;

struct AppState {
    workspace: Arc<KeywordWorkspace>,
    jobs: Mutex<HashMap<String, RunningJob>>,
}

impl AppState {
    fn new(workspace: KeywordWorkspace) -> Self {
        Self {
            workspace: Arc::new(workspace),
            jobs: Mutex::new(HashMap::new()),
        }
    }
}

fn with_commands<R: tauri::Runtime>(
    builder: tauri::Builder<R>,
    state: AppState,
) -> tauri::Builder<R> {
    builder
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            commands::app_info,
            commands::get_settings,
            commands::save_settings,
            commands::list_projects,
            commands::create_project,
            commands::update_project,
            commands::delete_project,
            commands::list_runs,
            commands::add_keywords,
            commands::preview_import,
            commands::import_file,
            commands::list_keywords,
            commands::set_status,
            commands::tag_keywords,
            commands::keyword_facets,
            commands::keyword_serp,
            commands::estimate_discovery,
            commands::start_discovery,
            commands::estimate_enrichment,
            commands::start_enrichment,
            commands::estimate_serps,
            commands::start_serps,
            commands::start_cluster,
            commands::list_clusterings,
            commands::list_clusters,
            commands::cluster_members,
            commands::place_keywords,
            commands::unplace_keywords,
            commands::remap,
            commands::list_providers,
            commands::set_key,
            commands::remove_key,
            commands::set_budget,
            commands::set_enabled,
            commands::usage,
            commands::google_status,
            commands::set_google_client,
            commands::set_ads_account,
            commands::start_google_login,
            commands::google_logout,
            commands::cloud_status,
            commands::start_account_login,
            commands::account_logout,
            commands::set_source_mode,
            commands::open_account_page,
            commands::search_console_properties,
            commands::start_search_console,
            commands::secret_state,
            commands::browser_status,
            commands::reset_browser_profile,
            commands::export_keywords,
            commands::export_clusters,
            commands::list_jobs,
            commands::cancel_job,
            commands::clear_jobs,
        ])
}

/// Starts the desktop application.
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();
    let directory = std::env::var_os(seonexus_kw_app::DATA_DIR_VARIABLE).map(Into::into);
    let workspace = match KeywordWorkspace::resolve(directory) {
        Ok(workspace) => workspace,
        Err(error) => {
            tracing::error!(%error, "cannot open the SEO Keyword Research workspace");
            std::process::exit(1);
        }
    };
    // Runs left `running` by a program that stopped are closed as failed.
    if let Ok(store) = workspace.store() {
        let _ = store.close_stale_runs(&seonexus_kw_app::timestamp());
    }
    let application = with_commands(tauri::Builder::default(), AppState::new(workspace))
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            #[cfg(target_os = "macos")]
            menu::install(app.handle())?;
            #[cfg(not(target_os = "macos"))]
            let _ = app;
            Ok(())
        })
        .on_menu_event(|app, event| {
            if event.id() == "keywords-quit" && !protect_jobs(app) {
                close_browser(app);
                app.exit(0);
            }
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event
                && protect_jobs(window.app_handle())
            {
                api.prevent_close();
            }
        })
        .build(tauri::generate_context!());
    match application {
        Ok(application) => application.run(|app, event| match event {
            tauri::RunEvent::ExitRequested { api, .. } if protect_jobs(app) => api.prevent_exit(),
            tauri::RunEvent::Exit => close_browser(app),
            _ => {}
        }),
        Err(error) => {
            tracing::error!(%error, "application failed");
            std::process::exit(1);
        }
    }
}

/// Keeps the window open while paid runs, clusterings or imports run.
fn protect_jobs<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> bool {
    let state = app.state::<AppState>();
    let busy = state
        .jobs
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .values()
        .any(|running| running.job.is_active());
    if busy {
        let _ = app.emit("keywords-close-blocked", ());
    }
    busy
}

/// The browser window of the result pages belongs to the app; it closes with it.
fn close_browser<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
    app.state::<AppState>().workspace.close_browser();
}

#[cfg(test)]
mod ipc_tests;
