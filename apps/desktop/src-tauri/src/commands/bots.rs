use seonexus_log_app::bots::{
    self, BotInfo, CustomBot, IpCheck, RangeSourceStatus, RangeUpdateReport, VerifySiteReport,
    VerifySiteRequest,
};
use tauri::State;

use super::blocking;
use crate::error::CommandResult;
use crate::state::AppState;

#[tauri::command]
pub(crate) async fn list_bots(
    state: State<'_, AppState>,
    category: Option<String>,
) -> CommandResult<Vec<BotInfo>> {
    let workspace = state.workspace.clone();
    blocking(move || Ok(bots::list_bots(&workspace, category.as_deref()))).await
}

#[tauri::command]
pub(crate) async fn list_custom_bots(state: State<'_, AppState>) -> CommandResult<Vec<CustomBot>> {
    let workspace = state.workspace.clone();
    blocking(move || Ok(bots::list_custom_bots(&workspace)?)).await
}

/// Replaces the whole list of custom bots.
#[tauri::command]
pub(crate) async fn save_custom_bots(
    state: State<'_, AppState>,
    bots: Vec<CustomBot>,
) -> CommandResult<Vec<CustomBot>> {
    let workspace = state.workspace.clone();
    blocking(move || Ok(bots::save_custom_bots(&workspace, bots)?)).await
}

/// Writes the custom bots to a JSON file chosen by the user.
#[tauri::command]
pub(crate) async fn export_custom_bots(
    state: State<'_, AppState>,
    path: String,
) -> CommandResult<usize> {
    let workspace = state.workspace.clone();
    blocking(move || {
        let list = bots::list_custom_bots(&workspace)?;
        let json = serde_json::to_string_pretty(&serde_json::json!({ "version": 1, "bots": list }))
            .map_err(|e| crate::error::CommandError::internal(e.to_string()))?;
        std::fs::write(&path, json)
            .map_err(|e| crate::error::CommandError::internal(format!("{path}: {e}")))?;
        Ok(list.len())
    })
    .await
}

/// Merges the bots of a JSON file into the custom bots (same ids are replaced).
#[tauri::command]
pub(crate) async fn import_custom_bots(
    state: State<'_, AppState>,
    path: String,
) -> CommandResult<Vec<CustomBot>> {
    let workspace = state.workspace.clone();
    blocking(move || {
        let text = std::fs::read_to_string(&path)
            .map_err(|e| crate::error::CommandError::internal(format!("{path}: {e}")))?;
        let imported = bots::parse_custom_bots(&text)?;
        let mut list = bots::list_custom_bots(&workspace)?;
        list.retain(|existing| !imported.iter().any(|bot| bot.id == existing.id));
        list.extend(imported);
        Ok(bots::save_custom_bots(&workspace, list)?)
    })
    .await
}

#[tauri::command]
pub(crate) async fn bot_range_status(
    state: State<'_, AppState>,
) -> CommandResult<Vec<RangeSourceStatus>> {
    let workspace = state.workspace.clone();
    blocking(move || Ok(bots::bot_range_status(&workspace))).await
}

#[tauri::command]
pub(crate) async fn update_bot_ranges(
    state: State<'_, AppState>,
) -> CommandResult<RangeUpdateReport> {
    let workspace = state.workspace.clone();
    blocking(move || Ok(bots::update_bot_ranges(&workspace)?)).await
}

#[tauri::command]
pub(crate) async fn verify_site_bots(
    state: State<'_, AppState>,
    site: String,
    dns: bool,
    all: bool,
) -> CommandResult<VerifySiteReport> {
    let workspace = state.workspace.clone();
    blocking(move || {
        let request = VerifySiteRequest { site, dns, all };
        Ok(bots::verify_site_bots(&workspace, &request)?)
    })
    .await
}

#[tauri::command]
pub(crate) async fn verify_ip(
    state: State<'_, AppState>,
    ip: String,
    bot: String,
    dns: bool,
) -> CommandResult<IpCheck> {
    let workspace = state.workspace.clone();
    blocking(move || Ok(bots::verify_ip(&workspace, &ip, &bot, dns)?)).await
}
