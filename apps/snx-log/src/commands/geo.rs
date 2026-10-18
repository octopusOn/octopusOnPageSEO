use clap::{Args, Subcommand};
use miette::IntoDiagnostic;
use seonexus_log_app::Workspace;
use seonexus_log_app::geo::{geo_status, update_geo_data};

use super::output::{write_json, write_text};

#[derive(Debug, Args)]
pub(crate) struct GeoArgs {
    #[command(subcommand)]
    command: GeoCommand,
}

#[derive(Debug, Subcommand)]
enum GeoCommand {
    /// Show the installed IP geolocation data.
    Status {
        /// Print as JSON.
        #[arg(long)]
        json: bool,
    },
    /// Download the current DB-IP Lite country and ASN databases (about 20 MB); client IPs
    /// then get a country and network in queries (`country:`, `as_org:`, `group:country`).
    Update {
        /// Print as JSON.
        #[arg(long)]
        json: bool,
    },
}

pub(crate) fn run(workspace: &Workspace, args: &GeoArgs) -> miette::Result<()> {
    match &args.command {
        GeoCommand::Status { json } => {
            let status = geo_status(workspace);
            if *json {
                return write_json(&status);
            }
            write_text(|out| {
                match &status.release {
                    Some(release) => writeln!(
                        out,
                        "Country data {}, ASN data {} (downloaded {})",
                        release.country, release.asn, release.fetched_at
                    )?,
                    None => writeln!(
                        out,
                        "No IP geolocation data; run `octopuson-log geo update`."
                    )?,
                }
                writeln!(out, "{}", status.attribution)
            })
        }
        GeoCommand::Update { json } => {
            let report = update_geo_data(workspace).into_diagnostic()?;
            if *json {
                return write_json(&report);
            }
            tracing::info!(
                country = %report.release.country,
                asn = %report.release.asn,
                country_ranges = report.country_ranges,
                asn_ranges = report.asn_ranges,
                "IP geolocation data installed; sites are updated when next used"
            );
            write_text(|out| writeln!(out, "{}", seonexus_log_app::geo::ATTRIBUTION))
        }
    }
}
