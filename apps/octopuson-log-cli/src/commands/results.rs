//! Output of result tables: terminal table, CSV/JSON on stdout or a file.

use std::io::{self, Write};
use std::path::PathBuf;

use clap::Args;
use miette::IntoDiagnostic;
use octopuson_log_app::report::{CsvWriter, JsonWriter};
use octopuson_log_app::table::{Cell, Column, TableSink};
use octopuson_log_app::table_file::TableFile;

use super::table::TextTable;

/// How a result table is shown or saved.
#[derive(Debug, Clone, Args)]
pub(crate) struct ResultArgs {
    /// Write every result row to a file; the format follows the extension (.csv, .json, .xlsx).
    #[arg(short, long, value_name = "FILE", conflicts_with_all = ["csv", "json", "rows", "offset"])]
    pub(crate) output: Option<PathBuf>,

    /// Print every result row as CSV.
    #[arg(long, conflicts_with = "json")]
    pub(crate) csv: bool,

    /// Print every result row as JSON.
    #[arg(long)]
    pub(crate) json: bool,

    /// Rows shown in the terminal table.
    #[arg(long, default_value_t = 50, value_name = "N")]
    pub(crate) rows: u64,

    /// Rows skipped before the first one shown in the terminal table.
    #[arg(long, default_value_t = 0, value_name = "N")]
    pub(crate) offset: u64,
}

impl ResultArgs {
    /// Whether rows go to the terminal as an aligned table.
    pub(crate) fn is_table(&self) -> bool {
        self.output.is_none() && !self.csv && !self.json
    }
}

/// Streams a table produced by `produce` to the destination chosen in `args`. In table mode
/// a footer tells which rows were shown.
pub(crate) fn emit<T, E>(
    args: &ResultArgs,
    produce: impl FnOnce(&mut dyn TableSink) -> Result<T, E>,
) -> miette::Result<T>
where
    E: std::error::Error + Send + Sync + 'static,
{
    if let Some(path) = &args.output {
        let mut file = TableFile::create(path, None).into_diagnostic()?;
        let value = produce(file.sink()).into_diagnostic()?;
        file.commit().into_diagnostic()?;
        tracing::info!("saved {}", path.display());
        return Ok(value);
    }
    let stdout = io::stdout();
    if args.csv {
        return produce(&mut CsvWriter::new(stdout.lock())).into_diagnostic();
    }
    if args.json {
        return produce(&mut JsonWriter::new(stdout.lock())).into_diagnostic();
    }
    let mut table = TextTable::default();
    let mut paged = Paged {
        inner: &mut table,
        skip: args.offset,
        take: args.rows,
        seen: 0,
    };
    let value = produce(&mut paged).into_diagnostic()?;
    let total = paged.seen;
    let mut out = stdout.lock();
    table
        .write(&mut out, usize::MAX)
        .and_then(|()| write_footer(&mut out, args.offset, table.len(), total))
        .into_diagnostic()?;
    Ok(value)
}

fn write_footer(out: &mut dyn Write, offset: u64, shown: usize, total: u64) -> io::Result<()> {
    writeln!(out)?;
    if shown == 0 {
        return writeln!(out, "No rows.");
    }
    let last = offset + shown as u64;
    let more = if last < total {
        " (more: --offset, --rows, --csv or -o FILE)"
    } else {
        ""
    };
    writeln!(out, "Rows {}–{last} of {total}{more}", offset + 1)
}

/// Keeps one window of rows and counts all of them.
struct Paged<'a> {
    inner: &'a mut TextTable,
    skip: u64,
    take: u64,
    seen: u64,
}

impl TableSink for Paged<'_> {
    fn begin(&mut self, columns: &[Column]) -> io::Result<()> {
        self.inner.begin(columns)
    }

    fn row(&mut self, cells: &[Cell]) -> io::Result<()> {
        let index = self.seen;
        self.seen += 1;
        if index >= self.skip && index < self.skip.saturating_add(self.take) {
            self.inner.row(cells)?;
        }
        Ok(())
    }

    fn finish(&mut self) -> io::Result<()> {
        self.inner.finish()
    }
}
