use std::io::{self, Write};

use clap::Args;
use miette::IntoDiagnostic;
use seonexus_log_app::Workspace;
use seonexus_log_app::analysis::{
    AnomalyKind, DEFAULT_CRAWLER_FILTER, NormalizeOptions, OverviewRequest, PeriodRequest,
    QueryMode, RobotsReport, RobotsRequest, Share, SiteCrawlOverview, UrlListRequest, UrlSource,
    compare_periods, compare_url_list, crawl_overview, robots_compliance,
};

use super::output::{write_json, write_text};
use super::results::{ResultArgs, emit};

/// `snx-log overview`.
#[derive(Debug, Args)]
pub(crate) struct OverviewArgs {
    /// Site to summarize.
    site: String,

    /// Period as a date value: `last30d`, `2026-09-01..2026-09-30`, `2026-10-07`.
    #[arg(long, value_name = "DATES")]
    period: Option<String>,

    /// Print as JSON.
    #[arg(long)]
    json: bool,
}

pub(crate) fn overview(workspace: &Workspace, args: &OverviewArgs) -> miette::Result<()> {
    let request = OverviewRequest {
        site: args.site.clone(),
        period: args.period.clone(),
    };
    let overview = crawl_overview(workspace, &request).into_diagnostic()?;
    if args.json {
        return write_json(&overview);
    }
    write_text(|out| render_overview(out, &overview))
}

fn percent(ratio: f64) -> String {
    format!("{:.1}%", ratio * 100.0)
}

fn render_shares(out: &mut dyn Write, shares: &[Share]) -> io::Result<()> {
    for share in shares {
        writeln!(
            out,
            "  {:<14} {:>12}  {:>6}",
            share.name,
            share.value,
            percent(share.ratio)
        )?;
    }
    Ok(())
}

fn render_overview(out: &mut dyn Write, report: &SiteCrawlOverview) -> io::Result<()> {
    let overview = &report.overview;
    let period = report.period.as_deref().unwrap_or("all data");
    writeln!(out, "Site           {} ({period})", report.site)?;
    writeln!(out, "Hits           {}", overview.total_hits)?;
    writeln!(out, "\nTraffic by category")?;
    render_shares(out, &overview.categories)?;

    let search = &overview.search;
    writeln!(out, "\nVerified search-engine crawl")?;
    writeln!(out, "  Hits           {}", search.hits)?;
    writeln!(out, "  URLs crawled   {}", search.unique_urls)?;
    writeln!(out, "  Hits per day   {:.0}", search.hits_per_day)?;
    writeln!(out, "  URLs per day   {:.0}", search.unique_urls_per_day)?;
    writeln!(out, "  Errors         {}", search.errors)?;
    if let Some(bytes) = search.avg_bytes {
        writeln!(out, "  Avg. bytes     {bytes:.0}")?;
    }
    if let Some(ms) = search.avg_ms {
        writeln!(out, "  Avg. time (ms) {ms:.0}")?;
    }
    writeln!(
        out,
        "  Non-2xx        {}",
        percent(search.non_success_ratio)
    )?;
    writeln!(out, "  Parameter URLs {}", percent(search.parameter_ratio))?;
    writeln!(out, "  Static assets  {}", percent(search.asset_ratio))?;
    writeln!(out, "  Status classes")?;
    for share in &search.status_classes {
        writeln!(
            out,
            "    {:<12} {:>12}  {:>6}",
            share.name,
            share.value,
            percent(share.ratio)
        )?;
    }

    writeln!(out, "\nBots (verified / spoofed / unverified)")?;
    for bot in overview.bots.iter().take(25) {
        writeln!(
            out,
            "  {:<28} {:>10} {:>10} {:>10}",
            bot.bot, bot.verified, bot.spoofed, bot.unverified
        )?;
    }
    if overview.spoofed_hits > 0 {
        writeln!(
            out,
            "\n{} hits claimed to be a bot but failed verification (template spoofed-bots).",
            overview.spoofed_hits
        )?;
    }
    if !report.alerts.is_empty() {
        writeln!(out, "\nAlerts")?;
        for alert in &report.alerts {
            let what = match alert.kind {
                AnomalyKind::Spike => format!(
                    "spike: {:.0} hits (usually {:.0})",
                    alert.value, alert.baseline
                ),
                AnomalyKind::Drop => format!(
                    "drop: {:.0} hits (usually {:.0})",
                    alert.value, alert.baseline
                ),
                AnomalyKind::ServerErrors => format!(
                    "server errors: {} of hits (usually {})",
                    percent(alert.value),
                    percent(alert.baseline)
                ),
            };
            writeln!(out, "  {}  {:<10} {what}", alert.day, alert.series)?;
        }
    }
    Ok(())
}

/// `snx-log compare-urls`.
#[derive(Debug, Args)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "independent command-line switches"
)]
pub(crate) struct CompareUrlsArgs {
    /// Site whose logs are compared.
    site: String,

    /// URL lists: files (plain list, crawler CSV export, XML sitemap; .gz allowed) or
    /// sitemap URLs. Sitemap indexes are followed.
    #[arg(required = true, value_name = "LIST")]
    lists: Vec<String>,

    /// Which hits count as requests; use '' for every hit.
    #[arg(long, value_name = "QUERY", default_value = DEFAULT_CRAWLER_FILTER)]
    filter: String,

    /// Only compare list URLs on this host.
    #[arg(long)]
    host: Option<String>,

    /// Ignore query strings entirely.
    #[arg(long, conflicts_with = "keep_tracking")]
    ignore_query: bool,

    /// Keep tracking parameters such as `utm_source` (removed by default).
    #[arg(long)]
    keep_tracking: bool,

    /// Treat /a/ and /a as the same URL.
    #[arg(long)]
    ignore_trailing_slash: bool,

    /// Compare paths case-insensitively.
    #[arg(long)]
    ignore_case: bool,

    #[command(flatten)]
    output: ResultArgs,
}

pub(crate) fn compare_urls(workspace: &Workspace, args: &CompareUrlsArgs) -> miette::Result<()> {
    let query = match (args.ignore_query, args.keep_tracking) {
        (true, _) => QueryMode::DropAll,
        (false, true) => QueryMode::Keep,
        (false, false) => QueryMode::DropTracking,
    };
    let request = UrlListRequest {
        site: args.site.clone(),
        sources: args.lists.iter().map(|l| UrlSource::parse(l)).collect(),
        filter: Some(args.filter.clone()),
        normalize: NormalizeOptions {
            query,
            ignore_trailing_slash: args.ignore_trailing_slash,
            ignore_case: args.ignore_case,
        },
        host: args.host.clone(),
    };
    let report = emit(&args.output, |sink| {
        compare_url_list(workspace, &request, sink)
    })?;
    let summary = report.summary;
    let message = format!(
        "{} listed, {} requested: {} in both, {} never requested (not-in-logs), {} not in the \
         list (not-in-list); {} list entries skipped; filter: {}",
        summary.listed,
        summary.logged,
        summary.both,
        summary.not_in_logs,
        summary.not_in_list,
        report.skipped,
        if report.filter.is_empty() {
            "all hits"
        } else {
            &report.filter
        }
    );
    if args.output.is_table() {
        write_text(|out| writeln!(out, "{message}"))
    } else {
        tracing::info!("{message}");
        Ok(())
    }
}

/// `snx-log robots`.
#[derive(Debug, Args)]
pub(crate) struct RobotsArgs {
    /// Site whose bot requests are checked.
    site: String,

    /// robots.txt file or URL (default: https://<site host>/robots.txt).
    #[arg(long, value_name = "FILE|URL")]
    robots: Option<String>,

    /// Extra conditions, e.g. `date:last30d verification:verified`.
    #[arg(long, value_name = "QUERY")]
    filter: Option<String>,

    #[command(flatten)]
    output: ResultArgs,
}

pub(crate) fn robots(workspace: &Workspace, args: &RobotsArgs) -> miette::Result<()> {
    let request = RobotsRequest {
        site: args.site.clone(),
        robots: args.robots.clone(),
        filter: args.filter.clone(),
    };
    let report = emit(&args.output, |sink| {
        robots_compliance(workspace, &request, sink)
    })?;
    if args.output.is_table() {
        write_text(|out| render_robots(out, &report))
    } else {
        tracing::info!(
            rows = report.rows,
            "robots.txt {} checked against bot requests",
            report.robots
        );
        Ok(())
    }
}

fn render_robots(out: &mut dyn Write, report: &RobotsReport) -> io::Result<()> {
    writeln!(out, "\nrobots.txt  {}", report.robots)?;
    writeln!(
        out,
        "{:<26} {:<14} {:<10} {:>10} {:>12} {:>10}",
        "bot", "group", "blocked", "hits", "disallowed", "verified"
    )?;
    for bot in &report.bots {
        writeln!(
            out,
            "{:<26} {:<14} {:<10} {:>10} {:>12} {:>10}{}",
            bot.bot,
            bot.group.as_deref().unwrap_or("–"),
            if bot.blocked { "yes" } else { "" },
            bot.hits,
            bot.disallowed_hits,
            bot.verified_disallowed_hits,
            if bot.user_triggered && bot.disallowed_hits > 0 {
                "  (user-triggered)"
            } else {
                ""
            }
        )?;
    }
    Ok(())
}

/// `snx-log compare-periods`.
#[derive(Debug, Args)]
pub(crate) struct ComparePeriodsArgs {
    /// Site to analyse.
    site: String,

    /// Earlier period, e.g. 2026-09-01..2026-09-30.
    #[arg(long, value_name = "DATES")]
    before: String,

    /// Later period, e.g. 2026-10-01..2026-10-31 or last30d.
    #[arg(long, value_name = "DATES")]
    after: String,

    /// What to compare: `url`, `directory`, `bot`, `status`, `status_class`, `ua`, …
    #[arg(long, default_value = "url")]
    group: String,

    /// Extra filter, e.g. 'bot:googlebot* verification:verified'.
    #[arg(long, value_name = "QUERY")]
    filter: Option<String>,

    #[command(flatten)]
    output: ResultArgs,
}

pub(crate) fn compare_periods_command(
    workspace: &Workspace,
    args: &ComparePeriodsArgs,
) -> miette::Result<()> {
    let request = PeriodRequest {
        site: args.site.clone(),
        before: args.before.clone(),
        after: args.after.clone(),
        group: args.group.clone(),
        filter: args.filter.clone(),
    };
    let report = emit(&args.output, |sink| {
        compare_periods(workspace, &request, sink)
    })?;
    let s = report.summary;
    let message = format!(
        "hits {} → {}; {} new, {} lost, {} up, {} down",
        s.before_hits, s.after_hits, s.new, s.lost, s.up, s.down
    );
    if args.output.is_table() {
        write_text(|out| writeln!(out, "{message}"))
    } else {
        tracing::info!("{message}");
        Ok(())
    }
}
