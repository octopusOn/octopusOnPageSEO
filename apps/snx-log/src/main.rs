//! `snx-log` — octopusSEO Log Analyser command-line interface.

mod cli;
mod commands;
mod telemetry;

use std::io;

use clap::Parser;
use miette::IntoDiagnostic;
use seonexus_log_app::Workspace;

use crate::cli::{Cli, Command};

fn main() -> miette::Result<()> {
    let args = Cli::parse();
    telemetry::init(args.global.verbosity());
    match run(&args) {
        // Output piped into `head` and similar closes early; that is not a failure.
        Err(report) if is_broken_pipe(&report) => Ok(()),
        other => other,
    }
}

fn run(args: &Cli) -> miette::Result<()> {
    let workspace = || {
        Workspace::resolve(args.global.data_dir.clone())
            .map(|workspace| workspace.with_memory_limit_mb(args.global.memory_limit))
            .into_diagnostic()
    };
    match &args.command {
        Command::Inspect(inspect) => commands::inspect::run(inspect),
        Command::Client(client) => commands::client::run(&workspace()?, client),
        Command::Site(site) => commands::site::run(&workspace()?, site),
        Command::Import(import) => {
            commands::import::run(&workspace()?, import, args.global.show_progress())
        }
        Command::Query(query) => commands::query::run(&workspace()?, query),
        Command::Bots(bots) => commands::bots::run(&workspace()?, bots),
        Command::Geo(geo) => commands::geo::run(&workspace()?, geo),
        Command::Saved(saved) => commands::saved::run(&workspace()?, saved),
        Command::Overview(overview) => commands::analysis::overview(&workspace()?, overview),
        Command::CompareUrls(compare) => commands::analysis::compare_urls(&workspace()?, compare),
        Command::Robots(robots) => commands::analysis::robots(&workspace()?, robots),
        Command::ComparePeriods(compare) => {
            commands::analysis::compare_periods_command(&workspace()?, compare)
        }
        Command::Templates { json } => commands::query::list_templates(*json),
    }
}

fn is_broken_pipe(report: &miette::Report) -> bool {
    report.chain().any(|error| {
        error
            .downcast_ref::<io::Error>()
            .is_some_and(|error| error.kind() == io::ErrorKind::BrokenPipe)
    })
}
