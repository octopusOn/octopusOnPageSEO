//! Developer automation: `cargo xtask <command>`.
//!
//! Keeping CI steps here (instead of only in YAML) guarantees that contributors run the exact
//! same checks locally as GitHub Actions does.

#![allow(clippy::print_stderr)]

use std::process::{Command, ExitCode};

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "xtask")]
struct Xtask {
    #[command(subcommand)]
    command: Task,
}

#[derive(Clone, Copy, Subcommand)]
enum Task {
    /// Run every check CI runs: fmt, clippy, tests, cargo-deny and the frontend checks.
    Ci,
    /// Check formatting.
    Fmt,
    /// Run clippy with warnings denied.
    Lint,
    /// Run the test suite.
    Test,
    /// Run cargo-deny (licenses, advisories, bans, sources).
    Deny,
    /// Type-check, test and build the desktop frontend and the website (needs pnpm).
    Frontend,
    /// Download crawler IP lists into the snapshot bundled with the application.
    BotRanges,
}

fn main() -> ExitCode {
    match run(Xtask::parse().command) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("xtask failed: {error:#}");
            ExitCode::FAILURE
        }
    }
}

fn run(task: Task) -> Result<()> {
    match task {
        Task::Ci => {
            fmt()?;
            lint()?;
            test()?;
            deny()?;
            frontend(false)
        }
        Task::Fmt => fmt(),
        Task::Lint => lint(),
        Task::Test => test(),
        Task::Deny => deny(),
        Task::Frontend => frontend(true),
        Task::BotRanges => bot_ranges(),
    }
}

/// Bundled snapshot read by `seonexus_botverify::ranges::RangeSnapshot::bundled`.
const BOT_RANGES_SNAPSHOT: &str = "crates/seonexus-botverify/data/ip-ranges.json";

fn bot_ranges() -> Result<()> {
    let download = seonexus_log_app::bots::download_ranges();
    for failure in &download.failures {
        eprintln!("warning: {}: {}", failure.id, failure.error);
    }
    if download.snapshot.sources.is_empty() {
        bail!("no IP list could be downloaded");
    }
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .context("xtask lives inside the workspace")?;
    let path = root.join(BOT_RANGES_SNAPSHOT);
    // Keep lists that failed to download from the previous snapshot.
    let previous = std::fs::read_to_string(&path)
        .ok()
        .and_then(|text| seonexus_log_app::bots::RangeSnapshot::from_json(&text).ok())
        .unwrap_or_default();
    let snapshot = previous.merged_with(download.snapshot);
    std::fs::write(&path, snapshot.to_json() + "\n")
        .with_context(|| format!("cannot write {}", path.display()))?;
    eprintln!(
        "wrote {} lists to {}",
        snapshot.sources.len(),
        path.display()
    );
    Ok(())
}

fn fmt() -> Result<()> {
    cargo(&["fmt", "--all", "--check"])
}

fn lint() -> Result<()> {
    cargo(&[
        "clippy",
        "--workspace",
        "--all-targets",
        "--locked",
        "--",
        "-D",
        "warnings",
    ])
}

fn test() -> Result<()> {
    // Report every failing test binary, not only the first.
    cargo(&[
        "test",
        "--workspace",
        "--all-targets",
        "--locked",
        "--no-fail-fast",
    ])?;
    // The desktop shell has no doc tests; building it here would put tauri-build's empty
    // `msvcrt.lib` (static VC runtime) on every doc test's linker path on Windows.
    cargo(&[
        "test",
        "--workspace",
        "--exclude",
        "seonexus-desktop",
        "--doc",
        "--locked",
    ])
}

fn deny() -> Result<()> {
    if !is_cargo_subcommand_installed("deny") {
        eprintln!(
            "warning: cargo-deny not installed, skipping (cargo install --locked cargo-deny)"
        );
        return Ok(());
    }
    cargo(&["deny", "--workspace", "check"])
}

/// `required` makes a missing pnpm an error instead of a skipped step.
fn frontend(required: bool) -> Result<()> {
    if !is_installed("pnpm", &["--version"]) {
        if required {
            bail!("pnpm is not installed (https://pnpm.io/installation)");
        }
        eprintln!("warning: pnpm not installed, skipping frontend checks");
        return Ok(());
    }
    exec("pnpm", &["install", "--frozen-lockfile"])?;
    exec("pnpm", &["--filter", "@seonexus/desktop", "check"])?;
    exec("pnpm", &["--filter", "@seonexus/desktop", "build"])?;
    exec("pnpm", &["--filter", "@octopusseo/web", "check"])?;
    exec("pnpm", &["--filter", "@octopusseo/web", "build"])
}

fn exec(program: &str, args: &[&str]) -> Result<()> {
    eprintln!("$ {program} {}", args.join(" "));
    let status = Command::new(program)
        .args(args)
        .status()
        .with_context(|| format!("failed to spawn `{program}`"))?;
    if !status.success() {
        bail!("`{program} {}` exited with {status}", args.join(" "));
    }
    Ok(())
}

fn is_installed(program: &str, args: &[&str]) -> bool {
    Command::new(program)
        .args(args)
        .output()
        .is_ok_and(|output| output.status.success())
}

fn cargo(args: &[&str]) -> Result<()> {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned());
    eprintln!("$ cargo {}", args.join(" "));
    let status = Command::new(&cargo)
        .args(args)
        .status()
        .with_context(|| format!("failed to spawn `{cargo}`"))?;
    if !status.success() {
        bail!("`cargo {}` exited with {status}", args.join(" "));
    }
    Ok(())
}

fn is_cargo_subcommand_installed(name: &str) -> bool {
    is_installed("cargo", &[name, "--version"])
}
