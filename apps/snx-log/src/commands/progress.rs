use std::cell::Cell;
use std::io::{self, IsTerminal, Write};
use std::path::Path;
use std::time::{Duration, Instant};

use seonexus_log_app::import::{FileImportReport, FileOutcome, ImportProgress};

const REFRESH: Duration = Duration::from_millis(250);

/// Prints import progress to stderr: a live line on terminals, one line per file otherwise.
pub(super) struct StderrProgress {
    live: bool,
    lines: Cell<u64>,
    bytes: Cell<u64>,
    started: Cell<Instant>,
    last_draw: Cell<Instant>,
}

impl StderrProgress {
    pub(super) fn new() -> Self {
        let now = Instant::now();
        Self {
            live: io::stderr().is_terminal(),
            lines: Cell::new(0),
            bytes: Cell::new(0),
            started: Cell::new(now),
            last_draw: Cell::new(now),
        }
    }

    fn draw(&self) {
        let seconds = self.started.get().elapsed().as_secs_f64().max(0.001);
        let lines = self.lines.get();
        #[allow(
            clippy::cast_precision_loss,
            reason = "display-only rates tolerate rounding"
        )]
        let (rate, megabytes) = (lines as f64 / seconds, self.bytes.get() as f64 / 1e6);
        write_stderr(&format!(
            "\r  {lines} lines, {megabytes:.0} MB, {rate:.0} lines/s   "
        ));
    }
}

impl ImportProgress for StderrProgress {
    fn file_started(&self, path: &Path, index: usize, total: usize) {
        self.lines.set(0);
        self.bytes.set(0);
        self.started.set(Instant::now());
        write_stderr(&format!("[{}/{total}] {}\n", index + 1, path.display()));
    }

    fn advanced(&self, lines: u64, bytes: u64) {
        self.lines.set(self.lines.get() + lines);
        self.bytes.set(self.bytes.get() + bytes);
        if self.live && self.last_draw.get().elapsed() >= REFRESH {
            self.last_draw.set(Instant::now());
            self.draw();
        }
    }

    fn file_finished(&self, report: &FileImportReport) {
        if self.live && self.lines.get() > 0 {
            self.draw();
            write_stderr("\n");
        }
        let message = match &report.outcome {
            FileOutcome::AlreadyImported => "  skipped: already imported\n".to_owned(),
            FileOutcome::Unrecognized { summary } => format!(
                "  skipped: none of {} lines matched the site's format\n",
                summary.total_lines
            ),
            FileOutcome::Imported {
                inserted_rows,
                duplicate_rows,
                summary,
                ..
            } => format!(
                "  {inserted_rows} hits stored, {duplicate_rows} duplicates, {} lines skipped\n",
                summary.skipped_lines()
            ),
        };
        write_stderr(&message);
    }
}

/// Progress output is best effort; a closed stderr must not abort an import.
fn write_stderr(text: &str) {
    let mut stderr = io::stderr().lock();
    let _ = stderr
        .write_all(text.as_bytes())
        .and_then(|()| stderr.flush());
}
