//! The desktop shell of the OctopusOn GEO-SEO Platform: one window, one command.
//!
//! The window calls every operation through [`platform_call`] with the command's name and
//! JSON arguments, exactly as the web version calls `POST /api/call/{command}`; job changes
//! arrive as [`JOB_EVENT`] events. While the program is open, scheduled work (rank checks)
//! starts when it is due; work missed while it was closed starts soon after it opens. Signed
//! in to the account, the program keeps its data the same as the web version's ([`start_sync`]),
//! and the web then runs the scheduled work instead.

use std::io::Write as _;
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use octopuson_platform_app::jobs::NoEvents;
use octopuson_platform_app::router::Opener;
use octopuson_platform_app::{CallError, EventSink, Job, Platform, Workspace};
use serde_json::Value;
use tauri::{Emitter, Manager, State};

/// Event carrying job changes to the window.
pub const JOB_EVENT: &str = "platform-job";

/// How often the schedule is looked at.
const SCHEDULE_TICK: Duration = Duration::from_secs(60);

/// Wait after opening before the first look, so the window comes up first.
const SCHEDULE_START: Duration = Duration::from_secs(20);

/// How often the program syncs with the web when nothing changed here.
const SYNC_EVERY: Duration = Duration::from_secs(30);

/// Wait after a change before syncing, so a burst of edits travels together.
const SYNC_SETTLE: Duration = Duration::from_secs(3);

/// Wait after opening before the first sync.
const SYNC_START: Duration = Duration::from_secs(5);

/// Longest wait between syncs while they fail.
const SYNC_BACKOFF: Duration = Duration::from_secs(300);

type Emit = Box<dyn Fn(&Job) + Send + Sync>;

/// Sends job changes to the window once it exists.
#[derive(Default)]
struct WindowSink(OnceLock<Emit>);

impl EventSink for WindowSink {
    fn job(&self, job: &Job) {
        if let Some(emit) = self.0.get() {
            emit(job);
        }
    }
}

struct AppState {
    platform: Arc<Platform>,
    sink: Arc<WindowSink>,
}

impl AppState {
    fn new(workspace: Workspace) -> Self {
        let sink = Arc::new(WindowSink::default());
        let open: Opener = Arc::new(open_browser);
        let events: Arc<dyn EventSink> = sink.clone();
        Self {
            platform: Arc::new(Platform::new(workspace, events, open)),
            sink,
        }
    }
}

/// Calls a platform command off the window's thread.
#[tauri::command]
async fn platform_call(
    state: State<'_, AppState>,
    command: String,
    args: Option<Value>,
) -> Result<Value, CallError> {
    let platform = Arc::clone(&state.platform);
    tauri::async_runtime::spawn_blocking(move || {
        platform.call(&command, args.unwrap_or(Value::Null))
    })
    .await
    .map_err(|error| CallError {
        code: octopuson_platform_app::ErrorCode::Internal,
        message: error.to_string(),
        retry_after: None,
    })?
}

/// Writes a file the window offers to save (base64 bytes) where the person chose in the save
/// dialog.
#[tauri::command]
async fn save_file(path: std::path::PathBuf, data: String) -> Result<(), String> {
    use base64::Engine as _;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(data)
        .map_err(|error| error.to_string())?;
    tauri::async_runtime::spawn_blocking(move || std::fs::write(&path, bytes))
        .await
        .map_err(|error| error.to_string())?
        .map_err(|error| error.to_string())
}

fn with_commands<R: tauri::Runtime>(
    builder: tauri::Builder<R>,
    state: AppState,
) -> tauri::Builder<R> {
    builder
        .manage(state)
        .invoke_handler(tauri::generate_handler![platform_call, save_file])
        .setup(|app| {
            let handle = app.handle().clone();
            let state = app.state::<AppState>();
            let _ = state.sink.0.set(Box::new(move |job: &Job| {
                let _ = handle.emit(JOB_EVENT, job);
            }));
            Ok(())
        })
}

/// Starts the scheduled work that is due, now and then every minute.
fn start_schedule(platform: Arc<Platform>) {
    let spawned = std::thread::Builder::new()
        .name("schedule".to_owned())
        .spawn(move || {
            std::thread::sleep(SCHEDULE_START);
            loop {
                // Syncing with the account, the web runs it: nothing is paid twice.
                if platform.workspace().schedules_here()
                    && let Err(error) = platform.run_due()
                {
                    tracing::warn!(%error, "scheduled work did not start");
                }
                std::thread::sleep(SCHEDULE_TICK);
            }
        });
    if let Err(error) = spawned {
        tracing::error!(%error, "the schedule cannot run");
    }
}

/// Syncs with the account's web workspace every half minute, a few seconds after a change made
/// here, and after a job ends; waits longer while the web cannot be reached.
fn start_sync(platform: Arc<Platform>) {
    use octopuson_platform_app::sync::{self, RoundOutcome};
    let spawned = std::thread::Builder::new()
        .name("sync".to_owned())
        .spawn(move || {
            std::thread::sleep(SYNC_START);
            let workspace = Arc::clone(platform.workspace());
            let mut seen = workspace.generation();
            let mut running = platform.jobs().running();
            let mut changed: Option<Instant> = None;
            let mut last: Option<Instant> = None;
            let mut wait = SYNC_EVERY;
            loop {
                let generation = workspace.generation();
                let now_running = platform.jobs().running();
                if generation != seen || now_running < running {
                    seen = generation;
                    changed = Some(Instant::now());
                }
                running = now_running;
                let settled = changed.is_some_and(|at| at.elapsed() >= SYNC_SETTLE);
                let due = last.is_none_or(|at| at.elapsed() >= wait);
                if settled || due {
                    changed = None;
                    last = Some(Instant::now());
                    match sync::round(&workspace) {
                        Ok(RoundOutcome::Done(report)) => {
                            wait = SYNC_EVERY;
                            if report.received > 0 {
                                platform.announce_sync();
                            }
                        }
                        Ok(_) => wait = SYNC_EVERY,
                        Err(error) => {
                            sync::note_failure(&workspace, &error);
                            tracing::warn!(%error, "sync failed");
                            wait = (wait * 2).min(SYNC_BACKOFF);
                        }
                    }
                }
                std::thread::sleep(Duration::from_secs(1));
            }
        });
    if let Err(error) = spawned {
        tracing::error!(%error, "the sync cannot run");
    }
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

/// Serves the MCP tools on standard input and output instead of opening the window (the
/// program started as `<program> mcp`); the exit code. Logs would corrupt the answers on
/// standard output, so none are written, and the schedule stays with the window.
pub fn mcp() -> i32 {
    let directory = std::env::var_os(octopuson_platform_app::DATA_DIR_VARIABLE).map(Into::into);
    let workspace = match Workspace::resolve(directory) {
        Ok(workspace) => workspace,
        Err(error) => {
            let _ = writeln!(
                std::io::stderr(),
                "cannot open the GEO-SEO Platform workspace: {error}"
            );
            return 1;
        }
    };
    let open: Opener = Arc::new(open_browser);
    let platform = Arc::new(Platform::new(workspace, Arc::new(NoEvents), open));
    match octopuson_platform_app::mcp::stdio(&platform) {
        Ok(()) => 0,
        Err(error) => {
            let _ = writeln!(std::io::stderr(), "MCP stopped: {error}");
            1
        }
    }
}

/// Starts the desktop application.
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();
    let directory = std::env::var_os(octopuson_platform_app::DATA_DIR_VARIABLE).map(Into::into);
    let workspace = match Workspace::resolve(directory) {
        Ok(workspace) => workspace,
        Err(error) => {
            tracing::error!(%error, "cannot open the GEO-SEO Platform workspace");
            std::process::exit(1);
        }
    };
    let state = AppState::new(workspace);
    start_schedule(Arc::clone(&state.platform));
    start_sync(Arc::clone(&state.platform));
    let application = with_commands(
        tauri::Builder::default().plugin(tauri_plugin_dialog::init()),
        state,
    )
    .build(tauri::generate_context!());
    match application {
        Ok(application) => application.run(|app, event| {
            if let tauri::RunEvent::Exit = event {
                let platform = &app.state::<AppState>().platform;
                platform.jobs().cancel_all();
                // Changes made just now keep their time, not the next start's.
                octopuson_platform_app::sync::note_changes(platform.workspace());
            }
        }),
        Err(error) => {
            tracing::error!(%error, "application failed");
            std::process::exit(1);
        }
    }
}

#[cfg(test)]
mod ipc_tests;
