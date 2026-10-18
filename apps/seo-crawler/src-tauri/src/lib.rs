//! Thin desktop adapters for the shared Spider use cases.

mod account;
mod commands;
mod jobs;
#[cfg(target_os = "macos")]
mod menu;
mod opportunities;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use octopuson_spider_app::{
    CrawlActivity, CrawlControl, CrawlMeta, CrawlPhase, CrawlProgress, SpiderWorkspace,
};
use serde::Serialize;
use tauri::{Emitter, Manager};

#[derive(Debug, Clone, Serialize)]
struct Job {
    id: String,
    url: String,
    rendering: octopuson_spider_app::RenderMode,
    status: String,
    progress: CrawlProgress,
    crawl_id: Option<String>,
    phase: CrawlPhase,
    activity: CrawlActivity,
    revision: u64,
    meta: Option<CrawlMeta>,
    error: Option<String>,
}

struct RunningJob {
    job: Job,
    control: Arc<CrawlControl>,
    order: u64,
}
struct AppState {
    workspace: SpiderWorkspace,
    jobs: Mutex<HashMap<String, RunningJob>>,
    opportunities: octopuson_spider_app::opportunities::OpportunityWorkspace,
    opportunity_jobs: octopuson_spider_app::opportunities::OpportunityJobs,
}

fn with_commands<R: tauri::Runtime>(
    builder: tauri::Builder<R>,
    state: AppState,
) -> tauri::Builder<R> {
    builder
        .manage(state)
        .manage(account::AccountState::new())
        .invoke_handler(tauri::generate_handler![
            account::account_status,
            account::account_connect,
            account::account_cancel,
            account::account_logout,
            account::account_open,
            opportunities::opportunity_status,
            opportunities::opportunity_project_save,
            opportunities::opportunity_project_remove,
            opportunities::opportunity_cache_clear,
            opportunities::opportunity_import_preview,
            opportunities::opportunity_import,
            opportunities::opportunity_google_start,
            opportunities::opportunity_google_client_import,
            opportunities::opportunity_google_cancel,
            opportunities::opportunity_google_disconnect,
            opportunities::opportunity_google_catalog,
            opportunities::opportunity_sync,
            opportunities::opportunity_jobs,
            opportunities::opportunity_cancel,
            opportunities::opportunity_analyze,
            opportunities::opportunity_result,
            opportunities::opportunity_page,
            opportunities::opportunity_detail,
            opportunities::opportunity_export,
            opportunities::opportunity_crawl_evidence,
            commands::schema_test,
            commands::resource_usage,
            commands::list_crawls,
            commands::crawl_storage,
            commands::list_jobs,
            commands::start_crawl,
            commands::control_crawl,
            commands::crawl_overview,
            commands::crawl_page,
            commands::score_remediation,
            commands::url_detail,
            commands::url_redirects,
            commands::url_content_match,
            commands::url_duplicate_groups,
            commands::export_crawl,
            commands::comparison_summary,
            commands::comparison_page,
            commands::comparison_detail,
            commands::export_comparison,
            commands::finding_comparison_summary,
            commands::finding_comparison_page,
            commands::export_finding_comparison,
            commands::sitemap_plan,
            commands::export_sitemap,
            commands::delete_crawl,
        ])
}

/// Starts the desktop application.
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();
    let directory = std::env::var_os("OCTOPUSSEO_SPIDER_DATA_DIR").map(Into::into);
    let workspace = match SpiderWorkspace::resolve(directory) {
        Ok(workspace) => workspace,
        Err(error) => {
            tracing::error!(%error,"cannot open crawl workspace");
            std::process::exit(1);
        }
    };
    let opportunities =
        octopuson_spider_app::opportunities::OpportunityWorkspace::for_spider(&workspace);
    let state = AppState {
        workspace,
        jobs: Mutex::new(HashMap::new()),
        opportunities,
        opportunity_jobs: octopuson_spider_app::opportunities::OpportunityJobs::default(),
    };
    let application = with_commands(tauri::Builder::default(), state)
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            #[cfg(target_os = "macos")]
            menu::install(app.handle())?;
            let _ = app;
            Ok(())
        })
        .on_menu_event(|app, event| {
            if event.id() == "spider-quit" && !protect_crawls(app) {
                app.exit(0);
            }
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event
                && protect_crawls(window.app_handle())
            {
                api.prevent_close();
            }
        })
        .build(tauri::generate_context!());
    match application {
        Ok(application) => application.run(|app, event| {
            if let tauri::RunEvent::ExitRequested { api, .. } = event
                && protect_crawls(app)
            {
                api.prevent_exit();
            }
        }),
        Err(error) => {
            tracing::error!(%error,"application failed");
            std::process::exit(1);
        }
    }
}

fn protect_crawls<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> bool {
    let state = app.state::<AppState>();
    let busy = state
        .jobs
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .values()
        .any(|job| job.job.is_active());
    let busy = busy || state.opportunity_jobs.busy();
    if busy {
        let _ = app.emit("spider-close-blocked", ());
    }
    busy
}

#[cfg(test)]
mod ipc_tests;
