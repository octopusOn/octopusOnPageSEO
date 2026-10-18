use octopuson_log_app::catalog::{self, AddSiteRequest, ClientInfo, SiteInfo, SiteOverview};
use octopuson_log_app::settings::{self, SiteSettings};
use octopuson_log_app::{FormatChoice, FormatInfo, builtin_formats};
use serde::Deserialize;
use tauri::State;

use super::blocking;
use crate::error::CommandResult;
use crate::state::AppState;

/// Log format as chosen in the UI.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub(crate) enum FormatInput {
    Auto,
    Builtin { id: String },
    Nginx { format: String },
    Apache { format: String },
}

impl From<FormatInput> for FormatChoice {
    fn from(input: FormatInput) -> Self {
        match input {
            FormatInput::Auto => Self::Auto,
            FormatInput::Builtin { id } => Self::Builtin(id),
            FormatInput::Nginx { format } => Self::Nginx(format),
            FormatInput::Apache { format } => Self::Apache(format),
        }
    }
}

#[tauri::command]
pub(crate) async fn site_settings(
    state: State<'_, AppState>,
    site: String,
) -> CommandResult<SiteSettings> {
    let workspace = state.workspace.clone();
    blocking(move || Ok(settings::site_settings(&workspace, &site)?)).await
}

#[tauri::command]
pub(crate) async fn set_site_settings(
    state: State<'_, AppState>,
    site: String,
    settings: SiteSettings,
) -> CommandResult<SiteSettings> {
    let workspace = state.workspace.clone();
    blocking(move || Ok(settings::set_site_settings(&workspace, &site, settings)?)).await
}

#[tauri::command]
pub(crate) async fn delete_import(
    state: State<'_, AppState>,
    site: String,
    import_id: i64,
) -> CommandResult<()> {
    let workspace = state.workspace.clone();
    blocking(move || Ok(catalog::delete_import(&workspace, &site, import_id)?)).await
}

#[tauri::command]
pub(crate) async fn list_clients(state: State<'_, AppState>) -> CommandResult<Vec<ClientInfo>> {
    let workspace = state.workspace.clone();
    blocking(move || Ok(catalog::list_clients(&workspace)?)).await
}

#[tauri::command]
pub(crate) async fn add_client(
    state: State<'_, AppState>,
    name: String,
) -> CommandResult<ClientInfo> {
    let workspace = state.workspace.clone();
    blocking(move || Ok(catalog::add_client(&workspace, &name)?)).await
}

#[tauri::command]
pub(crate) async fn remove_client(state: State<'_, AppState>, name: String) -> CommandResult<()> {
    let workspace = state.workspace.clone();
    blocking(move || Ok(catalog::remove_client(&workspace, &name)?)).await
}

#[tauri::command]
pub(crate) async fn list_sites(state: State<'_, AppState>) -> CommandResult<Vec<SiteInfo>> {
    let workspace = state.workspace.clone();
    blocking(move || Ok(catalog::list_sites(&workspace)?)).await
}

#[tauri::command]
pub(crate) async fn add_site(
    state: State<'_, AppState>,
    client: String,
    name: String,
    format: FormatInput,
) -> CommandResult<SiteInfo> {
    let workspace = state.workspace.clone();
    blocking(move || {
        let request = AddSiteRequest {
            client,
            name,
            format: format.into(),
        };
        Ok(catalog::add_site(&workspace, &request)?)
    })
    .await
}

#[tauri::command]
pub(crate) async fn set_site_format(
    state: State<'_, AppState>,
    name: String,
    format: FormatInput,
) -> CommandResult<SiteInfo> {
    let workspace = state.workspace.clone();
    blocking(move || Ok(catalog::set_site_format(&workspace, &name, &format.into())?)).await
}

#[tauri::command]
pub(crate) async fn remove_site(state: State<'_, AppState>, name: String) -> CommandResult<()> {
    let workspace = state.workspace.clone();
    blocking(move || Ok(catalog::remove_site(&workspace, &name)?)).await
}

#[tauri::command]
pub(crate) async fn site_overview(
    state: State<'_, AppState>,
    name: String,
) -> CommandResult<SiteOverview> {
    let workspace = state.workspace.clone();
    blocking(move || Ok(catalog::site_overview(&workspace, &name)?)).await
}

#[tauri::command]
pub(crate) async fn list_formats() -> CommandResult<Vec<FormatInfo>> {
    Ok(builtin_formats())
}
