//! Rank checks that are due run while the app is open; checks missed while it was closed run
//! shortly after it opens. `octopuson-serp track --due` does the same from cron.

use std::time::Duration;

use tauri::Manager;

use crate::AppState;
use crate::jobs::{self, JobStatus};

/// Time for the window to come up before the first check.
const FIRST_CHECK: Duration = Duration::from_secs(20);
/// How often due projects are looked for.
const INTERVAL: Duration = Duration::from_secs(10 * 60);
/// How often a running check is looked at.
const POLL: Duration = Duration::from_secs(5);

/// Starts looking for due projects in the background.
pub(crate) fn start<R: tauri::Runtime>(app: tauri::AppHandle<R>) {
    let spawned = std::thread::Builder::new()
        .name("serp-scheduler".to_owned())
        .spawn(move || {
            std::thread::sleep(FIRST_CHECK);
            loop {
                run_due(&app);
                std::thread::sleep(INTERVAL);
            }
        });
    if let Err(error) = spawned {
        tracing::warn!(%error, "cannot start the rank check scheduler");
    }
}

/// Checks every due project, one after the other, so the browser mode is never asked twice
/// at once.
fn run_due<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
    let state = app.state::<AppState>();
    let due = match state.workspace.due_projects() {
        Ok(due) => due,
        Err(error) => {
            tracing::warn!(%error, "cannot look for due rank checks");
            return;
        }
    };
    for project_id in due {
        let Ok(project) = state.workspace.project(project_id) else {
            continue;
        };
        if project.keywords == 0 {
            continue;
        }
        let id = match jobs::start_track(app, project.id, project.name) {
            Ok(id) => id,
            Err(error) => {
                tracing::warn!(%error, project_id, "cannot start a scheduled rank check");
                continue;
            }
        };
        while jobs::status(&state, &id)
            .is_some_and(|status| matches!(status, JobStatus::Running | JobStatus::Challenge))
        {
            std::thread::sleep(POLL);
        }
    }
}
