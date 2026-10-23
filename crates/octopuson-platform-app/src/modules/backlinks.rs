//! Backlinks: a site's link profile and authority (OA), its backlinks (live, new, lost, broken),
//! referring domains, anchors, most linked pages, links won and lost month by month, and the
//! authority of many sites at once.
//!
//! Reports go through `backlinks_estimate`, `get_backlinks` and `fetch_backlinks` with a
//! `report` argument, like the other paid modules.

use octopuson_dataforseo::Client;
use octopuson_dataforseo::backlinks::{self as api, BacklinkQuery, LinkStatus};
use octopuson_dataforseo::query::{BACKLINK_FIELDS, FieldSpec, ListQuery, REFERRING_DOMAIN_FIELDS};
use octopuson_platform::CacheClass;
use serde::Deserialize;
use serde_json::Value;

use crate::fetch::{Estimate, Fetched, Paid};
use crate::router::{Context, Router};
use crate::{AppError, Workspace};

/// Module id, in spending records.
const MODULE: &str = "backlinks";

/// Rows a list report returns unless asked otherwise.
const DEFAULT_ROWS: u32 = 100;

/// Months of links won and lost.
const NEW_LOST_MONTHS: u32 = 12;

/// A backlink report.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Report {
    /// The link profile and authority.
    #[default]
    Summary,
    /// Backlinks (live, new, lost or broken).
    Backlinks,
    /// Domains linking.
    ReferringDomains,
    /// Anchor texts.
    Anchors,
    /// The target's pages by links.
    Pages,
    /// Links won and lost per month.
    NewLost,
    /// The authority of several sites.
    Bulk,
    /// The link profile month by month since 2019.
    History,
    /// Each linking site judged by the toxic-link rules, the riskiest first.
    Audit,
}

#[derive(Debug, Deserialize)]
struct Ask {
    project_id: i64,
    #[serde(default)]
    report: Report,
    /// A domain or page other than the project's.
    #[serde(default)]
    target: Option<String>,
    /// Rows of a list report (10–1000).
    #[serde(default)]
    limit: Option<u32>,
    /// Which backlinks (backlink report).
    #[serde(default)]
    status: LinkStatus,
    /// Only links passing authority (backlink report).
    #[serde(default)]
    dofollow_only: bool,
    /// The best link of each site only (backlink report).
    #[serde(default)]
    one_per_domain: bool,
    /// Sites to rank (bulk report); the project's domain and competitors when empty.
    #[serde(default)]
    targets: Vec<String>,
    /// Filters, order and the first row of a list report, asked of DataForSEO.
    #[serde(default)]
    query: Option<ListQuery>,
    /// Pay for fresh data even while the kept answer is fresh.
    #[serde(default)]
    force: bool,
}

impl Report {
    /// The fields a list report filters and sorts by on DataForSEO's side.
    fn fields(self) -> Option<&'static [FieldSpec]> {
        match self {
            Self::Backlinks => Some(BACKLINK_FIELDS),
            Self::ReferringDomains => Some(REFERRING_DOMAIN_FIELDS),
            _ => None,
        }
    }
}

struct Question {
    report: Report,
    target: String,
    paid: Paid<'static>,
}

fn question(workspace: &Workspace, ask: &Ask) -> Result<Question, AppError> {
    let project = workspace.store()?.project(ask.project_id)?;
    let target = api::target(
        ask.target
            .as_deref()
            .map(str::trim)
            .filter(|target| !target.is_empty())
            .unwrap_or(&project.domain),
    )?;
    let rows = octopuson_dataforseo::site::rows(ask.limit.unwrap_or(DEFAULT_ROWS));
    let (endpoint, mut task, estimate_usd) = match ask.report {
        Report::Summary => (api::SUMMARY, api::summary_task(&target)?, api::estimate(1)),
        Report::Backlinks => {
            let query = BacklinkQuery {
                status: ask.status,
                dofollow_only: ask.dofollow_only,
                one_per_domain: ask.one_per_domain,
            };
            (
                api::BACKLINKS,
                api::backlinks_task(&target, query, rows)?,
                api::estimate(rows),
            )
        }
        Report::ReferringDomains => (
            api::REFERRING_DOMAINS,
            api::referring_domains_task(&target, rows)?,
            api::estimate(rows),
        ),
        Report::Anchors => (
            api::ANCHORS,
            api::anchors_task(&target, rows)?,
            api::estimate(rows),
        ),
        Report::Pages => (
            api::DOMAIN_PAGES,
            api::pages_task(&target, rows)?,
            api::estimate(rows),
        ),
        Report::NewLost => (
            api::NEW_LOST,
            api::new_lost_task(&target)?,
            api::estimate(NEW_LOST_MONTHS),
        ),
        Report::Audit => {
            let rows = octopuson_dataforseo::site::rows(ask.limit.unwrap_or(api::AUDIT_ROWS));
            (
                api::BACKLINKS,
                api::audit_task(&target, rows)?,
                api::estimate(rows),
            )
        }
        Report::History => {
            let (year, month) = crate::this_month();
            (
                api::HISTORY,
                api::history_task(&target, api::HISTORY_START)?,
                api::estimate(api::history_months(year, month)),
            )
        }
        Report::Bulk => {
            let targets = if ask.targets.is_empty() {
                let mut targets = vec![project.domain.clone()];
                targets.extend(project.competitors.iter().cloned());
                targets
            } else {
                ask.targets.clone()
            };
            let task = api::bulk_ranks_task(&targets)?;
            let count = task["targets"]
                .as_array()
                .map_or(0, Vec::len)
                .try_into()
                .unwrap_or(u32::MAX);
            (api::BULK_RANKS, task, api::estimate(count))
        }
    };
    let travels = super::queried(&mut task, ask.query.as_ref(), ask.report.fields())?;
    Ok(Question {
        report: ask.report,
        target,
        paid: Paid {
            travels,
            module: MODULE,
            provider: octopuson_dataforseo::ID,
            endpoint,
            class: CacheClass::Backlinks,
            project_id: Some(ask.project_id),
            task,
            estimate_usd,
        },
    })
}

/// The report read from a result list, as pages get it.
fn read(report: Report, result: &[Value]) -> Value {
    let value = match report {
        Report::Summary => serde_json::to_value(api::summary_from_result(result)),
        Report::Backlinks => serde_json::to_value(api::backlinks_from_result(result)),
        Report::ReferringDomains => {
            serde_json::to_value(api::referring_domains_from_result(result))
        }
        Report::Anchors => serde_json::to_value(api::anchors_from_result(result)),
        Report::Pages => serde_json::to_value(api::pages_from_result(result)),
        Report::NewLost => serde_json::to_value(api::new_lost_from_result(result)),
        Report::Bulk => serde_json::to_value(api::bulk_ranks_from_result(result)),
        Report::History => serde_json::to_value(api::history_from_result(result)),
        Report::Audit => serde_json::to_value(octopuson_dataforseo::link_audit::assess(
            &api::backlinks_from_result(result),
        )),
    };
    value.unwrap_or(Value::Null)
}

fn backlinks_estimate(context: &Context<'_>, ask: Ask) -> Result<Estimate, AppError> {
    let question = question(context.workspace, &ask)?;
    context.workspace.estimate(&question.paid)
}

fn get_backlinks(context: &Context<'_>, ask: Ask) -> Result<Option<Fetched<Value>>, AppError> {
    let question = question(context.workspace, &ask)?;
    let report = question.report;
    context
        .workspace
        .kept(&question.paid, |result| read(report, result))
}

fn fetch_backlinks(context: &Context<'_>, ask: Ask) -> Result<Fetched<Value>, AppError> {
    let Question {
        report,
        target,
        paid,
    } = question(context.workspace, &ask)?;
    let fetched = context.workspace.fetch(
        &paid,
        ask.force,
        |route| {
            let credentials = route.key(octopuson_dataforseo::SECRET)?;
            let client = Client::new(route.transport, &credentials)?;
            Ok(client.post(paid.endpoint, &paid.task)?.into())
        },
        |result| read(report, result),
    )?;
    if report == Report::Summary {
        super::history::note_links(context.workspace, ask.project_id, &target, &fetched);
    }
    Ok(fetched)
}

/// Adds the commands.
pub fn register(router: &mut Router) {
    router.add("backlinks_estimate", backlinks_estimate);
    router.add("get_backlinks", get_backlinks);
    router.add("fetch_backlinks", fetch_backlinks);
}
