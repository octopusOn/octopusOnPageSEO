//! The OctopusOn SEO account: balance, one-click sign-in on the desktop, account pages.

use std::sync::Arc;

use serde::Deserialize;

use crate::cloud::AccountStatus;
use crate::jobs::JobDone;
use crate::modules::app::Opened;
use crate::router::{Context, NoArgs, Router};
use crate::{AppError, Host};

fn account_status(context: &Context<'_>, _: NoArgs) -> Result<AccountStatus, AppError> {
    context.workspace.account_status()
}

/// Starts the one-click sign-in as a job; its id.
fn start_account_login(context: &Context<'_>, _: NoArgs) -> Result<String, AppError> {
    if context.workspace.host() == Host::Web {
        return Err(AppError::Unsupported(
            "the web version is signed in with the account it was opened with".to_owned(),
        ));
    }
    let workspace = Arc::clone(context.workspace);
    let open = Arc::clone(context.open);
    context
        .jobs
        .start("account-login", "OctopusOn SEO", None, move |control| {
            control.progress("browser", 0, 0);
            let connected = workspace.connect(&|url| open(url), &|| control.cancelled())?;
            Ok(JobDone::with(connected))
        })
}

fn account_logout(context: &Context<'_>, _: NoArgs) -> Result<(), AppError> {
    if context.workspace.host() == Host::Web {
        return Err(AppError::Unsupported(
            "sign out of the web version with its sign-out button".to_owned(),
        ));
    }
    context.workspace.sign_out()
}

#[derive(Debug, Deserialize)]
struct Page {
    page: String,
}

fn account_page(context: &Context<'_>, arguments: Page) -> Result<Opened, AppError> {
    let url = context.workspace.account_page(&arguments.page);
    let opened = context.workspace.host() == Host::Desktop && (context.open)(&url);
    Ok(Opened { url, opened })
}

/// Adds the commands.
pub fn register(router: &mut Router) {
    router.add("account_status", account_status);
    router.add("start_account_login", start_account_login);
    router.add("account_logout", account_logout);
    router.add("account_page", account_page);
}
