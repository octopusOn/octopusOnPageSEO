use std::io::{self, Write};
use std::path::PathBuf;

use clap::Args;
use miette::IntoDiagnostic;
use octopuson_log_app::inspect::{InspectReport, InspectRequest, inspect};

use super::format_args::FormatArgs;
use super::output::{write_json, write_text};
use super::summary::render_summary;

#[derive(Debug, Args)]
pub(crate) struct InspectArgs {
    /// Log files or directories (plain, .gz, .zst, .bz2 or .zip).
    #[arg(required = true, value_name = "PATH")]
    files: Vec<PathBuf>,

    #[command(flatten)]
    format: FormatArgs,

    /// Number of skipped-line examples to show.
    #[arg(long, default_value_t = 10)]
    samples: usize,

    /// Print the report as JSON.
    #[arg(long)]
    json: bool,
}

pub(crate) fn run(args: &InspectArgs) -> miette::Result<()> {
    let request = InspectRequest {
        files: args.files.clone(),
        format: args.format.choice(),
        max_error_samples: args.samples,
    };
    let report = inspect(&request).into_diagnostic()?;
    if args.json {
        write_json(&report)
    } else {
        write_text(|out| render(out, &report))
    }
}

fn render(out: &mut dyn Write, report: &InspectReport) -> io::Result<()> {
    let confidence = report
        .detection_confidence
        .map(|c| format!(" (auto-detected, {:.0}% of sample matched)", c * 100.0))
        .unwrap_or_default();
    writeln!(
        out,
        "Format       {} [{}]{confidence}",
        report.format_name, report.format_id
    )?;
    writeln!(
        out,
        "Throughput   {} lines/s ({} ms)",
        report.throughput.lines_per_second, report.throughput.elapsed_ms
    )?;
    render_summary(out, &report.summary)
}
