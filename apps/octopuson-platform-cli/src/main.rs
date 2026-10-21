//! `octopuson-platform` — the OctopusOn GEO-SEO Platform from the command line: any of its
//! commands with JSON arguments, and its MCP server for AI assistants over standard input and
//! output. It works on the desktop program's data folder (or `--data-dir`).

use std::io::{self, Write as _};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use clap::{Parser, Subcommand};
use miette::{IntoDiagnostic, miette};
use octopuson_platform_app::jobs::NoEvents;
use octopuson_platform_app::router::Opener;
use octopuson_platform_app::{JobStatus, Platform, Workspace};
use serde_json::Value;

/// OctopusOn GEO-SEO Platform: SEO and AI-visibility data of your projects.
#[derive(Debug, Parser)]
#[command(name = "octopuson-platform", version, about, long_about = None)]
struct Cli {
    /// Data folder (defaults to the desktop program's).
    #[arg(long, global = true, env = octopuson_platform_app::DATA_DIR_VARIABLE, value_name = "DIR")]
    data_dir: Option<PathBuf>,

    /// Do not print progress.
    #[arg(short, long, global = true)]
    quiet: bool,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Serve the MCP tools over standard input and output (for Claude, ChatGPT, Cursor …).
    Mcp,
    /// Call a platform command with JSON arguments and print its JSON answer. A command that
    /// starts a job (a rank check, an audit, a sync …) waits for the job.
    Call {
        /// Command name (`list_projects`, `rank_report`, …); `commands` lists them.
        command: String,
        /// Arguments as JSON (`{"project_id": 1}`).
        #[arg(default_value = "{}")]
        arguments: String,
        /// Return the job id at once instead of waiting.
        #[arg(long)]
        no_wait: bool,
    },
    /// List every command.
    Commands,
    /// List the projects.
    Projects,
}

/// Opens an address in the default browser (account and Google sign-ins).
fn open_browser(url: &str) -> bool {
    let status = if cfg!(target_os = "macos") {
        std::process::Command::new("open").arg(url).status()
    } else if cfg!(windows) {
        std::process::Command::new("rundll32")
            .args(["url.dll,FileProtocolHandler", url])
            .status()
    } else {
        std::process::Command::new("xdg-open").arg(url).status()
    };
    status.is_ok_and(|status| status.success())
}

fn platform(data_dir: Option<PathBuf>) -> miette::Result<Arc<Platform>> {
    let workspace = Workspace::resolve(data_dir).map_err(|error| miette!("{error}"))?;
    let open: Opener = Arc::new(open_browser);
    Ok(Arc::new(Platform::new(workspace, Arc::new(NoEvents), open)))
}

fn print(value: &Value) -> miette::Result<()> {
    let mut out = io::stdout().lock();
    serde_json::to_writer_pretty(&mut out, value).into_diagnostic()?;
    writeln!(out).into_diagnostic()
}

/// Waits for a job, printing its progress; its last state.
fn wait(platform: &Platform, id: &str, quiet: bool) -> Option<octopuson_platform_app::Job> {
    let mut shown = String::new();
    loop {
        let job = platform.jobs().get(id)?;
        if job.status != JobStatus::Running {
            if !quiet && !shown.is_empty() {
                let _ = writeln!(io::stderr());
            }
            return Some(job);
        }
        let line = format!("{} {}/{} {}", job.kind, job.done, job.total, job.step);
        if !quiet && line != shown {
            let _ = write!(io::stderr(), "\r{line:<70}");
            shown = line;
        }
        std::thread::sleep(Duration::from_millis(250));
    }
}

fn main() -> miette::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Mcp => {
            let platform = platform(cli.data_dir)?;
            octopuson_platform_app::mcp::stdio(&platform).into_diagnostic()
        }
        Command::Call {
            command,
            arguments,
            no_wait,
        } => {
            let arguments: Value = serde_json::from_str(&arguments)
                .map_err(|error| miette!("the arguments are not JSON: {error}"))?;
            let platform = platform(cli.data_dir)?;
            let answer = platform
                .call(&command, arguments)
                .map_err(|error| miette!("{} ({:?})", error.message, error.code))?;
            let job = answer
                .as_str()
                .filter(|id| !no_wait && id.starts_with("job-"))
                .and_then(|id| wait(&platform, id, cli.quiet));
            match job {
                Some(job) => {
                    print(&serde_json::to_value(&job).into_diagnostic()?)?;
                    if matches!(job.status, JobStatus::Failed) {
                        return Err(miette!(
                            "the job failed: {}",
                            job.error.map(|error| error.message).unwrap_or_default()
                        ));
                    }
                    Ok(())
                }
                None => print(&answer),
            }
        }
        Command::Commands => {
            let router = octopuson_platform_app::modules::router();
            let mut out = io::stdout().lock();
            for name in router.names() {
                writeln!(out, "{name}").into_diagnostic()?;
            }
            Ok(())
        }
        Command::Projects => {
            let platform = platform(cli.data_dir)?;
            let projects = platform
                .call("list_projects", Value::Null)
                .map_err(|error| miette!("{}", error.message))?;
            let mut out = io::stdout().lock();
            for project in projects.as_array().into_iter().flatten() {
                writeln!(
                    out,
                    "{}\t{}\t{}\t{}-{}",
                    project["id"],
                    project["name"].as_str().unwrap_or_default(),
                    project["domain"].as_str().unwrap_or_default(),
                    project["country"].as_str().unwrap_or_default(),
                    project["language"].as_str().unwrap_or_default(),
                )
                .into_diagnostic()?;
            }
            Ok(())
        }
    }
}
