//! `octopuson-index` — OctopusOn Index Checker command-line interface.

#![allow(clippy::print_stdout, clippy::print_stderr)]

mod account;

use std::path::PathBuf;
use std::sync::Arc;

use clap::{Parser, Subcommand};
use miette::IntoDiagnostic;
use octopuson_finder::{
    ApiConfig, FinderEvent, FinderPageRequest, FinderWorkspace, IndexEngine, ScanConfig,
    ScanControl, ScanMeta,
};

use crate::account::{AccountCommand, ProvidersCommand};

#[derive(Debug, Parser)]
#[command(
    name = "octopuson-index",
    about = "OctopusOn Index Checker — discover indexed URLs across Google, Yandex, Bing, DuckDuckGo, Wayback and APIs, and detect 404, 4xx, 5xx, Soft 404 and thin 200 pages."
)]
struct Cli {
    /// Custom data directory.
    #[arg(long, env = "OCTOPUSSEO_FINDER_DATA_DIR", global = true)]
    data_dir: Option<PathBuf>,

    /// Emit JSON output.
    #[arg(long, global = true)]
    json: bool,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Discover indexed URLs of a domain and verify their HTTP status & content.
    Scan {
        /// Domain or start URL (e.g. `example.com`).
        domain: String,
        /// Comma-separated free search engines (`google,yandex,bing,duckduckgo`).
        #[arg(long, default_value = "google,yandex,bing,duckduckgo")]
        engines: String,
        /// Disable free Internet Archive Wayback CDX discovery.
        #[arg(long)]
        no_wayback: bool,
        /// Disable free Common Crawl CDX discovery.
        #[arg(long)]
        no_commoncrawl: bool,
        /// Disable XML Sitemap discovery (`robots.txt` + `/sitemap.xml`).
        #[arg(long)]
        no_sitemaps: bool,
        /// Disable automatic subpath query expansion.
        #[arg(long)]
        no_expand: bool,
        /// Pages per search query (`1..=10`).
        #[arg(long, default_value_t = 3)]
        pages: u16,
        /// Minimum visible word count required for a 200 OK page.
        #[arg(long, default_value_t = 60)]
        min_words: u32,
        /// Disable reporting non-404/410 4xx responses.
        #[arg(long)]
        no_4xx: bool,
        /// Disable reporting 5xx server errors.
        #[arg(long)]
        no_5xx: bool,
        /// Disable reporting 200 OK thin-content / Soft-404 pages.
        #[arg(long)]
        no_thin_200: bool,
        /// Extra seed subpaths or keywords.
        #[arg(long)]
        seed_query: Vec<String>,
        /// Optional path to a .txt or .csv file containing URLs exported from Google Search Console / Yandex Webmaster.
        #[arg(long)]
        urls_file: Option<PathBuf>,
        /// Google Custom Search API key.
        #[arg(long, default_value = "")]
        google_api_key: String,
        /// Google Programmable Search Engine ID (`cx`).
        #[arg(long, default_value = "")]
        google_cx: String,
        /// Yandex XML/Search API key.
        #[arg(long, default_value = "")]
        yandex_api_key: String,
        /// Yandex XML user or folder ID.
        #[arg(long, default_value = "")]
        yandex_user: String,
        /// Serper.dev API key. Without one, Serper runs on OctopusOn SEO credits when an account
        /// is connected (`octopuson-index account login`); see `octopuson-index providers mode`.
        #[arg(long, default_value = "")]
        serper_api_key: String,
        /// SerpApi.com API key. Without one, SerpApi runs on OctopusOn SEO credits when an
        /// account is connected.
        #[arg(long, default_value = "")]
        serpapi_key: String,
    },
    /// The OctopusOn SEO account that pays for Serper and SerpApi with credits.
    Account {
        #[command(subcommand)]
        command: AccountCommand,
    },
    /// The paid search APIs and how they are paid (own key or OctopusOn SEO credits).
    Providers {
        #[command(subcommand)]
        command: ProvidersCommand,
    },
    /// List saved Index Checker scans.
    List,
    /// Show results of a saved scan (`latest` or scan ID).
    Show {
        /// Scan ID or `latest`.
        scan: String,
        /// Category filter (`issues`, `error_404`, `error_4xx`, `error_5xx`, `thin_soft_404`, `redirected`, `noindex`, `sitemap_issues`, `orphan_indexed`, `all`).
        #[arg(long, default_value = "issues")]
        category: String,
        /// Engine filter (`google`, `yandex`, `bing`, `duckduckgo`, `wayback`, `commoncrawl`, `sitemap`, `multi`).
        #[arg(long, default_value = "")]
        engine: String,
        /// Substring search on URL, SERP title or live title.
        #[arg(long, default_value = "")]
        search: String,
        /// Filter pages with word count <= `max_words`.
        #[arg(long)]
        max_words: Option<u32>,
        /// Sort order (`priority`, `words_asc`, `words_desc`, `time_desc`, `status_desc`, `hops_desc`, `engines_desc`, `url_asc`).
        #[arg(long, default_value = "priority")]
        sort_by: String,
        /// Maximum rows to display.
        #[arg(long, default_value_t = 100)]
        limit: u32,
    },
    /// Show top-level directory / path cluster health breakdown of a saved scan.
    Clusters {
        /// Scan ID or `latest`.
        #[arg(default_value = "latest")]
        scan: String,
    },
    /// Export scan results to `.xlsx`, `.csv` or `.json`.
    Export {
        /// Scan ID or `latest`.
        scan: String,
        /// Destination file path (`.xlsx`, `.csv`, `.json`).
        #[arg(short, long)]
        out: PathBuf,
        /// Category filter (`issues`, `error_404`, `error_4xx`, `error_5xx`, `thin_soft_404`, `all`).
        #[arg(long, default_value = "issues")]
        category: String,
    },
    /// Generate 301 redirect, 410 Gone, CMS CSV, clean XML sitemap, redirect chains CSV, `IndexNow` JSON, Markdown report or removal rules.
    Redirects {
        /// Scan ID or `latest`.
        scan: String,
        /// Output format (`nginx`, `nginx_cluster`, `apache`, `apache_410`, `cloudflare`, `cloudflare_csv`, `vercel`, `nextjs`, `caddy`, `iis`, `nginx_410`, `wordpress_csv`, `shopify_csv`, `removal_list`, `clean_sitemap`, `redirect_chains_csv`, `indexnow_json`, `markdown_report`).
        #[arg(long, default_value = "nginx")]
        server: String,
        /// Optional output file path; prints to stdout if omitted.
        #[arg(short, long)]
        out: Option<PathBuf>,
    },
    /// Re-verify broken URLs (or a single URL) in a saved scan and update it in place.
    Reverify {
        /// Scan ID or `latest`.
        scan: String,
        /// Optional single URL to re-verify; if omitted, all issue URLs in the scan are re-verified.
        #[arg(long)]
        url: Option<String>,
    },
    /// Compare two saved scans (`base` vs `target`) to report resolved, newly broken, and persistent 404s.
    Compare {
        /// Baseline (older) scan ID.
        base: String,
        /// Target (newer) scan ID or `latest`.
        #[arg(default_value = "latest")]
        target: String,
    },
    /// Update suggested 301 redirect target for a single URL or wildcard prefix (e.g. `/kampanya/*`).
    SetTarget {
        /// Scan ID or `latest`.
        scan: String,
        /// Exact URL or wildcard prefix ending with `*` (e.g. `/kampanya/*`).
        url_or_prefix: String,
        /// New 301 redirect destination URL.
        target: String,
    },
    /// List ranked smart 301 redirect candidates (matching live `200 OK` pages) for a broken URL.
    Candidates {
        /// Scan ID or `latest`.
        scan: String,
        /// Broken URL to match against healthy `200 OK` pages.
        url: String,
    },
}

#[allow(clippy::too_many_lines)]
fn main() -> miette::Result<()> {
    let cli = Cli::parse();
    let workspace = FinderWorkspace::resolve(cli.data_dir).into_diagnostic()?;

    match cli.command {
        Command::Scan {
            domain,
            engines,
            no_wayback,
            no_commoncrawl,
            no_sitemaps,
            no_expand,
            pages,
            min_words,
            no_4xx,
            no_5xx,
            no_thin_200,
            seed_query,
            urls_file,
            google_api_key,
            google_cx,
            yandex_api_key,
            yandex_user,
            serper_api_key,
            serpapi_key,
        } => {
            let parsed_engines: Vec<IndexEngine> = engines
                .split(',')
                .filter(|s| !s.trim().is_empty())
                .filter_map(|s| s.parse::<IndexEngine>().ok())
                .collect();
            let extra_urls = match urls_file {
                Some(path) => vec![std::fs::read_to_string(&path).into_diagnostic()?],
                None => Vec::new(),
            };
            let config = ScanConfig {
                domain,
                engines: parsed_engines,
                use_wayback: !no_wayback,
                use_commoncrawl: !no_commoncrawl,
                use_sitemaps: !no_sitemaps,
                expand_subpaths: !no_expand,
                pages_per_query: pages,
                seed_queries: seed_query,
                extra_urls,
                detect_all_4xx: !no_4xx,
                detect_5xx: !no_5xx,
                detect_thin_200: !no_thin_200,
                min_word_count: min_words,
                api: ApiConfig {
                    google_api_key,
                    google_cx,
                    yandex_api_key,
                    yandex_user,
                    serper_api_key,
                    serpapi_key,
                },
                ..ScanConfig::default()
            };
            run_scan_command(&workspace, config, cli.json)?;
        }
        Command::Account { command } => account::run_account(&workspace, &command, cli.json)?,
        Command::Providers { command } => {
            account::run_providers(&workspace, &command, cli.json)?;
        }
        Command::List => run_list_command(&workspace, cli.json)?,
        Command::Show {
            scan,
            category,
            engine,
            search,
            max_words,
            sort_by,
            limit,
        } => run_show_command(
            &workspace,
            &scan,
            &FinderPageRequest {
                category,
                engine,
                search,
                max_words,
                sort_by,
                offset: 0,
                limit,
            },
            cli.json,
        )?,
        Command::Clusters { scan } => run_clusters_command(&workspace, &scan, cli.json)?,
        Command::Export {
            scan,
            out,
            category,
        } => {
            let written = workspace
                .store()
                .export_table(
                    &scan,
                    &FinderPageRequest {
                        category,
                        limit: 500,
                        ..FinderPageRequest::default()
                    },
                    &out,
                    None,
                )
                .into_diagnostic()?;
            println!("Exported {written} rows to {}", out.display());
        }
        Command::Redirects { scan, server, out } => {
            let rules = workspace
                .store()
                .generate_redirect_rules(&scan, &server)
                .into_diagnostic()?;
            if let Some(dest) = out {
                std::fs::write(&dest, &rules).into_diagnostic()?;
                println!("Wrote redirect rules ({server}) to {}", dest.display());
            } else {
                println!("{rules}");
            }
        }
        Command::Reverify { scan, url } => {
            let overview = workspace
                .reverify_scan(&scan, url.as_deref())
                .into_diagnostic()?;
            if cli.json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&overview).into_diagnostic()?
                );
            } else {
                println!(
                    "Re-verified scan `{}`: health={}% — {} 404/410, {} 4xx, {} 5xx, {} soft-404, {} thin-200",
                    overview.meta.id,
                    overview.health_score,
                    overview.meta.count_404,
                    overview.meta.count_4xx,
                    overview.meta.count_5xx,
                    overview.meta.count_soft_404,
                    overview.meta.count_thin_200
                );
            }
        }
        Command::Compare { base, target } => {
            let diff = workspace.compare_scans(&base, &target).into_diagnostic()?;
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&diff).into_diagnostic()?);
            } else {
                println!(
                    "Comparison `{}` -> `{}` ({}): resolved={}, new_issues={}, persistent_issues={}",
                    diff.base_scan_id,
                    diff.target_scan_id,
                    diff.domain,
                    diff.resolved_count,
                    diff.new_issue_count,
                    diff.persistent_issue_count
                );
                for r in diff.rows.iter().take(50) {
                    println!(
                        "  [{:?}]\tHTTP {} -> {}\t{}\t-> {}",
                        r.diff, r.base_status, r.target_status, r.url, r.suggested_target
                    );
                }
            }
        }
        Command::SetTarget {
            scan,
            url_or_prefix,
            target,
        } => {
            workspace
                .store()
                .update_suggested_target(&scan, &url_or_prefix, &target)
                .into_diagnostic()?;
            println!("Updated 301 target for `{url_or_prefix}` -> `{target}`");
        }
        Command::Candidates { scan, url } => {
            let candidates = workspace
                .store()
                .redirect_candidates(&scan, &url)
                .into_diagnostic()?;
            if cli.json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&candidates).into_diagnostic()?
                );
            } else {
                for c in candidates {
                    println!(
                        "{}%\t[{}]\t{}\t({})",
                        c.similarity, c.kind, c.target_url, c.title
                    );
                }
            }
        }
    }
    Ok(())
}

fn run_clusters_command(
    workspace: &FinderWorkspace,
    scan: &str,
    json_output: bool,
) -> miette::Result<()> {
    let overview = workspace.store().overview(scan).into_diagnostic()?;
    if json_output {
        println!(
            "{}",
            serde_json::to_string_pretty(&overview.clusters).into_diagnostic()?
        );
    } else {
        for c in overview.clusters {
            println!(
                "{}/*\ttotal={}\tissues={}\t404={}\tsoft/thin={}\tavg_words={}\thint={}",
                c.prefix,
                c.total_urls,
                c.issue_urls,
                c.count_404,
                c.count_soft_thin,
                c.avg_word_count,
                c.action_hint
            );
        }
    }
    Ok(())
}

fn run_list_command(workspace: &FinderWorkspace, json_output: bool) -> miette::Result<()> {
    let scans = workspace.store().list_scans().into_diagnostic()?;
    if json_output {
        println!(
            "{}",
            serde_json::to_string_pretty(&scans).into_diagnostic()?
        );
    } else {
        for s in scans {
            println!(
                "{}\t{}\t{}\tverified={}\t404={}\tthin/soft404={}",
                s.id,
                s.domain,
                s.status,
                s.verified_urls,
                s.count_404,
                s.count_soft_404 + s.count_thin_200
            );
        }
    }
    Ok(())
}

fn run_show_command(
    workspace: &FinderWorkspace,
    scan: &str,
    req: &FinderPageRequest,
    json_output: bool,
) -> miette::Result<()> {
    let page = workspace.store().query_page(scan, req).into_diagnostic()?;
    if json_output {
        println!("{}", serde_json::to_string_pretty(&page).into_diagnostic()?);
    } else {
        println!("Total matching URLs: {}", page.total);
        for row in page.rows {
            println!(
                "[{}] HTTP {} -> {}\twords={}\tengines={}\t{}",
                row.category.as_str(),
                row.initial_status,
                row.final_status,
                row.word_count,
                row.engines,
                row.url
            );
        }
    }
    Ok(())
}

fn run_scan_command(
    workspace: &FinderWorkspace,
    config: ScanConfig,
    json_output: bool,
) -> miette::Result<()> {
    let control = Arc::new(ScanControl::new());
    let ctrl_clone = Arc::clone(&control);
    let _ = ctrlc::set_handler(move || {
        ctrl_clone.stop();
    });
    let meta = workspace
        .run_scan(config, &control, |event| {
            if !json_output && let FinderEvent::Progress(p) = event {
                eprintln!(
                    "[{}] discovered={} verified={} 404={} thin/soft404={}",
                    p.current_item,
                    p.discovered_urls,
                    p.verified_urls,
                    p.found_404,
                    p.found_thin_soft404
                );
            }
        })
        .into_diagnostic()?;
    if json_output {
        println!("{}", serde_json::to_string_pretty(&meta).into_diagnostic()?);
    } else {
        println!(
            "Scan `{}` ({}) finished: {} indexed URLs verified — {} 404/410, {} 4xx, {} 5xx, {} soft-404, {} thin-200",
            meta.id,
            meta.domain,
            meta.verified_urls,
            meta.count_404,
            meta.count_4xx,
            meta.count_5xx,
            meta.count_soft_404,
            meta.count_thin_200
        );
        print_credits(&meta);
    }
    Ok(())
}

/// What the sources on OctopusOn SEO credits used, if any.
fn print_credits(meta: &ScanMeta) {
    let credits = &meta.credits;
    if !credits.sources.is_empty() {
        #[allow(clippy::cast_precision_loss)]
        let usd = credits.charged_micros as f64 / 1_000_000.0;
        println!(
            "OctopusOn SEO credits ({}): {} requests, ${usd:.4} charged",
            credits.sources.join(", "),
            credits.requests
        );
    }
    if let Some(stopped) = &credits.stopped {
        eprintln!("credits stopped: {stopped}");
    }
}
