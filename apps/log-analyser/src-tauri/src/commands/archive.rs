use std::path::PathBuf;

use octopuson_log_app::archive::{self, ArchiveReport};
use octopuson_log_app::catalog::SiteInfo;
use tauri::State;

use super::blocking;
use crate::error::CommandResult;
use crate::state::AppState;

#[tauri::command]
pub(crate) async fn export_site(
    state: State<'_, AppState>,
    site: String,
    path: String,
) -> CommandResult<ArchiveReport> {
    let workspace = state.workspace.clone();
    blocking(move || {
        Ok(archive::export_site(
            &workspace,
            &site,
            &PathBuf::from(path),
        )?)
    })
    .await
}

#[tauri::command]
pub(crate) async fn import_site(
    state: State<'_, AppState>,
    path: String,
    name: Option<String>,
    client: Option<String>,
) -> CommandResult<SiteInfo> {
    let workspace = state.workspace.clone();
    blocking(move || {
        Ok(archive::import_site(
            &workspace,
            &PathBuf::from(path),
            name.as_deref(),
            client.as_deref(),
        )?)
    })
    .await
}
