//! Scheduled work of every account (rank checks, site audits): every minute, the accounts whose
//! work is due get it started, and when their work is due next is noted again. Accounts with
//! rank checks waiting in the provider's queue are due every minute too, to read the answers.
//!
//! Each account's next due time sits in the server's database, so a tick reads one table
//! instead of opening every account's workspace. Commands that may move it (a new schedule,
//! new keywords, a run started by hand) note it again right after they answer.

use std::sync::Arc;
use std::time::Duration;

use octopuson_platform_app::Platform;
use octopuson_platform_app::modules::changes_schedule;
use octopuson_platform_app::modules::rank::COLLECT_EVERY;

use crate::{AppState, unix_now};

/// How often the schedule is looked at.
pub const TICK: Duration = Duration::from_secs(60);

/// Accounts started in one tick; the others wait for the next.
const PER_TICK: usize = 8;

/// Seconds before an account whose work could not start is tried again.
const RETRY: i64 = 300;

/// When an account's scheduled work or its queued checks are due next. After a tick at `now`,
/// work that could not start waits [`RETRY`] seconds and the queue is read again a minute
/// later at the soonest; otherwise both are taken as they are.
fn next_of(platform: &Platform, now: Option<i64>) -> Result<Option<i64>, String> {
    let due = platform
        .next_due()
        .map_err(|error| error.to_string())?
        .map(|at| now.map_or(at, |now| at.max(now + RETRY)));
    let collect = platform
        .next_collect()
        .map_err(|error| error.to_string())?
        .map(|at| now.map_or(at, |now| at.max(now + COLLECT_EVERY)));
    Ok(due.into_iter().chain(collect).min())
}

/// Notes when an account's scheduled work is due next.
pub(crate) fn note(state: &AppState, user_id: &str, platform: &Platform) {
    let noted = next_of(platform, None).and_then(|next| {
        state
            .store
            .set_next_due(user_id, next)
            .map_err(|error| error.to_string())
    });
    if let Err(error) = noted {
        tracing::warn!(%error, "the next scheduled run was not noted");
    }
}

/// Notes the schedule after a command that may have moved it.
pub(crate) fn after_call(state: &AppState, user_id: &str, platform: &Platform, command: &str) {
    if changes_schedule(command) {
        note(state, user_id, platform);
    }
}

/// Starts the work of the accounts due at `now`; returns how many accounts were looked at.
pub fn tick(state: &AppState, now: i64) -> usize {
    let due = match state.store.due_accounts(now, PER_TICK) {
        Ok(due) => due,
        Err(error) => {
            tracing::warn!(%error, "the schedule cannot be read");
            return 0;
        }
    };
    for user_id in &due {
        let account = match state.accounts.get(user_id) {
            Ok(account) => account,
            Err(error) => {
                tracing::warn!(%error, "a scheduled account cannot be opened");
                let _ = state.store.set_next_due(user_id, None);
                continue;
            }
        };
        if let Err(error) = account.platform.run_due() {
            tracing::warn!(%error, "scheduled work did not start");
        }
        // Queued answers are read beside the tick, so a long read holds up no other account;
        // a read still going makes the next one return at once.
        let platform = Arc::clone(&account.platform);
        let reading = std::thread::Builder::new()
            .name("rank-queue".to_owned())
            .spawn(move || {
                if let Err(error) = platform.collect_queued() {
                    tracing::warn!(%error, "queued rank checks were not read");
                }
            });
        if let Err(error) = reading {
            tracing::warn!(%error, "queued rank checks cannot be read");
        }
        // Work that is still due (it could not start) is tried again a little later.
        let next = next_of(&account.platform, Some(now)).ok().flatten();
        if let Err(error) = state.store.set_next_due(user_id, next) {
            tracing::warn!(%error, "the next scheduled run was not noted");
        }
    }
    due.len()
}

/// Runs [`tick`] every minute until the server stops.
pub async fn run(state: Arc<AppState>) {
    let mut interval = tokio::time::interval(TICK);
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        interval.tick().await;
        let worker = Arc::clone(&state);
        let _ = tokio::task::spawn_blocking(move || tick(&worker, unix_now())).await;
    }
}
