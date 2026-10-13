use std::io::{self, Write};
use std::path::PathBuf;

use clap::Args;
use miette::IntoDiagnostic;
use seonexus_log_app::import::{
    FileOutcome, ImportProgress, ImportReport, ImportRequest, NoProgress, import_logs,
};
use seonexus_log_app::{CancelToken, ImportOptions, Workspace};

use super::output::{write_json, write_text};
use super::progress::StderrProgress;
use super::summary::render_summary;

#[derive(Debug, Args)]
pub(crate) struct ImportArgs {
    /// Site to import into.
    site: String,

    /// Log files or directories (plain, .gz, .zst, .bz2 or .zip).
    #[arg(required = true, value_name = "PATH")]
    paths: Vec<PathBuf>,

    /// Number of skipped-line examples to keep per file.
    #[arg(long, default_value_t = 10)]
    samples: usize,

    /// Skip reverse/forward DNS verification of bots (offline use); affected bots stay
    /// unverified until `snx-log bots verify`.
    #[arg(long)]
    no_dns: bool,

    /// Print the report as JSON.
    #[arg(long)]
    json: bool,

    /// Rows staged in memory before writing Parquet (advanced tuning).
    #[arg(long, hide = true, default_value_t = ImportOptions::default().flush_rows)]
    flush_rows: usize,
}

pub(crate) fn run(
    workspace: &Workspace,
    args: &ImportArgs,
    show_progress: bool,
) -> miette::Result<()> {
    let request = ImportRequest {
        site: args.site.clone(),
        paths: args.paths.clone(),
        max_error_samples: args.samples,
        options: ImportOptions {
            flush_rows: args.flush_rows,
        },
        verify_dns: !args.no_dns,
    };
    let progress: Box<dyn ImportProgress> = if show_progress {
        Box::new(StderrProgress::new())
    } else {
        Box::new(NoProgress)
    };
    let report = import_logs(workspace, &request, progress.as_ref(), &CancelToken::new())
        .into_diagnostic()?;
    if args.json {
        write_json(&report)
    } else {
        write_text(|out| render(out, &report))
    }
}

fn render(out: &mut dyn Write, report: &ImportReport) -> io::Result<()> {
    writeln!(out, "Site         {} [{}]", report.site, report.format_id)?;
    write!(
        out,
        "Files        {} imported, {} already imported",
        report.imported_files(),
        report.skipped_files()
    )?;
    match report.unrecognized_files() {
        0 => writeln!(out)?,
        n => writeln!(out, ", {n} not in the site's format (not stored)")?,
    }
    writeln!(
        out,
        "Hits         {} stored, {} duplicates dropped",
        report.inserted_rows, report.duplicate_rows
    )?;
    writeln!(
        out,
        "Throughput   {} lines/s ({} ms)",
        report.throughput.lines_per_second, report.throughput.elapsed_ms
    )?;
    for file in &report.files {
        match &file.outcome {
            FileOutcome::Imported { summary, .. } => {
                writeln!(out, "\n{}", file.path)?;
                render_summary(out, summary)?;
            }
            FileOutcome::Unrecognized { summary } => {
                writeln!(out, "\n{}  (no line matched; nothing stored)", file.path)?;
                render_summary(out, summary)?;
            }
            FileOutcome::AlreadyImported => {}
        }
    }
    Ok(())
}
