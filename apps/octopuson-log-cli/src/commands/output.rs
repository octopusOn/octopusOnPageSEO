use std::io::{self, Write};

use miette::IntoDiagnostic;
use serde::Serialize;

/// Writes JSON to stdout. A closed pipe (`| head`) is not an error.
pub(super) fn write_json<T: Serialize>(value: &T) -> miette::Result<()> {
    let mut stdout = io::stdout().lock();
    let result = serde_json::to_writer_pretty(&mut stdout, value)
        .map_err(io::Error::from)
        .and_then(|()| writeln!(stdout));
    ignore_broken_pipe(result)
}

/// Runs `render` against a locked stdout. A closed pipe is not an error.
pub(super) fn write_text(
    render: impl FnOnce(&mut dyn Write) -> io::Result<()>,
) -> miette::Result<()> {
    let mut stdout = io::stdout().lock();
    ignore_broken_pipe(render(&mut stdout).and_then(|()| stdout.flush()))
}

/// Treats a closed stdout pipe as success; other errors are reported.
pub(super) fn ignore_broken_pipe(result: io::Result<()>) -> miette::Result<()> {
    match result {
        Err(error) if error.kind() == io::ErrorKind::BrokenPipe => Ok(()),
        other => other.into_diagnostic(),
    }
}
