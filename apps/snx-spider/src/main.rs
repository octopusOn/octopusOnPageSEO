//! `octopuson-crawler` — OctopusOn SEO Crawler command-line interface.

mod cli;
mod commands;
mod table;

use std::io;

use clap::Parser;

use crate::cli::Cli;

fn main() -> miette::Result<()> {
    let args = Cli::parse();
    match commands::run(&args) {
        // Output piped into `head` and similar closes early; that is not a failure.
        Err(report) if is_broken_pipe(&report) => Ok(()),
        other => other,
    }
}

fn is_broken_pipe(report: &miette::Report) -> bool {
    report.chain().any(|error| {
        error
            .downcast_ref::<io::Error>()
            .is_some_and(|error| error.kind() == io::ErrorKind::BrokenPipe)
    })
}
