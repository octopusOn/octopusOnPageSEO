use seonexus_log_app::geo::{self, GeoStatus, GeoUpdateReport};
use tauri::State;

use super::blocking;
use crate::error::CommandResult;
use crate::state::AppState;

#[tauri::command]
pub(crate) async fn geo_status(state: State<'_, AppState>) -> CommandResult<GeoStatus> {
    let workspace = state.workspace.clone();
    blocking(move || Ok(geo::geo_status(&workspace))).await
}

#[tauri::command]
pub(crate) async fn update_geo_data(state: State<'_, AppState>) -> CommandResult<GeoUpdateReport> {
    let workspace = state.workspace.clone();
    blocking(move || Ok(geo::update_geo_data(&workspace)?)).await
}
