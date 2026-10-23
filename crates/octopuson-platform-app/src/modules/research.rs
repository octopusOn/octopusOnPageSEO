//! Keyword research in depth: a keyword's searches since 2021, its result pages month by month,
//! the weight of each result (authority, linking sites, traffic), its parent topic and traffic
//! potential, its searches in other countries, and keyword lists grouped by the result pages
//! they share.
//!
//! Paid reports go through `research_estimate`, `get_research` and `fetch_research` with a
//! `report` argument; the result-page weights and the parent topic read the result page kept by
//! the Keyword Explorer, so it is fetched there first. Grouping a list fetches each keyword's
//! latest result page as a job (`start_serp_clustering`); the groups are then drawn from what
//! is kept, free (`serp_clusters`).

use std::collections::BTreeMap;
use std::sync::Arc;

use octopuson_dataforseo::keywords::{self as labs, KeywordData};
use octopuson_dataforseo::research::{self, PageReach};
use octopuson_dataforseo::site::{self, RankedKeyword};
use octopuson_dataforseo::{Client, Market, backlinks, serp};
use octopuson_keywords::serp::{Mode, SerpInput, SerpOptions, cluster_serp};
use octopuson_platform::{CacheClass, ListKeyword};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::fetch::{Estimate, Fetched, Paid, Route};
use crate::jobs::{JobDone, JobStatus};
use crate::router::{Context, Router};
use crate::{AppError, Workspace};

/// Module id, in spending records.
const MODULE: &str = "research";

/// Job kind of grouping a list by result pages.
const JOB_KIND: &str = "serp_clusters";

/// Months of past result pages unless asked otherwise (DataForSEO keeps a year).
const SERP_MONTHS: u32 = 12;

/// Keywords of a page read for its parent topic.
const PAGE_ROWS: u32 = 100;

/// Most markets a keyword is measured in at once.
const MAX_MARKETS: usize = 10;

/// Markets a keyword is measured in unless others are asked: big search markets for Turkish
/// agencies' clients.
const DEFAULT_MARKETS: &[&str] = &[
    "us-en", "gb-en", "de-de", "fr-fr", "nl-nl", "es-es", "it-it", "az-az", "ru-ru", "ae-ar",
];

/// A research report.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Report {
    /// Searches month by month since 2021.
    #[default]
    History,
    /// Result pages month by month (a year).
    SerpHistory,
    /// Each result of the kept result page with its authority, linking sites and traffic.
    SerpMetrics,
    /// The first result's keywords: the parent topic and the traffic potential.
    Parent,
    /// Searches of the keyword in other countries.
    Countries,
}

#[derive(Debug, Deserialize)]
struct Ask {
    project_id: i64,
    #[serde(default)]
    report: Report,
    /// The keyword.
    seed: String,
    /// The mobile result page (result-page reports).
    #[serde(default)]
    mobile: bool,
    /// Another market than the project's: `country-language`.
    #[serde(default)]
    market: Option<String>,
    /// Markets to measure in (`countries`): `country-language` each.
    #[serde(default)]
    markets: Vec<String>,
    /// Pay for fresh data even while the kept answer is fresh.
    #[serde(default)]
    force: bool,
}

struct Question {
    report: Report,
    /// The kept result page's organic addresses, for the reports that read it.
    results: Vec<String>,
    /// The markets of `countries`, in the order of the parts.
    markets: Vec<String>,
    parts: Vec<Paid<'static>>,
}

fn paid(
    project_id: i64,
    endpoint: &'static str,
    class: CacheClass,
    task: Value,
    estimate_usd: f64,
) -> Paid<'static> {
    Paid {
        travels: true,
        module: MODULE,
        provider: octopuson_dataforseo::ID,
        endpoint,
        class,
        project_id: Some(project_id),
        task,
        estimate_usd,
    }
}

/// The result page the Keyword Explorer keeps for `seed`: the same question as its `serp`
/// report, so its kept answer is found.
fn serp_paid(
    project_id: i64,
    seed: &str,
    market: &Market,
    mobile: bool,
) -> Result<Paid<'static>, AppError> {
    Ok(paid(
        project_id,
        serp::ORGANIC,
        CacheClass::Serp,
        serp::organic_task(seed, market, 10, mobile)?,
        serp::LIVE_PAGE,
    ))
}

/// The organic addresses of the kept result page, from the top.
fn kept_results(workspace: &Workspace, page: &Paid<'_>) -> Result<Vec<String>, AppError> {
    let kept = workspace
        .kept(page, serp::organic_from_result)?
        .ok_or_else(|| AppError::Invalid("serp: fetch the result page first".to_owned()))?;
    Ok(kept
        .data
        .organic
        .iter()
        .filter_map(|item| item.url.clone())
        .take(research::TOP_RESULTS)
        .collect())
}

/// The first day of the month `months` before this one.
fn months_back(months: u32) -> String {
    let (year, month) = crate::this_month();
    site::months_before(year, month, months)
}

fn question(workspace: &Workspace, ask: &Ask) -> Result<Question, AppError> {
    let project = workspace.store()?.project(ask.project_id)?;
    let market = super::market_of(&project, ask.market.as_deref())?;
    let seed = ask.seed.split_whitespace().collect::<Vec<_>>().join(" ");
    if seed.is_empty() {
        return Err(AppError::Invalid("seed: enter a keyword".to_owned()));
    }
    let id = ask.project_id;
    let mut results = Vec::new();
    let mut markets = Vec::new();
    let parts = match ask.report {
        Report::History => vec![paid(
            id,
            research::KEYWORD_HISTORY,
            CacheClass::Keywords,
            research::keyword_history_task(std::slice::from_ref(&seed), &market)?,
            research::keyword_history_estimate(1),
        )],
        Report::SerpHistory => vec![paid(
            id,
            research::SERP_HISTORY,
            CacheClass::Domain,
            research::serp_history_task(&seed, &market, &months_back(SERP_MONTHS))?,
            research::serp_history_estimate(SERP_MONTHS),
        )],
        Report::SerpMetrics => {
            results = kept_results(workspace, &serp_paid(id, &seed, &market, ask.mobile)?)?;
            if results.is_empty() {
                return Err(AppError::Invalid(
                    "serp: the result page has no organic results".to_owned(),
                ));
            }
            let count = u32::try_from(results.len()).unwrap_or(u32::MAX);
            vec![
                paid(
                    id,
                    backlinks::BULK_RANKS,
                    CacheClass::Backlinks,
                    backlinks::bulk_ranks_task(&results)?,
                    backlinks::estimate(count),
                ),
                paid(
                    id,
                    research::BULK_REFERRING_DOMAINS,
                    CacheClass::Backlinks,
                    research::bulk_referring_domains_task(&results)?,
                    backlinks::estimate(count),
                ),
                paid(
                    id,
                    site::BULK_TRAFFIC,
                    CacheClass::Domain,
                    research::bulk_page_traffic_task(&results, &market)?,
                    site::estimate(count),
                ),
            ]
        }
        Report::Parent => {
            results = kept_results(workspace, &serp_paid(id, &seed, &market, ask.mobile)?)?;
            let first = results.first().ok_or_else(|| {
                AppError::Invalid("serp: the result page has no organic results".to_owned())
            })?;
            vec![paid(
                id,
                site::RANKED_KEYWORDS,
                CacheClass::Domain,
                research::page_keywords_task(first, &market, PAGE_ROWS)?,
                site::estimate(PAGE_ROWS),
            )]
        }
        Report::Countries => {
            let asked: Vec<String> = if ask.markets.is_empty() {
                DEFAULT_MARKETS
                    .iter()
                    .map(|&name| name.to_owned())
                    .collect()
            } else {
                ask.markets.clone()
            };
            let mut parts = Vec::new();
            for name in asked.iter().take(MAX_MARKETS) {
                let other = super::market_of(&project, Some(name))?;
                markets.push(format!("{}-{}", other.country, other.language));
                parts.push(paid(
                    id,
                    labs::OVERVIEW,
                    CacheClass::Keywords,
                    labs::overview_task(std::slice::from_ref(&seed), &other)?,
                    labs::overview_estimate(1),
                ));
            }
            parts
        }
    };
    Ok(Question {
        report: ask.report,
        results,
        markets,
        parts,
    })
}

/// One result of a result page and its weight.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
struct ResultWeight {
    rank: u32,
    url: String,
    /// The page's authority, 0–100.
    page_rank: Option<u32>,
    referring_domains: Option<u64>,
    /// Estimated monthly organic traffic of the page.
    traffic: Option<f64>,
    /// Keywords the page ranks for.
    keywords: Option<u64>,
}

/// The parent topic of a keyword: what its first result ranks for.
#[derive(Debug, Clone, PartialEq, Serialize)]
struct ParentTopic {
    /// The first result.
    url: String,
    /// The keyword bringing it the most traffic.
    parent: Option<RankedKeyword>,
    /// Its traffic over every keyword it ranks for: what ranking first could bring.
    reach: PageReach,
    /// Its keywords, most traffic first ("also ranks for").
    keywords: Vec<RankedKeyword>,
}

/// The keyword in one market.
#[derive(Debug, Clone, PartialEq, Serialize)]
struct MarketVolume {
    market: String,
    data: Option<KeywordData>,
}

fn read(question: &Question, answers: &[Vec<Value>]) -> Value {
    let empty: Vec<Value> = Vec::new();
    let part = |index: usize| answers.get(index).unwrap_or(&empty).as_slice();
    let value = match question.report {
        Report::History => serde_json::to_value(
            research::keyword_history_from_result(part(0))
                .into_iter()
                .next()
                .map(|history| history.months)
                .unwrap_or_default(),
        ),
        Report::SerpHistory => serde_json::to_value(research::serp_history_from_result(part(0))),
        Report::SerpMetrics => {
            let ranks = backlinks::bulk_ranks_from_result(part(0));
            let referring = research::bulk_referring_domains_from_result(part(1));
            let traffic = site::bulk_traffic_from_result(part(2));
            let rows: Vec<ResultWeight> = question
                .results
                .iter()
                .enumerate()
                .map(|(index, url)| {
                    let same =
                        |target: &str| target.trim_end_matches('/') == url.trim_end_matches('/');
                    let traffic = traffic.iter().find(|row| same(&row.target));
                    ResultWeight {
                        rank: u32::try_from(index + 1).unwrap_or(u32::MAX),
                        url: url.clone(),
                        page_rank: ranks
                            .iter()
                            .find(|row| same(&row.target))
                            .and_then(|row| row.rank),
                        referring_domains: referring
                            .iter()
                            .find(|row| same(&row.target))
                            .map(|row| row.referring_domains),
                        traffic: traffic.map(|row| row.organic_traffic),
                        keywords: traffic.map(|row| row.organic_keywords),
                    }
                })
                .collect();
            serde_json::to_value(rows)
        }
        Report::Parent => {
            let ranked = site::ranked_keywords_from_result(part(0));
            let parent = ranked
                .keywords
                .iter()
                .max_by(|left, right| left.traffic.total_cmp(&right.traffic))
                .cloned();
            serde_json::to_value(ParentTopic {
                url: question.results.first().cloned().unwrap_or_default(),
                parent,
                reach: research::page_reach(part(0)),
                keywords: ranked.keywords,
            })
        }
        Report::Countries => serde_json::to_value(
            question
                .markets
                .iter()
                .enumerate()
                .map(|(index, market)| MarketVolume {
                    market: market.clone(),
                    data: labs::list_from_result(part(index))
                        .keywords
                        .into_iter()
                        .next(),
                })
                .collect::<Vec<_>>(),
        ),
    };
    value.unwrap_or(Value::Null)
}

fn regrouped(fetched: Fetched<Vec<Vec<Value>>>, question: &Question) -> Fetched<Value> {
    Fetched {
        data: read(question, &fetched.data),
        fetched_at: fetched.fetched_at,
        fresh: fetched.fresh,
        cached: fetched.cached,
        spent_usd: fetched.spent_usd,
        via: fetched.via,
        saved_usd: fetched.saved_usd,
        total: None,
    }
}

fn ask_dataforseo(route: &Route<'_>, part: &Paid<'_>) -> Result<crate::fetch::Reply, AppError> {
    let credentials = route.key(octopuson_dataforseo::SECRET)?;
    let client = Client::new(route.transport, &credentials)?;
    Ok(client.post(part.endpoint, &part.task)?.into())
}

fn research_estimate(context: &Context<'_>, ask: Ask) -> Result<Estimate, AppError> {
    let question = question(context.workspace, &ask)?;
    context.workspace.estimate_many(&question.parts)
}

fn get_research(context: &Context<'_>, ask: Ask) -> Result<Option<Fetched<Value>>, AppError> {
    let question = question(context.workspace, &ask)?;
    Ok(context
        .workspace
        .kept_many(&question.parts)?
        .map(|fetched| regrouped(fetched, &question)))
}

fn fetch_research(context: &Context<'_>, ask: Ask) -> Result<Fetched<Value>, AppError> {
    let question = question(context.workspace, &ask)?;
    let fetched = context
        .workspace
        .fetch_many(&question.parts, ask.force, ask_dataforseo)?;
    Ok(regrouped(fetched, &question))
}

// ------------------------------------------------------------------- Groups by result pages

#[derive(Debug, Deserialize)]
struct ListAsk {
    list_id: i64,
}

/// The latest result page of each keyword of a list, as asked for grouping: the month before
/// this one onward, so a page or two each.
fn list_parts(
    workspace: &Workspace,
    list_id: i64,
) -> Result<(Vec<ListKeyword>, Vec<Paid<'static>>), AppError> {
    let store = workspace.store()?;
    let list = store.keyword_list(list_id)?;
    let project = store.project(list.project_id)?;
    let entries = store.list_keywords(list_id)?;
    drop(store);
    let market = super::market_of(&project, None)?;
    let from = months_back(1);
    let parts = entries
        .iter()
        .map(|entry| {
            Ok(paid(
                list.project_id,
                research::SERP_HISTORY,
                CacheClass::Domain,
                research::serp_history_task(&entry.keyword, &market, &from)?,
                research::serp_history_estimate(1),
            ))
        })
        .collect::<Result<Vec<_>, AppError>>()?;
    Ok((entries, parts))
}

/// What grouping a list would fetch.
#[derive(Debug, Clone, PartialEq, Serialize)]
struct ClusterEstimate {
    keywords: usize,
    /// Keywords whose latest result page is kept already.
    kept: usize,
    /// The price of the rest; none when nothing is missing.
    estimate: Option<Estimate>,
}

fn missing<'a>(
    workspace: &Workspace,
    parts: &'a [Paid<'static>],
) -> Result<Vec<&'a Paid<'static>>, AppError> {
    let mut missing = Vec::new();
    for part in parts {
        let fresh = workspace.kept(part, |_| ())?.is_some_and(|kept| kept.fresh);
        if !fresh {
            missing.push(part);
        }
    }
    Ok(missing)
}

fn serp_cluster_estimate(context: &Context<'_>, ask: ListAsk) -> Result<ClusterEstimate, AppError> {
    let (entries, parts) = list_parts(context.workspace, ask.list_id)?;
    let missing: Vec<Paid<'static>> = missing(context.workspace, &parts)?
        .into_iter()
        .cloned()
        .collect();
    let estimate = if missing.is_empty() {
        None
    } else {
        Some(context.workspace.estimate_many(&missing)?)
    };
    Ok(ClusterEstimate {
        keywords: entries.len(),
        kept: parts.len() - missing.len(),
        estimate,
    })
}

/// Fetches the latest result page of each keyword of a list not kept yet, as a job; its id.
fn start_serp_clustering(context: &Context<'_>, ask: ListAsk) -> Result<String, AppError> {
    let (_, parts) = list_parts(context.workspace, ask.list_id)?;
    let project_id = context
        .workspace
        .store()?
        .keyword_list(ask.list_id)?
        .project_id;
    let list_name = context.workspace.store()?.keyword_list(ask.list_id)?.name;
    let workspace = Arc::clone(context.workspace);
    context.jobs.start(JOB_KIND, &list_name, Some(project_id), move |control| {
        let todo: Vec<Paid<'static>> = missing(&workspace, &parts)?.into_iter().cloned().collect();
        let total = u64::try_from(todo.len()).unwrap_or(u64::MAX);
        let mut failed = 0_u64;
        let mut spent = 0.0;
        for (index, part) in todo.iter().enumerate() {
            if control.cancelled() {
                break;
            }
            control.progress(
                part.task["keyword"].as_str().unwrap_or_default(),
                u64::try_from(index).unwrap_or(u64::MAX),
                total,
            );
            match workspace.fetch(part, false, |route| ask_dataforseo(route, part), |_| ()) {
                Ok(fetched) => {
                    spent += fetched.spent_usd;
                    control.spent(fetched.spent_usd);
                }
                // Out of credits, over budget or no source: the rest would fail the same way.
                Err(error @ (AppError::Quota(_) | AppError::NoSource(_) | AppError::Budget(_) | AppError::Auth(_))) => {
                    return Err(error);
                }
                Err(error) => {
                    failed += 1;
                    tracing::warn!(%error, "a result page for grouping was not fetched");
                }
            }
        }
        control.progress("", total, total);
        Ok(JobDone {
            status: if failed == 0 && !control.cancelled() {
                JobStatus::Done
            } else {
                JobStatus::Partial
            },
            result: Some(json!({ "list_id": ask.list_id, "fetched": total - failed, "failed": failed, "spent_usd": spent })),
        })
    })
}

#[derive(Debug, Deserialize)]
struct ClustersAsk {
    list_id: i64,
    /// Least shared top-ten results (1–9).
    #[serde(default)]
    threshold: Option<u16>,
    /// `soft` (shares with the pillar) or `hard` (with every member).
    #[serde(default)]
    mode: Mode,
}

/// A keyword of a group.
#[derive(Debug, Clone, PartialEq, Serialize)]
struct GroupMember {
    keyword: String,
    volume: Option<u64>,
    difficulty: Option<u32>,
    intent: Option<String>,
    /// Top-ten results shared with the pillar.
    shared: Option<u16>,
}

/// Keywords one page can target, led by the most searched, as the result pages show.
#[derive(Debug, Clone, PartialEq, Serialize)]
struct SerpGroup {
    pillar: String,
    volume: u64,
    /// Difficulty weighted by searches.
    difficulty: Option<f64>,
    /// The intent with the most searches.
    intent: Option<String>,
    /// The pages ranking for the pillar now.
    pages: Vec<String>,
    keywords: Vec<GroupMember>,
}

/// A list grouped by result pages.
#[derive(Debug, Clone, PartialEq, Serialize)]
struct SerpGroups {
    groups: Vec<SerpGroup>,
    /// Keywords whose result page is not kept (not fetched yet, or not in DataForSEO's data).
    without_page: Vec<String>,
}

fn number_of(data: &Value, name: &str) -> Option<u64> {
    data.get(name).and_then(Value::as_u64)
}

/// The groups of a list from the result pages kept: free, nothing is fetched.
#[allow(clippy::cast_precision_loss)] // searches as weights
fn serp_clusters(context: &Context<'_>, ask: ClustersAsk) -> Result<SerpGroups, AppError> {
    let (entries, parts) = list_parts(context.workspace, ask.list_id)?;
    let mut pages: Vec<Option<Vec<String>>> = Vec::with_capacity(parts.len());
    for part in &parts {
        pages.push(
            context
                .workspace
                .kept(part, research::latest_results)?
                .and_then(|kept| kept.data)
                .filter(|results| !results.is_empty()),
        );
    }
    let inputs: Vec<SerpInput<'_>> = entries
        .iter()
        .zip(&pages)
        .map(|(entry, page)| SerpInput {
            keyword: &entry.keyword,
            volume: number_of(&entry.data, "volume"),
            results: page.as_deref(),
        })
        .collect();
    let options = SerpOptions {
        threshold: ask.threshold.unwrap_or(3).clamp(1, 9),
        mode: ask.mode,
        ..SerpOptions::default()
    };
    let clustering = cluster_serp(&inputs, &options);
    let member = |index: usize, shared: Option<u16>| {
        let entry = &entries[index];
        GroupMember {
            keyword: entry.keyword.clone(),
            volume: number_of(&entry.data, "volume"),
            difficulty: number_of(&entry.data, "difficulty").and_then(|kd| u32::try_from(kd).ok()),
            intent: entry
                .data
                .get("intent")
                .and_then(Value::as_str)
                .map(str::to_owned),
            shared,
        }
    };
    let mut grouped = vec![false; entries.len()];
    let mut groups: Vec<SerpGroup> = Vec::new();
    for cluster in &clustering.clusters {
        if pages.get(cluster.pillar).is_none_or(Option::is_none) {
            continue;
        }
        let keywords: Vec<GroupMember> = cluster
            .members
            .iter()
            .filter(|item| item.index < entries.len())
            .map(|item| {
                grouped[item.index] = true;
                member(item.index, item.shared)
            })
            .collect();
        let volume: u64 = keywords.iter().filter_map(|keyword| keyword.volume).sum();
        let weighted: Vec<(f64, f64)> = keywords
            .iter()
            .filter_map(|keyword| {
                Some((
                    f64::from(keyword.difficulty?),
                    keyword.volume.map_or(1.0, |volume| volume as f64),
                ))
            })
            .collect();
        let weight: f64 = weighted.iter().map(|(_, weight)| weight).sum();
        let difficulty = (weight > 0.0).then(|| {
            (weighted.iter().map(|(kd, weight)| kd * weight).sum::<f64>() / weight * 10.0).round()
                / 10.0
        });
        let mut intents: BTreeMap<String, u64> = BTreeMap::new();
        for keyword in &keywords {
            if let Some(intent) = &keyword.intent {
                *intents.entry(intent.clone()).or_default() += keyword.volume.unwrap_or(1);
            }
        }
        groups.push(SerpGroup {
            pillar: entries[cluster.pillar].keyword.clone(),
            volume,
            difficulty,
            intent: intents
                .into_iter()
                .max_by_key(|(_, volume)| *volume)
                .map(|(intent, _)| intent),
            pages: pages[cluster.pillar].clone().unwrap_or_default(),
            keywords,
        });
    }
    groups.sort_by_key(|group| std::cmp::Reverse(group.volume));
    let without_page = entries
        .iter()
        .zip(&pages)
        .enumerate()
        .filter(|(index, (_, page))| page.is_none() && !grouped[*index])
        .map(|(_, (entry, _))| entry.keyword.clone())
        .collect();
    Ok(SerpGroups {
        groups,
        without_page,
    })
}

/// Adds the commands.
pub fn register(router: &mut Router) {
    router.add("research_estimate", research_estimate);
    router.add("get_research", get_research);
    router.add("fetch_research", fetch_research);
    router.add("serp_cluster_estimate", serp_cluster_estimate);
    router.add("start_serp_clustering", start_serp_clustering);
    router.add("serp_clusters", serp_clusters);
}
