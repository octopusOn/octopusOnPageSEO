use serde::Serialize;
use tauri::State;

use crate::error::CommandResult;
use crate::state::AppState;

/// Facts shown in the settings screen.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AppInfo {
    version: &'static str,
    data_dir: String,
    memory_limit_mb: u32,
}

#[tauri::command]
pub(crate) async fn app_info(state: State<'_, AppState>) -> CommandResult<AppInfo> {
    Ok(AppInfo {
        version: env!("CARGO_PKG_VERSION"),
        data_dir: state.workspace.root().display().to_string(),
        memory_limit_mb: state.workspace.memory_limit_mb(),
    })
}
