use std::collections::BTreeMap;
use std::time::Duration;

use serde::Serialize;

/// Parse results shared by the `inspect` and `import` reports.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ParseSummary {
    /// Per-stream line counts (ZIP archives contribute one entry per file).
    pub streams: Vec<StreamSummary>,
    /// All lines read, including skipped ones.
    pub total_lines: u64,
    /// Lines parsed successfully.
    pub parsed_lines: u64,
    /// Parsed lines not stored because of the site's import settings (part of
    /// `parsed_lines`).
    pub filtered_lines: u64,
    /// Skipped-line counts by reason.
    pub skipped_by_reason: BTreeMap<String, u64>,
    /// First examples of skipped lines.
    pub error_samples: Vec<ErrorSample>,
    /// Earliest request time (RFC 3339, UTC).
    pub first_request: Option<String>,
    /// Latest request time (RFC 3339, UTC).
    pub last_request: Option<String>,
    /// Hits per status class (`2xx`, `3xx`, …).
    pub status_classes: BTreeMap<String, u64>,
}

impl ParseSummary {
    /// Lines that could not be parsed.
    pub fn skipped_lines(&self) -> u64 {
        self.total_lines - self.parsed_lines
    }
}

/// Line counts of one stream.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StreamSummary {
    /// File path, or `archive.zip!/entry` for ZIP entries.
    pub name: String,
    /// Lines read.
    pub lines: u64,
    /// Lines parsed.
    pub parsed: u64,
}

/// One skipped line, shortened for display.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ErrorSample {
    /// Stream containing the line.
    pub stream: String,
    /// 1-based line number in the stream.
    pub line_number: u64,
    /// Why the line was skipped.
    pub reason: String,
    /// Beginning of the line.
    pub excerpt: String,
}

/// Wall-clock cost of an operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Throughput {
    /// Processing time in milliseconds.
    pub elapsed_ms: u64,
    /// Lines processed per second.
    pub lines_per_second: u64,
}

impl Throughput {
    pub(crate) fn new(lines: u64, elapsed: Duration) -> Self {
        let micros = elapsed.as_micros().max(1);
        Self {
            elapsed_ms: u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX),
            lines_per_second: u64::try_from(u128::from(lines) * 1_000_000 / micros)
                .unwrap_or(u64::MAX),
        }
    }
}
