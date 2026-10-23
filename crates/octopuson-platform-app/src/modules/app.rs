//! The program itself: version, where it runs, settings.

use serde::{Deserialize, Serialize};

use crate::router::{Context, NoArgs, Router};
use crate::settings::Settings;
use crate::{APPLICATION, AppError, Host};

/// The program as the About box shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppInfo {
    /// Program name.
    pub name: String,
    /// Version.
    pub version: String,
    /// Where it runs.
    pub host: Host,
    /// The data folder, on the desktop.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_dir: Option<String>,
    /// On the desktop, the program AI assistants start with the argument `mcp` to reach this
    /// workspace: this very executable (the desktop program or `octopuson-platform`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mcp_command: Option<String>,
}

fn app_info(context: &Context<'_>, _: NoArgs) -> Result<AppInfo, AppError> {
    let workspace = context.workspace;
    Ok(AppInfo {
        name: APPLICATION.to_owned(),
        version: env!("PLATFORM_VERSION").to_owned(),
        host: workspace.host(),
        data_dir: (workspace.host() == Host::Desktop)
            .then(|| workspace.data_dir().display().to_string()),
        mcp_command: (workspace.host() == Host::Desktop)
            .then(|| std::env::current_exe().ok())
            .flatten()
            .map(|path| path.display().to_string()),
    })
}

fn get_settings(context: &Context<'_>, _: NoArgs) -> Result<Settings, AppError> {
    context.workspace.settings()
}

#[derive(Debug, Deserialize)]
struct SaveSettings {
    settings: Settings,
}

fn save_settings(context: &Context<'_>, arguments: SaveSettings) -> Result<Settings, AppError> {
    context.workspace.save_settings(arguments.settings)
}

#[derive(Debug, Deserialize)]
struct OpenUrl {
    url: String,
}

/// An address and whether it was opened here (on the web the page opens it).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Opened {
    /// The address.
    pub url: String,
    /// The person's browser was opened with it.
    pub opened: bool,
}

/// Opens an https address in the person's browser on the desktop.
fn open_url(context: &Context<'_>, arguments: OpenUrl) -> Result<Opened, AppError> {
    let url = arguments.url.trim().to_owned();
    if !url.starts_with("https://") || url.chars().any(char::is_whitespace) {
        return Err(AppError::Invalid(
            "only https addresses are opened".to_owned(),
        ));
    }
    let opened = context.workspace.host() == Host::Desktop && (context.open)(&url);
    Ok(Opened { url, opened })
}

/// Adds the commands.
pub fn register(router: &mut Router) {
    router.add("open_url", open_url);
    router.add("app_info", app_info);
    router.add("get_settings", get_settings);
    router.add("save_settings", save_settings);
}
