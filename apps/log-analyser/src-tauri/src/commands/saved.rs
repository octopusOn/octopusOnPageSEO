use std::path::PathBuf;

use octopuson_log_app::saved::{self, SaveQueryRequest, SavedImportReport, SavedQueryInfo};
use tauri::State;

use super::blocking;
use crate::error::CommandResult;
use crate::state::AppState;

#[tauri::command]
pub(crate) async fn list_saved_queries(
    state: State<'_, AppState>,
) -> CommandResult<Vec<SavedQueryInfo>> {
    let workspace = state.workspace.clone();
    blocking(move || Ok(saved::list_saved_queries(&workspace)?)).await
}

#[tauri::command]
pub(crate) async fn save_query(
    state: State<'_, AppState>,
    name: String,
    description: String,
    query: String,
    overwrite: bool,
) -> CommandResult<SavedQueryInfo> {
    let workspace = state.workspace.clone();
    blocking(move || {
        let request = SaveQueryRequest {
            name,
            description,
            query,
            overwrite,
        };
        Ok(saved::save_query(&workspace, &request)?)
    })
    .await
}

#[tauri::command]
pub(crate) async fn remove_saved_query(
    state: State<'_, AppState>,
    name: String,
) -> CommandResult<()> {
    let workspace = state.workspace.clone();
    blocking(move || Ok(saved::remove_saved_query(&workspace, &name)?)).await
}

#[tauri::command]
pub(crate) async fn export_saved_queries(
    state: State<'_, AppState>,
    path: PathBuf,
) -> CommandResult<usize> {
    let workspace = state.workspace.clone();
    blocking(move || Ok(saved::export_saved_queries(&workspace, &path)?)).await
}

#[tauri::command]
pub(crate) async fn import_saved_queries(
    state: State<'_, AppState>,
    path: PathBuf,
    overwrite: bool,
) -> CommandResult<SavedImportReport> {
    let workspace = state.workspace.clone();
    blocking(move || Ok(saved::import_saved_queries(&workspace, &path, overwrite)?)).await
}
