//! Gap analysis and the market: every keyword of the site and its competitors side by side in
//! Semrush's groups (shared, missing, weak, strong, untapped, unique), keywords a competitor's
//! page ranks for and the site's page does not, sites linking to competitors but not to the
//! site, many sites compared side by side (traffic and authority), each one's traffic month by
//! month (their shares of the market), the categories the site ranks in and the domains
//! leading a category.
//!
//! A gap asks DataForSEO once per competitor; the parts are estimated, kept and paid together
//! through `gap_estimate`, `get_gap` and `fetch_gap`.

use octopuson_dataforseo::site::KeywordSet;
use octopuson_dataforseo::{Client, backlinks, gap, market as markets, site};
use octopuson_platform::{CacheClass, ProjectRow};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::fetch::{Estimate, Fetched, Paid};
use crate::router::{Context, Router};
use crate::{AppError, Workspace};

/// Module id, in spending records.
const MODULE: &str = "gap";

/// Rows per competitor unless asked otherwise.
const DEFAULT_ROWS: u32 = 100;

/// Most sites compared side by side.
const MAX_BULK: usize = 100;

/// A gap report.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Report {
    /// Keywords competitors rank for and the site does not.
    #[default]
    Missing,
    /// Keywords the site shares with competitors, weak where they rank higher.
    Shared,
    /// Sites linking to every competitor but not to the site.
    Links,
    /// Traffic and authority of many sites.
    Bulk,
    /// Every keyword of the site and its competitors in Semrush's groups.
    Keywords,
    /// Keywords a competitor's page ranks for and the site's page does not.
    Pages,
    /// The organic traffic of the site and its competitors month by month.
    Share,
    /// The categories the site ranks in.
    Categories,
    /// The domains with the most traffic in categories.
    Players,
}

#[derive(Debug, Deserialize)]
struct Ask {
    project_id: i64,
    #[serde(default)]
    report: Report,
    /// Competitors to compare with; the project's when empty.
    #[serde(default)]
    competitors: Vec<String>,
    /// Sites to compare side by side (bulk); the project's domain and competitors when empty.
    #[serde(default)]
    targets: Vec<String>,
    /// Rows per competitor (10–1000).
    #[serde(default)]
    limit: Option<u32>,
    /// Another market than the project's: `country-language`.
    #[serde(default)]
    market: Option<String>,
    /// The site's page (page gap).
    #[serde(default)]
    own_page: Option<String>,
    /// The competitor's page (page gap).
    #[serde(default)]
    competitor_page: Option<String>,
    /// Months of traffic history (share; 12 unless asked, back to October 2020).
    #[serde(default)]
    months: Option<u32>,
    /// Category codes (players).
    #[serde(default)]
    categories: Vec<u32>,
    #[serde(default)]
    force: bool,
}

/// Months of traffic history unless asked otherwise.
const SHARE_MONTHS: u32 = 12;

/// A site in the side-by-side comparison.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
struct BulkRow {
    target: String,
    organic_traffic: f64,
    organic_keywords: u64,
    paid_traffic: f64,
    rank: Option<u32>,
}

struct Question {
    report: Report,
    own: String,
    names: Vec<String>,
    parts: Vec<Paid<'static>>,
}

fn paid(
    project: &ProjectRow,
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
        project_id: Some(project.id),
        task,
        estimate_usd,
    }
}

#[allow(clippy::too_many_lines)] // one arm per report
fn question(workspace: &Workspace, ask: &Ask) -> Result<Question, AppError> {
    let project = workspace.store()?.project(ask.project_id)?;
    let market = super::market_of(&project, ask.market.as_deref())?;
    let rows = site::rows(ask.limit.unwrap_or(DEFAULT_ROWS));
    let own = project.domain.clone();
    let chosen = if ask.competitors.is_empty() {
        project.competitors.clone()
    } else {
        ask.competitors.clone()
    };
    let (names, parts) = match ask.report {
        Report::Missing | Report::Shared => {
            let competitors = gap::competitors(&own, &chosen)?;
            let shared = ask.report == Report::Shared;
            let parts = competitors
                .iter()
                .map(|competitor| {
                    let task = if shared {
                        gap::keyword_intersection_task(&own, competitor, &market, true, rows)?
                    } else {
                        gap::keyword_intersection_task(competitor, &own, &market, false, rows)?
                    };
                    Ok(paid(
                        &project,
                        gap::KEYWORD_INTERSECTION,
                        CacheClass::Domain,
                        task,
                        site::estimate(rows),
                    ))
                })
                .collect::<Result<Vec<_>, AppError>>()?;
            (competitors, parts)
        }
        Report::Links => {
            let competitors = gap::competitors(&own, &chosen)?;
            let task = gap::link_gap_task(&own, &competitors, rows)?;
            let part = paid(
                &project,
                gap::LINK_INTERSECTION,
                CacheClass::Backlinks,
                task,
                backlinks::estimate(rows),
            );
            (competitors, vec![part])
        }
        Report::Bulk => {
            let mut targets = if ask.targets.is_empty() {
                let mut targets = vec![own.clone()];
                targets.extend(project.competitors.iter().cloned());
                targets
            } else {
                ask.targets.clone()
            };
            targets.truncate(MAX_BULK);
            let traffic = site::bulk_traffic_task(&targets, &market)?;
            // The ranks follow the cleaned list of the traffic task.
            let names: Vec<String> = traffic["targets"]
                .as_array()
                .map(|list| {
                    list.iter()
                        .filter_map(Value::as_str)
                        .map(str::to_owned)
                        .collect()
                })
                .unwrap_or_default();
            let count = u32::try_from(names.len()).unwrap_or(u32::MAX);
            let ranks = backlinks::bulk_ranks_task(&names)?;
            let parts = vec![
                paid(
                    &project,
                    site::BULK_TRAFFIC,
                    CacheClass::Domain,
                    traffic,
                    site::estimate(count),
                ),
                paid(
                    &project,
                    backlinks::BULK_RANKS,
                    CacheClass::Backlinks,
                    ranks,
                    backlinks::estimate(count),
                ),
            ];
            (names, parts)
        }
        Report::Keywords => {
            let competitors = gap::competitors(&own, &chosen)?;
            let mut names = vec![own.clone()];
            names.extend(competitors);
            // Each domain's own keywords: the same questions as the Site Explorer's lists, so
            // their kept answers serve both.
            let parts = names
                .iter()
                .map(|domain| {
                    Ok(paid(
                        &project,
                        site::RANKED_KEYWORDS,
                        CacheClass::Domain,
                        site::ranked_set_task(domain, &market, rows, KeywordSet::Organic)?,
                        site::estimate(rows),
                    ))
                })
                .collect::<Result<Vec<_>, AppError>>()?;
            (names, parts)
        }
        Report::Pages => {
            let (Some(own_page), Some(competitor_page)) =
                (ask.own_page.as_deref(), ask.competitor_page.as_deref())
            else {
                return Err(AppError::Invalid(
                    "pages: enter your page and the competitor's page".to_owned(),
                ));
            };
            let task = gap::page_gap_task(competitor_page, own_page, &market, rows)?;
            let part = paid(
                &project,
                gap::PAGE_INTERSECTION,
                CacheClass::Domain,
                task,
                site::estimate(rows),
            );
            (vec![competitor_page.to_owned()], vec![part])
        }
        Report::Share => {
            let mut targets = vec![own.clone()];
            if ask.targets.is_empty() {
                targets.extend(chosen.iter().cloned());
            } else {
                targets.extend(ask.targets.iter().cloned());
            }
            let (year, month) = crate::this_month();
            let months = ask.months.map_or(SHARE_MONTHS, |months| {
                months.clamp(3, site::months_since_start(year, month))
            });
            let task = markets::traffic_history_task(
                &targets,
                &market,
                &site::months_before(year, month, months),
            )?;
            let names: Vec<String> = task["targets"]
                .as_array()
                .map(|list| {
                    list.iter()
                        .filter_map(Value::as_str)
                        .map(str::to_owned)
                        .collect()
                })
                .unwrap_or_default();
            let part = paid(
                &project,
                markets::TRAFFIC_HISTORY,
                CacheClass::Domain,
                task,
                markets::traffic_history_estimate(names.len()),
            );
            (names, vec![part])
        }
        Report::Categories => {
            let task = markets::categories_task(&own, &market)?;
            let part = paid(
                &project,
                markets::CATEGORIES_FOR_DOMAIN,
                CacheClass::Domain,
                task,
                markets::categories_estimate(),
            );
            (vec![own.clone()], vec![part])
        }
        Report::Players => {
            let (year, month) = crate::this_month();
            // The last full month against the same month a year before.
            let later = site::months_before(year, month, 1);
            let earlier = site::months_before(year, month, 13);
            let task = markets::players_task(&ask.categories, &market, &earlier, &later)?;
            let part = paid(
                &project,
                markets::CATEGORY_DOMAINS,
                CacheClass::Domain,
                task,
                markets::players_estimate(),
            );
            (Vec::new(), vec![part])
        }
    };
    Ok(Question {
        report: ask.report,
        own,
        names,
        parts,
    })
}

fn first(results: &[Vec<Value>]) -> &[Value] {
    results.first().map_or(&[][..], Vec::as_slice)
}

/// The report read from the parts' result lists, as pages get it.
fn read(question: &Question, results: &[Vec<Value>]) -> Value {
    let answers: Vec<(String, Vec<Value>)> = question
        .names
        .iter()
        .cloned()
        .zip(results.iter().cloned())
        .collect();
    let value = match question.report {
        Report::Missing => serde_json::to_value(gap::missing(&question.own, &answers)),
        Report::Shared => serde_json::to_value(gap::shared(&question.own, &answers)),
        Report::Keywords => {
            let rows = question
                .parts
                .first()
                .and_then(|part| part.task["limit"].as_u64())
                .unwrap_or(0);
            serde_json::to_value(gap::matrix(
                &question.own,
                &question.names[1..],
                results,
                u32::try_from(rows).unwrap_or(u32::MAX),
            ))
        }
        Report::Pages => serde_json::to_value(gap::page_gap_from_result(first(results))),
        Report::Share => serde_json::to_value(markets::traffic_history_from_result(first(results))),
        Report::Categories => serde_json::to_value(markets::categories_from_result(first(results))),
        Report::Players => serde_json::to_value(markets::players_from_result(first(results))),
        Report::Links => serde_json::to_value(gap::link_gap_from_result(
            &question.names,
            results.first().map_or(&[][..], Vec::as_slice),
        )),
        Report::Bulk => {
            let traffic =
                site::bulk_traffic_from_result(results.first().map_or(&[][..], Vec::as_slice));
            let ranks =
                backlinks::bulk_ranks_from_result(results.get(1).map_or(&[][..], Vec::as_slice));
            let rows: Vec<BulkRow> = question
                .names
                .iter()
                .map(|name| {
                    let traffic = traffic.iter().find(|row| &row.target == name);
                    BulkRow {
                        target: name.clone(),
                        organic_traffic: traffic.map_or(0.0, |row| row.organic_traffic),
                        organic_keywords: traffic.map_or(0, |row| row.organic_keywords),
                        paid_traffic: traffic.map_or(0.0, |row| row.paid_traffic),
                        rank: ranks
                            .iter()
                            .find(|row| &row.target == name)
                            .and_then(|row| row.rank),
                    }
                })
                .collect();
            serde_json::to_value(rows)
        }
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

fn gap_estimate(context: &Context<'_>, ask: Ask) -> Result<Estimate, AppError> {
    let question = question(context.workspace, &ask)?;
    context.workspace.estimate_many(&question.parts)
}

fn get_gap(context: &Context<'_>, ask: Ask) -> Result<Option<Fetched<Value>>, AppError> {
    let question = question(context.workspace, &ask)?;
    Ok(context
        .workspace
        .kept_many(&question.parts)?
        .map(|fetched| regrouped(fetched, &question)))
}

fn fetch_gap(context: &Context<'_>, ask: Ask) -> Result<Fetched<Value>, AppError> {
    let question = question(context.workspace, &ask)?;
    let fetched = context
        .workspace
        .fetch_many(&question.parts, ask.force, |route, part| {
            let credentials = route.key(octopuson_dataforseo::SECRET)?;
            let client = Client::new(route.transport, &credentials)?;
            Ok(client.post(part.endpoint, &part.task)?.into())
        })?;
    Ok(regrouped(fetched, &question))
}

#[derive(Debug, Deserialize)]
struct NamesAsk {
    project_id: i64,
}

/// The category list DataForSEO gives free: one question for every project.
fn names_paid(project_id: i64) -> Paid<'static> {
    Paid {
        travels: true,
        module: MODULE,
        provider: octopuson_dataforseo::ID,
        endpoint: markets::CATEGORY_NAMES,
        class: CacheClass::Keywords,
        project_id: Some(project_id),
        task: serde_json::json!({}),
        estimate_usd: 0.0,
    }
}

/// The category names, when the list is kept.
fn get_category_names(
    context: &Context<'_>,
    ask: NamesAsk,
) -> Result<Option<Vec<markets::CategoryName>>, AppError> {
    Ok(context
        .workspace
        .kept(
            &names_paid(ask.project_id),
            markets::category_names_from_result,
        )?
        .map(|kept| kept.data))
}

/// Fetches the category names (free on DataForSEO).
fn fetch_category_names(
    context: &Context<'_>,
    ask: NamesAsk,
) -> Result<Vec<markets::CategoryName>, AppError> {
    let paid = names_paid(ask.project_id);
    Ok(context
        .workspace
        .fetch(
            &paid,
            false,
            |route| {
                let credentials = route.key(octopuson_dataforseo::SECRET)?;
                let client = Client::new(route.transport, &credentials)?;
                Ok(client.get(markets::CATEGORY_NAMES)?.into())
            },
            markets::category_names_from_result,
        )?
        .data)
}

/// Adds the commands.
pub fn register(router: &mut Router) {
    router.add("gap_estimate", gap_estimate);
    router.add("get_gap", get_gap);
    router.add("fetch_gap", fetch_gap);
    router.add("get_category_names", get_category_names);
    router.add("fetch_category_names", fetch_category_names);
}
