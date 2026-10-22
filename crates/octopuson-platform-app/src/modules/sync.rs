//! Syncing this desktop with the account's web workspace: where it stands, switching it on and
//! off, syncing now and the choice a first sync may wait for.

use serde::Deserialize;

use crate::router::{Context, NoArgs, Router};
use crate::sync::{self, RoundOutcome, SyncStatus};
use crate::{AppError, Host};

fn sync_status(context: &Context<'_>, _: NoArgs) -> Result<SyncStatus, AppError> {
    sync::status(context.workspace)
}

fn desktop_only(context: &Context<'_>) -> Result<(), AppError> {
    if context.workspace.host() == Host::Web {
        return Err(AppError::Unsupported(
            "the web version is the account's workspace: desktops sync with it".to_owned(),
        ));
    }
    Ok(())
}

#[derive(Debug, Deserialize)]
struct Enabled {
    enabled: bool,
}

fn set_sync(context: &Context<'_>, arguments: Enabled) -> Result<SyncStatus, AppError> {
    desktop_only(context)?;
    sync::set_enabled(context.workspace, arguments.enabled)?;
    sync::status(context.workspace)
}

/// Syncs now and answers where syncing stands afterwards.
fn sync_now(context: &Context<'_>, _: NoArgs) -> Result<SyncStatus, AppError> {
    desktop_only(context)?;
    match sync::round(context.workspace) {
        Ok(RoundOutcome::Done(report)) if report.received > 0 => context.jobs.announce("sync"),
        Ok(_) => {}
        Err(error) => {
            sync::note_failure(context.workspace, &error);
            return Err(error);
        }
    }
    sync::status(context.workspace)
}

#[derive(Debug, Deserialize)]
struct Chosen {
    choice: String,
}

/// Answers the choice a sync waits for; the next sync acts on it.
fn sync_choose(context: &Context<'_>, arguments: Chosen) -> Result<SyncStatus, AppError> {
    desktop_only(context)?;
    sync::choose(context.workspace, &arguments.choice)?;
    if arguments.choice == "download" {
        context.jobs.announce("sync");
    }
    sync::status(context.workspace)
}

/// Adds the commands.
pub fn register(router: &mut Router) {
    router.add("sync_status", sync_status);
    router.add("set_sync", set_sync);
    router.add("sync_now", sync_now);
    router.add("sync_choose", sync_choose);
}
