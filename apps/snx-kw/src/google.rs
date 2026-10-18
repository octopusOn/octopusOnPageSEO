//! `octopuson-keywords google …`: the user's own OAuth client, sign-in, Search Console and Google Ads.

use std::io::{self, BufRead, IsTerminal};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use miette::IntoDiagnostic;
use seonexus_kw_app::KeywordWorkspace;
use seonexus_kw_app::google::{GoogleScopes, SearchConsoleRequest};

use crate::cli::GoogleCommand;
use crate::commands::{print_json, write_stderr, write_text};
use crate::output::table;

/// Opens an address in the default browser; `false` when no browser could be started.
pub(crate) fn open_browser(url: &str) -> bool {
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

fn login(workspace: &KeywordWorkspace, ads: bool, ads_only: bool) -> miette::Result<()> {
    let scopes = GoogleScopes {
        search_console: !ads_only,
        ads: ads || ads_only,
    };
    let cancelled = Arc::new(AtomicBool::new(false));
    let handler = Arc::clone(&cancelled);
    let _ = ctrlc::set_handler(move || handler.store(true, Ordering::SeqCst));
    let open = |url: &str| {
        write_stderr(&format!(
            "Sign in to Google in your browser. If it does not open, visit:\n{url}\n"
        ));
        open_browser(url)
    };
    let status = workspace
        .google_login(scopes, &open, &|| cancelled.load(Ordering::SeqCst))
        .into_diagnostic()?;
    write_text(|out| writeln!(out, "signed in; granted: {}", status.scopes.join(", ")))
}

fn import(
    workspace: &KeywordWorkspace,
    project: i64,
    site: &str,
    days: u32,
    all_countries: bool,
    json: bool,
) -> miette::Result<()> {
    let request = SearchConsoleRequest {
        site_url: site.to_owned(),
        days,
        country_only: !all_countries,
        ..SearchConsoleRequest::default()
    };
    let outcome = workspace
        .import_search_console(project, &request, &|| false)
        .into_diagnostic()?;
    if json {
        return print_json(&outcome);
    }
    write_text(|out| {
        writeln!(
            out,
            "{} rows for {}: {} queries added, {} already in the project; {} pages for mapping",
            outcome.rows, outcome.period, outcome.added.added, outcome.added.known, outcome.pages
        )
    })
}

pub(crate) fn run_google(
    workspace: &KeywordWorkspace,
    command: &GoogleCommand,
) -> miette::Result<()> {
    match command {
        GoogleCommand::Client { client_id } => {
            let stdin = io::stdin();
            if stdin.is_terminal() {
                write_stderr(
                    "Paste the client secret and press Enter (it is not kept in your shell history):\n",
                );
            }
            let mut secret = String::new();
            stdin.lock().read_line(&mut secret).into_diagnostic()?;
            workspace
                .set_google_client(client_id, secret.trim())
                .into_diagnostic()?;
            write_text(|out| {
                writeln!(
                    out,
                    "saved the OAuth client; sign in with `octopuson-keywords google login`"
                )
            })
        }
        GoogleCommand::Login { ads, ads_only } => login(workspace, *ads, *ads_only),
        GoogleCommand::Logout => {
            workspace.google_logout().into_diagnostic()?;
            write_text(|out| writeln!(out, "signed out"))
        }
        GoogleCommand::Status { json } => {
            let status = workspace.google_status().into_diagnostic()?;
            if *json {
                return print_json(&status);
            }
            write_text(|out| {
                writeln!(
                    out,
                    "client id      {}",
                    status.client_id.as_deref().unwrap_or("–")
                )?;
                writeln!(
                    out,
                    "client secret  {}",
                    if status.client_secret { "saved" } else { "–" }
                )?;
                writeln!(
                    out,
                    "signed in      {}",
                    if status.connected { "yes" } else { "no" }
                )?;
                writeln!(out, "scopes         {}", status.scopes.join(", "))?;
                writeln!(
                    out,
                    "ads customer   {}",
                    status.ads_customer_id.as_deref().unwrap_or("–")
                )
            })
        }
        GoogleCommand::Sites { json } => {
            let properties = workspace.search_console_properties().into_diagnostic()?;
            if *json {
                return print_json(&properties);
            }
            let rows: Vec<Vec<String>> = properties
                .iter()
                .map(|property| vec![property.site_url.clone(), property.permission.clone()])
                .collect();
            write_text(|out| table(out, &["property", "permission"], &rows))
        }
        GoogleCommand::Import {
            project,
            site,
            days,
            all_countries,
            json,
        } => import(workspace, *project, site, *days, *all_countries, *json),
        GoogleCommand::AdsAccount {
            customer_id,
            manager,
        } => {
            workspace
                .set_ads_account(Some(customer_id), manager.as_deref())
                .into_diagnostic()?;
            write_text(|out| writeln!(out, "Google Ads keyword planning runs under {customer_id}"))
        }
    }
}
