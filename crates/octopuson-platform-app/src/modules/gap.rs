//! Gap analysis: keywords competitors rank for and the site does not, keywords they share (and
//! where the site is weaker), sites linking to competitors but not to the site, and many sites
//! compared side by side (traffic and authority).
//!
//! A gap asks DataForSEO once per competitor; the parts are estimated, kept and paid together
//! through `gap_estimate`, `get_gap` and `fetch_gap`.

use octopuson_dataforseo::{Client, Market, backlinks, gap, site};
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
    #[serde(default)]
    force: bool,
}

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
        module: MODULE,
        provider: octopuson_dataforseo::ID,
        endpoint,
        class,
        project_id: Some(project.id),
        task,
        estimate_usd,
    }
}

fn question(workspace: &Workspace, ask: &Ask) -> Result<Question, AppError> {
    let project = workspace.store()?.project(ask.project_id)?;
    let market = Market::new(&project.country, &project.language);
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
    };
    Ok(Question {
        report: ask.report,
        own,
        names,
        parts,
    })
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

/// Adds the commands.
pub fn register(router: &mut Router) {
    router.add("gap_estimate", gap_estimate);
    router.add("get_gap", get_gap);
    router.add("fetch_gap", fetch_gap);
}
