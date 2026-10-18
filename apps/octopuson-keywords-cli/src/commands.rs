use std::io::{self, BufRead, IsTerminal, Write};

use miette::{IntoDiagnostic, miette};
use octopuson_keywords::{Level, Mode};
use octopuson_kw_app::KeywordWorkspace;
use octopuson_kw_app::cluster::ClusterRequest;
use octopuson_kw_app::settings::Method;
use octopuson_kw_app::store::{
    ClusterQuery, KeywordQuery, KeywordRow, KeywordSort, KeywordStatus, ProjectRow, ProjectSpec,
};

use crate::cli::{
    Cli, ClusterArgs, Command, ExportCommand, KeywordsArgs, LevelArg, MethodArg, ModeArg,
    ProjectArgs, ProjectCommand, SortArg,
};
use crate::output::{table, usd};

pub(crate) fn run(args: &Cli) -> miette::Result<()> {
    let workspace = KeywordWorkspace::resolve(args.data_dir.clone()).into_diagnostic()?;
    match &args.command {
        Command::Project(command) => run_project(&workspace, command),
        Command::Add {
            project,
            keywords,
            source,
        } => run_add(&workspace, *project, keywords, source),
        Command::Import {
            project,
            file,
            preview,
            json,
        } => run_import(&workspace, *project, file, *preview, *json),
        Command::Keywords(keywords) => run_keywords(&workspace, keywords),
        Command::Exclude { project, ids } => {
            let changed = workspace
                .set_status(*project, ids, KeywordStatus::Excluded)
                .into_diagnostic()?;
            write_text(|out| writeln!(out, "{changed} keywords excluded"))
        }
        Command::Include { project, ids } => {
            let changed = workspace
                .set_status(*project, ids, KeywordStatus::Active)
                .into_diagnostic()?;
            write_text(|out| writeln!(out, "{changed} keywords included"))
        }
        Command::Discover(discover) => crate::runs::run_discover(&workspace, discover),
        Command::Enrich(enrich) => crate::runs::run_enrich(&workspace, enrich),
        Command::Providers(command) => crate::runs::run_providers(&workspace, command),
        Command::Google(command) => crate::google::run_google(&workspace, command),
        Command::Account(command) => crate::account::run_account(&workspace, command),
        Command::Serp(serp) => crate::runs::run_serp(&workspace, serp),
        Command::Map { project } => {
            let clustering = workspace.remap(*project).into_diagnostic()?;
            write_text(|out| {
                writeln!(
                    out,
                    "mapped clustering {} again: {} clusters",
                    clustering.id, clustering.clusters
                )
            })
        }
        Command::Suggest(suggest) => crate::runs::run_suggest(&workspace, suggest),
        Command::Browser(browser) => crate::runs::run_browser(&workspace, browser),
        Command::Cluster(cluster) => run_cluster(&workspace, cluster),
        Command::Clusters {
            project,
            clustering,
            search,
            limit,
            json,
        } => run_clusters(
            &workspace,
            *project,
            *clustering,
            search.as_deref(),
            *limit,
            *json,
        ),
        Command::Export(export) => run_export(&workspace, export),
        Command::Usage { json } => {
            let usage = workspace.usage().into_diagnostic()?;
            if *json {
                return print_json(&usage);
            }
            let rows: Vec<Vec<String>> = usage
                .iter()
                .map(|row| {
                    vec![
                        row.month.clone(),
                        row.provider.clone(),
                        row.requests.to_string(),
                        row.items.to_string(),
                        row.failures.to_string(),
                        usd(row.cost_usd),
                    ]
                })
                .collect();
            write_text(|out| {
                table(
                    out,
                    &["month", "provider", "requests", "items", "failures", "usd"],
                    &rows,
                )
            })
        }
    }
}

fn run_project(workspace: &KeywordWorkspace, command: &ProjectCommand) -> miette::Result<()> {
    match command {
        ProjectCommand::Create(args) => {
            let project = workspace
                .create_project(&spec_of(workspace, args)?)
                .into_diagnostic()?;
            write_text(|out| writeln!(out, "created project {}: {}", project.id, project.spec.name))
        }
        ProjectCommand::List { json } => {
            let projects = workspace.projects().into_diagnostic()?;
            if *json {
                return print_json(&projects);
            }
            let rows: Vec<Vec<String>> = projects.iter().map(project_row).collect();
            write_text(|out| {
                table(
                    out,
                    &[
                        "id", "name", "market", "engines", "site", "keywords", "excluded",
                        "clusters",
                    ],
                    &rows,
                )
            })
        }
        ProjectCommand::Show { id, json } => {
            let project = workspace.project(*id).into_diagnostic()?;
            if *json {
                return print_json(&project);
            }
            write_text(|out| {
                writeln!(out, "{} (project {})", project.spec.name, project.id)?;
                writeln!(
                    out,
                    "market       {}-{}",
                    project.spec.language, project.spec.country
                )?;
                writeln!(out, "engines      {}", project.spec.engines.join(", "))?;
                writeln!(
                    out,
                    "site         {}",
                    project.spec.site.as_deref().unwrap_or("–")
                )?;
                writeln!(
                    out,
                    "competitors  {}",
                    or_dash(&project.spec.competitors.join(", "))
                )?;
                writeln!(
                    out,
                    "keywords     {} ({} excluded)",
                    project.keywords, project.excluded
                )?;
                writeln!(
                    out,
                    "clusters     {}",
                    project
                        .clusters
                        .map_or_else(|| "–".to_owned(), |count| count.to_string())
                )?;
                writeln!(out, "created      {}", project.created_at)
            })
        }
        ProjectCommand::Delete { id, yes } => {
            let project = workspace.project(*id).into_diagnostic()?;
            if !yes {
                confirm(&format!(
                    "Delete project {} \"{}\" with its {} keywords?",
                    project.id,
                    project.spec.name,
                    project.keywords + project.excluded
                ))?;
            }
            workspace.delete_project(*id).into_diagnostic()?;
            write_text(|out| writeln!(out, "deleted project {id}"))
        }
    }
}

fn spec_of(workspace: &KeywordWorkspace, args: &ProjectArgs) -> miette::Result<ProjectSpec> {
    let defaults = workspace.settings().into_diagnostic()?.projects;
    Ok(ProjectSpec {
        name: args.name.clone(),
        country: args.country.clone().unwrap_or(defaults.country),
        language: args.language.clone().unwrap_or(defaults.language),
        engines: if args.engines.is_empty() {
            defaults.engines
        } else {
            args.engines
                .iter()
                .map(|engine| engine.as_str().to_owned())
                .collect()
        },
        site: args.site.clone(),
        competitors: args.competitors.clone(),
    })
}

fn project_row(project: &ProjectRow) -> Vec<String> {
    vec![
        project.id.to_string(),
        project.spec.name.clone(),
        format!("{}-{}", project.spec.language, project.spec.country),
        project.spec.engines.join(", "),
        project.spec.site.clone().unwrap_or_default(),
        project.keywords.to_string(),
        project.excluded.to_string(),
        project
            .clusters
            .map(|count| count.to_string())
            .unwrap_or_default(),
    ]
}

fn run_add(
    workspace: &KeywordWorkspace,
    project: i64,
    keywords: &[String],
    source: &str,
) -> miette::Result<()> {
    let keywords = if keywords.is_empty() {
        let stdin = io::stdin();
        if stdin.is_terminal() {
            return Err(miette!(
                "give keywords as arguments or pipe them in, one per line"
            ));
        }
        stdin
            .lock()
            .lines()
            .collect::<io::Result<Vec<String>>>()
            .into_diagnostic()?
            .into_iter()
            .filter(|line| !line.trim().is_empty())
            .collect()
    } else {
        keywords.to_vec()
    };
    let outcome = workspace
        .add_keywords(project, &keywords, source, None)
        .into_diagnostic()?;
    write_text(|out| {
        writeln!(
            out,
            "{} added, {} already in the project, {} not keywords",
            outcome.added.added, outcome.added.known, outcome.rejected
        )
    })
}

fn run_import(
    workspace: &KeywordWorkspace,
    project: i64,
    file: &std::path::Path,
    preview: bool,
    json: bool,
) -> miette::Result<()> {
    if preview {
        let preview = workspace.preview_import(file).into_diagnostic()?;
        if json {
            return print_json(&preview);
        }
        let header: Vec<String> = preview
            .columns
            .iter()
            .map(|column| match &column.role {
                Some(role) => format!("{} → {}", or_dash(&column.header), role_name(role)),
                None => format!("{} (left out)", or_dash(&column.header)),
            })
            .collect();
        let header: Vec<&str> = header.iter().map(String::as_str).collect();
        return write_text(|out| {
            table(out, &header, &preview.rows)?;
            writeln!(out, "{} data rows", preview.total)
        });
    }
    let outcome = workspace.import(project, file).into_diagnostic()?;
    if json {
        return print_json(&outcome);
    }
    write_text(|out| {
        writeln!(
            out,
            "{} rows: {} keywords added, {} already in the project, {} not keywords; numbers for {}",
            outcome.rows,
            outcome.keywords.added.added,
            outcome.keywords.added.known,
            outcome.keywords.rejected,
            outcome.metrics
        )?;
        let read: Vec<String> = outcome
            .columns
            .iter()
            .filter_map(|column| {
                column
                    .role
                    .as_ref()
                    .map(|role| format!("{} → {}", or_dash(&column.header), role_name(role)))
            })
            .collect();
        writeln!(out, "columns read: {}", read.join(", "))
    })
}

fn role_name(role: &octopuson_kw_app::import::ColumnRole) -> String {
    use octopuson_kw_app::import::ColumnRole;
    match role {
        ColumnRole::Keyword => "keyword".to_owned(),
        ColumnRole::Volume => "volume".to_owned(),
        ColumnRole::Cpc => "cpc".to_owned(),
        ColumnRole::Competition | ColumnRole::CompetitionIndex => "competition".to_owned(),
        ColumnRole::Difficulty => "difficulty".to_owned(),
        ColumnRole::Clicks => "clicks".to_owned(),
        ColumnRole::Impressions => "impressions".to_owned(),
        ColumnRole::Position => "position".to_owned(),
        ColumnRole::Month(month) => format!("searches {month}"),
    }
}

const fn sort_of(sort: SortArg) -> KeywordSort {
    match sort {
        SortArg::Keyword => KeywordSort::Keyword,
        SortArg::Volume => KeywordSort::Volume,
        SortArg::Cpc => KeywordSort::Cpc,
        SortArg::Competition => KeywordSort::Competition,
        SortArg::Difficulty => KeywordSort::Difficulty,
        SortArg::Intent => KeywordSort::Intent,
        SortArg::Cluster => KeywordSort::Cluster,
        SortArg::Added => KeywordSort::Added,
    }
}

fn run_keywords(workspace: &KeywordWorkspace, args: &KeywordsArgs) -> miette::Result<()> {
    let query = KeywordQuery {
        search: args.search.clone(),
        status: Some(if args.excluded {
            KeywordStatus::Excluded
        } else {
            KeywordStatus::Active
        }),
        tag: args.tag.clone(),
        source: args.source.clone(),
        intent: args.intent.clone(),
        min_volume: args.min_volume,
        max_volume: args.max_volume,
        cluster: args.cluster.map(|number| number.saturating_sub(1)),
        sort: sort_of(args.sort),
        descending: !args.ascending,
        offset: args.offset,
        limit: args.limit,
    };
    let page = workspace.keywords(args.project, &query).into_diagnostic()?;
    if args.json {
        return print_json(&page);
    }
    let rows: Vec<Vec<String>> = page.rows.iter().map(keyword_row).collect();
    write_text(|out| {
        table(
            out,
            &[
                "id", "keyword", "volume", "cpc", "comp", "kd", "intent", "cluster", "sources",
            ],
            &rows,
        )?;
        writeln!(out, "{} of {} keywords", page.rows.len(), page.total)
    })
}

fn keyword_row(row: &KeywordRow) -> Vec<String> {
    vec![
        row.id.to_string(),
        row.keyword.clone(),
        row.volume
            .map(|volume| volume.to_string())
            .unwrap_or_default(),
        row.cpc_usd.map(usd).unwrap_or_default(),
        row.competition
            .map(|value| format!("{value:.2}"))
            .unwrap_or_default(),
        row.difficulty
            .map(|value| format!("{value:.0}"))
            .unwrap_or_default(),
        row.intent.clone().unwrap_or_default(),
        row.cluster
            .map(|number| format!("{}{}", number + 1, if row.pillar { " ★" } else { "" }))
            .unwrap_or_default(),
        row.sources.join(", "),
    ]
}

fn run_cluster(workspace: &KeywordWorkspace, args: &ClusterArgs) -> miette::Result<()> {
    let defaults = workspace.settings().into_diagnostic()?.clustering;
    let mut request = ClusterRequest::from_defaults(&defaults);
    if let Some(method) = args.method {
        request.method = match method {
            MethodArg::Lexical => Method::Lexical,
            MethodArg::Serp => Method::Serp,
            MethodArg::Hybrid => Method::Hybrid,
        };
    }
    if let Some(threshold) = args.threshold {
        request.threshold = threshold;
    }
    if let Some(threshold) = args.serp_threshold {
        request.serp_threshold = threshold;
    }
    if let Some(mode) = args.mode {
        request.mode = match mode {
            ModeArg::Soft => Mode::Soft,
            ModeArg::Hard => Mode::Hard,
        };
    }
    if let Some(level) = args.level {
        request.level = match level {
            LevelArg::Page => Level::Page,
            LevelArg::Site => Level::Site,
        };
    }
    request.engine = args.engine.map(|engine| engine.as_str().to_owned());
    request.device.clone_from(&args.device);
    let outcome = workspace
        .cluster(args.project, &request)
        .into_diagnostic()?;
    if args.json {
        return print_json(&outcome);
    }
    write_text(|out| {
        writeln!(
            out,
            "clustering {}: {} keywords in {} clusters ({}; {} verified by result pages, {} keywords with a result page, {} placed by hand)",
            outcome.clustering.id,
            outcome.clustering.keywords,
            outcome.clustering.clusters,
            outcome.clustering.spec.method,
            outcome.verified,
            outcome.with_serp,
            outcome.placed
        )?;
        if outcome.level == Level::Site && request.level == Level::Page {
            writeln!(
                out,
                "some result pages hold sites only (Google hidden links), so sites were compared"
            )?;
        }
        Ok(())
    })
}

fn run_clusters(
    workspace: &KeywordWorkspace,
    project: i64,
    clustering: Option<i64>,
    search: Option<&str>,
    limit: u64,
    json: bool,
) -> miette::Result<()> {
    let store = workspace.store().into_diagnostic()?;
    let clustering = match clustering {
        Some(id) => store.clustering(id).into_diagnostic()?,
        None => store
            .latest_clustering(project)
            .into_diagnostic()?
            .ok_or_else(|| {
                miette!("project {project} has no clustering yet; run `octopuson-keywords cluster {project}`")
            })?,
    };
    let language = store.project(project).into_diagnostic()?.spec.language;
    let search = search
        .and_then(|text| {
            octopuson_keywords::normalize(text, octopuson_kw_app::text_language(&language))
        })
        .map(|normalized| normalized.key);
    let page = store
        .clusters(
            clustering.id,
            &ClusterQuery {
                search,
                limit,
                ..ClusterQuery::default()
            },
        )
        .into_diagnostic()?;
    if json {
        return print_json(&page);
    }
    let rows: Vec<Vec<String>> = page
        .rows
        .iter()
        .map(|cluster| {
            vec![
                (cluster.number + 1).to_string(),
                cluster.label.clone(),
                cluster.size.to_string(),
                cluster.volume.to_string(),
                format!(
                    "{}{}",
                    cluster.intent.as_deref().unwrap_or(""),
                    if cluster.mixed_intent { " (mixed)" } else { "" }
                ),
                cluster
                    .difficulty
                    .map(|value| format!("{value:.0}"))
                    .unwrap_or_default(),
                if cluster.verified { "yes" } else { "" }.to_owned(),
                cluster.target_url.clone().unwrap_or_else(|| {
                    if cluster.gap {
                        "gap".to_owned()
                    } else {
                        String::new()
                    }
                }),
            ]
        })
        .collect();
    write_text(|out| {
        table(
            out,
            &[
                "#", "pillar", "keywords", "volume", "intent", "kd", "verified", "page",
            ],
            &rows,
        )?;
        writeln!(
            out,
            "{} of {} clusters (clustering {}, {})",
            page.rows.len(),
            page.total,
            clustering.id,
            clustering.spec.method
        )
    })
}

fn run_export(workspace: &KeywordWorkspace, export: &ExportCommand) -> miette::Result<()> {
    let (written, file) = match export {
        ExportCommand::Keywords {
            project,
            file,
            excluded,
        } => {
            let query = KeywordQuery {
                status: Some(if *excluded {
                    KeywordStatus::Excluded
                } else {
                    KeywordStatus::Active
                }),
                ..KeywordQuery::default()
            };
            (
                workspace
                    .export_keywords(*project, &query, file)
                    .into_diagnostic()?,
                file,
            )
        }
        ExportCommand::Clusters {
            project,
            file,
            clustering,
        } => (
            workspace
                .export_clusters(*project, *clustering, file)
                .into_diagnostic()?,
            file,
        ),
    };
    write_text(|out| writeln!(out, "wrote {written} rows to {}", file.display()))
}

fn or_dash(text: &str) -> &str {
    if text.is_empty() { "–" } else { text }
}

pub(crate) fn confirm(question: &str) -> miette::Result<()> {
    let stdin = io::stdin();
    if !stdin.is_terminal() {
        return Err(miette!("{question} Pass --yes to confirm."));
    }
    write_stderr(&format!("{question} [y/N] "));
    let mut answer = String::new();
    stdin.lock().read_line(&mut answer).into_diagnostic()?;
    if matches!(answer.trim(), "y" | "Y" | "yes" | "e" | "E" | "evet") {
        Ok(())
    } else {
        Err(miette!("cancelled"))
    }
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

/// Questions are best effort; a closed stderr must not abort.
pub(crate) fn write_stderr(text: &str) {
    let mut stderr = io::stderr().lock();
    let _ = stderr
        .write_all(text.as_bytes())
        .and_then(|()| stderr.flush());
}
