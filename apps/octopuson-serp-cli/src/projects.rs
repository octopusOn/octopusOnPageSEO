//! `project` and `track`.

use std::fs;
use std::io::{self, IsTerminal, Write};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use miette::{IntoDiagnostic, miette};
use octopuson_serp::{Device, MatchMode, Target};
use octopuson_serp_app::SerpWorkspace;
use octopuson_serp_app::rank::{ProjectSpec, RankReport, Schedule, TrackEvent, TrackerSpec};

use crate::cli::{DeviceArg, MatchArg, ProjectCommand, ScheduleArg};
use crate::commands::{engine_of, print_json, write_stderr, write_text};
use crate::output::{table, usd};

pub(crate) fn run_project(
    workspace: &SerpWorkspace,
    command: &ProjectCommand,
) -> miette::Result<()> {
    match command {
        ProjectCommand::Create { .. } => create(workspace, command),
        ProjectCommand::List { json } => list(workspace, *json),
        ProjectCommand::Show { id, json } => {
            let report = workspace.rank_report(*id).into_diagnostic()?;
            if *json {
                return print_json(&report);
            }
            write_text(|out| print_report(out, &report))
        }
        ProjectCommand::Delete { id } => {
            workspace.delete_project(*id).into_diagnostic()?;
            write_text(|out| writeln!(out, "deleted project {id}"))
        }
        ProjectCommand::AddKeywords {
            id,
            keywords,
            file,
            tags,
        } => {
            let mut all = keywords.clone();
            if let Some(file) = file {
                let text = fs::read_to_string(file).into_diagnostic()?;
                all.extend(
                    text.lines()
                        .map(str::trim)
                        .filter(|line| !line.is_empty())
                        .map(str::to_owned),
                );
            }
            if all.is_empty() {
                return Err(miette!("no keywords given"));
            }
            let added = workspace.add_keywords(*id, &all, tags).into_diagnostic()?;
            write_text(|out| writeln!(out, "added {added} of {} keywords", all.len()))
        }
        ProjectCommand::RemoveKeyword { id, keyword } => {
            workspace.remove_keyword(*id, *keyword).into_diagnostic()?;
            write_text(|out| writeln!(out, "removed keyword {keyword}"))
        }
        ProjectCommand::Competitors { id, domains } => {
            workspace.set_competitors(*id, domains).into_diagnostic()?;
            write_text(|out| writeln!(out, "competitors: {}", domains.join(", ")))
        }
    }
}

fn create(workspace: &SerpWorkspace, command: &ProjectCommand) -> miette::Result<()> {
    let ProjectCommand::Create {
        name,
        site,
        match_mode,
        engine,
        country,
        language,
        device,
        depth,
        schedule,
        providers,
    } = command
    else {
        return Ok(());
    };

    let mode = match match_mode {
        MatchArg::Domain => MatchMode::Domain,
        MatchArg::Host => MatchMode::Host,
        MatchArg::Prefix => MatchMode::Prefix,
    };
    let project = workspace
        .create_project(&ProjectSpec {
            name: name.clone(),
            target: Target::parse(site, mode).into_diagnostic()?,
            schedule: match schedule {
                ScheduleArg::Daily => Schedule::Daily,
                ScheduleArg::Weekly => Schedule::Weekly,
                ScheduleArg::Manual => Schedule::Manual,
            },
            trackers: vec![TrackerSpec {
                engine: engine_of(*engine),
                country: country.to_ascii_lowercase(),
                language: language.to_ascii_lowercase(),
                device: match device {
                    DeviceArg::Desktop => Device::Desktop,
                    DeviceArg::Mobile => Device::Mobile,
                },
                depth: *depth,
                location: None,
            }],
            providers: (!providers.is_empty()).then(|| providers.clone()),
        })
        .into_diagnostic()?;
    write_text(|out| writeln!(out, "created project {} ({})", project.id, project.name))
}

fn list(workspace: &SerpWorkspace, json: bool) -> miette::Result<()> {
    let projects = workspace.projects().into_diagnostic()?;
    if json {
        return print_json(&projects);
    }
    let rows = projects
        .iter()
        .map(|project| {
            vec![
                project.id.to_string(),
                project.name.clone(),
                project.target.value.clone(),
                project
                    .trackers
                    .iter()
                    .map(|tracker| {
                        format!(
                            "{} {}-{} {}",
                            tracker.engine,
                            tracker.language,
                            tracker.country,
                            tracker.device.as_str()
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(", "),
                project.keywords.to_string(),
                format!("{:?}", project.schedule).to_lowercase(),
                project.last_run_at.clone().unwrap_or_default(),
            ]
        })
        .collect::<Vec<_>>();
    write_text(|out| {
        table(
            out,
            &[
                "ID",
                "NAME",
                "SITE",
                "TRACKED ON",
                "KEYWORDS",
                "SCHEDULE",
                "LAST CHECK",
            ],
            &rows,
        )
    })
}

pub(crate) fn run_track(
    workspace: &SerpWorkspace,
    id: Option<i64>,
    due: bool,
    yes: bool,
    progress: bool,
) -> miette::Result<()> {
    let ids = match (id, due) {
        (Some(id), false) => vec![id],
        (None, true) => workspace.due_projects().into_diagnostic()?,
        _ => return Err(miette!("give a project id or --due")),
    };
    if ids.is_empty() {
        return write_text(|out| writeln!(out, "no project is due"));
    }
    let settings = workspace.settings().into_diagnostic()?;
    let cancelled = Arc::new(AtomicBool::new(false));
    let handler = Arc::clone(&cancelled);
    let _ = ctrlc::set_handler(move || {
        if handler.swap(true, Ordering::SeqCst) {
            std::process::exit(130);
        }
    });
    let live = progress && io::stderr().is_terminal();
    for id in ids {
        let estimate = workspace.estimate_track(id).into_diagnostic()?;
        if !yes && estimate.usd > settings.confirm_above_usd {
            write_stderr(&format!(
                "project {id}: {} searches would cost about {} USD; run again with --yes\n",
                estimate.searches,
                usd(estimate.usd)
            ));
            continue;
        }
        let summary = workspace
            .track(id, &|| cancelled.load(Ordering::SeqCst), &|event| {
                if let TrackEvent::Keyword {
                    done,
                    total,
                    keyword,
                    engine,
                } = event
                    && live
                {
                    write_stderr(&format!("[{}/{total}] {engine}: {keyword}\n", done + 1));
                }
            })
            .into_diagnostic()?;
        write_text(|out| {
            writeln!(
                out,
                "project {id}: {} checked, {} failed, {:?}, {} USD",
                summary.checked,
                summary.failed,
                summary.status,
                usd(summary.cost_usd)
            )
        })?;
    }
    Ok(())
}

fn print_report(out: &mut dyn Write, report: &RankReport) -> io::Result<()> {
    let project = &report.project;
    writeln!(
        out,
        "{} — {} ({} keywords)",
        project.name, project.target.value, project.keywords
    )?;
    if let Some(point) = report.visibility.last() {
        writeln!(
            out,
            "visibility {:.1}% on {} · top 3: {} · top 10: {} · ranking: {}",
            point.visibility, point.date, point.top3, point.top10, point.ranked
        )?;
    }
    let trackers: std::collections::HashMap<i64, String> = project
        .trackers
        .iter()
        .map(|tracker| {
            (
                tracker.id,
                format!("{} {}", tracker.engine, tracker.device.as_str()),
            )
        })
        .collect();
    let rows = report
        .rows
        .iter()
        .map(|row| {
            vec![
                row.keyword_id.to_string(),
                row.keyword.clone(),
                trackers.get(&row.tracker_id).cloned().unwrap_or_default(),
                row.position
                    .map_or_else(|| "–".to_owned(), |position| position.to_string()),
                row.change
                    .map(|change| format!("{change:+}"))
                    .unwrap_or_default(),
                row.best.map(|best| best.to_string()).unwrap_or_default(),
                row.volume
                    .map(|volume| volume.to_string())
                    .unwrap_or_default(),
                row.url.clone().unwrap_or_default(),
                row.error.clone().unwrap_or_default(),
            ]
        })
        .collect::<Vec<_>>();
    table(
        out,
        &[
            "ID", "KEYWORD", "ON", "POS", "CHANGE", "BEST", "VOLUME", "URL", "ERROR",
        ],
        &rows,
    )?;
    let shares = report
        .competitors
        .iter()
        .map(|share| {
            vec![
                if share.own {
                    format!("{} (you)", share.domain)
                } else {
                    share.domain.clone()
                },
                format!("{:.1}%", share.visibility),
                share.keywords.to_string(),
                share
                    .average_position
                    .map(|position| format!("{position:.1}"))
                    .unwrap_or_default(),
            ]
        })
        .collect::<Vec<_>>();
    table(
        out,
        &["DOMAIN", "VISIBILITY", "KEYWORDS", "AVG. POS"],
        &shares,
    )
}
