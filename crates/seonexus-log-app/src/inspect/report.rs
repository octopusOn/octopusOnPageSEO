use serde::Serialize;

use crate::stats::{ParseSummary, Throughput};

/// Result of the `inspect` use-case. Serializable for `--json` output and the desktop UI.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct InspectReport {
    /// Format id used for parsing.
    pub format_id: String,
    /// Human-readable format name.
    pub format_name: String,
    /// Share of sample lines matched during auto-detection; `None` if chosen explicitly.
    pub detection_confidence: Option<f64>,
    /// What was parsed.
    #[serde(flatten)]
    pub summary: ParseSummary,
    /// How long it took.
    #[serde(flatten)]
    pub throughput: Throughput,
}
