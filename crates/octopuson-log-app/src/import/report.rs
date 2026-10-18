use serde::Serialize;

use crate::stats::{ParseSummary, Throughput};

/// Result of the `import` use-case.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ImportReport {
    /// Site name.
    pub site: String,
    /// Format id used for parsing.
    pub format_id: String,
    /// Per-file results in input order.
    pub files: Vec<FileImportReport>,
    /// Hits written across all files.
    pub inserted_rows: u64,
    /// Hits dropped because they were already stored.
    pub duplicate_rows: u64,
    /// Lines read across all imported files.
    pub total_lines: u64,
    /// Whether bots were verified by DNS in addition to IP lists.
    pub dns_verification: bool,
    /// How long it took.
    #[serde(flatten)]
    pub throughput: Throughput,
}

impl ImportReport {
    /// Files skipped because they were imported before.
    pub fn skipped_files(&self) -> usize {
        self.count(|outcome| matches!(outcome, FileOutcome::AlreadyImported))
    }

    /// Files in which no line matched the site's format.
    pub fn unrecognized_files(&self) -> usize {
        self.count(|outcome| matches!(outcome, FileOutcome::Unrecognized { .. }))
    }

    /// Files whose hits were stored.
    pub fn imported_files(&self) -> usize {
        self.count(|outcome| matches!(outcome, FileOutcome::Imported { .. }))
    }

    fn count(&self, predicate: impl Fn(&FileOutcome) -> bool) -> usize {
        self.files
            .iter()
            .filter(|file| predicate(&file.outcome))
            .count()
    }
}

/// Result for one input file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FileImportReport {
    /// Path as given.
    pub path: String,
    /// What happened.
    #[serde(flatten)]
    pub outcome: FileOutcome,
}

/// What happened to one input file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum FileOutcome {
    /// Same content was imported before; nothing was read.
    AlreadyImported,
    /// No line matched the site's format; nothing was stored and the file is not remembered.
    Unrecognized {
        /// Parse details (skipped-line reasons and examples).
        #[serde(flatten)]
        summary: ParseSummary,
    },
    /// The file was parsed and stored.
    Imported {
        /// Identifier of the import in the site history.
        import_id: i64,
        /// Hits written.
        inserted_rows: u64,
        /// Hits dropped as duplicates of earlier imports.
        duplicate_rows: u64,
        /// Parse details.
        #[serde(flatten)]
        summary: ParseSummary,
    },
}
