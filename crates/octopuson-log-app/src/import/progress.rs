use std::path::Path;

use super::FileImportReport;

/// Receives import progress. All methods are called from the importing thread and default
/// to doing nothing, so front-ends implement only what they display.
pub trait ImportProgress {
    /// A file is about to be processed (`index` is 0-based).
    fn file_started(&self, _path: &Path, _index: usize, _total: usize) {}

    /// More lines of the current file were processed (`bytes` are uncompressed).
    fn advanced(&self, _lines: u64, _bytes: u64) {}

    /// A file was imported or skipped.
    fn file_finished(&self, _report: &FileImportReport) {}
}

/// Progress sink that ignores all events.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoProgress;

impl ImportProgress for NoProgress {}
