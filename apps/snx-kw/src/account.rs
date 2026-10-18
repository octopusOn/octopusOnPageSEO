//! `octopuson-keywords account …`: the OctopusOn SEO account that pays for sources on credits.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use miette::IntoDiagnostic;
use seonexus_kw_app::KeywordWorkspace;
use seonexus_kw_app::runner::RunControl;

use crate::cli::AccountCommand;
use crate::commands::{print_json, write_stderr, write_text};
use crate::google::open_browser;

fn login(workspace: &KeywordWorkspace) -> miette::Result<()> {
    let sign_in = workspace.start_sign_in().into_diagnostic()?;
    write_stderr(&format!(
        "Approve the code {} in your browser. If it does not open, visit {} and enter the code.\n",
        sign_in.user_code, sign_in.verification_uri
    ));
    open_browser(&sign_in.verification_uri_complete);
    let cancelled = Arc::new(AtomicBool::new(false));
    let handler = Arc::clone(&cancelled);
    let _ = ctrlc::set_handler(move || handler.store(true, Ordering::SeqCst));
    let control = RunControl {
        cancelled: &|| cancelled.load(Ordering::SeqCst),
        progress: &|_| {},
        sleep: &|duration: Duration| std::thread::sleep(duration),
    };
    if !workspace
        .finish_sign_in(&sign_in, &control)
        .into_diagnostic()?
    {
        return write_text(|out| writeln!(out, "stopped; not signed in"));
    }
    status(workspace, false)
}

fn connect(workspace: &KeywordWorkspace) -> miette::Result<()> {
    write_stderr("Opening the OctopusOn SEO account site in your browser…\n");
    let cancelled = Arc::new(AtomicBool::new(false));
    let handler = Arc::clone(&cancelled);
    let _ = ctrlc::set_handler(move || handler.store(true, Ordering::SeqCst));
    let control = RunControl {
        cancelled: &|| cancelled.load(Ordering::SeqCst),
        progress: &|_| {},
        sleep: &|duration: Duration| std::thread::sleep(duration),
    };
    if !workspace
        .connect(&open_browser, &control)
        .into_diagnostic()?
    {
        return write_text(|out| writeln!(out, "stopped; not signed in"));
    }
    status(workspace, false)
}

fn status(workspace: &KeywordWorkspace, json: bool) -> miette::Result<()> {
    let status = workspace.cloud_status().into_diagnostic()?;
    if json {
        return print_json(&status);
    }
    write_text(|out| match (&status.account, status.signed_in) {
        (Some(account), _) => {
            writeln!(
                out,
                "signed in as {}: {:.2} credits available{}",
                account.email.as_deref().unwrap_or("?"),
                account.credits(),
                if account.low_balance { " (low)" } else { "" }
            )?;
            writeln!(
                out,
                "buy credits: {}/kredi",
                account.portal_url.trim_end_matches('/')
            )
        }
        (None, true) => writeln!(
            out,
            "signed in; the account cannot be read now: {}",
            status.error.as_deref().unwrap_or("unknown error")
        ),
        (None, false) => writeln!(
            out,
            "not signed in (`octopuson-keywords account login`); gateway {}",
            status.api
        ),
    })
}

pub(crate) fn run_account(
    workspace: &KeywordWorkspace,
    command: &AccountCommand,
) -> miette::Result<()> {
    match command {
        AccountCommand::Login { code: true } => login(workspace),
        AccountCommand::Login { code: false } => connect(workspace),
        AccountCommand::Logout => {
            workspace.sign_out().into_diagnostic()?;
            write_text(|out| writeln!(out, "signed out"))
        }
        AccountCommand::Status { json } => status(workspace, *json),
    }
}
