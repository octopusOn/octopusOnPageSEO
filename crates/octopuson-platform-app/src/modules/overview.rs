//! Domain overview (Site Explorer): what a domain ranks for, the traffic it brings, its history,
//! keywords, best pages, competitors and subdomains, and how it compares with the project's
//! competitors.
//!
//! Every report goes through the same three commands with a `report` argument: what is kept
//! (`get_overview`), what fetching would cost (`overview_estimate`) and fetching
//! (`fetch_overview`).

use octopuson_dataforseo::labs::{self, DomainOverview};
use octopuson_dataforseo::{Client, Market, site};
use octopuson_platform::{CacheClass, ProjectRow, normalize_domain};
use serde::Deserialize;
use serde_json::Value;

use crate::fetch::{Estimate, Fetched, Paid};
use crate::router::{Context, Router};
use crate::{AppError, Workspace, now};

/// Module id, in spending records.
const MODULE: &str = "overview";

/// Rows a table report returns unless asked otherwise.
const DEFAULT_ROWS: u32 = 100;

/// A Site Explorer report.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Report {
    /// Organic and paid rankings now.
    #[default]
    Summary,
    /// Rankings month by month.
    History,
    /// The keywords the domain ranks for.
    Keywords,
    /// The domain's pages by traffic.
    Pages,
    /// Domains competing for the same keywords.
    Competitors,
    /// The domain's subdomains.
    Subdomains,
    /// The domain and the project's competitors side by side.
    Compare,
}

#[derive(Debug, Deserialize)]
struct Ask {
    project_id: i64,
    /// Another domain than the project's (a competitor); the project's market is used.
    #[serde(default)]
    domain: Option<String>,
    #[serde(default)]
    report: Report,
    /// Rows of a table report (10–1000).
    #[serde(default)]
    limit: Option<u32>,
    /// Pay for fresh data even while the kept answer is fresh.
    #[serde(default)]
    force: bool,
}

struct Question {
    report: Report,
    domain: String,
    paid: Paid<'static>,
}

/// The current year and month.
fn this_month() -> (u32, u32) {
    let now = now();
    let year = now.get(0..4).and_then(|y| y.parse().ok()).unwrap_or(2026);
    let month = now.get(5..7).and_then(|m| m.parse().ok()).unwrap_or(1);
    (year, month)
}

fn question(workspace: &Workspace, ask: &Ask) -> Result<Question, AppError> {
    let project: ProjectRow = workspace.store()?.project(ask.project_id)?;
    let domain = ask
        .domain
        .as_deref()
        .map(normalize_domain)
        .filter(|domain| !domain.is_empty())
        .unwrap_or_else(|| project.domain.clone());
    let market = Market::new(&project.country, &project.language);
    let rows = site::rows(ask.limit.unwrap_or(DEFAULT_ROWS));
    let (endpoint, task, estimate_usd) = match ask.report {
        Report::Summary => (
            labs::DOMAIN_OVERVIEW,
            labs::domain_overview_task(&domain, &market)?,
            labs::domain_overview_estimate(),
        ),
        Report::History => {
            let (year, month) = this_month();
            let from = site::months_before(year, month, site::HISTORY_MONTHS);
            (
                site::HISTORY,
                site::history_task(&domain, &market, &from)?,
                site::history_estimate(),
            )
        }
        Report::Keywords => (
            site::RANKED_KEYWORDS,
            site::ranked_keywords_task(&domain, &market, rows)?,
            site::estimate(rows),
        ),
        Report::Pages => (
            site::RELEVANT_PAGES,
            site::pages_task(&domain, &market, rows)?,
            site::estimate(rows),
        ),
        Report::Competitors => (
            site::COMPETITORS,
            site::competitors_task(&domain, &market, rows)?,
            site::estimate(rows),
        ),
        Report::Subdomains => (
            site::SUBDOMAINS,
            site::subdomains_task(&domain, &market, rows)?,
            site::estimate(rows),
        ),
        Report::Compare => {
            if project.competitors.is_empty() {
                return Err(AppError::Invalid(
                    "competitors: add competitors to the project to compare".to_owned(),
                ));
            }
            let mut targets = vec![domain.clone()];
            targets.extend(project.competitors.iter().cloned());
            let task = site::bulk_traffic_task(&targets, &market)?;
            let count = u32::try_from(targets.len()).unwrap_or(u32::MAX);
            (site::BULK_TRAFFIC, task, site::estimate(count))
        }
    };
    Ok(Question {
        report: ask.report,
        domain,
        paid: Paid {
            module: MODULE,
            provider: octopuson_dataforseo::ID,
            endpoint,
            class: CacheClass::Domain,
            project_id: Some(ask.project_id),
            task,
            estimate_usd,
        },
    })
}

/// The report read from a result list, as pages get it.
fn read(report: Report, domain: &str, result: &[Value]) -> Value {
    let value = match report {
        Report::Summary => serde_json::to_value(DomainOverview::from_result(domain, result)),
        Report::History => serde_json::to_value(site::history_from_result(result)),
        Report::Keywords => serde_json::to_value(site::ranked_keywords_from_result(result)),
        Report::Pages => serde_json::to_value(site::pages_from_result(result)),
        Report::Competitors => serde_json::to_value(site::competitors_from_result(domain, result)),
        Report::Subdomains => serde_json::to_value(site::subdomains_from_result(result)),
        Report::Compare => serde_json::to_value(site::bulk_traffic_from_result(result)),
    };
    value.unwrap_or(Value::Null)
}

fn overview_estimate(context: &Context<'_>, ask: Ask) -> Result<Estimate, AppError> {
    let question = question(context.workspace, &ask)?;
    context.workspace.estimate(&question.paid)
}

fn get_overview(context: &Context<'_>, ask: Ask) -> Result<Option<Fetched<Value>>, AppError> {
    let question = question(context.workspace, &ask)?;
    let (report, domain) = (question.report, question.domain.clone());
    context
        .workspace
        .kept(&question.paid, |result| read(report, &domain, result))
}

fn fetch_overview(context: &Context<'_>, ask: Ask) -> Result<Fetched<Value>, AppError> {
    let Question {
        report,
        domain,
        paid,
    } = question(context.workspace, &ask)?;
    context.workspace.fetch(
        &paid,
        ask.force,
        |route| {
            let credentials = route.key(octopuson_dataforseo::SECRET)?;
            let client = Client::new(route.transport, &credentials)?;
            Ok(client.post(paid.endpoint, &paid.task)?.into())
        },
        |result| read(report, &domain, result),
    )
}

/// Adds the commands.
pub fn register(router: &mut Router) {
    router.add("overview_estimate", overview_estimate);
    router.add("get_overview", get_overview);
    router.add("fetch_overview", fetch_overview);
}
