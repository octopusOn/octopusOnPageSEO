use octopuson_core::Timestamp;

use crate::fingerprint::FileFingerprint;

/// Tuning of an import session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImportOptions {
    /// Staged rows that trigger a flush to Parquet. Higher values mean fewer, larger files
    /// and more memory.
    pub flush_rows: usize,
}

impl Default for ImportOptions {
    fn default() -> Self {
        Self {
            flush_rows: 4_000_000,
        }
    }
}

/// The source file of an import.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportedFile {
    /// Content fingerprint used to skip re-imports.
    pub fingerprint: FileFingerprint,
    /// Path as given by the user (for display only).
    pub path: String,
}

/// Example of a skipped line, kept for the import history.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkippedSample {
    /// Stream (file or archive entry) name.
    pub stream: String,
    /// 1-based line number.
    pub line_number: u64,
    /// Why the line was skipped.
    pub reason: String,
    /// Beginning of the line.
    pub excerpt: String,
}

/// Parse statistics recorded with a committed import.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportRecord<'a> {
    /// Serialized format choice used for parsing.
    pub format_spec: &'a str,
    /// Imported file.
    pub file: &'a ImportedFile,
    /// All lines read.
    pub total_lines: u64,
    /// Lines parsed successfully (before de-duplication).
    pub parsed_lines: u64,
    /// Earliest request time.
    pub first_request: Option<Timestamp>,
    /// Latest request time.
    pub last_request: Option<Timestamp>,
    /// Skipped-line counts by reason.
    pub skipped: &'a [(String, u64)],
    /// Skipped-line examples.
    pub samples: &'a [SkippedSample],
}

/// Result of a committed import.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImportOutcome {
    /// Identifier of the import.
    pub import_id: i64,
    /// Hits written.
    pub inserted_rows: u64,
    /// Hits dropped because earlier imports already contained them.
    pub duplicate_rows: u64,
}
