use std::io::{self, IsTerminal, Write};
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use miette::{IntoDiagnostic, WrapErr, miette};
use octopuson_core::table::TableSink;
use octopuson_spider_app::{
    AgentPreset, CrawlConfig, CrawlControl, CrawlMeta, CrawlMode, CrawlProgress, CrawlTab,
    ExportFormat, RenderMode, RobotsPolicy, SpiderWorkspace,
};

use crate::cli::{
    Agent, Cli, Command, CrawlArgs, ExportArgs, Format, QueryArgs, Rendering, Robots, ShowArgs,
};
use crate::table::TextTable;

const PROGRESS_REFRESH: Duration = Duration::from_millis(500);

pub(crate) fn run(args: &Cli) -> miette::Result<()> {
    let workspace = SpiderWorkspace::resolve(args.data_dir.clone()).into_diagnostic()?;
    match &args.command {
        Command::Crawl(crawl) => run_crawl(&workspace, crawl, !args.quiet),
        Command::Crawls { json } => list(&workspace, *json),
        Command::Show(show) => run_show(&workspace, show),
        Command::Export(export) => run_export(&workspace, export),
        Command::Query(query) => run_query(&workspace, query),
        Command::Delete { crawl } => {
            let id = resolve_id(&workspace, crawl)?;
            workspace.delete(&id).into_diagnostic()?;
            write_text(|out| writeln!(out, "deleted {id}"))
        }
    }
}

fn run_crawl(
    workspace: &SpiderWorkspace,
    args: &CrawlArgs,
    show_progress: bool,
) -> miette::Result<()> {
    let config = crawl_config(args)?;
    let control = Arc::new(CrawlControl::new());
    let handler = Arc::clone(&control);
    // A second Ctrl+C while stopping ends the process the usual way.
    let _ = ctrlc::set_handler(move || {
        if handler.is_stopped() {
            std::process::exit(130);
        }
        handler.stop();
    });
    let live = show_progress && io::stderr().is_terminal();
    let mut last = Instant::now();
    let meta = workspace
        .crawl(config, &control, |progress| {
            if live && last.elapsed() >= PROGRESS_REFRESH {
                last = Instant::now();
                write_stderr(&format!("\r{}   ", progress_line(progress)));
            }
        })
        .into_diagnostic()?;
    if live {
        write_stderr("\n");
    }
    print_summary(&meta)?;
    if let Some(dir) = &args.export {
        let exported = workspace
            .export_all(&meta.id, dir, export_format(args.format))
            .into_diagnostic()?;
        write_text(|out| {
            for tab in &exported {
                writeln!(out, "  {:>8} rows  {}", tab.rows, tab.path.display())?;
            }
            Ok(())
        })?;
    }
    Ok(())
}

fn crawl_config(args: &CrawlArgs) -> miette::Result<CrawlConfig> {
    let mut config = match &args.config {
        Some(path) => {
            let text = std::fs::read_to_string(path)
                .into_diagnostic()
                .wrap_err_with(|| format!("cannot read {}", path.display()))?;
            serde_json::from_str(&text)
                .into_diagnostic()
                .wrap_err("invalid settings file")?
        }
        None => CrawlConfig::default(),
    };
    let mut urls = args.urls.clone();
    if args.list || args.list_file.is_some() {
        config.mode = CrawlMode::List;
    }
    if let Some(file) = &args.list_file {
        urls.extend(read_list(file)?);
    }
    if !urls.is_empty() {
        config.start_urls = urls;
    }
    let limits = &mut config.limits;
    if let Some(max) = args.max_urls {
        limits.max_urls = max;
    }
    if args.max_depth.is_some() {
        limits.max_depth = args.max_depth;
    }
    if let Some(timeout) = args.timeout {
        limits.timeout_seconds = timeout;
    }
    if let Some(threads) = args.threads {
        config.speed.threads = threads;
    }
    if let Some(rps) = args.rps {
        config.speed.max_requests_per_second = rps;
    }
    if let Some(agent) = args.agent {
        config.agent = match agent {
            Agent::Octopus => AgentPreset::Octopus,
            Agent::GooglebotSmartphone => AgentPreset::GooglebotSmartphone,
            Agent::GooglebotDesktop => AgentPreset::GooglebotDesktop,
            Agent::Bingbot => AgentPreset::Bingbot,
            Agent::Chrome => AgentPreset::Chrome,
        };
    }
    if args.user_agent.is_some() {
        config.user_agent.clone_from(&args.user_agent);
    }
    if let Some(robots) = args.robots {
        config.robots = match robots {
            Robots::Respect => RobotsPolicy::Respect,
            Robots::IgnoreButReport => RobotsPolicy::IgnoreButReport,
            Robots::Ignore => RobotsPolicy::Ignore,
        };
    }
    let urls = &mut config.urls;
    urls.subdomains |= args.subdomains;
    urls.start_folder_only |= args.start_folder_only;
    urls.include.extend(args.include.iter().cloned());
    urls.exclude.extend(args.exclude.iter().cloned());
    urls.strip_params.extend(args.strip_param.iter().cloned());
    urls.strip_all_params |= args.strip_all_params;
    let follow = &mut config.follow;
    follow.internal_nofollow |= args.follow_nofollow;
    if args.no_external {
        follow.check_external = false;
    }
    if args.no_resources {
        follow.images = false;
        follow.stylesheets = false;
        follow.scripts = false;
    }
    if args.no_sitemaps {
        follow.sitemaps = false;
    }
    follow.sitemap_urls.extend(args.sitemap.iter().cloned());
    apply_rendering(args, &mut config.rendering);
    config.storage.html |= args.store_html;
    config.storage.headers |= args.store_headers;
    Ok(config)
}

fn apply_rendering(args: &CrawlArgs, rendering: &mut octopuson_spider_app::Rendering) {
    if args.no_render_cache {
        rendering.cache_resources = false;
    }
    if let Some(mode) = args.render {
        rendering.mode = match mode {
            Rendering::Html => RenderMode::Html,
            Rendering::Javascript => RenderMode::Javascript,
        };
    }
    if let Some(wait) = args.render_wait_ms {
        rendering.wait_ms = wait;
    }
    if let Some(timeout) = args.render_timeout {
        rendering.timeout_seconds = timeout;
    }
    if let Some(concurrency) = args.render_concurrency {
        rendering.concurrency = concurrency;
    }
    if let Some(mut viewport) = args.render_viewport {
        viewport.mobile = args.render_mobile;
        rendering.viewport = Some(viewport);
    }
    if let Some(browser) = &args.browser {
        rendering.browser_path = Some(browser.clone());
    }
}

fn read_list(path: &Path) -> miette::Result<Vec<String>> {
    let text = std::fs::read_to_string(path)
        .into_diagnostic()
        .wrap_err_with(|| format!("cannot read {}", path.display()))?;
    Ok(text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(str::to_owned)
        .collect())
}

fn progress_line(progress: &CrawlProgress) -> String {
    let seconds = Duration::from_millis(progress.elapsed_ms)
        .as_secs_f64()
        .max(0.001);
    let rate = f64::from(progress.crawled) / seconds;
    format!(
        "  {} crawled, {} queued, {} errors, {:.1} URL/s, {}",
        progress.crawled,
        progress.queued,
        progress.errors,
        rate,
        clock(progress.elapsed_ms)
    )
}

fn clock(milliseconds: u64) -> String {
    let seconds = milliseconds / 1000;
    format!(
        "{:02}:{:02}:{:02}",
        seconds / 3600,
        seconds / 60 % 60,
        seconds % 60
    )
}

fn print_summary(meta: &CrawlMeta) -> miette::Result<()> {
    let progress = &meta.progress;
    write_text(|out| {
        writeln!(out, "crawl {} ({:?})", meta.id, meta.status)?;
        writeln!(
            out,
            "  {} URLs crawled, {} discovered, {} errors, {} sitemaps, {}",
            progress.crawled,
            progress.discovered,
            progress.errors,
            progress.sitemaps,
            clock(progress.elapsed_ms)
        )?;
        if meta.limit_reached {
            writeln!(
                out,
                "  stopped at the URL limit; raise --max-urls to crawl further"
            )?;
        }
        Ok(())
    })
}

fn list(workspace: &SpiderWorkspace, json: bool) -> miette::Result<()> {
    let crawls = workspace.crawls().into_diagnostic()?;
    if json {
        let mut stdout = io::stdout().lock();
        let result = serde_json::to_writer_pretty(&mut stdout, &crawls)
            .map_err(io::Error::from)
            .and_then(|()| writeln!(stdout));
        return ignore_broken_pipe(result);
    }
    write_text(|out| {
        if crawls.is_empty() {
            return writeln!(
                out,
                "no crawls yet; start one with `octopuson-crawler crawl <url>`"
            );
        }
        for meta in &crawls {
            writeln!(
                out,
                "{}  {:<9}  {:>7} URLs  {}",
                meta.id,
                format!("{:?}", meta.status).to_lowercase(),
                meta.progress.crawled,
                meta.start_urls.first().map_or("", String::as_str)
            )?;
        }
        Ok(())
    })
}

fn run_show(workspace: &SpiderWorkspace, args: &ShowArgs) -> miette::Result<()> {
    let id = resolve_id(workspace, &args.crawl)?;
    let tab = parse_tab(&args.tab)?;
    let reader = workspace.open(&id).into_diagnostic()?;
    if args.json {
        let mut buffer = octopuson_core::table::TableBuffer::default();
        reader.tab(tab, &mut buffer).into_diagnostic()?;
        let rows: Vec<serde_json::Map<String, serde_json::Value>> = buffer
            .rows
            .iter()
            .take(args.limit)
            .map(|row| {
                buffer
                    .columns
                    .iter()
                    .zip(row)
                    .map(|(column, cell)| {
                        (
                            column.name.clone(),
                            serde_json::to_value(cell).unwrap_or_default(),
                        )
                    })
                    .collect()
            })
            .collect();
        let mut stdout = io::stdout().lock();
        let result = serde_json::to_writer_pretty(&mut stdout, &rows)
            .map_err(io::Error::from)
            .and_then(|()| writeln!(stdout));
        return ignore_broken_pipe(result);
    }
    let mut table = TextTable::default();
    reader.tab(tab, &mut table).into_diagnostic()?;
    print_table(&table, args.limit)
}

fn run_export(workspace: &SpiderWorkspace, args: &ExportArgs) -> miette::Result<()> {
    let id = resolve_id(workspace, &args.crawl)?;
    let format = args.format.map(export_format);
    let exported = match &args.tab {
        Some(tab) => vec![
            workspace
                .export(&id, parse_tab(tab)?, &args.out, format)
                .into_diagnostic()?,
        ],
        None => workspace
            .export_all(&id, &args.out, format.unwrap_or(ExportFormat::Csv))
            .into_diagnostic()?,
    };
    write_text(|out| {
        for tab in &exported {
            writeln!(out, "{:>8} rows  {}", tab.rows, tab.path.display())?;
        }
        Ok(())
    })
}

fn run_query(workspace: &SpiderWorkspace, args: &QueryArgs) -> miette::Result<()> {
    let id = resolve_id(workspace, &args.crawl)?;
    let reader = workspace.open(&id).into_diagnostic()?;
    if let Some(path) = &args.out {
        let format = ExportFormat::from_path(path).ok_or_else(|| {
            miette!(
                "cannot tell the format of {} (use .csv, .xlsx or .json)",
                path.display()
            )
        })?;
        let file = std::fs::File::create(path).into_diagnostic()?;
        let mut sink: Box<dyn TableSink> = format.writer(file);
        let rows = reader.query(&args.sql, sink.as_mut()).into_diagnostic()?;
        return write_text(|out| writeln!(out, "{rows:>8} rows  {}", path.display()));
    }
    let mut table = TextTable::default();
    reader.query(&args.sql, &mut table).into_diagnostic()?;
    print_table(&table, args.limit)
}

fn print_table(table: &TextTable, limit: usize) -> miette::Result<()> {
    write_text(|out| {
        table.write(out, limit)?;
        if table.len() > limit {
            writeln!(
                out,
                "… {} more rows (use --limit or export)",
                table.len() - limit
            )?;
        }
        Ok(())
    })
}

/// `latest` names the newest crawl.
fn resolve_id(workspace: &SpiderWorkspace, crawl: &str) -> miette::Result<String> {
    if crawl != "latest" {
        return Ok(crawl.to_owned());
    }
    workspace
        .crawls()
        .into_diagnostic()?
        .into_iter()
        .next()
        .map(|meta| meta.id)
        .ok_or_else(|| miette!("no crawls yet"))
}

fn parse_tab(name: &str) -> miette::Result<CrawlTab> {
    CrawlTab::from_name(name).ok_or_else(|| {
        let names: Vec<&str> = CrawlTab::ALL.iter().map(|tab| tab.name()).collect();
        miette!("unknown tab `{name}`; tabs: {}", names.join(", "))
    })
}

fn export_format(format: Format) -> ExportFormat {
    match format {
        Format::Csv => ExportFormat::Csv,
        Format::Xlsx => ExportFormat::Xlsx,
        Format::Json => ExportFormat::Json,
    }
}

/// Runs `render` against a locked stdout. A closed pipe is not an error.
fn write_text(render: impl FnOnce(&mut dyn Write) -> io::Result<()>) -> miette::Result<()> {
    let mut stdout = io::stdout().lock();
    ignore_broken_pipe(render(&mut stdout).and_then(|()| stdout.flush()))
}

fn ignore_broken_pipe(result: io::Result<()>) -> miette::Result<()> {
    match result {
        Err(error) if error.kind() == io::ErrorKind::BrokenPipe => Ok(()),
        other => other.into_diagnostic(),
    }
}

/// Progress output is best effort; a closed stderr must not abort a crawl.
fn write_stderr(text: &str) {
    let mut stderr = io::stderr().lock();
    let _ = stderr
        .write_all(text.as_bytes())
        .and_then(|()| stderr.flush());
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::*;

    fn config(argv: &[&str]) -> CrawlConfig {
        let cli = Cli::parse_from([&["octopuson-crawler", "crawl"], argv].concat());
        let Command::Crawl(args) = cli.command else {
            panic!("not a crawl");
        };
        crawl_config(&args).unwrap()
    }

    #[test]
    fn flags_override_defaults() {
        let crawl = config(&[
            "https://example.com",
            "--no-external",
            "--max-depth",
            "3",
            "--robots",
            "ignore",
            "--render",
            "javascript",
            "--render-wait-ms",
            "1500",
            "--render-concurrency",
            "1",
        ]);
        assert_eq!(crawl.start_urls, ["https://example.com"]);
        assert!(!crawl.follow.check_external);
        assert_eq!(crawl.limits.max_depth, Some(3));
        assert_eq!(crawl.robots, RobotsPolicy::Ignore);
        assert_eq!(crawl.rendering.mode, RenderMode::Javascript);
        assert_eq!(crawl.rendering.wait_ms, 1500);
        assert_eq!(crawl.rendering.concurrency, 1);
        assert_eq!(crawl.mode, CrawlMode::Spider);
        let list = config(&["--list", "https://a.com/x"]);
        assert_eq!(list.start_urls, ["https://a.com/x"]);
        assert_eq!(list.mode, CrawlMode::List);
    }

    #[test]
    fn viewport_flags_preserve_request_identity_and_support_explicit_layouts() {
        let mobile = config(&[
            "https://example.com",
            "--render",
            "javascript",
            "--render-viewport",
            "390x844",
            "--render-mobile",
            "--agent",
            "chrome",
        ]);
        assert_eq!(mobile.agent, AgentPreset::Chrome);
        assert_eq!(
            mobile.rendering.viewport,
            Some(octopuson_spider_app::Viewport {
                width: 390,
                height: 844,
                mobile: true
            })
        );
        let desktop = config(&[
            "https://example.com",
            "--render-viewport",
            "768X1024",
            "--agent",
            "googlebot-smartphone",
        ]);
        assert_eq!(desktop.agent, AgentPreset::GooglebotSmartphone);
        assert_eq!(
            desktop.rendering.viewport,
            Some(octopuson_spider_app::Viewport {
                width: 768,
                height: 1024,
                mobile: false
            })
        );
        assert_eq!(config(&["https://example.com"]).rendering.viewport, None);
    }

    #[test]
    fn progress_is_readable() {
        let progress = CrawlProgress {
            crawled: 50,
            queued: 7,
            errors: 1,
            elapsed_ms: 10_000,
            ..CrawlProgress::default()
        };
        assert_eq!(
            progress_line(&progress),
            "  50 crawled, 7 queued, 1 errors, 5.0 URL/s, 00:00:10"
        );
    }
}
