//! Domain overview (Site Explorer): what a domain ranks for, the traffic it brings, its history,
//! keywords (organic, lost and paid, with the ads shown), best pages, competitors and
//! subdomains, its traffic in every country, and how it compares with the project's competitors.
//!
//! Every report goes through the same three commands with a `report` argument: what is kept
//! (`get_overview`), what fetching would cost (`overview_estimate`) and fetching
//! (`fetch_overview`).

use octopuson_dataforseo::labs::{self, DomainOverview};
use octopuson_dataforseo::query::{FieldSpec, ListQuery, RANKED_KEYWORD_FIELDS};
use octopuson_dataforseo::site::KeywordSet;
use octopuson_dataforseo::{Client, Market, site};
use octopuson_platform::{AdCopy, AdSeen, CacheClass, ProjectRow, normalize_domain};
use serde::Deserialize;
use serde_json::Value;

use crate::fetch::{Estimate, Fetched, Paid};
use crate::router::{Context, Router};
use crate::{AppError, Workspace};

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
    /// The domain's rankings in every country.
    Countries,
}

#[derive(Debug, Deserialize)]
struct Ask {
    project_id: i64,
    /// Another domain than the project's (a competitor).
    #[serde(default)]
    domain: Option<String>,
    #[serde(default)]
    report: Report,
    /// Rows of a table report (10–1000).
    #[serde(default)]
    limit: Option<u32>,
    /// Another market than the project's: `country-language`.
    #[serde(default)]
    market: Option<String>,
    /// Filters, order and the first row of a list report, asked of DataForSEO.
    #[serde(default)]
    query: Option<ListQuery>,
    /// Months of history (24 unless asked; more reach back to October 2020).
    #[serde(default)]
    months: Option<u32>,
    /// Which keywords: organic (now), lost or paid.
    #[serde(default)]
    set: KeywordSet,
    /// More domains to compare beside the project's competitors.
    #[serde(default)]
    targets: Vec<String>,
    /// Pay for fresh data even while the kept answer is fresh.
    #[serde(default)]
    force: bool,
}

/// Fewest months of history asked for.
const MIN_MONTHS: u32 = 6;

struct Question {
    report: Report,
    domain: String,
    market: Market,
    paid: Paid<'static>,
}

impl Report {
    /// The fields a list report filters and sorts by on DataForSEO's side.
    fn fields(self) -> Option<&'static [FieldSpec]> {
        match self {
            Self::Keywords => Some(RANKED_KEYWORD_FIELDS),
            _ => None,
        }
    }
}

fn question(workspace: &Workspace, ask: &Ask) -> Result<Question, AppError> {
    let project: ProjectRow = workspace.store()?.project(ask.project_id)?;
    let domain = ask
        .domain
        .as_deref()
        .map(normalize_domain)
        .filter(|domain| !domain.is_empty())
        .unwrap_or_else(|| project.domain.clone());
    let market = super::market_of(&project, ask.market.as_deref())?;
    let rows = site::rows(ask.limit.unwrap_or(DEFAULT_ROWS));
    let (endpoint, mut task, estimate_usd) = match ask.report {
        Report::Summary => (
            labs::DOMAIN_OVERVIEW,
            labs::domain_overview_task(&domain, &market)?,
            labs::domain_overview_estimate(),
        ),
        Report::History => {
            let (year, month) = crate::this_month();
            let months = ask.months.map_or(site::HISTORY_MONTHS, |months| {
                months.clamp(MIN_MONTHS, site::months_since_start(year, month))
            });
            let from = site::months_before(year, month, months);
            (
                site::HISTORY,
                site::history_task(&domain, &market, &from)?,
                site::history_estimate_for(months),
            )
        }
        Report::Keywords => (
            site::RANKED_KEYWORDS,
            site::ranked_set_task(&domain, &market, rows, ask.set)?,
            site::estimate(rows),
        ),
        Report::Countries => (
            labs::DOMAIN_OVERVIEW,
            site::countries_task(&domain)?,
            site::estimate(site::COUNTRY_ROWS),
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
            let extra: Vec<String> = ask
                .targets
                .iter()
                .map(|target| normalize_domain(target))
                .filter(|target| !target.is_empty())
                .collect();
            if project.competitors.is_empty() && extra.is_empty() {
                return Err(AppError::Invalid(
                    "competitors: add competitors to the project to compare".to_owned(),
                ));
            }
            let mut targets = vec![domain.clone()];
            targets.extend(project.competitors.iter().cloned());
            targets.extend(extra);
            let task = site::bulk_traffic_task(&targets, &market)?;
            let count = u32::try_from(targets.len()).unwrap_or(u32::MAX);
            (site::BULK_TRAFFIC, task, site::estimate(count))
        }
    };
    let travels = super::queried(&mut task, ask.query.as_ref(), ask.report.fields())?;
    Ok(Question {
        report: ask.report,
        domain,
        market,
        paid: Paid {
            travels,
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
        Report::Countries => serde_json::to_value(site::countries_from_result(result)),
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
        market,
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
        |result| read(report, &domain, result),
    )?;
    if report == Report::Summary {
        super::history::note_domain(
            context.workspace,
            ask.project_id,
            &domain,
            &market,
            &fetched,
        );
    }
    if report == Report::Keywords && ask.set == KeywordSet::Paid {
        note_ads(context.workspace, ask.project_id, &domain, &fetched);
    }
    Ok(fetched)
}

/// Keeps the ads a paid-keywords answer shows, so the domain's ad history grows.
fn note_ads(workspace: &Workspace, project_id: i64, domain: &str, fetched: &Fetched<Value>) {
    let text = |value: &Value, name: &str| {
        value
            .get(name)
            .and_then(Value::as_str)
            .filter(|text| !text.is_empty())
            .map(str::to_owned)
    };
    let ads: Vec<AdSeen> = fetched
        .data
        .get("keywords")
        .and_then(Value::as_array)
        .map(|rows| {
            rows.iter()
                .filter_map(|row| {
                    let ad = row.get("ad")?;
                    Some(AdSeen {
                        title: text(ad, "title")?,
                        description: text(ad, "description"),
                        breadcrumb: text(ad, "breadcrumb"),
                        url: text(row, "url"),
                        keyword: text(row, "keyword")?,
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    if ads.is_empty() {
        return;
    }
    let noted = workspace
        .store()
        .and_then(|store| Ok(store.note_ads(project_id, domain, &ads, &fetched.fetched_at)?));
    if let Err(error) = noted {
        tracing::warn!(project = project_id, %error, "ads were not kept");
    }
}

#[derive(Debug, Deserialize)]
struct AdsAsk {
    project_id: i64,
    /// Another domain than the project's.
    #[serde(default)]
    domain: Option<String>,
}

/// The ads a domain was seen showing in fetched paid keywords: free, nothing is fetched.
fn list_ads(context: &Context<'_>, ask: AdsAsk) -> Result<Vec<AdCopy>, AppError> {
    let store = context.workspace.store()?;
    let project = store.project(ask.project_id)?;
    let domain = ask
        .domain
        .as_deref()
        .map(normalize_domain)
        .filter(|domain| !domain.is_empty())
        .unwrap_or(project.domain);
    Ok(store.ad_copies(ask.project_id, &domain)?)
}

/// Adds the commands.
pub fn register(router: &mut Router) {
    router.add("overview_estimate", overview_estimate);
    router.add("get_overview", get_overview);
    router.add("fetch_overview", fetch_overview);
    router.add("list_ads", list_ads);
}
