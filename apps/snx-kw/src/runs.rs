//! Paid runs: discovery, enrichment, result pages and the settings of their sources.

use seonexus_kw_app::cloud_api::{SourceMode, Via};
use std::io::{self, BufRead, IsTerminal};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use miette::{IntoDiagnostic, miette};
use seonexus_keywords::{Expansion, FilterSpec};
use seonexus_kw_app::KeywordWorkspace;
use seonexus_kw_app::browser::BROWSER;
use seonexus_kw_app::discover::{DiscoverEstimate, DiscoverRequest, DiscoverySource};
use seonexus_kw_app::enrich::{EnrichRequest, Enrichment, VolumeSource};
use seonexus_kw_app::runner::{ChallengeKind, Progress, RunControl, RunSummary};
use seonexus_kw_app::serps::SerpRequest;

use crate::cli::{
    AskArg, BrowserCommand, DiscoverArgs, EnrichArgs, ExpandArg, ProvidersCommand, SourceArg,
};
use crate::commands::{confirm, print_json, write_stderr, write_text};
use crate::output::{table, usd};

const fn source_of(source: SourceArg) -> DiscoverySource {
    match source {
        SourceArg::Wordstat => DiscoverySource::Wordstat,
        SourceArg::AdsIdeas => DiscoverySource::AdsIdeas,
        SourceArg::LabsIdeas => DiscoverySource::LabsIdeas,
        SourceArg::Related => DiscoverySource::Related,
        SourceArg::Suggestions => DiscoverySource::Suggestions,
        SourceArg::Competitors => DiscoverySource::Competitors,
        SourceArg::Autocomplete => DiscoverySource::Autocomplete,
        SourceArg::SerpapiAutocomplete => DiscoverySource::SerpapiAutocomplete,
        SourceArg::GoogleAds => DiscoverySource::GoogleAds,
        SourceArg::GoogleSuggest => DiscoverySource::GoogleSuggest,
        SourceArg::YandexSuggest => DiscoverySource::YandexSuggest,
    }
}

const fn expansion_of(expansion: ExpandArg) -> Expansion {
    match expansion {
        ExpandArg::Alphabet => Expansion::Alphabet,
        ExpandArg::TurkishLetters => Expansion::TurkishLetters,
        ExpandArg::Questions => Expansion::Questions,
        ExpandArg::Commercial => Expansion::Commercial,
    }
}

/// Runs `work` with Ctrl-C stopping it after the current request (a second Ctrl-C exits).
fn with_control<T>(quiet: bool, work: impl FnOnce(&RunControl<'_>) -> T) -> T {
    let cancelled = Arc::new(AtomicBool::new(false));
    let handler = Arc::clone(&cancelled);
    let _ = ctrlc::set_handler(move || {
        if handler.swap(true, Ordering::SeqCst) {
            std::process::exit(130);
        }
        write_stderr("\nstopping after the current request…\n");
    });
    let is_cancelled = || cancelled.load(Ordering::SeqCst);
    let progress = |progress: &Progress| {
        if let Some(check) = &progress.check {
            // Shown even when quiet: nothing goes on until the person acts.
            write_stderr(&format!(
                "The search engine shows a {} page in the browser window. Please complete it \
                 there; the run continues by itself (waiting until about {}). ({})\n",
                match check.kind {
                    ChallengeKind::Consent => "consent",
                    ChallengeKind::Captcha => "verification (CAPTCHA)",
                    ChallengeKind::Blocked => "blocked",
                },
                check.until,
                check.url
            ));
            return;
        }
        if quiet {
            return;
        }
        match &progress.waiting_until {
            Some(until) => write_stderr(&format!(
                "[{}/{}] waiting for the Wordstat hourly quota (about {until})\n",
                progress.done, progress.planned
            )),
            None => write_stderr(&format!(
                "[{}/{}] {}: {} keywords so far\n",
                progress.done, progress.planned, progress.step, progress.found
            )),
        }
    };
    let sleep = |duration| std::thread::sleep(duration);
    work(&RunControl {
        cancelled: &is_cancelled,
        progress: &progress,
        sleep: &sleep,
    })
}

fn print_estimate(estimate: &DiscoverEstimate) -> miette::Result<()> {
    let rows: Vec<Vec<String>> = estimate
        .sources
        .iter()
        .map(|source| {
            vec![
                serde_json::to_value(source.source)
                    .ok()
                    .and_then(|value| value.as_str().map(str::to_owned))
                    .unwrap_or_default(),
                source.provider.clone(),
                source.requests.to_string(),
                usd(source.cost_usd),
                source.reason.clone().unwrap_or_else(|| "ready".to_owned()),
            ]
        })
        .collect();
    write_text(|out| {
        table(
            out,
            &["source", "provider", "requests", "usd", "state"],
            &rows,
        )?;
        writeln!(
            out,
            "{} requests, about {} USD",
            estimate.requests,
            usd(estimate.cost_usd)
        )?;
        if estimate.wordstat_hours > 1.0 {
            writeln!(
                out,
                "Wordstat's hourly quota stretches this over about {:.1} hours",
                estimate.wordstat_hours
            )?;
        }
        Ok(())
    })
}

fn check_cost(workspace: &KeywordWorkspace, cost: f64, yes: bool) -> miette::Result<()> {
    let limit = workspace.settings().into_diagnostic()?.confirm_above_usd;
    if cost > limit && !yes {
        confirm(&format!(
            "This costs about {} USD, above your limit of {} USD. Continue?",
            usd(cost),
            usd(limit)
        ))?;
    }
    Ok(())
}

fn print_summary(run: &RunSummary) -> miette::Result<()> {
    write_text(|out| {
        writeln!(
            out,
            "run {} {}: {} requests, {} USD, {} keywords found",
            run.run_id,
            run.status.as_str(),
            run.requests,
            usd(run.cost_usd),
            run.found
        )?;
        for failure in &run.failures {
            writeln!(
                out,
                "  {} ({}): {}",
                failure.provider, failure.step, failure.error
            )?;
        }
        Ok(())
    })
}

pub(crate) fn run_discover(
    workspace: &KeywordWorkspace,
    args: &DiscoverArgs,
) -> miette::Result<()> {
    let mut seeds = args.seeds.clone();
    if let Some(file) = &args.seeds_file {
        let text = std::fs::read_to_string(file).into_diagnostic()?;
        seeds.extend(text.lines().map(str::to_owned));
    }
    let request = DiscoverRequest {
        seeds,
        competitors: args.competitors.clone(),
        sources: args.sources.iter().copied().map(source_of).collect(),
        expansions: args.expansions.iter().copied().map(expansion_of).collect(),
        limit: args.limit,
        filter: FilterSpec {
            include: args.include.clone(),
            exclude: args.exclude.clone(),
            min_volume: args.min_volume,
            max_words: args.max_words,
            ..FilterSpec::default()
        },
    };
    let estimate = workspace
        .estimate_discovery(args.project, &request)
        .into_diagnostic()?;
    if args.estimate {
        return if args.json {
            print_json(&estimate)
        } else {
            print_estimate(&estimate)
        };
    }
    if !args.json {
        print_estimate(&estimate)?;
    }
    if estimate.requests == 0 {
        return Err(miette!("no source can run; see the state column"));
    }
    check_cost(workspace, estimate.cost_usd, args.yes)?;
    let outcome = with_control(args.json, |control| {
        workspace.discover(args.project, &request, control)
    })
    .into_diagnostic()?;
    if args.json {
        return print_json(&outcome);
    }
    print_summary(&outcome.run)?;
    write_text(|out| {
        writeln!(
            out,
            "{} kept by the filter: {} new, {} already in the project",
            outcome.kept, outcome.added.added, outcome.added.known
        )
    })
}

pub(crate) fn run_enrich(workspace: &KeywordWorkspace, args: &EnrichArgs) -> miette::Result<()> {
    let mut asks: Vec<Enrichment> = args
        .asks
        .iter()
        .map(|ask| match ask {
            AskArg::Volume => Enrichment::Volume,
            AskArg::Difficulty => Enrichment::Difficulty,
            AskArg::Intent => Enrichment::Intent,
        })
        .collect();
    if asks.is_empty() {
        asks.push(Enrichment::Volume);
    }
    let request = EnrichRequest {
        asks,
        volume_source: if args.google_ads {
            VolumeSource::GoogleAds
        } else {
            VolumeSource::Dataforseo
        },
        only_missing: !args.all,
    };
    let estimate = workspace
        .estimate_enrichment(args.project, &request)
        .into_diagnostic()?;
    if args.json && args.estimate {
        return print_json(&estimate);
    }
    if !args.json {
        let [volume, difficulty, intent] = estimate.keywords;
        write_text(|out| {
            writeln!(
                out,
                "volume {volume}, difficulty {difficulty}, intent {intent} keywords: {} requests, about {} USD{}",
                estimate.requests,
                usd(estimate.cost_usd),
                estimate
                    .reason
                    .as_ref()
                    .map(|reason| format!(" ({reason})"))
                    .unwrap_or_default()
            )
        })?;
    }
    if args.estimate {
        return Ok(());
    }
    if let Some(reason) = &estimate.reason {
        return Err(miette!("{reason}"));
    }
    check_cost(workspace, estimate.cost_usd, args.yes)?;
    let summary = with_control(args.json, |control| {
        workspace.enrich(args.project, &request, control)
    })
    .into_diagnostic()?;
    if args.json {
        return print_json(&summary);
    }
    print_summary(&summary)
}

fn list_providers(workspace: &KeywordWorkspace, json: bool) -> miette::Result<()> {
    let providers = workspace.providers().into_diagnostic()?;
    if json {
        return print_json(&providers);
    }
    let rows: Vec<Vec<String>> = providers
        .iter()
        .map(|provider| {
            vec![
                provider.id.clone(),
                provider.name.clone(),
                provider
                    .secrets
                    .iter()
                    .map(|secret| {
                        format!(
                            "{}: {}",
                            secret.name,
                            serde_json::to_value(secret.state)
                                .ok()
                                .and_then(|value| value.as_str().map(str::to_owned))
                                .unwrap_or_default()
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(", "),
                match provider.via {
                    Some(Via::Own) if provider.secrets.is_empty() => "yes".to_owned(),
                    Some(Via::Own) => "own key".to_owned(),
                    Some(Via::Credit) => format!(
                        "credits ×{}",
                        provider
                            .credit
                            .as_ref()
                            .map_or(1.0, |credit| credit.price_multiplier)
                    ),
                    None if provider.ready => "yes".to_owned(),
                    None => "no".to_owned(),
                },
                if provider.enabled { "on" } else { "off" }.to_owned(),
                provider.budget_usd.map(usd).unwrap_or_default(),
                usd(provider.spent_usd),
            ]
        })
        .collect();
    write_text(|out| {
        table(
            out,
            &[
                "id",
                "name",
                "keys",
                "ready",
                "switch",
                "budget/month",
                "spent",
            ],
            &rows,
        )
    })
}

pub(crate) fn run_providers(
    workspace: &KeywordWorkspace,
    command: &ProvidersCommand,
) -> miette::Result<()> {
    match command {
        ProvidersCommand::List { json } => list_providers(workspace, *json),
        ProvidersCommand::SetKey { provider, name } => {
            let stdin = io::stdin();
            if stdin.is_terminal() {
                write_stderr(&format!(
                    "Paste the value of `{name}` and press Enter (it is not kept in your shell history):\n"
                ));
            }
            let mut value = String::new();
            stdin.lock().read_line(&mut value).into_diagnostic()?;
            workspace
                .set_key(provider, name, value.trim())
                .into_diagnostic()?;
            write_text(|out| {
                writeln!(
                    out,
                    "saved `{name}` of {provider} in the {} store",
                    workspace.secrets().backend_name()
                )
            })
        }
        ProvidersCommand::RemoveKey { provider, name } => {
            workspace.remove_key(provider, name).into_diagnostic()?;
            write_text(|out| writeln!(out, "removed `{name}` of {provider}"))
        }
        ProvidersCommand::Budget {
            provider,
            usd: value,
        } => {
            let budget = if value.eq_ignore_ascii_case("none") {
                None
            } else {
                Some(
                    value
                        .parse::<f64>()
                        .ok()
                        .filter(|budget| budget.is_finite() && *budget >= 0.0)
                        .ok_or_else(|| miette!("give US dollars (like 10 or 2.5) or `none`"))?,
                )
            };
            workspace.set_budget(provider, budget).into_diagnostic()?;
            write_text(|out| match budget {
                Some(budget) => writeln!(out, "{provider}: at most {} USD a month", usd(budget)),
                None => writeln!(out, "{provider}: no monthly budget"),
            })
        }
        ProvidersCommand::Enable { provider } => {
            workspace.set_enabled(provider, true).into_diagnostic()?;
            write_text(|out| writeln!(out, "{provider} switched on"))
        }
        ProvidersCommand::Disable { provider } => {
            workspace.set_enabled(provider, false).into_diagnostic()?;
            write_text(|out| writeln!(out, "{provider} switched off"))
        }
        ProvidersCommand::Mode { provider, mode } => {
            let (mode, name) = match mode.to_ascii_lowercase().as_str() {
                "auto" => (SourceMode::Auto, "automatic: your key if set, else credits"),
                "own" => (SourceMode::Own, "your own key only"),
                "credit" => (SourceMode::Credit, "OctopusOn SEO credits"),
                _ => return Err(miette!("give auto, own or credit")),
            };
            workspace
                .set_source_mode(provider, mode)
                .into_diagnostic()?;
            write_text(|out| writeln!(out, "{provider}: {name}"))
        }
    }
}

pub(crate) fn run_serp(
    workspace: &KeywordWorkspace,
    args: &crate::cli::SerpArgs,
) -> miette::Result<()> {
    let request = SerpRequest {
        engine: args.engine.map(|engine| engine.as_str().to_owned()),
        device: args.device.clone(),
        top: args.top,
        per_cluster: args.per_cluster,
        refresh: args.refresh,
    };
    let estimate = workspace
        .estimate_serps(args.project, &request)
        .into_diagnostic()?;
    if args.json && args.estimate {
        return print_json(&estimate);
    }
    if !args.json {
        write_text(|out| {
            writeln!(
                out,
                "{} keywords need a result page ({} fresh ones reused): {}",
                estimate.keywords,
                estimate.cached,
                estimate.provider.as_deref().map_or_else(
                    || "no provider can be used".to_owned(),
                    |provider| format!("{provider}, about {} USD", usd(estimate.cost_usd))
                )
            )?;
            for (provider, reason) in &estimate.skipped {
                writeln!(out, "  {provider}: {reason}")?;
            }
            Ok(())
        })?;
    }
    if args.estimate || estimate.keywords == 0 {
        return Ok(());
    }
    if estimate.provider.is_none() {
        return Err(miette!(
            "set up a result-page provider first (`octopuson-keywords providers list`)"
        ));
    }
    check_cost(workspace, estimate.cost_usd, args.yes)?;
    let outcome = with_control(args.json, |control| {
        workspace.fetch_serps(args.project, &request, control)
    })
    .into_diagnostic()?;
    if args.json {
        return print_json(&outcome);
    }
    print_summary(&outcome.run)?;
    write_text(|out| {
        writeln!(
            out,
            "{} pages of your site seen on result pages; cluster again with `--method hybrid` or `serp`",
            outcome.site_pages
        )
    })
}

pub(crate) fn run_suggest(
    workspace: &KeywordWorkspace,
    args: &crate::cli::SuggestArgs,
) -> miette::Result<()> {
    use crate::cli::Switch;
    let mut settings = workspace.settings().into_diagnostic()?;
    let turning_on = args.google == Some(Switch::On) || args.yandex == Some(Switch::On);
    if turning_on && settings.suggest.accepted_at.is_none() && !args.accept_terms {
        return Err(miette!(
            "The free suggestion endpoints are not official APIs. Google's use is a grey area; \
             Yandex's terms forbid automated queries without permission. Requests go one at a \
             time with the program's own user agent, at most {} per engine and day, and stop \
             for the day at the first refusal. Paid sources (DataForSEO, SerpApi) are the safe \
             choice. Pass --accept-terms to switch them on anyway.",
            settings.suggest.daily_limit
        ));
    }
    if let Some(google) = args.google {
        settings.suggest.google = google == Switch::On;
    }
    if let Some(yandex) = args.yandex {
        settings.suggest.yandex = yandex == Switch::On;
    }
    if turning_on && settings.suggest.accepted_at.is_none() {
        settings.suggest.accepted_at = Some(seonexus_kw_app::timestamp());
    }
    if let Some(pause) = args.pause_ms {
        settings.suggest.pause_ms = pause;
    }
    if let Some(limit) = args.daily_limit {
        settings.suggest.daily_limit = limit;
    }
    workspace.save_settings(&settings).into_diagnostic()?;
    let state = |on: bool| if on { "on" } else { "off" };
    write_text(|out| {
        writeln!(
            out,
            "Google suggestions {}, Yandex suggestions {}; one request every {} ms, at most {} a day",
            state(settings.suggest.google),
            state(settings.suggest.yandex),
            settings.suggest.pause_ms,
            settings.suggest.daily_limit
        )
    })
}

/// What the browser mode is, printed before it is added and with its state.
const BROWSER_WARNING: &str = "Browser mode opens a visible Chrome or Edge window with its own \
    profile and reads one result page at a time, with a fixed pause and a daily limit per \
    engine. A consent page or CAPTCHA waits for you in that window; the program never solves \
    it. Automated searching may conflict with the engines' terms: keep it for small lists and \
    use an API source for large ones. Google hides result addresses there, so its pages give \
    sites only: clustering then compares sites, and mapping cannot use them.";

/// Prints where the browser mode stands.
fn print_browser_status(workspace: &KeywordWorkspace, json: bool) -> miette::Result<()> {
    let status = workspace.browser_status().into_diagnostic()?;
    if json {
        return print_json(&status);
    }
    write_text(|out| {
        writeln!(
            out,
            "executable: {}",
            status
                .executable
                .as_deref()
                .unwrap_or("Chrome or Edge not found")
        )?;
        writeln!(
            out,
            "{}; in the chains of: {}",
            if status.enabled {
                "switched on"
            } else {
                "switched off"
            },
            if status.engines.is_empty() {
                "none (add it with `octopuson-keywords browser add <engine>`)".to_owned()
            } else {
                status.engines.join(", ")
            }
        )?;
        writeln!(
            out,
            "one page every {} s, at most {} pages per engine and day",
            status.delay_secs, status.daily_cap
        )?;
        for (engine, used) in &status.used_today {
            writeln!(out, "  {engine}: {used} today")?;
        }
        writeln!(out, "profile: {}", status.profile_dir)?;
        writeln!(out, "\n{BROWSER_WARNING}")
    })
}

pub(crate) fn run_browser(
    workspace: &KeywordWorkspace,
    command: &BrowserCommand,
) -> miette::Result<()> {
    match command {
        BrowserCommand::Status { json } => print_browser_status(workspace, *json),
        BrowserCommand::Set {
            executable,
            delay_secs,
            daily_cap,
            proxy,
        } => {
            let mut settings = workspace.settings().into_diagnostic()?;
            let browser = &mut settings.browser;
            if let Some(executable) = executable {
                browser.executable = (!executable.eq_ignore_ascii_case("auto"))
                    .then(|| executable.trim().to_owned());
            }
            if let Some(delay) = delay_secs {
                browser.delay_secs = *delay;
            }
            if let Some(cap) = daily_cap {
                browser.daily_cap = *cap;
            }
            if let Some(proxy) = proxy {
                browser.proxy =
                    (!proxy.eq_ignore_ascii_case("none")).then(|| proxy.trim().to_owned());
            }
            workspace.save_settings(&settings).into_diagnostic()?;
            let browser = &settings.browser;
            write_text(|out| {
                writeln!(
                    out,
                    "browser: {}, one page every {} s, at most {} per engine and day, {}",
                    browser
                        .executable
                        .as_deref()
                        .unwrap_or("Chrome or Edge found automatically"),
                    browser.delay_secs,
                    browser.daily_cap,
                    browser
                        .proxy
                        .as_deref()
                        .map_or_else(|| "no proxy".to_owned(), |proxy| format!("proxy {proxy}"))
                )
            })
        }
        BrowserCommand::Add { engine } | BrowserCommand::Remove { engine } => {
            let adding = matches!(command, BrowserCommand::Add { .. });
            let mut settings = workspace.settings().into_diagnostic()?;
            let chain = settings
                .serp
                .chains
                .entry(engine.as_str().to_owned())
                .or_default();
            chain.retain(|id| id != BROWSER);
            if adding {
                chain.push(BROWSER.to_owned());
            }
            let order = chain.join(", ");
            workspace.save_settings(&settings).into_diagnostic()?;
            write_text(|out| {
                if adding {
                    writeln!(out, "{BROWSER_WARNING}\n")?;
                }
                writeln!(
                    out,
                    "{} result pages: {}",
                    engine.as_str(),
                    if order.is_empty() {
                        "no source"
                    } else {
                        order.as_str()
                    }
                )
            })
        }
        BrowserCommand::ResetProfile => {
            workspace.reset_browser_profile().into_diagnostic()?;
            write_text(|out| writeln!(out, "browser profile deleted (cookies, consent choices)"))
        }
    }
}
