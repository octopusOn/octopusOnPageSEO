//! Import progress forwarded to the frontend over a Tauri channel.

use std::cell::Cell;
use std::path::Path;
use std::time::{Duration, Instant};

use octopuson_log_app::import::{FileImportReport, ImportProgress};
use serde::Serialize;
use tauri::ipc::Channel;

/// Minimum time between progress events of one file.
const INTERVAL: Duration = Duration::from_millis(200);

/// An import progress event.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "event", rename_all = "camelCase")]
pub(crate) enum ImportEvent {
    /// A file is about to be processed.
    #[serde(rename_all = "camelCase")]
    FileStarted {
        path: String,
        index: usize,
        total: usize,
    },
    /// Lines and uncompressed bytes processed so far in the current file.
    Progress { lines: u64, bytes: u64 },
    /// A file was imported or skipped.
    FileFinished { report: FileImportReport },
}

/// Sends throttled progress events; delivery failures (closed window) are ignored.
pub(crate) struct ChannelProgress {
    channel: Channel<ImportEvent>,
    lines: Cell<u64>,
    bytes: Cell<u64>,
    last_sent: Cell<Instant>,
}

impl ChannelProgress {
    pub(crate) fn new(channel: Channel<ImportEvent>) -> Self {
        Self {
            channel,
            lines: Cell::new(0),
            bytes: Cell::new(0),
            last_sent: Cell::new(Instant::now()),
        }
    }

    fn send(&self, event: ImportEvent) {
        if let Err(error) = self.channel.send(event) {
            tracing::debug!(%error, "progress event not delivered");
        }
    }
}

impl ImportProgress for ChannelProgress {
    fn file_started(&self, path: &Path, index: usize, total: usize) {
        self.lines.set(0);
        self.bytes.set(0);
        self.send(ImportEvent::FileStarted {
            path: path.display().to_string(),
            index,
            total,
        });
    }

    fn advanced(&self, lines: u64, bytes: u64) {
        self.lines.set(self.lines.get() + lines);
        self.bytes.set(self.bytes.get() + bytes);
        if self.last_sent.get().elapsed() >= INTERVAL {
            self.last_sent.set(Instant::now());
            self.send(ImportEvent::Progress {
                lines: self.lines.get(),
                bytes: self.bytes.get(),
            });
        }
    }

    fn file_finished(&self, report: &FileImportReport) {
        self.send(ImportEvent::FileFinished {
            report: report.clone(),
        });
    }
}
