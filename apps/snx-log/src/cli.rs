use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};
use tracing::Level;

use crate::commands::analysis::{ComparePeriodsArgs, CompareUrlsArgs, OverviewArgs, RobotsArgs};
use crate::commands::bots::BotsArgs;
use crate::commands::client::ClientArgs;
use crate::commands::geo::GeoArgs;
use crate::commands::import::ImportArgs;
use crate::commands::inspect::InspectArgs;
use crate::commands::query::QueryArgs;
use crate::commands::saved::SavedArgs;
use crate::commands::site::SiteArgs;

/// OctopusOn Log Analyser — SEO-focused server log analysis.
#[derive(Debug, Parser)]
#[command(name = "octopuson-log", version, about, long_about = None)]
pub(crate) struct Cli {
    #[command(flatten)]
    pub(crate) global: GlobalArgs,

    #[command(subcommand)]
    pub(crate) command: Command,
}

#[derive(Debug, Subcommand)]
pub(crate) enum Command {
    /// Parse log files without importing them and report format, counts and skipped lines.
    Inspect(InspectArgs),
    /// Manage clients.
    Client(ClientArgs),
    /// Manage sites.
    Site(SiteArgs),
    /// Import log files into a site (already imported files and lines are skipped).
    Import(ImportArgs),
    /// Run a filter or report against a site; print it or export it to CSV/JSON/XLSX.
    Query(QueryArgs),
    /// Summarize traffic and how search engines spend their crawl budget.
    Overview(OverviewArgs),
    /// Compare a crawl export or sitemap with logged URLs (uncrawled and orphan pages).
    CompareUrls(CompareUrlsArgs),
    /// Compare hits per URL, directory, bot, … between two periods.
    ComparePeriods(ComparePeriodsArgs),
    /// Check bot requests against robots.txt: disallowed URLs crawled, bots blocked.
    Robots(RobotsArgs),
    /// Bot catalog, crawler IP lists and bot verification.
    Bots(BotsArgs),
    /// IP geolocation data (country and network per client IP).
    Geo(GeoArgs),
    /// Save, list, share and delete named queries.
    Saved(SavedArgs),
    /// List the built-in report templates.
    Templates {
        /// Print as JSON.
        #[arg(long)]
        json: bool,
    },
}

/// Options shared by every subcommand.
#[derive(Debug, Args)]
pub(crate) struct GlobalArgs {
    /// Increase log verbosity (-v debug, -vv trace).
    #[arg(short, long, action = clap::ArgAction::Count, global = true)]
    verbose: u8,

    /// Only print errors.
    #[arg(short, long, global = true, conflicts_with = "verbose")]
    quiet: bool,

    /// Data directory (defaults to the platform application data directory).
    #[arg(long, global = true, env = "OCTOPUSSEO_DATA_DIR", value_name = "DIR")]
    pub(crate) data_dir: Option<PathBuf>,

    /// Storage engine memory limit in MiB; larger imports spill to disk.
    #[arg(
        long,
        global = true,
        env = "OCTOPUSSEO_MEMORY_LIMIT_MB",
        value_name = "MIB",
        default_value_t = 1024
    )]
    pub(crate) memory_limit: u32,
}

impl GlobalArgs {
    pub(crate) fn show_progress(&self) -> bool {
        !self.quiet
    }

    pub(crate) fn verbosity(&self) -> Level {
        if self.quiet {
            return Level::ERROR;
        }
        match self.verbose {
            0 => Level::INFO,
            1 => Level::DEBUG,
            _ => Level::TRACE,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn cli_definition_is_valid() {
        Cli::command().debug_assert();
    }

    #[test]
    fn verbosity_flags_map_to_levels() {
        let parse = |flags: &[&str]| {
            let argv = [&["octopuson-log"], flags, &["inspect", "a.log"]].concat();
            Cli::parse_from(argv).global.verbosity()
        };
        assert_eq!(parse(&[]), Level::INFO);
        assert_eq!(parse(&["-v"]), Level::DEBUG);
        assert_eq!(parse(&["-vv"]), Level::TRACE);
        assert_eq!(parse(&["-q"]), Level::ERROR);
    }

    #[test]
    fn query_words_are_collected() {
        let cli = Cli::parse_from([
            "octopuson-log",
            "query",
            "acme.com",
            "status:404",
            "group:url",
        ]);
        assert!(matches!(cli.command, Command::Query(_)));
        let saved = Cli::try_parse_from([
            "octopuson-log",
            "query",
            "a",
            "--saved",
            "x",
            "--template",
            "y",
        ]);
        assert!(saved.is_err(), "--saved and --template exclude each other");
        let export_with_csv =
            Cli::try_parse_from(["octopuson-log", "query", "a", "x:1", "-o", "f.csv", "--csv"]);
        assert!(export_with_csv.is_err());
    }

    #[test]
    fn custom_formats_conflict() {
        let result = Cli::try_parse_from([
            "octopuson-log",
            "inspect",
            "a.log",
            "--nginx-format",
            "x",
            "--apache-format",
            "y",
        ]);
        assert!(result.is_err());
    }
}
