//! `import` use-case: parse log files and store them in a site, skipping files and lines
//! that were imported before.
//!
//! Each file is imported in its own transaction, so a failure or cancellation keeps the files
//! completed so far and leaves no partial data from the interrupted one.

mod file;
mod progress;
mod report;

pub use progress::{ImportProgress, NoProgress};
pub use report::{FileImportReport, FileOutcome, ImportReport};

use std::io;
use std::path::PathBuf;
use std::time::Instant;

use seonexus_logparse::FormatRegistry;
use seonexus_logparse::input::InputError;
use seonexus_store::{Catalog, FileFingerprint, ImportOptions, SiteStore, StoreError};

use crate::bots::bot_context;
use crate::cancel::CancelToken;
use crate::format_choice::{FormatChoice, FormatError};
use crate::inputs::expand_inputs;
use crate::settings::{ImportFilter, SettingsError, SiteSettings, effective_host};
use crate::stats::Throughput;
use crate::workspace::Workspace;
use file::{FileImportContext, import_file};

/// Input of [`import_logs`].
#[derive(Debug, Clone)]
pub struct ImportRequest {
    /// Site name.
    pub site: String,
    /// Files or directories to import.
    pub paths: Vec<PathBuf>,
    /// Skipped-line examples kept per file.
    pub max_error_samples: usize,
    /// Storage tuning.
    pub options: ImportOptions,
    /// Verify bots by reverse/forward DNS where IP lists cannot decide (needs network).
    pub verify_dns: bool,
}

/// Errors of [`import_logs`].
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ImportError {
    /// No input files were found.
    #[error("no input files given")]
    NoFiles,
    /// A path could not be listed or fingerprinted.
    #[error("cannot read `{path}`")]
    Path {
        /// Path involved.
        path: PathBuf,
        /// Underlying error.
        #[source]
        source: io::Error,
    },
    /// The site's format could not be resolved.
    #[error(transparent)]
    Format(#[from] FormatError),
    /// A file could not be opened or decoded.
    #[error(transparent)]
    Input(#[from] InputError),
    /// Reading a stream failed midway (I/O or corrupt compressed data).
    #[error("failed while reading `{stream}`")]
    Read {
        /// Stream being read.
        stream: String,
        /// Underlying error.
        #[source]
        source: io::Error,
    },
    /// Storage failure.
    #[error(transparent)]
    Store(#[from] StoreError),
    /// The site's import settings are invalid.
    #[error(transparent)]
    Settings(#[from] SettingsError),
    /// The user cancelled; the current file was rolled back.
    #[error("import cancelled")]
    Cancelled,
}

/// Imports files into a site.
///
/// # Errors
/// See [`ImportError`]. Unparseable lines are not errors; they are counted per file.
pub fn import_logs(
    workspace: &Workspace,
    request: &ImportRequest,
    progress: &dyn ImportProgress,
    cancel: &CancelToken,
) -> Result<ImportReport, ImportError> {
    let started = Instant::now();
    let files = expand_inputs(&request.paths).map_err(|source| ImportError::Path {
        path: request.paths.first().cloned().unwrap_or_default(),
        source,
    })?;
    let first_file = files.first().ok_or(ImportError::NoFiles)?;
    let site = Catalog::open(workspace.data_dir())?.site_by_name(&request.site)?;
    let resolved = FormatChoice::from_spec(&site.format_spec)?
        .resolve(&FormatRegistry::builtin(), first_file)?;
    let settings = SiteSettings::from_json(&site.settings)?;
    let filter = ImportFilter::new(&settings)?;
    let mut store = SiteStore::open(workspace.data_dir(), site.id, workspace.settings())?;
    store.set_site_host(effective_host(&site.name, &settings).as_deref())?;
    let bots = bot_context(workspace, request.verify_dns);
    store.reclassify_user_agents(&bots.classifier, &bots.verifier)?;
    let context = FileImportContext {
        format: resolved.format.as_ref(),
        format_spec: &site.format_spec,
        max_error_samples: request.max_error_samples,
        options: request.options,
        bots: &bots,
        filter: &filter,
        progress,
        cancel,
    };

    let mut reports = Vec::with_capacity(files.len());
    for (index, path) in files.iter().enumerate() {
        progress.file_started(path, index, files.len());
        let fingerprint = FileFingerprint::of_file(path).map_err(|source| ImportError::Path {
            path: path.clone(),
            source,
        })?;
        let outcome = if store.is_imported(&fingerprint)? {
            FileOutcome::AlreadyImported
        } else {
            import_file(&mut store, path, fingerprint, &context)?
        };
        let report = FileImportReport {
            path: path.display().to_string(),
            outcome,
        };
        progress.file_finished(&report);
        reports.push(report);
    }
    // New client IPs get their country and network (when the data is installed).
    crate::geo::refresh_site_geo(workspace, &mut store)?;
    let mut report = summarize(site.name, resolved.format.id(), reports, started);
    report.dns_verification = bots.verifier.uses_dns();
    Ok(report)
}

fn summarize(
    site: String,
    format_id: &str,
    files: Vec<FileImportReport>,
    started: Instant,
) -> ImportReport {
    let (mut inserted, mut duplicates, mut lines) = (0, 0, 0);
    for file in &files {
        match &file.outcome {
            FileOutcome::Imported {
                inserted_rows,
                duplicate_rows,
                summary,
                ..
            } => {
                inserted += inserted_rows;
                duplicates += duplicate_rows;
                lines += summary.total_lines;
            }
            FileOutcome::Unrecognized { summary } => lines += summary.total_lines,
            FileOutcome::AlreadyImported => {}
        }
    }
    ImportReport {
        site,
        format_id: format_id.to_owned(),
        files,
        inserted_rows: inserted,
        duplicate_rows: duplicates,
        total_lines: lines,
        dns_verification: false,
        throughput: Throughput::new(lines, started.elapsed()),
    }
}

#[cfg(test)]
mod tests;
