//! Tauri shell of the OctopusOn Log Analyser.
//!
//! Every command is a thin adapter: it deserializes the request, runs a `octopuson-log-app`
//! use-case on a blocking worker thread and serializes the result. No analysis logic lives
//! here, so the desktop app and the `octopuson-log` CLI always behave identically (ADR 0002/0004).

mod commands;
mod error;
mod progress;
mod state;

#[cfg(test)]
mod ipc_tests;

use tracing_subscriber::EnvFilter;

/// Registers the application state and every IPC command.
fn with_commands<R: tauri::Runtime>(
    builder: tauri::Builder<R>,
    state: state::AppState,
) -> tauri::Builder<R> {
    builder
        .manage(state)
        .manage(commands::account::AccountState::new())
        .invoke_handler(tauri::generate_handler![
            commands::app::app_info,
            commands::account::account_status,
            commands::account::account_connect,
            commands::account::account_cancel,
            commands::account::account_logout,
            commands::account::account_open,
            commands::catalog::list_clients,
            commands::catalog::add_client,
            commands::catalog::remove_client,
            commands::catalog::list_sites,
            commands::catalog::add_site,
            commands::catalog::set_site_format,
            commands::catalog::remove_site,
            commands::catalog::site_overview,
            commands::catalog::list_formats,
            commands::archive::export_site,
            commands::archive::import_site,
            commands::catalog::site_settings,
            commands::catalog::set_site_settings,
            commands::catalog::delete_import,
            commands::import::inspect_files,
            commands::import::import_logs,
            commands::import::cancel_import,
            commands::query::list_templates,
            commands::query::check_query,
            commands::query::parse_query,
            commands::query::query_schema,
            commands::query::suggest_values,
            commands::query::run_query,
            commands::query::export_query,
            commands::query::save_table,
            commands::saved::list_saved_queries,
            commands::saved::save_query,
            commands::saved::remove_saved_query,
            commands::saved::export_saved_queries,
            commands::saved::import_saved_queries,
            commands::geo::geo_status,
            commands::geo::update_geo_data,
            commands::bots::list_bots,
            commands::bots::list_custom_bots,
            commands::bots::save_custom_bots,
            commands::bots::export_custom_bots,
            commands::bots::import_custom_bots,
            commands::bots::bot_range_status,
            commands::bots::update_bot_ranges,
            commands::bots::verify_site_bots,
            commands::bots::verify_ip,
            commands::analysis::crawl_overview,
            commands::analysis::compare_urls,
            commands::analysis::robots_compliance,
            commands::analysis::compare_periods,
        ])
}

/// Starts the application.
pub fn run() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(filter).init();

    let state = match state::AppState::new() {
        Ok(state) => state,
        Err(error) => {
            tracing::error!(%error, "cannot determine the data directory");
            std::process::exit(1);
        }
    };
    let result = with_commands(tauri::Builder::default(), state)
        .plugin(tauri_plugin_dialog::init())
        .run(tauri::generate_context!());
    if let Err(error) = result {
        tracing::error!(%error, "the application failed");
        std::process::exit(1);
    }
}
