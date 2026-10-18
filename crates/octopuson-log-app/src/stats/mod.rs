//! Parse statistics: per-chunk counters computed in parallel, merged in stream order.

mod chunk;
mod summary;

pub(crate) use chunk::ChunkStats;
pub use summary::{ErrorSample, ParseSummary, StreamSummary, Throughput};

use std::collections::BTreeMap;

use octopuson_core::Timestamp;
use octopuson_logparse::ParseErrorKind;

/// Accumulates [`ChunkStats`] across streams.
#[derive(Debug, Clone)]
pub(crate) struct ParseStats {
    streams: Vec<StreamSummary>,
    skipped: BTreeMap<ParseErrorKind, u64>,
    samples: Vec<ErrorSample>,
    max_samples: usize,
    first: Option<Timestamp>,
    last: Option<Timestamp>,
    status_classes: [u64; 10],
    filtered: u64,
}

impl ParseStats {
    pub(crate) fn new(max_samples: usize) -> Self {
        Self {
            streams: Vec::new(),
            skipped: BTreeMap::new(),
            samples: Vec::new(),
            max_samples,
            first: None,
            last: None,
            status_classes: [0; 10],
            filtered: 0,
        }
    }

    pub(crate) fn max_samples(&self) -> usize {
        self.max_samples
    }

    pub(crate) fn begin_stream(&mut self, name: &str) {
        self.streams.push(StreamSummary {
            name: name.to_owned(),
            lines: 0,
            parsed: 0,
        });
    }

    /// Merges the next chunk of the current stream.
    pub(crate) fn merge(&mut self, chunk: ChunkStats) {
        let Some(stream) = self.streams.last_mut() else {
            return;
        };
        stream.lines += chunk.lines;
        stream.parsed += chunk.parsed;
        self.filtered += chunk.filtered;
        for (kind, count) in chunk.skipped {
            *self.skipped.entry(kind).or_default() += count;
        }
        let room = self.max_samples.saturating_sub(self.samples.len());
        self.samples.extend(
            chunk
                .samples
                .into_iter()
                .take(room)
                .map(|sample| ErrorSample {
                    stream: stream.name.clone(),
                    line_number: sample.line_number,
                    reason: sample.kind.to_string(),
                    excerpt: sample.excerpt,
                }),
        );
        self.first = earliest(self.first, chunk.first);
        self.last = latest(self.last, chunk.last);
        for (total, add) in self.status_classes.iter_mut().zip(chunk.status_classes) {
            *total += add;
        }
    }

    pub(crate) fn total_lines(&self) -> u64 {
        self.streams.iter().map(|s| s.lines).sum()
    }

    pub(crate) fn parsed_lines(&self) -> u64 {
        self.streams.iter().map(|s| s.parsed).sum()
    }

    pub(crate) fn first(&self) -> Option<Timestamp> {
        self.first
    }

    pub(crate) fn last(&self) -> Option<Timestamp> {
        self.last
    }

    pub(crate) fn skipped_by_reason(&self) -> Vec<(String, u64)> {
        self.skipped
            .iter()
            .map(|(kind, count)| (kind.to_string(), *count))
            .collect()
    }

    pub(crate) fn samples(&self) -> &[ErrorSample] {
        &self.samples
    }

    pub(crate) fn into_summary(self) -> ParseSummary {
        ParseSummary {
            total_lines: self.total_lines(),
            parsed_lines: self.parsed_lines(),
            filtered_lines: self.filtered,
            skipped_by_reason: self.skipped_by_reason().into_iter().collect(),
            error_samples: self.samples,
            first_request: self.first.map(|t| t.to_string()),
            last_request: self.last.map(|t| t.to_string()),
            status_classes: (1..=9)
                .filter(|&class| self.status_classes[class] > 0)
                .map(|class| (format!("{class}xx"), self.status_classes[class]))
                .collect(),
            streams: self.streams,
        }
    }
}

fn earliest(a: Option<Timestamp>, b: Option<Timestamp>) -> Option<Timestamp> {
    match (a, b) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (a, b) => a.or(b),
    }
}

fn latest(a: Option<Timestamp>, b: Option<Timestamp>) -> Option<Timestamp> {
    match (a, b) {
        (Some(a), Some(b)) => Some(a.max(b)),
        (a, b) => a.or(b),
    }
}
