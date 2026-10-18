//! Automatic format detection from a sample of lines.

use std::sync::Arc;

use crate::delimited::DelimitedFormat;
use crate::format::LogFormat;
use crate::registry::FormatRegistry;
use crate::w3c::W3cFormat;

/// Result of [`detect`].
#[derive(Debug, Clone)]
pub struct Detection {
    /// The best matching format.
    pub format: Arc<dyn LogFormat>,
    /// Sample lines the format parsed successfully.
    pub matched: usize,
    /// Non-empty sample lines considered.
    pub sampled: usize,
}

impl Detection {
    /// Share of sampled lines that parsed, in `0.0..=1.0`.
    #[allow(
        clippy::cast_precision_loss,
        reason = "sample sizes are small; f64 is exact below 2^53"
    )]
    pub fn confidence(&self) -> f64 {
        if self.sampled == 0 {
            return 0.0;
        }
        self.matched as f64 / self.sampled as f64
    }
}

/// Formats described by the sample itself: a W3C `#Fields:` directive or a CSV/TSV header
/// row naming the time, client, URL and status columns.
pub fn header_formats<S: AsRef<str>>(sample: &[S]) -> Vec<Arc<dyn LogFormat>> {
    let mut formats: Vec<Arc<dyn LogFormat>> = Vec::new();
    if let Some(w3c) = W3cFormat::from_sample(sample) {
        formats.push(Arc::new(w3c));
    }
    if let Some(delimited) = DelimitedFormat::from_sample(sample) {
        formats.push(Arc::new(delimited));
    }
    formats
}

/// Picks the format that parses the most sample lines; ties go to the more specific format.
/// Besides the registry's formats, formats described by a header in the sample
/// ([`header_formats`]) take part; header and comment lines do not count as sampled lines.
///
/// Returns `None` if no format parses any non-empty line. Callers decide which confidence is
/// acceptable (the UI asks the user to confirm low-confidence detections).
pub fn detect<S: AsRef<str>>(registry: &FormatRegistry, sample: &[S]) -> Option<Detection> {
    let derived = header_formats(sample);
    let csv_header = DelimitedFormat::from_sample(sample).map(|f| f.header().to_owned());
    let lines: Vec<&str> = sample
        .iter()
        .map(AsRef::as_ref)
        .filter(|line| !line.trim().is_empty() && !line.starts_with('#'))
        .filter(|line| csv_header.as_deref() != Some(line.trim_end_matches(['\r', '\n'])))
        .collect();

    registry
        .iter()
        .chain(derived.iter())
        .map(|format| {
            let matched = lines.iter().filter(|l| format.parse(l).is_ok()).count();
            (format, matched)
        })
        .filter(|(_, matched)| *matched > 0)
        .max_by_key(|(format, matched)| (*matched, format.specificity()))
        .map(|(format, matched)| Detection {
            format: Arc::clone(format),
            matched,
            sampled: lines.len(),
        })
}
