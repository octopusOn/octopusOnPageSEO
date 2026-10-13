use std::path::PathBuf;

use seonexus_log_app::ImportOptions;
use seonexus_log_app::import::{ImportReport, ImportRequest, import_logs as run_import};
use seonexus_log_app::inspect::{InspectReport, InspectRequest, inspect};
use tauri::State;
use tauri::ipc::Channel;

use super::blocking;
use super::catalog::FormatInput;
use crate::error::CommandResult;
use crate::progress::{ChannelProgress, ImportEvent};
use crate::state::AppState;

/// Skipped-line examples kept per file.
const ERROR_SAMPLES: usize = 10;

#[tauri::command]
pub(crate) async fn inspect_files(
    paths: Vec<PathBuf>,
    format: FormatInput,
) -> CommandResult<InspectReport> {
    blocking(move || {
        let request = InspectRequest {
            files: paths,
            format: format.into(),
            max_error_samples: ERROR_SAMPLES,
        };
        Ok(inspect(&request)?)
    })
    .await
}

#[tauri::command]
pub(crate) async fn import_logs(
    state: State<'_, AppState>,
    import_id: String,
    site: String,
    paths: Vec<PathBuf>,
    verify_dns: bool,
    on_event: Channel<ImportEvent>,
) -> CommandResult<ImportReport> {
    let workspace = state.workspace.clone();
    let cancel = state.start_import(&import_id);
    let result = blocking(move || {
        let request = ImportRequest {
            site,
            paths,
            max_error_samples: ERROR_SAMPLES,
            options: ImportOptions::default(),
            verify_dns,
        };
        let progress = ChannelProgress::new(on_event);
        Ok(run_import(&workspace, &request, &progress, &cancel)?)
    })
    .await;
    state.finish_import(&import_id);
    result
}

#[tauri::command]
pub(crate) async fn cancel_import(
    state: State<'_, AppState>,
    import_id: String,
) -> CommandResult<bool> {
    Ok(state.cancel_import(&import_id))
}
