use seonexus_analytics::anomalies::{Anomaly, DayCount, detect};
use seonexus_analytics::overview::{CrawlOverview, OverviewInput};
use seonexus_core::table::Cell;
use serde::Serialize;

use super::AnalysisError;
use super::collect::{float, for_each_row, integer, key, with_filter};
use crate::workspace::Workspace;

/// Hits of verified search-engine crawlers.
const SEARCH: &str = "bot_category:search verification:verified";
/// Static assets: scripts, styles, images, fonts and framework bundles.
const ASSETS: &str = r#"(path:~"\.(js|mjs|css|png|jpe?g|gif|svg|webp|avif|ico|woff2?|ttf|eot|map)$" OR path:/_next/*)"#;

/// Input of [`crawl_overview`].
#[derive(Debug, Clone)]
pub struct OverviewRequest {
    /// Site name.
    pub site: String,
    /// Optional `date:` value, e.g. `last30d`.
    pub period: Option<String>,
}

/// The crawl overview of a site.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SiteCrawlOverview {
    /// Site name.
    pub site: String,
    /// Period as given.
    pub period: Option<String>,
    /// Indicators.
    #[serde(flatten)]
    pub overview: CrawlOverview,
    /// Unusual days (crawl drops and spikes, server-error bursts), newest first.
    pub alerts: Vec<Anomaly>,
}

/// Daily series watched for anomalies: name and filter.
const WATCHED: [(&str, &str); 4] = [
    ("googlebot", "bot:googlebot* verification:verified"),
    ("search", SEARCH),
    ("ai", "bot_category:ai-*"),
    ("all", ""),
];

/// Alerts shown at most.
const MAX_ALERTS: usize = 20;

/// Unusual days of the watched series within the period.
fn alerts(
    workspace: &Workspace,
    site: &str,
    period: Option<&str>,
) -> Result<Vec<Anomaly>, AnalysisError> {
    let mut found = Vec::new();
    for (name, filter) in WATCHED {
        let mut days = Vec::new();
        let text = with_filter(period, &format!("{filter} group:day metric:hits,hits_5xx"));
        for_each_row(workspace, site, &text, |cells| {
            if let Some(Cell::Date(day)) = cells.first() {
                days.push(DayCount {
                    day: *day,
                    hits: integer(cells.get(1)),
                    server_errors: integer(cells.get(2)),
                });
            }
        })?;
        found.extend(detect(name, &days));
    }
    found.sort_by(|a, b| b.day.cmp(&a.day).then_with(|| a.series.cmp(&b.series)));
    found.truncate(MAX_ALERTS);
    Ok(found)
}

/// Summarizes how traffic and search-engine crawl budget are spent.
///
/// # Errors
/// See [`AnalysisError`].
pub fn crawl_overview(
    workspace: &Workspace,
    request: &OverviewRequest,
) -> Result<SiteCrawlOverview, AnalysisError> {
    let period = request
        .period
        .as_deref()
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .map(|p| format!("date:{p}"));
    let site = &request.site;
    let query = |rest: &str| with_filter(period.as_deref(), rest);
    let single = |text: String| -> Result<Vec<u64>, AnalysisError> {
        let mut values = Vec::new();
        for_each_row(workspace, site, &text, |cells| {
            values = cells.iter().map(|cell| integer(Some(cell))).collect();
        })?;
        Ok(values)
    };

    let mut input = OverviewInput {
        total_hits: single(query("metric:hits"))?.first().copied().unwrap_or(0),
        ..OverviewInput::default()
    };
    for_each_row(
        workspace,
        site,
        &query("group:bot_category metric:hits"),
        |cells| {
            input
                .categories
                .push((key(cells.first()), integer(cells.get(1))));
        },
    )?;
    for_each_row(
        workspace,
        site,
        &query("bot:* group:bot,verification metric:hits"),
        |cells| {
            if let (Some(bot), Some(verdict)) = (key(cells.first()), key(cells.get(1))) {
                input.bots.push((bot, verdict, integer(cells.get(2))));
            }
        },
    )?;
    for_each_row(
        workspace,
        site,
        &query(&format!(
            "{SEARCH} group:status_class metric:hits sort:status_class:asc"
        )),
        |cells| {
            if let Some(class) = key(cells.first()) {
                input
                    .search_status_classes
                    .push((class, integer(cells.get(1))));
            }
        },
    )?;
    for_each_row(
        workspace,
        site,
        &query(&format!(
            "{SEARCH} metric:hits,unique_urls,errors,avg_bytes,avg_ms"
        )),
        |cells| {
            input.search_hits = integer(cells.first());
            input.search_unique_urls = integer(cells.get(1));
            input.search_errors = integer(cells.get(2));
            input.search_avg_bytes = float(cells.get(3));
            input.search_avg_ms = float(cells.get(4));
        },
    )?;
    for_each_row(
        workspace,
        site,
        &query(&format!("{SEARCH} group:day metric:unique_urls")),
        |cells| {
            input.search_days += 1;
            input.search_daily_unique_urls += integer(cells.get(1));
        },
    )?;
    input.search_parameter_hits = single(query(&format!("{SEARCH} params:true metric:hits")))?
        .first()
        .copied()
        .unwrap_or(0);
    input.search_asset_hits = single(query(&format!("{SEARCH} {ASSETS} metric:hits")))?
        .first()
        .copied()
        .unwrap_or(0);

    Ok(SiteCrawlOverview {
        site: request.site.clone(),
        period: request.period.clone(),
        overview: seonexus_analytics::overview::crawl_overview(&input),
        alerts: alerts(workspace, site, period.as_deref())?,
    })
}
