//! `analyze`, `analyses`, `analysis` and `rescore`.

use std::io::{self, IsTerminal, Write};

use miette::{IntoDiagnostic, miette};
use octopuson_serp::{MatchMode, SerpSnapshot, Target};
use octopuson_serp_app::analysis::{AnalysisEvent, AnalyzeRequest};
use octopuson_serp_app::crawl::{CrawlIdentity, PageCrawlOptions};
use octopuson_serp_app::domains::DomainFacts;
use octopuson_serp_app::{SearchRequest, SerpWorkspace};
use octopuson_serp_score::{Breakdown, KeywordAnalysis};
use octopuson_spider_app::CrawlControl;

use crate::cli::{AnalyzeArgs, MatchArg};
use crate::commands::{print_json, query_of, write_stderr, write_text};
use crate::output::table;

pub(crate) fn run_analyze(
    workspace: &SerpWorkspace,
    args: &AnalyzeArgs,
    progress: bool,
) -> miette::Result<()> {
    let [keyword] = args.query.keywords.as_slice() else {
        return Err(miette!("analyze one keyword at a time"));
    };
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
    let request = AnalyzeRequest {
        search: SearchRequest {
            query: query_of(workspace, &args.query, keyword)?,
            providers: (!args.query.providers.is_empty()).then(|| args.query.providers.clone()),
            target,
        },
        pages: args.pages,
        crawl: PageCrawlOptions {
            javascript: args.javascript,
            identity: CrawlIdentity::Octopus,
        },
        target_url: args.page.clone(),
        volume: args.volume,
        backlinks: args.backlinks,
    };
    if !args.yes {
        let settings = workspace.settings().into_diagnostic()?;
        let cost = workspace
            .estimate(&request.search)
            .into_diagnostic()?
            .into_iter()
            .find(|estimate| estimate.ready)
            .map_or(0.0, |estimate| estimate.estimate.usd);
        if cost > settings.confirm_above_usd {
            return Err(miette!(
                "the search would cost about {cost:.3} USD; run again with --yes"
            ));
        }
    }
    let control = std::sync::Arc::new(CrawlControl::new());
    let handler = std::sync::Arc::clone(&control);
    let _ = ctrlc::set_handler(move || {
        if handler.is_stopped() {
            std::process::exit(130);
        }
        handler.stop();
    });
    let live = progress && io::stderr().is_terminal();
    let outcome = workspace
        .analyze(&request, &control, &|event| {
            if !live {
                return;
            }
            let line = match event {
                AnalysisEvent::Search { event } => format!("search: {event:?}"),
                AnalysisEvent::Crawl { done, total } => format!("crawling {done}/{total}"),
                AnalysisEvent::Domains { count } => format!("looking up {count} domains"),
                AnalysisEvent::Volume => "monthly searches".to_owned(),
                AnalysisEvent::Scoring => "scoring".to_owned(),
            };
            write_stderr(&format!("{line}\n"));
        })
        .into_diagnostic()?;
    if args.json {
        return print_json(&outcome);
    }
    write_text(|out| {
        writeln!(
            out,
            "analysis {} · snapshot {}",
            outcome.analysis_id, outcome.snapshot_id
        )?;
        print_analysis(out, &outcome.analysis, &outcome.snapshot, &outcome.domains)
    })
}

pub(crate) fn run_analyses(
    workspace: &SerpWorkspace,
    keyword: Option<&str>,
    limit: usize,
    json: bool,
) -> miette::Result<()> {
    let analyses = workspace.analyses(keyword, limit).into_diagnostic()?;
    if json {
        return print_json(&analyses);
    }
    let rows = analyses
        .iter()
        .map(|analysis| {
            vec![
                analysis.id.to_string(),
                analysis.created_at.clone(),
                analysis.keyword.clone(),
                format!("{} {}", analysis.engine, analysis.country),
                format!("{:.0}", analysis.difficulty),
                analysis.band.clone(),
                analysis.intent.clone(),
                analysis
                    .opportunity
                    .map(|score| format!("{score:.0}"))
                    .unwrap_or_default(),
                analysis
                    .volume
                    .map(|volume| volume.to_string())
                    .unwrap_or_default(),
            ]
        })
        .collect::<Vec<_>>();
    write_text(|out| {
        table(
            out,
            &[
                "ID", "CREATED", "KEYWORD", "ENGINE", "KD", "BAND", "INTENT", "OPP.", "VOLUME",
            ],
            &rows,
        )
    })
}

pub(crate) fn run_analysis(workspace: &SerpWorkspace, id: i64, json: bool) -> miette::Result<()> {
    let (analysis, snapshot, domains) = workspace.analysis(id).into_diagnostic()?;
    if json {
        return print_json(&analysis);
    }
    write_text(|out| print_analysis(out, &analysis, &snapshot, &domains))
}

pub(crate) fn run_rescore(workspace: &SerpWorkspace, id: i64) -> miette::Result<()> {
    let analysis = workspace.rescore(id).into_diagnostic()?;
    write_text(|out| {
        writeln!(
            out,
            "analysis {id} scored again: difficulty {:.0} ({:?}) with {}",
            analysis.difficulty.score, analysis.band, analysis.models.difficulty
        )
    })
}

fn score(breakdown: Option<&Breakdown>) -> String {
    breakdown
        .map(|breakdown| format!("{:.0}", breakdown.score))
        .unwrap_or_default()
}

fn print_analysis(
    out: &mut dyn Write,
    analysis: &KeywordAnalysis,
    snapshot: &SerpSnapshot,
    domains: &[DomainFacts],
) -> io::Result<()> {
    let query = &snapshot.query;
    writeln!(
        out,
        "{} — {} {}-{} {}",
        query.keyword,
        query.engine,
        query.language,
        query.country,
        query.device.as_str()
    )?;
    writeln!(
        out,
        "difficulty {:.0}/100 ({:?}, confidence {:.0}%) · intent {:?}{} · volume {}",
        analysis.difficulty.score,
        analysis.band,
        analysis.difficulty.confidence * 100.0,
        analysis.intent.primary,
        analysis
            .intent
            .secondary
            .map(|intent| format!(" / {intent:?}"))
            .unwrap_or_default(),
        analysis
            .volume
            .map_or_else(|| "unknown".to_owned(), |volume| volume.to_string())
    )?;
    for component in &analysis.difficulty.components {
        writeln!(out, "  {:<12} {:+.1}", component.id, component.contribution)?;
    }
    let age = |domain: &str| {
        domains
            .iter()
            .find(|facts| facts.domain == domain)
            .and_then(|facts| facts.age_years)
            .map(|years| format!("{years:.0}y"))
            .unwrap_or_default()
    };
    let rows = analysis
        .pages
        .iter()
        .map(|page| {
            vec![
                page.position.to_string(),
                page.domain.clone(),
                score(page.content.as_ref()),
                score(page.technical.as_ref()),
                score(page.site.as_ref()),
                page.strength
                    .map(|strength| format!("{strength:.0}"))
                    .unwrap_or_default(),
                page.words.to_string(),
                age(&page.domain),
            ]
        })
        .collect::<Vec<_>>();
    table(
        out,
        &[
            "#", "DOMAIN", "CONTENT", "TECH", "SITE", "STRENGTH", "WORDS", "AGE",
        ],
        &rows,
    )?;
    let recommendations = &analysis.recommendations;
    if let Some(words) = recommendations.words {
        writeln!(
            out,
            "write {:.0}–{:.0} words (median {:.0})",
            words.low, words.high, words.median
        )?;
    }
    if !recommendations.schema.is_empty() {
        writeln!(
            out,
            "structured data: {}",
            recommendations.schema.join(", ")
        )?;
    }
    if !recommendations.headings.is_empty() {
        writeln!(out, "headings: {}", recommendations.headings.join(" · "))?;
    }
    if !recommendations.questions.is_empty() {
        writeln!(out, "answer: {}", recommendations.questions.join(" | "))?;
    }
    if !recommendations.terms.is_empty() {
        writeln!(out, "terms: {}", recommendations.terms.join(", "))?;
    }
    if let Some(target) = &analysis.target {
        writeln!(
            out,
            "{}: position {} · opportunity {:.0}/100",
            target.domain,
            target
                .position
                .map_or_else(|| "–".to_owned(), |position| position.to_string()),
            target.opportunity.score
        )?;
    }
    Ok(())
}
