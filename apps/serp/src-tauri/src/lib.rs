//! Thin desktop adapters for the shared SERP Checker use cases.

mod commands;
mod jobs;
#[cfg(target_os = "macos")]
mod menu;
mod scheduler;

use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};

use seonexus_serp_app::SerpWorkspace;
use tauri::{Emitter, Manager};

use crate::jobs::RunningJob;

struct AppState {
    workspace: Arc<SerpWorkspace>,
    jobs: Mutex<HashMap<String, RunningJob>>,
}

impl AppState {
    fn new(workspace: SerpWorkspace) -> Self {
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
            commands::list_providers,
            commands::set_secret,
            commands::remove_secret,
            commands::cloud_status,
            commands::start_account_login,
            commands::account_logout,
            commands::set_source_mode,
            commands::open_account_page,
            commands::custom_specs,
            commands::save_spec,
            commands::remove_spec,
            commands::import_specs,
            commands::export_specs,
            commands::test_spec,
            commands::list_parsers,
            commands::save_parser_override,
            commands::remove_parser_override,
            commands::close_browser,
            commands::reset_browser_profile,
            commands::estimate_search,
            commands::start_analysis,
            commands::start_search,
            commands::start_track,
            commands::list_jobs,
            commands::cancel_job,
            commands::clear_jobs,
            commands::list_analyses,
            commands::get_analysis,
            commands::rescore_analysis,
            commands::list_snapshots,
            commands::get_snapshot,
            commands::delete_snapshot,
            commands::usage,
            commands::domain_facts,
            commands::update_tranco,
            commands::list_projects,
            commands::create_project,
            commands::update_project,
            commands::delete_project,
            commands::add_keywords,
            commands::list_keywords,
            commands::remove_keyword,
            commands::set_competitors,
            commands::estimate_track,
            commands::rank_report,
            commands::stored_pages,
            commands::test_parser,
            commands::reparse_snapshot,
            commands::export_rank_report,
            commands::export_analysis,
        ])
}

/// Starts the desktop application.
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();
    let directory = std::env::var_os("OCTOPUSSEO_SERP_DATA_DIR").map(Into::into);
    let workspace = match SerpWorkspace::resolve(directory) {
        Ok(workspace) => workspace,
        Err(error) => {
            tracing::error!(%error, "cannot open the SERP Checker workspace");
            std::process::exit(1);
        }
    };
    let application = with_commands(tauri::Builder::default(), AppState::new(workspace))
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            #[cfg(target_os = "macos")]
            menu::install(app.handle())?;
            scheduler::start(app.handle().clone());
            Ok(())
        })
        .on_menu_event(|app, event| {
            if event.id() == "serp-quit" && !protect_jobs(app) {
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

/// Keeps the window open while searches, analyses or rank checks run.
fn protect_jobs<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> bool {
    let state = app.state::<AppState>();
    let busy = state
        .jobs
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .values()
        .any(|running| running.job.is_active());
    if busy {
        let _ = app.emit("serp-close-blocked", ());
    }
    busy
}

/// The search browser window belongs to the app; it closes with it.
fn close_browser<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
    app.state::<AppState>().workspace.close_browser();
}

#[cfg(test)]
mod ipc_tests;
