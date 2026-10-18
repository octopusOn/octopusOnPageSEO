use std::io::{self, Write};

use clap::{Args, Subcommand};
use miette::IntoDiagnostic;
use octopuson_log_app::Workspace;
use octopuson_log_app::archive::{export_site, import_site};
use octopuson_log_app::catalog::{
    AddSiteRequest, SiteOverview, add_site, delete_import, list_sites, remove_site,
    set_site_format, site_overview,
};
use octopuson_log_app::settings::{SiteSettings, TrafficScope, set_site_settings, site_settings};

use super::format_args::FormatArgs;
use super::output::{write_json, write_text};

#[derive(Debug, Args)]
pub(crate) struct SiteArgs {
    #[command(subcommand)]
    command: SiteCommand,
}

#[derive(Debug, Subcommand)]
enum SiteCommand {
    /// Create a site under a client.
    Add {
        /// Site name, usually the domain.
        name: String,
        /// Owning client.
        #[arg(long)]
        client: String,
        #[command(flatten)]
        format: FormatArgs,
    },
    /// List sites.
    List {
        /// Print as JSON.
        #[arg(long)]
        json: bool,
    },
    /// Show stored data volume and import history of a site.
    Show {
        /// Site name.
        name: String,
        /// Print as JSON.
        #[arg(long)]
        json: bool,
    },
    /// Change the log format used by later imports.
    SetFormat {
        /// Site name.
        name: String,
        #[command(flatten)]
        format: FormatArgs,
    },
    /// Show or change what imports store: own host, bots only, URL include/exclude patterns,
    /// parameters to strip. Without options the current settings are printed.
    Settings(SettingsArgs),
    /// Write a site with all its data, format and settings to one `.snxsite` file.
    Export {
        /// Site name.
        name: String,
        /// Archive to write, e.g. `acme.snxsite`.
        file: std::path::PathBuf,
    },
    /// Create a site from a `.snxsite` archive.
    Import {
        /// Archive to read.
        file: std::path::PathBuf,
        /// Site name (default: the archived name).
        #[arg(long)]
        name: Option<String>,
        /// Client (default: the archived client; created when missing).
        #[arg(long)]
        client: Option<String>,
    },
    /// Delete one import (e.g. a wrong file) with its data; see `site show` for ids.
    RemoveImport {
        /// Site name.
        name: String,
        /// Import id.
        id: i64,
        /// Confirm the deletion.
        #[arg(long)]
        yes: bool,
    },
    /// Delete a site and all its imported data.
    Remove {
        /// Site name.
        name: String,
        /// Confirm the deletion.
        #[arg(long)]
        yes: bool,
    },
}

pub(crate) fn run(workspace: &Workspace, args: &SiteArgs) -> miette::Result<()> {
    match &args.command {
        SiteCommand::Add {
            name,
            client,
            format,
        } => {
            let request = AddSiteRequest {
                client: client.clone(),
                name: name.clone(),
                format: format.choice(),
            };
            let site = add_site(workspace, &request).into_diagnostic()?;
            tracing::info!(id = site.id, format = %site.format, "site `{}` created", site.name);
            Ok(())
        }
        SiteCommand::List { json } => {
            let sites = list_sites(workspace).into_diagnostic()?;
            if *json {
                return write_json(&sites);
            }
            write_text(|out| {
                for site in &sites {
                    let format = site.format.lines().next().unwrap_or_default();
                    writeln!(
                        out,
                        "{:>4}  {:<30} {:<20} {format}",
                        site.id, site.name, site.client
                    )?;
                }
                Ok(())
            })
        }
        SiteCommand::Show { name, json } => {
            let overview = site_overview(workspace, name).into_diagnostic()?;
            if *json {
                return write_json(&overview);
            }
            write_text(|out| render_overview(out, &overview))
        }
        SiteCommand::SetFormat { name, format } => {
            let site = set_site_format(workspace, name, &format.choice()).into_diagnostic()?;
            tracing::info!(format = %site.format, "format of `{}` updated", site.name);
            Ok(())
        }
        SiteCommand::Settings(settings) => run_settings(workspace, settings),
        SiteCommand::Export { name, file } => {
            let report = export_site(workspace, name, file).into_diagnostic()?;
            tracing::info!(
                files = report.files,
                bytes = report.bytes,
                "site `{}` written to {}",
                report.site,
                report.path
            );
            Ok(())
        }
        SiteCommand::Import { file, name, client } => {
            let site = import_site(workspace, file, name.as_deref(), client.as_deref())
                .into_diagnostic()?;
            tracing::info!(client = %site.client, "site `{}` imported", site.name);
            Ok(())
        }
        SiteCommand::RemoveImport { name, id, yes } => {
            if !yes {
                return Err(miette::miette!(
                    help = "add --yes to confirm",
                    "this deletes import #{id} of `{name}` and the hits it stored"
                ));
            }
            delete_import(workspace, name, *id).into_diagnostic()?;
            tracing::info!("import #{id} of `{}` deleted", name.trim());
            Ok(())
        }
        SiteCommand::Remove { name, yes } => {
            if !yes {
                return Err(miette::miette!(
                    help = "add --yes to confirm",
                    "this deletes `{name}` and all of its imported logs"
                ));
            }
            remove_site(workspace, name).into_diagnostic()?;
            tracing::info!("site `{}` removed", name.trim());
            Ok(())
        }
    }
}

#[derive(Debug, Args)]
struct SettingsArgs {
    /// Site name.
    name: String,
    /// Start from the defaults instead of the current settings.
    #[arg(long)]
    reset: bool,
    /// The site's own host; referers from it count as internal (`-` to derive it from the
    /// site name).
    #[arg(long)]
    host: Option<String>,
    /// Store `all` requests or only `bots`.
    #[arg(long, value_parser = ["all", "bots"])]
    traffic: Option<String>,
    /// With `--traffic bots`: only these categories (repeatable; replaces the list).
    #[arg(long = "bot-category")]
    bot_categories: Vec<String>,
    /// Store only URLs matching this regular expression (repeatable; replaces the list).
    #[arg(long = "include")]
    include_urls: Vec<String>,
    /// Never store URLs matching this regular expression (repeatable; replaces the list).
    #[arg(long = "exclude")]
    exclude_urls: Vec<String>,
    /// Remove this query parameter from URLs (repeatable; replaces the list).
    #[arg(long = "strip-param")]
    strip_parameters: Vec<String>,
    /// Also remove tracking parameters such as utm_* and gclid.
    #[arg(long)]
    strip_tracking: Option<bool>,
    /// Print as JSON.
    #[arg(long)]
    json: bool,
}

impl SettingsArgs {
    fn changes_anything(&self) -> bool {
        self.reset
            || self.host.is_some()
            || self.traffic.is_some()
            || !self.bot_categories.is_empty()
            || !self.include_urls.is_empty()
            || !self.exclude_urls.is_empty()
            || !self.strip_parameters.is_empty()
            || self.strip_tracking.is_some()
    }
}

fn run_settings(workspace: &Workspace, args: &SettingsArgs) -> miette::Result<()> {
    let mut settings = if args.reset {
        SiteSettings::default()
    } else {
        site_settings(workspace, &args.name).into_diagnostic()?
    };
    if args.changes_anything() {
        if let Some(host) = &args.host {
            settings.host = (host != "-").then(|| host.clone());
        }
        if let Some(traffic) = &args.traffic {
            settings.traffic = if traffic == "bots" {
                TrafficScope::Bots
            } else {
                TrafficScope::All
            };
        }
        let replace = |target: &mut Vec<String>, given: &[String]| {
            if !given.is_empty() {
                *target = given.iter().filter(|v| !v.is_empty()).cloned().collect();
            }
        };
        replace(&mut settings.bot_categories, &args.bot_categories);
        replace(&mut settings.include_urls, &args.include_urls);
        replace(&mut settings.exclude_urls, &args.exclude_urls);
        replace(&mut settings.strip_parameters, &args.strip_parameters);
        if let Some(strip) = args.strip_tracking {
            settings.strip_tracking_parameters = strip;
        }
        settings = set_site_settings(workspace, &args.name, settings).into_diagnostic()?;
    }
    if args.json {
        return write_json(&settings);
    }
    write_text(|out| render_settings(out, &settings))
}

fn render_settings(out: &mut dyn Write, settings: &SiteSettings) -> io::Result<()> {
    let list = |items: &[String]| {
        if items.is_empty() {
            "–".to_owned()
        } else {
            items.join("  ")
        }
    };
    writeln!(
        out,
        "Host              {}",
        settings.host.as_deref().unwrap_or("(site name)")
    )?;
    let traffic = match settings.traffic {
        TrafficScope::All => "all requests".to_owned(),
        TrafficScope::Bots if settings.bot_categories.is_empty() => "bots only".to_owned(),
        TrafficScope::Bots => format!("bots only: {}", settings.bot_categories.join(", ")),
    };
    writeln!(out, "Stored traffic    {traffic}")?;
    writeln!(out, "Include URLs      {}", list(&settings.include_urls))?;
    writeln!(out, "Exclude URLs      {}", list(&settings.exclude_urls))?;
    writeln!(
        out,
        "Strip parameters  {}",
        list(&settings.strip_parameters)
    )?;
    writeln!(
        out,
        "Strip tracking    {}",
        if settings.strip_tracking_parameters {
            "yes"
        } else {
            "no"
        }
    )
}

fn render_overview(out: &mut dyn Write, overview: &SiteOverview) -> io::Result<()> {
    let site = &overview.site;
    writeln!(out, "Site         {} (client {})", site.name, site.client)?;
    writeln!(out, "Format       {}", site.format)?;
    writeln!(
        out,
        "Data         {} hits, {} URLs, {} user agents, {} imports",
        overview.hits, overview.urls, overview.user_agents, overview.imports
    )?;
    if let (Some(first), Some(last)) = (&overview.first_request, &overview.last_request) {
        writeln!(out, "Time range   {first} → {last}")?;
    }
    if overview.history.is_empty() {
        return Ok(());
    }
    writeln!(out, "\nImports")?;
    for import in &overview.history {
        writeln!(
            out,
            "  #{:<4} {}  {} hits, {} duplicates, {} of {} lines parsed  {}",
            import.id,
            import.finished_at,
            import.inserted_rows,
            import.duplicate_rows,
            import.parsed_lines,
            import.total_lines,
            import.path
        )?;
    }
    Ok(())
}
