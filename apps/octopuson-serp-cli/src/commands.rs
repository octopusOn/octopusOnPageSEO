use octopuson_serp_app::cloud_api::SourceMode;
use std::fs;
use std::io::{self, BufRead, IsTerminal, Write};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use miette::{IntoDiagnostic, WrapErr, miette};
use octopuson_serp::challenge::ChallengeKind;
use octopuson_serp::provider::SearchEvent;
use octopuson_serp::{Device, Engine, Location, MatchMode, SerpQuery, SerpSnapshot, Target};
use octopuson_serp_app::domains::DomainOptions;
use octopuson_serp_app::providers::{ProviderInfo, test_spec};
use octopuson_serp_app::secrets::SecretState;
use octopuson_serp_app::store::SnapshotFilter;
use octopuson_serp_app::{SearchOutcome, SearchRequest, SerpAppError, SerpWorkspace};

use crate::cli::{
    Cli, Command, DatasetsCommand, DeviceArg, EngineArg, ExportCommand, MatchArg, ProvidersCommand,
    QueryArgs, SearchArgs,
};
use crate::output::{table, usd};

pub(crate) fn run(args: &Cli) -> miette::Result<()> {
    let workspace = SerpWorkspace::resolve(args.data_dir.clone()).into_diagnostic()?;
    match &args.command {
        Command::Search(search) => run_search(&workspace, search, !args.quiet),
        Command::Estimate(query) => run_estimate(&workspace, query),
        Command::Analyze(analyze) => crate::analysis::run_analyze(&workspace, analyze, !args.quiet),
        Command::Analyses {
            keyword,
            limit,
            json,
        } => crate::analysis::run_analyses(&workspace, keyword.as_deref(), *limit, *json),
        Command::Analysis { id, json } => crate::analysis::run_analysis(&workspace, *id, *json),
        Command::Rescore { id } => crate::analysis::run_rescore(&workspace, *id),
        Command::Snapshots {
            keyword,
            engine,
            limit,
            json,
        } => {
            let filter = SnapshotFilter {
                keyword: keyword.clone(),
                engine: engine.map(engine_of),
                run_id: None,
                limit: *limit,
            };
            let snapshots = workspace.snapshots(&filter).into_diagnostic()?;
            if *json {
                return print_json(&snapshots);
            }
            let rows = snapshots
                .iter()
                .map(|snapshot| {
                    vec![
                        snapshot.id.to_string(),
                        snapshot.fetched_at.clone(),
                        snapshot.keyword.clone(),
                        format!(
                            "{} {}-{}",
                            snapshot.engine, snapshot.language, snapshot.country
                        ),
                        snapshot.device.as_str().to_owned(),
                        snapshot.organic_count.to_string(),
                        snapshot.provider.clone(),
                        usd(snapshot.cost_usd),
                    ]
                })
                .collect::<Vec<_>>();
            write_text(|out| {
                table(
                    out,
                    &[
                        "ID", "FETCHED", "KEYWORD", "ENGINE", "DEVICE", "RESULTS", "PROVIDER",
                        "USD",
                    ],
                    &rows,
                )
            })
        }
        Command::Show { id, json } => {
            let snapshot = workspace.snapshot(*id).into_diagnostic()?;
            if *json {
                return print_json(&snapshot);
            }
            write_text(|out| print_snapshot(out, &snapshot, None))
        }
        Command::Delete { id } => {
            workspace.delete_snapshot(*id).into_diagnostic()?;
            write_text(|out| writeln!(out, "deleted snapshot {id}"))
        }
        Command::Reparse { ids } => run_reparse(&workspace, ids),
        Command::Export(command) => run_export(&workspace, command),
        Command::Providers(command) => run_providers(&workspace, command),
        Command::Account(command) => crate::account::run_account(&workspace, command),
        Command::Project(command) => crate::projects::run_project(&workspace, command),
        Command::Track { id, due, yes } => {
            crate::projects::run_track(&workspace, *id, *due, *yes, !args.quiet)
        }
        Command::Domain {
            domains,
            refresh,
            backlinks,
            json,
        } => run_domains(&workspace, domains, *refresh, *backlinks, *json),
        Command::Datasets(DatasetsCommand::Tranco { accept_license }) => {
            run_tranco(&workspace, *accept_license)
        }
        Command::Usage { json } => run_usage(&workspace, *json),
    }
}

fn run_reparse(workspace: &SerpWorkspace, ids: &[i64]) -> miette::Result<()> {
    for id in ids {
        let snapshot = workspace.reparse_snapshot(*id).into_diagnostic()?;
        write_text(|out| {
            writeln!(
                out,
                "snapshot {id}: {} results read again",
                snapshot.organic.len()
            )
        })?;
    }
    Ok(())
}

fn run_export(workspace: &SerpWorkspace, command: &ExportCommand) -> miette::Result<()> {
    let (rows, output) = match command {
        ExportCommand::Rank { project, output } => (
            workspace
                .export_rank_report(*project, output)
                .into_diagnostic()?,
            output,
        ),
        ExportCommand::Analysis { id, output } => (
            workspace.export_analysis(*id, output).into_diagnostic()?,
            output,
        ),
    };
    write_text(|out| writeln!(out, "wrote {rows} rows to {}", output.display()))
}

fn run_usage(workspace: &SerpWorkspace, json: bool) -> miette::Result<()> {
    let usage = workspace.usage().into_diagnostic()?;
    if json {
        return print_json(&usage);
    }
    let rows = usage
        .iter()
        .map(|row| {
            vec![
                row.month.clone(),
                row.provider.clone(),
                row.requests.to_string(),
                row.successes.to_string(),
                row.failures.to_string(),
                usd(row.cost_usd),
            ]
        })
        .collect::<Vec<_>>();
    write_text(|out| {
        table(
            out,
            &[
                "MONTH",
                "PROVIDER",
                "REQUESTS",
                "SNAPSHOTS",
                "FAILURES",
                "USD",
            ],
            &rows,
        )
    })
}

pub(crate) const fn engine_of(engine: EngineArg) -> Engine {
    match engine {
        EngineArg::Google => Engine::Google,
        EngineArg::Bing => Engine::Bing,
        EngineArg::Yandex => Engine::Yandex,
    }
}

pub(crate) fn query_of(
    workspace: &SerpWorkspace,
    args: &QueryArgs,
    keyword: &str,
) -> miette::Result<SerpQuery> {
    let defaults = workspace.settings().into_diagnostic()?.defaults;
    let location = Location {
        name: args.location.clone(),
        google_canonical: args.google_location.clone(),
        yandex_lr: args.lr,
        provider_code: None,
    };
    let query = SerpQuery {
        engine: args.engine.map_or(Engine::Google, engine_of),
        keyword: keyword.to_owned(),
        country: args
            .country
            .clone()
            .unwrap_or(defaults.country)
            .to_ascii_lowercase(),
        language: args
            .language
            .clone()
            .unwrap_or(defaults.language)
            .to_ascii_lowercase(),
        location: (!location.is_empty()).then_some(location),
        device: args.device.map_or(defaults.device, |device| match device {
            DeviceArg::Desktop => Device::Desktop,
            DeviceArg::Mobile => Device::Mobile,
        }),
        depth: args.depth.unwrap_or(defaults.depth),
    };
    query.validate().into_diagnostic()?;
    Ok(query)
}

fn request_of(
    workspace: &SerpWorkspace,
    args: &QueryArgs,
    keyword: &str,
    target: Option<Target>,
) -> miette::Result<SearchRequest> {
    Ok(SearchRequest {
        query: query_of(workspace, args, keyword)?,
        providers: (!args.providers.is_empty()).then(|| args.providers.clone()),
        target,
    })
}

fn run_estimate(workspace: &SerpWorkspace, args: &QueryArgs) -> miette::Result<()> {
    let request = request_of(workspace, args, &args.keywords[0], None)?;
    let estimates = workspace.estimate(&request).into_diagnostic()?;
    let count = args.keywords.len();
    let rows = estimates
        .iter()
        .map(|estimate| {
            vec![
                estimate.provider.clone(),
                if estimate.ready {
                    "yes".to_owned()
                } else {
                    "no".to_owned()
                },
                (estimate.estimate.requests * u32::try_from(count).unwrap_or(u32::MAX)).to_string(),
                usd(estimate.estimate.usd * f64::from(u32::try_from(count).unwrap_or(u32::MAX))),
                estimate.reason.clone().unwrap_or_default(),
            ]
        })
        .collect::<Vec<_>>();
    write_text(|out| {
        table(
            out,
            &["PROVIDER", "READY", "REQUESTS", "USD", "NOTE"],
            &rows,
        )
    })
}

fn confirm_cost(workspace: &SerpWorkspace, args: &SearchArgs) -> miette::Result<()> {
    let settings = workspace.settings().into_diagnostic()?;
    let first = request_of(workspace, &args.query, &args.query.keywords[0], None)?;
    let per_keyword = workspace
        .estimate(&first)
        .into_diagnostic()?
        .into_iter()
        .find(|estimate| estimate.ready)
        .map_or(0.0, |estimate| estimate.estimate.usd);
    let keywords = f64::from(u32::try_from(args.query.keywords.len()).unwrap_or(u32::MAX));
    let total = per_keyword * keywords;
    if total > settings.confirm_above_usd {
        return Err(miette!(
            "this would cost about {} USD (limit {}); run again with --yes",
            usd(total),
            usd(settings.confirm_above_usd)
        ));
    }
    Ok(())
}

fn run_search(workspace: &SerpWorkspace, args: &SearchArgs, progress: bool) -> miette::Result<()> {
    let target = args
        .site
        .as_deref()
        .map(|site| {
            Target::parse(
                site,
                match args.match_mode {
                    MatchArg::Domain => MatchMode::Domain,
                    MatchArg::Host => MatchMode::Host,
                    MatchArg::Prefix => MatchMode::Prefix,
                },
            )
        })
        .transpose()
        .into_diagnostic()?;
    if !args.yes {
        confirm_cost(workspace, args)?;
    }
    let cancelled = Arc::new(AtomicBool::new(false));
    let handler = Arc::clone(&cancelled);
    let _ = ctrlc::set_handler(move || {
        if handler.swap(true, Ordering::SeqCst) {
            std::process::exit(130);
        }
    });
    let is_cancelled = || cancelled.load(Ordering::SeqCst);
    let mut outcomes: Vec<SearchOutcome> = Vec::new();
    let mut failures = 0;
    for keyword in &args.query.keywords {
        if is_cancelled() {
            break;
        }
        let request = request_of(workspace, &args.query, keyword, target.clone())?;
        if progress && io::stderr().is_terminal() {
            write_stderr(&format!(
                "searching `{keyword}` on {}…\n",
                request.query.engine
            ));
        }
        match workspace.search(&request, &is_cancelled, &|event| {
            print_event(event, progress);
        }) {
            Ok(outcome) => {
                if !args.json {
                    write_text(|out| print_snapshot(out, &outcome.snapshot, Some(&outcome)))?;
                }
                outcomes.push(outcome);
            }
            Err(SerpAppError::Search {
                error,
                attempts,
                diagnostic,
            }) => {
                failures += 1;
                write_stderr(&format!("`{keyword}`: {error}\n"));
                if let Some(path) = diagnostic {
                    write_stderr(&format!(
                        "  the page was saved to {}; fix the selectors with a parser override and search again\n",
                        path.display()
                    ));
                }
                for attempt in attempts {
                    write_stderr(&format!(
                        "  {}: {:?}{}\n",
                        attempt.provider,
                        attempt.outcome,
                        attempt
                            .message
                            .map(|message| format!(" — {message}"))
                            .unwrap_or_default()
                    ));
                }
            }
            Err(other) => return Err(other).into_diagnostic(),
        }
    }
    if args.json {
        print_json(&outcomes)?;
    }
    if failures > 0 {
        return Err(miette!(
            "{failures} of {} searches failed",
            args.query.keywords.len()
        ));
    }
    Ok(())
}

fn print_event(event: &SearchEvent, progress: bool) {
    let line = match event {
        SearchEvent::Waiting { provider, seconds } => {
            if !progress {
                return;
            }
            format!("{provider}: waiting {seconds} s before the next page…")
        }
        SearchEvent::Page {
            provider,
            page,
            pages,
        } => {
            if !progress {
                return;
            }
            format!("{provider}: page {page} of {pages}")
        }
        SearchEvent::Challenge { kind, url, .. } => format!(
            "The search engine shows a {} page. Please complete it in the browser window; the search continues by itself. ({url})",
            match kind {
                ChallengeKind::Consent => "consent",
                ChallengeKind::Captcha => "verification (CAPTCHA)",
                ChallengeKind::Blocked => "blocked",
            }
        ),
        SearchEvent::Resumed { .. } => "Thanks — continuing.".to_owned(),
    };
    write_stderr(&format!("{line}\n"));
}

fn print_snapshot(
    out: &mut dyn Write,
    snapshot: &SerpSnapshot,
    outcome: Option<&SearchOutcome>,
) -> io::Result<()> {
    let query = &snapshot.query;
    writeln!(
        out,
        "{} — {} {}-{} {} · {} · {}",
        query.keyword,
        query.engine,
        query.language,
        query.country,
        query.device.as_str(),
        snapshot.provider,
        snapshot.fetched_at
    )?;
    if let Some(outcome) = outcome {
        writeln!(
            out,
            "snapshot {} · {} request(s) · {} USD",
            outcome.snapshot_id,
            outcome.requests,
            usd(outcome.cost_usd)
        )?;
        if let Some(rank) = &outcome.rank {
            writeln!(out, "rank: #{} {}", rank.position, rank.url)?;
        }
    }
    if let Some(total) = snapshot.total_results {
        writeln!(out, "about {total} results")?;
    }
    let features: Vec<&str> = snapshot
        .features
        .iter()
        .map(|feature| feature.kind.as_str())
        .collect();
    if !features.is_empty() {
        writeln!(out, "features: {}", features.join(", "))?;
    }
    for warning in &snapshot.warnings {
        writeln!(out, "warning: {warning}")?;
    }
    let rows = snapshot
        .organic
        .iter()
        .map(|result| {
            vec![
                result.position.to_string(),
                result.domain.clone(),
                result.title.clone(),
                result.url.clone(),
            ]
        })
        .collect::<Vec<_>>();
    table(out, &["#", "DOMAIN", "TITLE", "URL"], &rows)?;
    writeln!(out)
}

fn run_providers(workspace: &SerpWorkspace, command: &ProvidersCommand) -> miette::Result<()> {
    match command {
        ProvidersCommand::List { json } => {
            let providers = workspace.providers().into_diagnostic()?;
            if *json {
                return print_json(&providers);
            }
            write_text(|out| print_providers(out, &providers, workspace.secrets().backend_name()))
        }
        ProvidersCommand::SetKey { provider, name } => {
            if io::stdin().is_terminal() {
                write_stderr(&format!(
                    "Paste the value of `{name}` and press Enter (it is not shown in your shell history):\n"
                ));
            }
            let mut value = String::new();
            io::stdin().lock().read_line(&mut value).into_diagnostic()?;
            workspace
                .set_secret(provider, name, value.trim())
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
            workspace.remove_secret(provider, name).into_diagnostic()?;
            write_text(|out| writeln!(out, "removed `{name}` of {provider}"))
        }
        ProvidersCommand::Add { file } => {
            let text = fs::read_to_string(file)
                .into_diagnostic()
                .wrap_err_with(|| format!("cannot read {}", file.display()))?;
            let ids = workspace.import_specs(&text).into_diagnostic()?;
            write_text(|out| writeln!(out, "added {}", ids.join(", ")))
        }
        ProvidersCommand::Remove { id } => {
            workspace.remove_spec(id).into_diagnostic()?;
            write_text(|out| writeln!(out, "removed {id}"))
        }
        ProvidersCommand::Export { ids, output } => {
            let json = workspace.export_specs(ids).into_diagnostic()?;
            match output {
                Some(path) => fs::write(path, json).into_diagnostic(),
                None => write_text(|out| writeln!(out, "{json}")),
            }
        }
        ProvidersCommand::Test {
            spec,
            response,
            engine,
            url,
            json,
        } => run_test(spec, response, *engine, url.as_deref(), *json),
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
        ProvidersCommand::Chain { .. }
        | ProvidersCommand::Enable { .. }
        | ProvidersCommand::Disable { .. }
        | ProvidersCommand::Budget { .. } => run_provider_settings(workspace, command),
    }
}

fn run_test(
    spec: &std::path::Path,
    response: &std::path::Path,
    engine: EngineArg,
    url: Option<&str>,
    json: bool,
) -> miette::Result<()> {
    let text = fs::read_to_string(spec).into_diagnostic()?;
    let specs =
        octopuson_serp_app::providers::parse_specs(&text).map_err(|error| miette!(error))?;
    let spec = specs
        .first()
        .ok_or_else(|| miette!("the file holds no spec"))?;
    let body = fs::read_to_string(response).into_diagnostic()?;
    let page = test_spec(spec, engine_of(engine), &body, url).into_diagnostic()?;
    if json {
        return print_json(&page);
    }
    write_text(|out| {
        writeln!(
            out,
            "{} of {} matched results read; {} features; error: {}",
            page.coverage.usable,
            page.coverage.matched,
            page.features.len(),
            page.error.as_deref().unwrap_or("none")
        )?;
        let rows = page
            .organic
            .iter()
            .map(|result| {
                vec![
                    result.position.to_string(),
                    result.title.clone(),
                    result.url.clone(),
                ]
            })
            .collect::<Vec<_>>();
        table(out, &["#", "TITLE", "URL"], &rows)
    })
}

fn run_provider_settings(
    workspace: &SerpWorkspace,
    command: &ProvidersCommand,
) -> miette::Result<()> {
    let mut settings = workspace.settings().into_diagnostic()?;
    match command {
        ProvidersCommand::Chain { engine, ids } => {
            let engine = engine_of(*engine);
            if !ids.is_empty() {
                settings.chains.insert(engine, ids.clone());
                workspace.save_settings(&settings).into_diagnostic()?;
            }
            let chain = settings.chains.get(&engine).cloned().unwrap_or_default();
            write_text(|out| writeln!(out, "{engine}: {}", chain.join(" → ")))
        }
        ProvidersCommand::Enable { id } | ProvidersCommand::Disable { id } => {
            let enable = matches!(command, ProvidersCommand::Enable { .. });
            if enable {
                settings.disabled.remove(id);
            } else {
                settings.disabled.insert(id.clone());
            }
            workspace.save_settings(&settings).into_diagnostic()?;
            write_text(|out| writeln!(out, "{id} {}", if enable { "enabled" } else { "disabled" }))
        }
        ProvidersCommand::Budget { id, usd: value } => {
            if value.eq_ignore_ascii_case("none") {
                settings.budgets.remove(id);
            } else {
                let budget: f64 = value
                    .parse()
                    .map_err(|_| miette!("`{value}` is not an amount in US dollars"))?;
                settings.budgets.insert(id.clone(), budget);
            }
            workspace.save_settings(&settings).into_diagnostic()?;
            write_text(|out| writeln!(out, "budget of {id}: {value} USD per month"))
        }
        _ => Ok(()),
    }
}

fn run_domains(
    workspace: &SerpWorkspace,
    domains: &[String],
    refresh: bool,
    backlinks: bool,
    json: bool,
) -> miette::Result<()> {
    let options = DomainOptions { refresh, backlinks };
    let facts = workspace
        .domain_facts(domains, options, &|| false)
        .into_diagnostic()?;
    if json {
        return print_json(&facts);
    }
    let rows = facts
        .iter()
        .map(|fact| {
            vec![
                fact.domain.clone(),
                fact.created.clone().unwrap_or_default(),
                fact.age_years
                    .map(|years| format!("{years:.1}"))
                    .unwrap_or_default(),
                fact.age_source
                    .map(|source| format!("{source:?}").to_lowercase())
                    .unwrap_or_default(),
                fact.open_page_rank
                    .map(|score| format!("{score:.2}"))
                    .unwrap_or_default(),
                fact.referring_domains
                    .map(|count| count.to_string())
                    .unwrap_or_default(),
                fact.tranco_rank
                    .map(|rank| rank.to_string())
                    .unwrap_or_default(),
                fact.errors
                    .iter()
                    .map(|(signal, error)| format!("{signal}: {error}"))
                    .collect::<Vec<_>>()
                    .join("; "),
            ]
        })
        .collect::<Vec<_>>();
    write_text(|out| {
        table(
            out,
            &[
                "DOMAIN",
                "CREATED",
                "YEARS",
                "FROM",
                "OPR",
                "REF. DOMAINS",
                "TRANCO",
                "NOTES",
            ],
            &rows,
        )
    })
}

fn run_tranco(workspace: &SerpWorkspace, accept: bool) -> miette::Result<()> {
    let dataset = workspace
        .store()
        .into_diagnostic()?
        .dataset("tranco")
        .into_diagnostic()?;
    if !accept {
        return write_text(|out| {
            writeln!(out, "{}", octopuson_domain_intel::tranco::NOTICE)?;
            match &dataset {
                Some(dataset) => writeln!(
                    out,
                    "imported {} domains (list {}) at {}",
                    dataset.rows,
                    dataset.version.as_deref().unwrap_or("?"),
                    dataset.imported_at
                ),
                None => writeln!(
                    out,
                    "not downloaded; run again with --accept-license to download it"
                ),
            }
        });
    }
    write_stderr("downloading the Tranco list…\n");
    let rows = workspace.update_tranco().into_diagnostic()?;
    write_text(|out| writeln!(out, "imported {rows} domains"))
}

fn print_providers(
    out: &mut dyn Write,
    providers: &[ProviderInfo],
    backend: &str,
) -> io::Result<()> {
    let rows = providers
        .iter()
        .map(|provider| {
            let secrets = provider
                .secrets
                .iter()
                .map(|secret| {
                    let state = match secret.state {
                        SecretState::Saved => "✓",
                        SecretState::Environment => "env",
                        SecretState::Unset => "missing",
                        SecretState::Rebind => "re-enter",
                    };
                    format!("{}:{state}", secret.name)
                })
                .collect::<Vec<_>>()
                .join(" ");
            let engines = provider
                .engines
                .iter()
                .map(|engine| engine.as_str())
                .collect::<Vec<_>>()
                .join(",");
            let state = match (&provider.error, provider.enabled, provider.ready) {
                (Some(_), _, _) => "invalid".to_owned(),
                (None, false, _) => "off".to_owned(),
                (None, true, true)
                    if provider.via == Some(octopuson_serp_app::cloud_api::Via::Credit) =>
                {
                    format!(
                        "credits ×{}",
                        provider
                            .credit
                            .as_ref()
                            .map_or(1.0, |credit| credit.price_multiplier)
                    )
                }
                (None, true, true) => "ready".to_owned(),
                (None, true, false) => "not set up".to_owned(),
            };
            vec![
                provider.id.clone(),
                provider.name.clone(),
                engines,
                state,
                secrets,
                usd(provider.per_request_usd),
                provider.budget_usd.map(usd).unwrap_or_default(),
            ]
        })
        .collect::<Vec<_>>();
    table(
        out,
        &[
            "ID", "NAME", "ENGINES", "STATE", "SECRETS", "USD/REQ", "BUDGET",
        ],
        &rows,
    )?;
    writeln!(out, "secrets are kept in the {backend} store")
}

pub(crate) fn print_json<T: serde::Serialize + ?Sized>(value: &T) -> miette::Result<()> {
    let text = serde_json::to_string_pretty(value).into_diagnostic()?;
    write_text(|out| writeln!(out, "{text}"))
}

pub(crate) fn write_text(
    render: impl FnOnce(&mut dyn Write) -> io::Result<()>,
) -> miette::Result<()> {
    let mut stdout = io::stdout().lock();
    match render(&mut stdout).and_then(|()| stdout.flush()) {
        Err(error) if error.kind() == io::ErrorKind::BrokenPipe => Ok(()),
        other => other.into_diagnostic(),
    }
}

/// Progress output is best effort; a closed stderr must not abort a search.
pub(crate) fn write_stderr(text: &str) {
    let mut stderr = io::stderr().lock();
    let _ = stderr
        .write_all(text.as_bytes())
        .and_then(|()| stderr.flush());
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::*;

    #[test]
    fn parses_searches_and_provider_commands() {
        let cli = Cli::try_parse_from([
            "octopuson-serp",
            "search",
            "kahve",
            "türk kahvesi",
            "--engine",
            "yandex",
            "--lr",
            "11508",
            "--site",
            "example.com.tr",
            "--provider",
            "yandex-cloud",
            "--yes",
        ])
        .unwrap();
        let Command::Search(search) = cli.command else {
            panic!("expected a search");
        };
        assert_eq!(search.query.keywords, ["kahve", "türk kahvesi"]);
        assert_eq!(search.query.engine, Some(EngineArg::Yandex));
        assert_eq!(search.query.providers, ["yandex-cloud"]);
        assert!(search.yes);
        assert!(
            Cli::try_parse_from([
                "octopuson-serp",
                "providers",
                "set-key",
                "serper",
                "api_key"
            ])
            .is_ok()
        );
        assert!(
            Cli::try_parse_from([
                "octopuson-serp",
                "providers",
                "chain",
                "google",
                "serper",
                "browser"
            ])
            .is_ok()
        );
        assert!(Cli::try_parse_from(["octopuson-serp", "search"]).is_err());
    }
}
