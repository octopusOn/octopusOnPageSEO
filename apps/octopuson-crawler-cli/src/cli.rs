use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};

/// OctopusOn SEO Crawler — crawl a website the way search engines do and audit it.
#[derive(Debug, Parser)]
#[command(name = "octopuson-crawler", version, about, long_about = None)]
pub(crate) struct Cli {
    /// Data directory (defaults to the platform application data directory).
    #[arg(
        long,
        global = true,
        env = "OCTOPUSSEO_SPIDER_DATA_DIR",
        value_name = "DIR"
    )]
    pub(crate) data_dir: Option<PathBuf>,

    /// Do not print progress.
    #[arg(short, long, global = true)]
    pub(crate) quiet: bool,

    #[command(subcommand)]
    pub(crate) command: Command,
}

#[derive(Debug, Subcommand)]
pub(crate) enum Command {
    /// Crawl a site (or a list of URLs) into a new saved crawl.
    Crawl(Box<CrawlArgs>),
    /// List saved crawls, newest first.
    Crawls {
        /// Print as JSON.
        #[arg(long)]
        json: bool,
    },
    /// Print a tab of a crawl.
    Show(ShowArgs),
    /// Export one tab, or every tab into a directory.
    Export(ExportArgs),
    /// Run a read-only SQL query over a crawl's tables and `tab_*` views.
    Query(QueryArgs),
    /// Delete a crawl.
    Delete {
        /// Crawl id (or `latest`).
        crawl: String,
    },
}

/// Settings of a new crawl; flags override a `--config` file.
#[derive(Debug, Args)]
#[allow(clippy::struct_excessive_bools)]
pub(crate) struct CrawlArgs {
    /// Start URLs (spider mode) or URLs to fetch (`--list`).
    #[arg(value_name = "URL")]
    pub(crate) urls: Vec<String>,

    /// List mode: fetch only the given URLs and the redirects they lead to.
    #[arg(long)]
    pub(crate) list: bool,

    /// Read URLs from FILE, one per line (implies `--list`).
    #[arg(long, value_name = "FILE")]
    pub(crate) list_file: Option<PathBuf>,

    /// JSON settings file (the format `crawl.json` stores under `config`).
    #[arg(long, value_name = "FILE")]
    pub(crate) config: Option<PathBuf>,

    /// Stop after fetching this many URLs.
    #[arg(long, value_name = "N")]
    pub(crate) max_urls: Option<u32>,

    /// Follow links at most this many clicks from the start URL.
    #[arg(long, value_name = "N")]
    pub(crate) max_depth: Option<u32>,

    /// Parallel requests.
    #[arg(long, value_name = "N")]
    pub(crate) threads: Option<usize>,

    /// Requests per second at most.
    #[arg(long, value_name = "N")]
    pub(crate) rps: Option<f64>,

    /// Seconds before a request is abandoned.
    #[arg(long, value_name = "SECONDS")]
    pub(crate) timeout: Option<u32>,

    /// Crawler to identify as (its robots.txt rules apply).
    #[arg(long, value_enum)]
    pub(crate) agent: Option<Agent>,

    /// Custom User-Agent header.
    #[arg(long, value_name = "UA")]
    pub(crate) user_agent: Option<String>,

    /// How robots.txt is honored.
    #[arg(long, value_enum)]
    pub(crate) robots: Option<Robots>,

    /// Treat subdomains of the start host as internal.
    #[arg(long)]
    pub(crate) subdomains: bool,

    /// Crawl only inside the start URL's folder.
    #[arg(long)]
    pub(crate) start_folder_only: bool,

    /// Crawl only URLs matching one of these regexes.
    #[arg(long, value_name = "REGEX")]
    pub(crate) include: Vec<String>,

    /// Skip URLs matching one of these regexes.
    #[arg(long, value_name = "REGEX")]
    pub(crate) exclude: Vec<String>,

    /// Remove this query parameter from every URL.
    #[arg(long, value_name = "NAME")]
    pub(crate) strip_param: Vec<String>,

    /// Remove every query string.
    #[arg(long)]
    pub(crate) strip_all_params: bool,

    /// Follow internal `rel="nofollow"` links.
    #[arg(long)]
    pub(crate) follow_nofollow: bool,

    /// Do not check external links.
    #[arg(long)]
    pub(crate) no_external: bool,

    /// Do not check images, stylesheets and scripts.
    #[arg(long)]
    pub(crate) no_resources: bool,

    /// Do not read sitemaps.
    #[arg(long)]
    pub(crate) no_sitemaps: bool,

    /// Read this sitemap too.
    #[arg(long, value_name = "URL")]
    pub(crate) sitemap: Vec<String>,

    /// Analyze server HTML or execute JavaScript using an installed browser.
    #[arg(long, value_enum)]
    pub(crate) render: Option<Rendering>,

    /// Minimum wait after `DOMContentLoaded`, in milliseconds.
    #[arg(long)]
    pub(crate) render_wait_ms: Option<u32>,

    /// Active rendering timeout per page; excludes waiting for a browser slot.
    #[arg(long)]
    pub(crate) render_timeout: Option<u32>,

    /// Concurrent browser pages (1–8).
    #[arg(long)]
    pub(crate) render_concurrency: Option<usize>,

    /// Disable crawl-local reuse of public immutable JavaScript and CSS.
    #[arg(long)]
    pub(crate) no_render_cache: bool,

    /// Browser layout in CSS pixels (240–3840 per dimension); desktop layout by default.
    #[arg(long, value_name = "WIDTHxHEIGHT", value_parser = parse_viewport)]
    pub(crate) render_viewport: Option<octopuson_spider_app::Viewport>,

    /// Use mobile layout with --render-viewport; does not change the User-Agent.
    #[arg(long, requires = "render_viewport")]
    pub(crate) render_mobile: bool,

    /// Chrome/Chromium/Edge executable (otherwise auto-detected).
    #[arg(long)]
    pub(crate) browser: Option<String>,

    /// Keep the HTML of internal pages.
    #[arg(long)]
    pub(crate) store_html: bool,

    /// Keep every response header.
    #[arg(long)]
    pub(crate) store_headers: bool,

    /// Export every tab into DIR when the crawl ends.
    #[arg(long, value_name = "DIR")]
    pub(crate) export: Option<PathBuf>,

    /// Format of `--export`.
    #[arg(long, value_enum, default_value_t = Format::Csv)]
    pub(crate) format: Format,
}

#[derive(Debug, Args)]
pub(crate) struct ShowArgs {
    /// Crawl id (or `latest`).
    pub(crate) crawl: String,

    /// Tab to print.
    #[arg(long, default_value = "internal")]
    pub(crate) tab: String,

    /// Rows to print at most.
    #[arg(long, default_value_t = 50)]
    pub(crate) limit: usize,

    /// Print as JSON.
    #[arg(long)]
    pub(crate) json: bool,
}

#[derive(Debug, Args)]
pub(crate) struct ExportArgs {
    /// Crawl id (or `latest`).
    pub(crate) crawl: String,

    /// Tab to export; without it every tab is written into `--out` as a directory.
    #[arg(long)]
    pub(crate) tab: Option<String>,

    /// Output file (one tab) or directory (every tab).
    #[arg(long, short)]
    pub(crate) out: PathBuf,

    /// File format (default: from the extension, CSV for directories).
    #[arg(long, value_enum)]
    pub(crate) format: Option<Format>,
}

#[derive(Debug, Args)]
pub(crate) struct QueryArgs {
    /// Crawl id (or `latest`).
    pub(crate) crawl: String,

    /// SQL, e.g. `SELECT address, title FROM tab_internal WHERE title_length > 60`.
    pub(crate) sql: String,

    /// Write the result to a file instead of printing it.
    #[arg(long, short)]
    pub(crate) out: Option<PathBuf>,

    /// Rows to print at most.
    #[arg(long, default_value_t = 50)]
    pub(crate) limit: usize,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub(crate) enum Rendering {
    Html,
    Javascript,
}

fn parse_viewport(raw: &str) -> Result<octopuson_spider_app::Viewport, String> {
    let invalid =
        || "use WIDTHxHEIGHT, with each dimension between 240 and 3840 CSS pixels".to_owned();
    let (width, height) = raw.split_once(['x', 'X']).ok_or_else(invalid)?;
    let viewport = octopuson_spider_app::Viewport {
        width: width.parse().map_err(|_| invalid())?,
        height: height.parse().map_err(|_| invalid())?,
        mobile: false,
    };
    if viewport.is_valid() {
        Ok(viewport)
    } else {
        Err(invalid())
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub(crate) enum Agent {
    Octopus,
    GooglebotSmartphone,
    GooglebotDesktop,
    Bingbot,
    Chrome,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub(crate) enum Robots {
    Respect,
    IgnoreButReport,
    Ignore,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(crate) enum Format {
    Csv,
    Xlsx,
    Json,
}

#[cfg(test)]
mod tests {
    use clap::CommandFactory;

    use super::*;

    #[test]
    fn cli_definition_is_valid() {
        Cli::command().debug_assert();
    }

    #[test]
    fn parses_a_crawl() {
        let cli = Cli::parse_from([
            "octopuson-crawler",
            "crawl",
            "example.com",
            "--max-urls",
            "100",
            "--agent",
            "googlebot-smartphone",
            "--exclude",
            r"\?sort=",
        ]);
        let Command::Crawl(args) = cli.command else {
            panic!("not a crawl");
        };
        assert_eq!(args.urls, ["example.com"]);
        assert_eq!(args.max_urls, Some(100));
        assert!(matches!(args.agent, Some(Agent::GooglebotSmartphone)));
    }

    #[test]
    fn rejects_invalid_viewports_and_an_ignored_mobile_flag() {
        for size in [
            "390",
            "390x",
            "0x844",
            "239x800",
            "390x3841",
            "390.5x844",
            "390x844x2",
        ] {
            assert!(
                Cli::try_parse_from([
                    "octopuson-crawler",
                    "crawl",
                    "example.com",
                    "--render-viewport",
                    size
                ])
                .is_err(),
                "{size}"
            );
        }
        assert!(
            Cli::try_parse_from([
                "octopuson-crawler",
                "crawl",
                "example.com",
                "--render-mobile"
            ])
            .is_err()
        );
    }
}
