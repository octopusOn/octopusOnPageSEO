//! The OctopusOn SEO account card. The Log Analyser has no paid sources, but the account is the
//! one every OctopusOn SEO program on this computer shares: connecting here connects them all.

use std::sync::Arc;
use std::time::Duration;

use octopuson_cloud::{AccountSession, AccountStatus};
use octopuson_net::ApiClient;
use tauri::State;

use crate::error::{CommandError, CommandResult};

/// The shared account of this program.
pub(crate) struct AccountState(Result<Arc<AccountSession>, String>);

impl AccountState {
    pub(crate) fn new() -> Self {
        let app = format!(
            "log-analyser/{} {}",
            env!("CARGO_PKG_VERSION"),
            std::env::consts::OS
        );
        Self(
            AccountSession::shared(app)
                .map(Arc::new)
                .map_err(|error| error.to_string()),
        )
    }

    fn session(&self) -> CommandResult<Arc<AccountSession>> {
        self.0.clone().map_err(CommandError::internal)
    }
}

fn transport() -> CommandResult<ApiClient> {
    ApiClient::new(
        concat!("octopusSEO-Log-Analyser/", env!("CARGO_PKG_VERSION")),
        Duration::from_secs(30),
        None,
    )
    .map_err(|error| CommandError::internal(error.to_string()))
}

/// Opens an address in the default browser.
fn open_browser(url: &str) -> bool {
    let command = if cfg!(target_os = "macos") {
        std::process::Command::new("open").arg(url).status()
    } else if cfg!(windows) {
        std::process::Command::new("rundll32")
            .args(["url.dll,FileProtocolHandler", url])
            .status()
    } else {
        std::process::Command::new("xdg-open").arg(url).status()
    };
    command.is_ok_and(|status| status.success())
}

async fn blocking<T: Send + 'static>(
    state: &State<'_, AccountState>,
    work: impl FnOnce(&AccountSession, &ApiClient) -> CommandResult<T> + Send + 'static,
) -> CommandResult<T> {
    let session = state.session()?;
    tauri::async_runtime::spawn_blocking(move || work(&session, &transport()?))
        .await
        .map_err(|error| CommandError::internal(error.to_string()))?
}

#[tauri::command]
pub(crate) async fn account_status(state: State<'_, AccountState>) -> CommandResult<AccountStatus> {
    blocking(&state, |session, transport| Ok(session.status(transport))).await
}

/// One-click sign-in: waits until the person comes back from the account site.
#[tauri::command]
pub(crate) async fn account_connect(state: State<'_, AccountState>) -> CommandResult<bool> {
    blocking(&state, |session, transport| {
        session
            .connect(transport, &open_browser)
            .map_err(|error| CommandError::internal(error.to_string()))
    })
    .await
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)] // Tauri hands state over by value.
pub(crate) fn account_cancel(state: State<'_, AccountState>) -> CommandResult<()> {
    state.session()?.cancel();
    Ok(())
}

#[tauri::command]
pub(crate) async fn account_logout(state: State<'_, AccountState>) -> CommandResult<()> {
    blocking(&state, |session, transport| {
        session
            .sign_out(transport)
            .map_err(|error| CommandError::internal(error.to_string()))
    })
    .await
}

/// Opens a page of the account site (buying credits, devices).
#[tauri::command]
pub(crate) async fn account_open(
    state: State<'_, AccountState>,
    page: String,
) -> CommandResult<()> {
    blocking(&state, move |session, transport| {
        if open_browser(&session.page(transport, &page)) {
            Ok(())
        } else {
            Err(CommandError::internal("cannot open the browser"))
        }
    })
    .await
}
