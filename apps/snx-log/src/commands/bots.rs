use std::io::{self, Write};

use clap::{Args, Subcommand};
use miette::IntoDiagnostic;
use seonexus_log_app::Workspace;
use seonexus_log_app::bots::{
    CustomBot, RangeSourceStatus, VerifySiteReport, VerifySiteRequest, bot_categories,
    bot_range_status, identify_user_agent, list_bots, list_custom_bots, parse_custom_bots,
    save_custom_bots, update_bot_ranges, verify_ip, verify_site_bots,
};

use super::output::{write_json, write_text};

#[derive(Debug, Args)]
pub(crate) struct BotsArgs {
    #[command(subcommand)]
    command: BotsCommand,
}

#[derive(Debug, Subcommand)]
enum BotsCommand {
    /// List known bots with their category and verification method.
    List {
        /// Only bots of this category (search, ai-training, ai-search, ai-assistant, ads,
        /// seo, social, monitoring, other).
        #[arg(long)]
        category: Option<String>,
        /// Print as JSON.
        #[arg(long)]
        json: bool,
    },
    /// Show which bot a user-agent string belongs to.
    Identify {
        /// User-agent string.
        user_agent: String,
        /// Print as JSON.
        #[arg(long)]
        json: bool,
    },
    /// Check whether an IP address belongs to a bot's operator.
    CheckIp {
        /// IPv4 or IPv6 address.
        ip: String,
        /// Bot id (see `octopuson-log bots list`).
        #[arg(long)]
        bot: String,
        /// Do not use reverse/forward DNS.
        #[arg(long)]
        no_dns: bool,
        /// Print as JSON.
        #[arg(long)]
        json: bool,
    },
    /// Show the crawler IP lists in use.
    Status {
        /// Print as JSON.
        #[arg(long)]
        json: bool,
    },
    /// Download the current crawler IP lists published by Google, Bing, OpenAI, …
    Update {
        /// Print as JSON.
        #[arg(long)]
        json: bool,
    },
    /// Manage your own bot definitions (matched before the built-in catalog).
    #[command(subcommand)]
    Custom(CustomCommand),
    /// Verify stored bot hits of a site again (pending DNS checks, or after `bots update`).
    Verify {
        /// Site name.
        site: String,
        /// Do not use reverse/forward DNS.
        #[arg(long)]
        no_dns: bool,
        /// Re-check every verdict, not only provisional ones.
        #[arg(long)]
        all: bool,
        /// Print as JSON.
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, Subcommand)]
enum CustomCommand {
    /// List custom bots.
    List {
        /// Print as JSON.
        #[arg(long)]
        json: bool,
    },
    /// Add or replace a custom bot.
    Add {
        /// Id used in queries (`bot:<id>`): lowercase letters, digits and -.
        id: String,
        /// Display name.
        #[arg(long)]
        name: Option<String>,
        /// Operator.
        #[arg(long, default_value = "")]
        operator: String,
        /// Category (search, ai-training, ai-search, ai-assistant, ads, seo, social,
        /// monitoring, other).
        #[arg(long, default_value = "other")]
        category: String,
        /// Case-insensitive user-agent substring (repeatable; any of them matches).
        #[arg(long = "pattern", required = true)]
        patterns: Vec<String>,
        /// IP address or CIDR prefix the bot crawls from (repeatable).
        #[arg(long = "ip-range")]
        ip_ranges: Vec<String>,
        /// URL of a published IP list (`ipv4Prefix` JSON), refreshed by `bots update`.
        #[arg(long)]
        ip_list_url: Option<String>,
        /// Host name suffix for reverse DNS verification (repeatable).
        #[arg(long = "dns-suffix")]
        dns_suffixes: Vec<String>,
    },
    /// Remove a custom bot.
    Remove {
        /// Bot id.
        id: String,
    },
    /// Write all custom bots to a JSON file (to share with colleagues).
    Export {
        /// Target file.
        file: std::path::PathBuf,
    },
    /// Add the bots of a JSON file written by `export` (same ids are replaced).
    Import {
        /// Source file.
        file: std::path::PathBuf,
    },
}

pub(crate) fn run(workspace: &Workspace, args: &BotsArgs) -> miette::Result<()> {
    match &args.command {
        BotsCommand::List { category, json } => list(workspace, category.as_deref(), *json),
        BotsCommand::Identify { user_agent, json } => identify(workspace, user_agent, *json),
        BotsCommand::Custom(command) => custom(workspace, command),
        BotsCommand::CheckIp {
            ip,
            bot,
            no_dns,
            json,
        } => {
            let check = verify_ip(workspace, ip, bot, !no_dns).into_diagnostic()?;
            if *json {
                return write_json(&check);
            }
            write_text(|out| {
                writeln!(
                    out,
                    "{} is {} as {} (method: {})",
                    check.ip, check.verdict, check.bot.name, check.method
                )
            })
        }
        BotsCommand::Status { json } => {
            let sources = bot_range_status(workspace);
            if *json {
                return write_json(&sources);
            }
            write_text(|out| render_status(out, &sources))
        }
        BotsCommand::Update { json } => update(workspace, *json),
        BotsCommand::Verify {
            site,
            no_dns,
            all,
            json,
        } => {
            let request = VerifySiteRequest {
                site: site.clone(),
                dns: !no_dns,
                all: *all,
            };
            let report = verify_site_bots(workspace, &request).into_diagnostic()?;
            if *json {
                return write_json(&report);
            }
            write_text(|out| render_verify(out, &report))
        }
    }
}

fn custom(workspace: &Workspace, command: &CustomCommand) -> miette::Result<()> {
    let mut bots = list_custom_bots(workspace).into_diagnostic()?;
    match command {
        CustomCommand::List { json } => {
            if *json {
                return write_json(&bots);
            }
            write_text(|out| {
                if bots.is_empty() {
                    return writeln!(
                        out,
                        "no custom bots; add one with `octopuson-log bots custom add`"
                    );
                }
                for bot in &bots {
                    let mut verification = Vec::new();
                    if !bot.ip_ranges.is_empty() {
                        verification.push(format!("{} ranges", bot.ip_ranges.len()));
                    }
                    if let Some(url) = &bot.ip_list_url {
                        verification.push(format!("list {url}"));
                    }
                    if !bot.dns_suffixes.is_empty() {
                        verification.push(format!("dns {}", bot.dns_suffixes.join(",")));
                    }
                    writeln!(
                        out,
                        "{:<24} {:<12} {:<30} {}",
                        bot.id,
                        bot.category,
                        bot.patterns.join(" | "),
                        if verification.is_empty() {
                            "not verifiable".to_owned()
                        } else {
                            verification.join("; ")
                        }
                    )?;
                }
                Ok(())
            })
        }
        CustomCommand::Add {
            id,
            name,
            operator,
            category,
            patterns,
            ip_ranges,
            ip_list_url,
            dns_suffixes,
        } => {
            let bot = CustomBot {
                id: id.clone(),
                name: name.clone().unwrap_or_default(),
                operator: operator.clone(),
                category: category.clone(),
                patterns: patterns.clone(),
                ip_ranges: ip_ranges.clone(),
                ip_list_url: ip_list_url.clone(),
                dns_suffixes: dns_suffixes.clone(),
            };
            let id = id.trim().to_ascii_lowercase();
            bots.retain(|existing| existing.id != id);
            bots.push(bot);
            save_custom_bots(workspace, bots).into_diagnostic()?;
            tracing::info!("custom bot `{id}` saved; stored hits are re-classified on next use");
            Ok(())
        }
        CustomCommand::Remove { id } => {
            let before = bots.len();
            bots.retain(|existing| existing.id != id.trim());
            if bots.len() == before {
                return Err(miette::miette!("no custom bot `{id}`"));
            }
            save_custom_bots(workspace, bots).into_diagnostic()?;
            tracing::info!("custom bot `{id}` removed");
            Ok(())
        }
        CustomCommand::Export { file } => {
            let json =
                serde_json::to_string_pretty(&serde_json::json!({ "version": 1, "bots": bots }))
                    .into_diagnostic()?;
            std::fs::write(file, json).into_diagnostic()?;
            tracing::info!(
                count = bots.len(),
                "custom bots written to {}",
                file.display()
            );
            Ok(())
        }
        CustomCommand::Import { file } => {
            let text = std::fs::read_to_string(file).into_diagnostic()?;
            let imported = parse_custom_bots(&text).into_diagnostic()?;
            let count = imported.len();
            bots.retain(|existing| !imported.iter().any(|bot| bot.id == existing.id));
            bots.extend(imported);
            save_custom_bots(workspace, bots).into_diagnostic()?;
            tracing::info!(count, "custom bots imported");
            Ok(())
        }
    }
}

fn list(workspace: &Workspace, category: Option<&str>, json: bool) -> miette::Result<()> {
    if let Some(name) = category
        && !bot_categories().contains(&name)
    {
        return Err(miette::miette!(
            "unknown category `{name}`; available: {}",
            bot_categories().join(", ")
        ));
    }
    let bots = list_bots(workspace, category);
    if json {
        return write_json(&bots);
    }
    write_text(|out| {
        writeln!(
            out,
            "{:<28} {:<14} {:<12} {:<20} name",
            "id", "category", "verify", "operator"
        )?;
        for bot in &bots {
            writeln!(
                out,
                "{:<28} {:<14} {:<12} {:<20} {}",
                bot.id, bot.category, bot.verification, bot.operator, bot.name
            )?;
        }
        Ok(())
    })
}

fn identify(workspace: &Workspace, user_agent: &str, json: bool) -> miette::Result<()> {
    let bot = identify_user_agent(workspace, user_agent);
    if json {
        return write_json(&bot);
    }
    write_text(|out| match &bot {
        Some(bot) => writeln!(
            out,
            "{} ({}, {}, operated by {})",
            bot.id,
            bot.name,
            bot.category,
            if bot.operator.is_empty() {
                "unknown"
            } else {
                bot.operator
            }
        ),
        None => writeln!(out, "not a known bot"),
    })
}

fn update(workspace: &Workspace, json: bool) -> miette::Result<()> {
    let report = update_bot_ranges(workspace).into_diagnostic()?;
    for failure in &report.failed {
        tracing::warn!(source = %failure.id, "{}", failure.error);
    }
    if json {
        return write_json(&report);
    }
    tracing::info!(
        updated = report.updated.len(),
        failed = report.failed.len(),
        "IP lists saved to {}",
        report.path
    );
    Ok(())
}

fn render_status(out: &mut dyn Write, sources: &[RangeSourceStatus]) -> io::Result<()> {
    writeln!(
        out,
        "{:<20} {:<12} {:>8}  {:<11} {:>5}  url",
        "list", "operator", "prefixes", "origin", "age"
    )?;
    for source in sources {
        let age = source
            .age_days
            .map_or_else(|| "-".to_owned(), |days| format!("{days}d"));
        writeln!(
            out,
            "{:<20} {:<12} {:>8}  {:<11} {:>5}  {}",
            source.id, source.operator, source.prefixes, source.origin, age, source.url
        )?;
    }
    Ok(())
}

fn render_verify(out: &mut dyn Write, report: &VerifySiteReport) -> io::Result<()> {
    writeln!(out, "Site         {}", report.site)?;
    let dns = match (report.dns_used, report.dns_unavailable) {
        (true, _) => "used",
        (false, true) => "requested but not responding",
        (false, false) => "not used",
    };
    writeln!(out, "DNS          {dns}")?;
    if report.reclassified_user_agents > 0 {
        writeln!(
            out,
            "Reclassified {} user agents",
            report.reclassified_user_agents
        )?;
    }
    writeln!(
        out,
        "Checked      {} IP/bot pairs, {} changed",
        report.checked, report.changed
    )?;
    if !report.rebuilt_bots.is_empty() {
        writeln!(out, "Rebuilt      {}", report.rebuilt_bots.join(", "))?;
    }
    let verdicts: Vec<String> = report
        .verdicts
        .iter()
        .map(|(verdict, count)| format!("{verdict}: {count}"))
        .collect();
    writeln!(out, "Verdicts     {}", verdicts.join(", "))
}
