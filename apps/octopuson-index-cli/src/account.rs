//! `octopuson-index account …` and `octopuson-index providers …`: the OctopusOn SEO account that pays for
//! Serper and SerpApi with credits, and how each paid search API is paid.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use clap::Subcommand;
use miette::{IntoDiagnostic, miette};
use octopuson_finder::{ApiConfig, FinderWorkspace, SourceMode, Via};

#[derive(Debug, Subcommand)]
pub(crate) enum AccountCommand {
    /// Connect this computer: the account site opens in your browser and comes straight back
    /// when you are signed in there. Every OctopusOn SEO program on the computer is connected.
    Login {
        /// Sign in with a code instead, approved on any device (for machines without a
        /// browser, such as over SSH).
        #[arg(long)]
        code: bool,
    },
    /// Disconnect this computer from the account.
    Logout,
    /// Show the account and its balance.
    Status,
}

#[derive(Debug, Subcommand)]
pub(crate) enum ProvidersCommand {
    /// List the paid search APIs and how they are paid.
    List,
    /// Choose how a paid search API is paid: `auto` (your key if given, else credits), `own`
    /// (your key only) or `credit` (OctopusOn SEO credits only; sign in with
    /// `octopuson-index account login`).
    Mode {
        /// `serper` or `serpapi`.
        source: String,
        /// `auto`, `own` or `credit`.
        mode: String,
    },
}

fn print_json(value: &impl serde::Serialize) -> miette::Result<()> {
    println!("{}", serde_json::to_string_pretty(value).into_diagnostic()?);
    Ok(())
}

/// Opens `url` in the default browser.
fn open_browser(url: &str) -> bool {
    let command = if cfg!(target_os = "macos") {
        std::process::Command::new("open").arg(url).status()
    } else if cfg!(windows) {
        std::process::Command::new("rundll32")
            .args(["url.dll,FileProtocolHandler", url])
            .status()
    } else {
        std::process::Command::new("xdg-open").arg(url).status()
    };
    command.is_ok_and(|status| status.success())
}

fn stop_on_ctrl_c() -> Arc<AtomicBool> {
    let cancelled = Arc::new(AtomicBool::new(false));
    let handler = Arc::clone(&cancelled);
    let _ = ctrlc::set_handler(move || handler.store(true, Ordering::SeqCst));
    cancelled
}

/// One click: the account site opens in the browser and sends a signed-in person back.
fn connect(workspace: &FinderWorkspace) -> miette::Result<()> {
    eprintln!("Opening the OctopusOn SEO account site in your browser…");
    let cancelled = stop_on_ctrl_c();
    let connected = workspace
        .connect(&open_browser, &|| cancelled.load(Ordering::SeqCst))
        .into_diagnostic()?;
    if !connected {
        println!("stopped; not signed in");
        return Ok(());
    }
    status(workspace, false)
}

/// With a code approved on any device.
fn login_with_code(workspace: &FinderWorkspace) -> miette::Result<()> {
    let sign_in = workspace.start_sign_in().into_diagnostic()?;
    eprintln!(
        "Approve the code {} in your browser. If it does not open, visit {} and enter the code.",
        sign_in.user_code, sign_in.verification_uri
    );
    open_browser(&sign_in.verification_uri_complete);
    let cancelled = stop_on_ctrl_c();
    let approved = workspace
        .finish_sign_in(
            &sign_in,
            &|| cancelled.load(Ordering::SeqCst),
            &std::thread::sleep,
        )
        .into_diagnostic()?;
    if !approved {
        println!("stopped; not signed in");
        return Ok(());
    }
    status(workspace, false)
}

fn status(workspace: &FinderWorkspace, json: bool) -> miette::Result<()> {
    let status = workspace.cloud_status().into_diagnostic()?;
    if json {
        return print_json(&status);
    }
    match (&status.account, status.signed_in) {
        (Some(account), _) => {
            println!(
                "signed in as {}: {:.2} credits available{}",
                account.email.as_deref().unwrap_or("?"),
                account.credits(),
                if account.low_balance { " (low)" } else { "" }
            );
            println!(
                "buy credits: {}/kredi",
                account.portal_url.trim_end_matches('/')
            );
        }
        (None, true) => println!(
            "signed in; the account cannot be read now: {}",
            status.error.as_deref().unwrap_or("unknown error")
        ),
        (None, false) => println!(
            "not signed in (`octopuson-index account login`); gateway {}",
            status.api
        ),
    }
    Ok(())
}

pub(crate) fn run_account(
    workspace: &FinderWorkspace,
    command: &AccountCommand,
    json: bool,
) -> miette::Result<()> {
    match command {
        AccountCommand::Login { code: true } => login_with_code(workspace),
        AccountCommand::Login { code: false } => connect(workspace),
        AccountCommand::Logout => {
            workspace.sign_out().into_diagnostic()?;
            println!("signed out");
            Ok(())
        }
        AccountCommand::Status => status(workspace, json),
    }
}

pub(crate) fn run_providers(
    workspace: &FinderWorkspace,
    command: &ProvidersCommand,
    json: bool,
) -> miette::Result<()> {
    match command {
        ProvidersCommand::List => {
            // Keys are given per scan (`scan --serper-api-key …`), so this shows how a scan
            // without keys would run.
            let sources = workspace
                .credit_sources(&ApiConfig::default())
                .into_diagnostic()?;
            if json {
                return print_json(&sources);
            }
            for source in sources {
                let paid = if source.creditable {
                    let without_key = match (source.via, source.mode, &source.credit) {
                        (Some(Via::Credit), _, _) => "credits",
                        (_, SourceMode::Own, _) | (_, _, Some(_)) => "skipped",
                        (_, _, None) => {
                            "skipped (connect an account, or the source is not on offer yet)"
                        }
                    };
                    let mode = match source.mode {
                        SourceMode::Auto => "auto",
                        SourceMode::Own => "own",
                        SourceMode::Credit => "credit",
                    };
                    format!("mode {mode}; without a key: {without_key}")
                } else {
                    "own key only (cannot run on OctopusOn SEO credits)".to_owned()
                };
                println!("{}\t{}\t{paid}", source.id, source.name);
            }
            Ok(())
        }
        ProvidersCommand::Mode { source, mode } => {
            let (mode, name) = match mode.to_ascii_lowercase().as_str() {
                "auto" => (
                    SourceMode::Auto,
                    "automatic: your key if given, else credits",
                ),
                "own" => (SourceMode::Own, "your own key only"),
                "credit" => (SourceMode::Credit, "OctopusOn SEO credits"),
                _ => return Err(miette!("give auto, own or credit")),
            };
            workspace.set_source_mode(source, mode).into_diagnostic()?;
            println!("{source}: {name}");
            Ok(())
        }
    }
}
