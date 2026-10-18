use octopuson_core::Timestamp;
use octopuson_logparse::{LogRecord, ParseErrorKind};

/// Maximum characters of a skipped line kept as an example.
const EXCERPT_CHARS: usize = 200;

/// A skipped line captured on a worker thread; the stream name is added when merging.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ChunkSample {
    pub(crate) line_number: u64,
    pub(crate) kind: ParseErrorKind,
    pub(crate) excerpt: String,
}

/// Counters for one chunk, computed independently on a worker thread.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ChunkStats {
    pub(crate) lines: u64,
    pub(crate) parsed: u64,
    /// Parsed lines not stored because of the site's import settings (part of `parsed`).
    pub(crate) filtered: u64,
    pub(crate) skipped: Vec<(ParseErrorKind, u64)>,
    pub(crate) samples: Vec<ChunkSample>,
    pub(crate) first: Option<Timestamp>,
    pub(crate) last: Option<Timestamp>,
    pub(crate) status_classes: [u64; 10],
    max_samples: usize,
}

impl ChunkStats {
    pub(crate) fn new(max_samples: usize) -> Self {
        Self {
            lines: 0,
            parsed: 0,
            filtered: 0,
            skipped: Vec::new(),
            samples: Vec::new(),
            first: None,
            last: None,
            status_classes: [0; 10],
            max_samples,
        }
    }

    pub(crate) fn record_parsed(&mut self, record: &LogRecord<'_>) {
        self.lines += 1;
        self.parsed += 1;
        let ts = record.timestamp;
        self.first = Some(self.first.map_or(ts, |first| first.min(ts)));
        self.last = Some(self.last.map_or(ts, |last| last.max(ts)));
        self.status_classes[usize::from(record.status / 100).min(9)] += 1;
    }

    /// A parsed line the import filter dropped; it does not count towards the time range or
    /// status classes of stored requests.
    pub(crate) fn record_filtered(&mut self) {
        self.lines += 1;
        self.parsed += 1;
        self.filtered += 1;
    }

    pub(crate) fn record_skipped(&mut self, line_number: u64, kind: ParseErrorKind, text: &str) {
        self.lines += 1;
        match self.skipped.iter_mut().find(|(known, _)| *known == kind) {
            Some((_, count)) => *count += 1,
            None => self.skipped.push((kind, 1)),
        }
        let routine = matches!(kind, ParseErrorKind::EmptyLine | ParseErrorKind::Comment);
        if !routine && self.samples.len() < self.max_samples {
            self.samples.push(ChunkSample {
                line_number,
                kind,
                excerpt: text.chars().take(EXCERPT_CHARS).collect(),
            });
        }
    }
}
