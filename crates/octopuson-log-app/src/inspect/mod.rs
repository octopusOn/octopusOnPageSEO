//! `inspect` use-case: parse files without storing them and report what was found.
//!
//! Lets users verify that a format is detected correctly and see skipped-line reasons before
//! importing gigabytes of logs.

mod report;

pub use report::InspectReport;

use std::io;
use std::ops::ControlFlow;
use std::path::PathBuf;
use std::time::Instant;

use octopuson_logparse::FormatRegistry;
use octopuson_logparse::input::{InputError, for_each_stream};

use crate::format_choice::{FormatChoice, FormatError};
use crate::inputs::expand_inputs;
use crate::pipeline::{self, parse_chunk};
use crate::stats::{ParseStats, Throughput};

/// Input of [`inspect`].
#[derive(Debug, Clone)]
pub struct InspectRequest {
    /// Files or directories to read (plain or compressed).
    pub files: Vec<PathBuf>,
    /// How to choose the log format.
    pub format: FormatChoice,
    /// Maximum number of skipped-line examples to keep.
    pub max_error_samples: usize,
}

/// Errors of [`inspect`].
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum InspectError {
    /// No input files were given.
    #[error("no input files given")]
    NoFiles,
    /// A directory could not be listed.
    #[error("cannot list `{path}`")]
    Path {
        /// Directory involved.
        path: PathBuf,
        /// Underlying error.
        #[source]
        source: io::Error,
    },
    /// The format could not be resolved.
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
}

/// Parses every line of the requested files and summarises the result.
///
/// # Errors
/// See [`InspectError`]. Unparseable lines are not errors; they are counted in the report.
pub fn inspect(request: &InspectRequest) -> Result<InspectReport, InspectError> {
    let files = expand_inputs(&request.files).map_err(|source| InspectError::Path {
        path: request.files.first().cloned().unwrap_or_default(),
        source,
    })?;
    let first_file = files.first().ok_or(InspectError::NoFiles)?;
    let resolved = request
        .format
        .resolve(&FormatRegistry::builtin(), first_file)?;
    let format = resolved.format.as_ref();
    let started = Instant::now();
    let mut stats = ParseStats::new(request.max_error_samples);
    let max_samples = stats.max_samples();

    for file in &files {
        for_each_stream::<InspectError, _>(file, |info, reader| {
            stats.begin_stream(&info.name);
            pipeline::run(
                reader,
                |chunk| parse_chunk(format, chunk, max_samples, |_, _| true),
                |chunk_stats| {
                    stats.merge(chunk_stats);
                    Ok(())
                },
                |source| InspectError::Read {
                    stream: info.name.clone(),
                    source,
                },
            )?;
            Ok(ControlFlow::Continue(()))
        })?;
    }
    let summary = stats.into_summary();
    Ok(InspectReport {
        format_id: format.id().to_owned(),
        format_name: format.display_name().to_owned(),
        detection_confidence: resolved.confidence,
        throughput: Throughput::new(summary.total_lines, started.elapsed()),
        summary,
    })
}

#[cfg(test)]
mod tests;
