use std::collections::{BTreeMap, BTreeSet};

use crate::{
    AnalysisRequest, BusinessGoal, CoverageSummary, DailyObservation, DataGrain, DataSource,
    DateRange, GoalObservation, GoalTotals, Metrics, Opportunity, OpportunityProject,
    OpportunityReport, OpportunitySignal, PerformanceDataset, PerformanceRow, QueryComparison,
    RULE_VERSION,
};
use url::Url;

/// HTTP URL identity preserves path case, language folders and every query parameter.
pub fn normalized_url(raw: &str) -> Option<String> {
    if raw.bytes().any(|b| b <= 32 || b == 127) {
        return None;
    }
    let mut url = Url::parse(raw).ok()?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return None;
    }
    url.set_fragment(None);
    Some(url.to_string())
}

#[derive(Default)]
struct Totals {
    clicks: f64,
    impressions: f64,
    position_sum: f64,
    position_impressions: f64,
    missing_position: bool,
}
impl Totals {
    fn add(&mut self, row: &PerformanceRow) {
        let (Some(clicks), Some(impressions)) = (row.clicks, row.impressions) else {
            return;
        };
        if !valid(clicks) || !valid(impressions) || clicks > impressions {
            return;
        }
        self.clicks += clicks;
        self.impressions += impressions;
        if let Some(position) = row.position.filter(|n| n.is_finite() && *n >= 1.0) {
            self.position_sum += position * impressions;
            self.position_impressions += impressions;
        } else if impressions > 0.0 {
            self.missing_position = true;
        }
    }
    fn metrics(&self) -> Metrics {
        Metrics {
            clicks: self.clicks,
            impressions: self.impressions,
            ctr: (self.impressions > 0.0).then(|| self.clicks / self.impressions),
            position: (!self.missing_position && self.position_impressions > 0.0)
                .then(|| self.position_sum / self.position_impressions),
        }
    }
}
fn valid_row(row: &PerformanceRow) -> bool {
    row.clicks
        .zip(row.impressions)
        .is_some_and(|(c, i)| valid(c) && valid(i) && c <= i)
}
fn in_scope(raw: &str, project: &OpportunityProject) -> bool {
    Url::parse(raw)
        .ok()
        .zip(Url::parse(&project.site_url).ok())
        .is_some_and(|(url, site)| {
            url.origin() == site.origin()
                && (site.path() == "/"
                    || url.path() == site.path()
                    || url
                        .path()
                        .starts_with(&format!("{}/", site.path().trim_end_matches('/'))))
        })
}
fn valid(n: f64) -> bool {
    n.is_finite() && n >= 0.0
}
fn branded(project: &OpportunityProject, query: Option<&str>) -> u8 {
    if project.brand_terms.is_empty() || query.is_none() {
        return 0;
    }
    let query = query.unwrap_or_default().to_lowercase();
    if project
        .brand_terms
        .iter()
        .any(|term| !term.trim().is_empty() && query.contains(&term.trim().to_lowercase()))
    {
        1
    } else {
        2
    }
}
fn matches(row: &PerformanceRow, project: &OpportunityProject, request: &AnalysisRequest) -> bool {
    if row
        .url
        .as_deref()
        .is_some_and(|raw| !in_scope(raw, project))
    {
        return false;
    }
    if !request.country.is_empty()
        && !row
            .country
            .as_deref()
            .is_some_and(|v| v.eq_ignore_ascii_case(&request.country))
    {
        return false;
    }
    if !request.device.is_empty()
        && !row
            .device
            .as_deref()
            .is_some_and(|v| v.eq_ignore_ascii_case(&request.device))
    {
        return false;
    }
    if !request.path_prefix.is_empty()
        && !row
            .url
            .as_deref()
            .and_then(|s| Url::parse(s).ok())
            .is_some_and(|url| url.path().starts_with(&request.path_prefix))
    {
        return false;
    }
    match request.brand.as_str() {
        "brand" => branded(project, row.query.as_deref()) == 1,
        "non-brand" => branded(project, row.query.as_deref()) == 2,
        _ => true,
    }
}
fn dataset<'a>(
    datasets: &'a [PerformanceDataset],
    project: &str,
    source: DataSource,
    grain: DataGrain,
    range: DateRange,
) -> Option<&'a PerformanceDataset> {
    datasets
        .iter()
        .filter(|d| {
            d.project_id == project && d.source == source && d.grain == grain && d.range == range
        })
        .max_by_key(|d| d.created_at_ms)
}
fn groups(
    data: Option<&PerformanceDataset>,
    project: &OpportunityProject,
    request: &AnalysisRequest,
    cancel: &dyn Fn() -> bool,
) -> BTreeMap<String, Metrics> {
    let mut totals: BTreeMap<String, Totals> = BTreeMap::new();
    for row in data
        .into_iter()
        .flat_map(|d| &d.rows)
        .filter(|r| valid_row(r) && matches(r, project, request))
        .take_while(|_| !cancel())
    {
        if let Some(url) = row.url.as_deref().and_then(normalized_url) {
            totals.entry(url).or_default().add(row);
        }
    }
    totals
        .into_iter()
        .map(|(url, total)| (url, total.metrics()))
        .collect()
}
fn total(
    data: Option<&PerformanceDataset>,
    project: &OpportunityProject,
    request: &AnalysisRequest,
) -> Option<Metrics> {
    let data = data?;
    let mut totals = Totals::default();
    let mut found = false;
    for row in data
        .rows
        .iter()
        .filter(|r| valid_row(r) && matches(r, project, request))
    {
        totals.add(row);
        found = true;
    }
    found.then(|| totals.metrics())
}
fn bucket(position: f64) -> u8 {
    if position <= 3.0 {
        0
    } else if position <= 7.0 {
        1
    } else if position <= 10.0 {
        2
    } else if position <= 20.0 {
        3
    } else {
        4
    }
}

type PeerKey = (u8, String, String, u8);
#[derive(Default)]
struct PeerGroup {
    clicks: f64,
    impressions: f64,
    urls: BTreeMap<String, (f64, f64)>,
}
type Peers = BTreeMap<PeerKey, PeerGroup>;
fn peers(
    data: Option<&PerformanceDataset>,
    project: &OpportunityProject,
    cancel: &dyn Fn() -> bool,
) -> Peers {
    let mut groups = Peers::new();
    for r in data
        .into_iter()
        .flat_map(|d| &d.rows)
        .take_while(|_| !cancel())
    {
        let (Some(url), Some(position), Some(clicks), Some(impressions)) = (
            r.url.as_deref().and_then(normalized_url),
            r.position,
            r.clicks,
            r.impressions,
        ) else {
            continue;
        };
        if !in_scope(&url, project)
            || !valid(clicks)
            || !valid(impressions)
            || impressions <= 0.0
            || clicks > impressions
            || !position.is_finite()
            || position < 1.0
        {
            continue;
        }
        let key = (
            bucket(position),
            r.country.clone().unwrap_or_default().to_lowercase(),
            r.device.clone().unwrap_or_default().to_lowercase(),
            branded(project, r.query.as_deref()),
        );
        let group = groups.entry(key).or_default();
        group.clicks += clicks;
        group.impressions += impressions;
        let own = group.urls.entry(url).or_default();
        own.0 += clicks;
        own.1 += impressions;
    }
    groups
}
fn benchmark(
    peers: &Peers,
    row: &PerformanceRow,
    project: &OpportunityProject,
    target: u8,
) -> (Option<f64>, usize) {
    let key = (
        target,
        row.country.clone().unwrap_or_default().to_lowercase(),
        row.device.clone().unwrap_or_default().to_lowercase(),
        branded(project, row.query.as_deref()),
    );
    let Some(group) = peers.get(&key) else {
        return (None, 0);
    };
    let own = row
        .url
        .as_deref()
        .and_then(normalized_url)
        .and_then(|url| group.urls.get(&url));
    let count = group.urls.len() - usize::from(own.is_some());
    let clicks = group.clicks - own.map_or(0.0, |v| v.0);
    let impressions = group.impressions - own.map_or(0.0, |v| v.1);
    (
        (count >= 10 && impressions >= 5000.0).then(|| clicks / impressions),
        count,
    )
}

/// Runs the versioned rules using observed rows only.
///
/// # Errors
/// Invalid project identity, date ranges, overlapping periods or missing GSC page observations.
pub fn analyze(
    project: &OpportunityProject,
    request: &AnalysisRequest,
    datasets: &[PerformanceDataset],
) -> Result<OpportunityReport, String> {
    analyze_with_cancel(project, request, datasets, &|| false)
}

/// Runs the analysis with cooperative cancellation.
/// # Errors
/// Returns the same validation errors as `analyze`, or an operation-cancelled error.
#[allow(clippy::too_many_lines)] // Orchestrates validated source selection and the separate rule/report helpers.
pub fn analyze_with_cancel(
    project: &OpportunityProject,
    request: &AnalysisRequest,
    datasets: &[PerformanceDataset],
    cancel: &dyn Fn() -> bool,
) -> Result<OpportunityReport, String> {
    if cancel() {
        return Err("Operation cancelled".into());
    }
    if project.id != request.project_id
        || !request.current.valid()
        || !request.previous.valid()
        || request.previous.end >= request.current.start
    {
        return Err("Choose valid, non-overlapping current and previous periods".into());
    }
    let mut warnings = Vec::new();
    let mut paired = request.current.days() == request.previous.days();
    if !paired {
        warnings.push("unequal-periods".into());
    }
    let GscSelection {
        current,
        previous,
        query_current,
        query_previous,
    } = select_gsc(project, request, datasets);
    if [current, previous]
        .into_iter()
        .flatten()
        .any(|d| d.coverage.warnings.iter().any(|w| w == "row-limit"))
    {
        paired = false;
        warnings.push("loss-incomplete".into());
    }
    if current.is_none() {
        return Err(
            "Import or synchronize current-period GSC Pages or Query + Page data first".into(),
        );
    }
    if current.is_some_and(|d| d.grain == DataGrain::QueryPages) {
        warnings.push("query-subset-totals".into());
        if current.is_some_and(|d| d.coverage.warnings.iter().any(|w| w == "gsc-top-rows")) {
            paired = false;
            warnings.push("loss-query-subset".into());
        }
    }
    if previous.is_none() {
        warnings.push("previous-unavailable".into());
    }
    if [current, previous]
        .into_iter()
        .flatten()
        .flat_map(|d| &d.rows)
        .any(|r| {
            matches(r, project, request)
                && r.impressions.is_some_and(|v| v > 0.0)
                && r.position.is_none_or(|p| !p.is_finite() || p < 1.0)
        })
    {
        warnings.push("position-incomplete".into());
    }
    if project.brand_terms.is_empty() {
        warnings.push("brand-not-configured".into());
    }
    let current_pages = groups(current, project, request, cancel);
    let previous_pages = groups(previous, project, request, cancel);
    let ga_stale = current.is_some_and(|d| {
        d.coverage
            .warnings
            .iter()
            .any(|w| w.starts_with("ga4-sync-unavailable:"))
    });
    let ga = dataset(
        datasets,
        &project.id,
        DataSource::Ga4,
        DataGrain::LandingPages,
        request.current,
    )
    .filter(|_| !ga_stale);
    if ga_stale {
        warnings.push("ga4-stale".into());
    }
    if cancel() {
        return Err("Operation cancelled".into());
    }
    let business = business_rows(project, request, ga, &mut warnings);
    let peer_rows = peers(query_current.or(current), project, cancel);
    if cancel() {
        return Err("Operation cancelled".into());
    }
    let mut opportunities = build_opportunities(
        project,
        request,
        current,
        &current_pages,
        &previous_pages,
        query_current,
        query_previous,
        &peer_rows,
        &business,
        paired,
        cancel,
    );
    if cancel() {
        return Err("Operation cancelled".into());
    }
    attach_queries(
        &mut opportunities,
        project,
        request,
        query_current,
        query_previous,
        cancel,
    );
    if cancel() {
        return Err("Operation cancelled".into());
    }
    let unknown_current_urls = previous_pages
        .keys()
        .filter(|url| !current_pages.contains_key(*url))
        .count();
    if unknown_current_urls > 0 {
        warnings.push("missing-rows-not-zero".into());
    }
    let coverage = coverage_summary(project, request, datasets);
    let daily = daily_observations(project, request, datasets, &mut warnings);
    let property_overview = if coverage
        .iter()
        .any(|c| c.source == DataSource::Gsc && c.grain == DataGrain::Daily && c.coverage.limited)
    {
        None
    } else {
        property_overview_totals(project, request, &daily)
    };
    let (current_total, previous_total) = if let Some(totals) = property_overview {
        warnings.push("property-overview-totals".into());
        totals
    } else {
        warnings.push("observed-page-totals".into());
        (
            total(current, project, request),
            total(previous, project, request),
        )
    };
    let current_ga4 = ga4_totals(project, request, ga);
    let previous_ga4 = ga4_totals(
        project,
        request,
        dataset(
            datasets,
            &project.id,
            DataSource::Ga4,
            DataGrain::LandingPages,
            request.previous,
        )
        .filter(|_| !ga_stale),
    );
    Ok(OpportunityReport {
        rule_version: RULE_VERSION.into(),
        request: request.clone(),
        current: current_total,
        previous: previous_total,
        sessions: current_ga4.as_ref().and_then(|g| g.sessions),
        key_events: current_ga4.as_ref().and_then(|g| g.key_events),
        revenue: current_ga4.as_ref().and_then(|g| g.revenue),
        currency: current_ga4.and_then(|g| g.currency),
        previous_ga4,
        opportunities,
        coverage,
        warnings,
        unknown_current_urls,
        daily,
    })
}

#[allow(clippy::too_many_arguments)]
fn build_opportunities(
    project: &OpportunityProject,
    request: &AnalysisRequest,
    data: Option<&PerformanceDataset>,
    pages: &BTreeMap<String, Metrics>,
    previous: &BTreeMap<String, Metrics>,
    queries: Option<&PerformanceDataset>,
    old_queries: Option<&PerformanceDataset>,
    peer_groups: &Peers,
    business: &BTreeMap<String, GoalObservation>,
    paired: bool,
    cancel: &dyn Fn() -> bool,
) -> Vec<Opportunity> {
    let mut output = BTreeMap::<String, Opportunity>::new();
    query_opportunities(
        &mut output,
        project,
        request,
        data,
        pages,
        previous,
        queries,
        old_queries,
        peer_groups,
        cancel,
    );
    if data.is_some_and(|d| d.grain == DataGrain::Pages)
        && queries.is_some()
        && matches!(request.brand.as_str(), "" | "all")
    {
        let page_peers = peers(data, project, cancel);
        query_opportunities(
            &mut output,
            project,
            request,
            data,
            pages,
            previous,
            None,
            None,
            &page_peers,
            cancel,
        );
    }
    if paired {
        for (url, page) in pages {
            if let Some(old) = previous.get(url).filter(|old| {
                old.clicks >= 50.0
                    && old.clicks - page.clicks >= 10.0
                    && page.clicks <= old.clicks * 0.8
            }) {
                entry(&mut output, url, page, Some(old))
                    .signals
                    .push(OpportunitySignal {
                        kind: "traffic-loss".into(),
                        query: None,
                        current: page.clone(),
                        previous: Some(old.clone()),
                        benchmark_ctr: None,
                        peer_urls: 0,
                        estimated_clicks: Some(old.clicks - page.clicks),
                        review_only: false,
                    });
            }
        }
    }
    for item in output.values_mut() {
        rank(item, data, queries, business);
    }
    let mut output: Vec<_> = output.into_values().collect();
    output.sort_by(|a, b| {
        (a.confidence == "review")
            .cmp(&(b.confidence == "review"))
            .then_with(|| b.priority.total_cmp(&a.priority))
            .then_with(|| a.url.cmp(&b.url))
    });
    output
}
fn entry<'a>(
    output: &'a mut BTreeMap<String, Opportunity>,
    url: &str,
    current: &Metrics,
    previous: Option<&Metrics>,
) -> &'a mut Opportunity {
    output.entry(url.into()).or_insert_with(|| Opportunity {
        url: url.into(),
        current: current.clone(),
        previous: previous.cloned(),
        queries: Vec::new(),
        signals: Vec::new(),
        confidence: String::new(),
        priority: 0.0,
        priority_basis: String::new(),
        impact_clicks: None,
        business: None,
        actions: Vec::new(),
    })
}
#[derive(Default)]
struct CompleteMetric {
    total: f64,
    seen: bool,
    missing: bool,
}
impl CompleteMetric {
    fn add(&mut self, value: Option<f64>) {
        if let Some(value) = value.filter(|v| valid(*v)) {
            self.total += value;
            self.seen = true;
        } else {
            self.missing = true;
        }
    }
    fn value(&self) -> Option<f64> {
        (self.seen && !self.missing).then_some(self.total)
    }
}
#[derive(Default)]
struct BusinessTotals {
    sessions: f64,
    events: CompleteMetric,
    rate: CompleteMetric,
    revenue: CompleteMetric,
}
impl BusinessTotals {
    fn value(&self, goal: &BusinessGoal) -> Option<f64> {
        if self.sessions < 100.0 {
            return None;
        }
        match goal {
            BusinessGoal::Revenue => self.revenue.value().map(|v| v / self.sessions),
            BusinessGoal::AllEvents | BusinessGoal::Event => {
                self.rate.value().map(|v| v / self.sessions)
            }
        }
    }
}
fn business_rows(
    project: &OpportunityProject,
    request: &AnalysisRequest,
    data: Option<&PerformanceDataset>,
    warnings: &mut Vec<String>,
) -> BTreeMap<String, GoalObservation> {
    let mut output = BTreeMap::new();
    let Some(data) = data else {
        warnings.push("ga4-unavailable".into());
        return output;
    };
    if !data.coverage.google_organic
        || !project.ga4_origin_confirmed
        || !data.coverage.origin_confirmed
        || data.coverage.goal_key != project.goal_key()
        || !request.country.is_empty()
        || !request.device.is_empty()
        || !matches!(request.brand.as_str(), "" | "all")
    {
        warnings.push("ga4-not-comparable".into());
        return output;
    }
    let mut grouped = BTreeMap::<String, BusinessTotals>::new();
    for row in &data.rows {
        let Some(url) = row
            .url
            .as_deref()
            .and_then(normalized_url)
            .filter(|u| in_scope(u, project))
        else {
            continue;
        };
        let Some(sessions) = row.sessions.filter(|n| valid(*n)) else {
            continue;
        };
        let entry = grouped.entry(url).or_default();
        entry.sessions += sessions;
        entry.events.add(row.key_events);
        entry.revenue.add(row.revenue);
        if sessions > 0.0 {
            entry.rate.add(
                row.goal_rate
                    .filter(|n| valid(*n) && *n <= 1.0)
                    .map(|v| v * sessions),
            );
        }
    }
    let limited = data
        .coverage
        .warnings
        .iter()
        .any(|w| matches!(w.as_str(), "ga4-limited" | "row-limit" | "ga4-incomplete"));
    let incomplete = grouped
        .values()
        .any(|row| row.sessions >= 100.0 && row.value(&project.goal).is_none());
    if limited || incomplete {
        warnings.push("ga4-business-incomplete".into());
    }
    let mut observed: Vec<f64> = grouped
        .values()
        .filter_map(|row| row.value(&project.goal))
        .filter(|v| *v > 0.0)
        .collect();
    observed.sort_by(f64::total_cmp);
    let median = observed.get(observed.len() / 2).copied();
    for (url, row) in grouped {
        let boost = if limited {
            1.0
        } else {
            row.value(&project.goal)
                .zip(median)
                .map_or(1.0, |(v, median)| 1.0 + (v / median).clamp(0.0, 1.0))
        };
        output.insert(
            url,
            GoalObservation {
                sessions: row.sessions,
                key_events: row.events.value(),
                goal_rate: row
                    .rate
                    .value()
                    .filter(|_| row.sessions > 0.0)
                    .map(|v| v / row.sessions),
                revenue: row.revenue.value(),
                currency: data.coverage.currency.clone(),
                boost,
            },
        );
    }
    output
}

fn coverage_summary(
    project: &OpportunityProject,
    request: &AnalysisRequest,
    datasets: &[PerformanceDataset],
) -> Vec<CoverageSummary> {
    datasets
        .iter()
        .filter(|d| {
            d.project_id == project.id
                && (d.range == request.current
                    || d.range == request.previous
                    || d.grain == DataGrain::Daily
                        && d.range.start <= request.previous.start
                        && d.range.end >= request.current.end)
        })
        .map(|d| CoverageSummary {
            source: d.source,
            grain: d.grain,
            range: d.range,
            rows: d.rows.len(),
            coverage: d.coverage.clone(),
            origin: d.origin.clone(),
            created_at_ms: d.created_at_ms,
        })
        .collect()
}
fn property_overview_totals(
    project: &OpportunityProject,
    request: &AnalysisRequest,
    daily: &[DailyObservation],
) -> Option<(Option<Metrics>, Option<Metrics>)> {
    if daily.is_empty() {
        return None;
    }
    let site = Url::parse(&project.site_url).ok()?;
    let same_property_scope = if let Some(domain) = project.gsc_property.strip_prefix("sc-domain:")
    {
        site.host_str() == Some(domain) && site.path() == "/"
    } else {
        Url::parse(&project.gsc_property).is_ok_and(|property| {
            property.origin() == site.origin() && property.path() == site.path()
        })
    };
    if !same_property_scope {
        return None;
    }
    let totals = |range: DateRange| {
        let mut sum = Totals::default();
        let mut found = false;
        for day in daily
            .iter()
            .filter(|d| d.date >= range.start && d.date <= range.end)
        {
            sum.add(&PerformanceRow {
                clicks: Some(day.metrics.clicks),
                impressions: Some(day.metrics.impressions),
                position: day.metrics.position,
                ..Default::default()
            });
            found = true;
        }
        found.then(|| sum.metrics())
    };
    Some((totals(request.current), totals(request.previous)))
}

fn daily_observations(
    project: &OpportunityProject,
    request: &AnalysisRequest,
    datasets: &[PerformanceDataset],
    warnings: &mut Vec<String>,
) -> Vec<DailyObservation> {
    let folder_scope = Url::parse(&project.site_url).is_ok_and(|site| {
        site.path() != "/"
            && Url::parse(&project.gsc_property)
                .ok()
                .is_none_or(|property| property.path() != site.path())
    });
    if folder_scope {
        warnings.push("daily-property-wide".into());
        return Vec::new();
    }
    let mut daily = BTreeMap::<_, Totals>::new();
    if let Some(d) = datasets
        .iter()
        .filter(|d| {
            d.project_id == project.id
                && d.source == DataSource::Gsc
                && d.grain == DataGrain::Daily
                && d.range.start <= request.previous.start
                && d.range.end >= request.current.end
        })
        .max_by_key(|d| d.created_at_ms)
    {
        for row in &d.rows {
            if let Some(date) = row.date {
                daily.entry(date).or_default().add(row);
            }
        }
    }
    if !request.country.is_empty()
        || !request.device.is_empty()
        || !request.path_prefix.is_empty()
        || !matches!(request.brand.as_str(), "" | "all")
    {
        daily.clear();
        warnings.push("daily-unsegmented".into());
    }
    daily
        .into_iter()
        .map(|(date, total)| DailyObservation {
            date,
            metrics: total.metrics(),
        })
        .collect()
}

struct GscSelection<'a> {
    current: Option<&'a PerformanceDataset>,
    previous: Option<&'a PerformanceDataset>,
    query_current: Option<&'a PerformanceDataset>,
    query_previous: Option<&'a PerformanceDataset>,
}
fn has_requested_segments(data: &PerformanceDataset, request: &AnalysisRequest) -> bool {
    (request.country.is_empty() || data.rows.iter().all(|row| row.country.is_some()))
        && (request.device.is_empty() || data.rows.iter().all(|row| row.device.is_some()))
}
fn select_gsc<'a>(
    project: &OpportunityProject,
    request: &AnalysisRequest,
    datasets: &'a [PerformanceDataset],
) -> GscSelection<'a> {
    let query_current = dataset(
        datasets,
        &project.id,
        DataSource::Gsc,
        DataGrain::QueryPages,
        request.current,
    );
    let query_previous = dataset(
        datasets,
        &project.id,
        DataSource::Gsc,
        DataGrain::QueryPages,
        request.previous,
    );
    let page_current = dataset(
        datasets,
        &project.id,
        DataSource::Gsc,
        DataGrain::Pages,
        request.current,
    );
    let page_previous = dataset(
        datasets,
        &project.id,
        DataSource::Gsc,
        DataGrain::Pages,
        request.previous,
    );
    let current = if request.brand.is_empty() || request.brand == "all" {
        page_current
            .filter(|d| has_requested_segments(d, request))
            .or(query_current)
            .or(page_current)
    } else {
        query_current
    };
    let previous = if request.brand.is_empty() || request.brand == "all" {
        page_previous
            .filter(|d| has_requested_segments(d, request))
            .or(query_previous)
            .or(page_previous)
    } else {
        query_previous
    };
    GscSelection {
        current,
        previous,
        query_current,
        query_previous,
    }
}

fn rank(
    item: &mut Opportunity,
    data: Option<&PerformanceDataset>,
    queries: Option<&PerformanceDataset>,
    business: &BTreeMap<String, GoalObservation>,
) {
    let limited =
        data.is_some_and(|d| d.coverage.limited) || queries.is_some_and(|d| d.coverage.limited);
    let has_proof = item.signals.iter().any(|s| !s.review_only);
    item.confidence = if !has_proof {
        "review"
    } else if limited {
        "limited"
    } else {
        "observed"
    }
    .into();
    item.impact_clicks = item
        .signals
        .iter()
        .filter_map(|s| s.estimated_clicks)
        .max_by(f64::total_cmp);
    item.business = business.get(&item.url).cloned();
    let boost = item.business.as_ref().map_or(1.0, |b| b.boost);
    let confidence = if !has_proof {
        0.35
    } else if limited {
        0.65
    } else {
        1.0
    };
    item.priority_basis = if item.impact_clicks.is_some() {
        "click-impact"
    } else {
        "impression-review"
    }
    .into();
    item.priority = item
        .impact_clicks
        .unwrap_or_else(|| item.current.impressions.ln_1p())
        * confidence
        * boost;
    item.actions = item
        .signals
        .iter()
        .map(|s| {
            match s.kind.as_str() {
                "low-ctr" => "review-title-and-search-intent",
                "near-win" => "improve-relevance-and-internal-links",
                _ => "inspect-impressions-position-and-ctr",
            }
            .to_string()
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    item.signals.sort_by(|a, b| {
        b.estimated_clicks
            .unwrap_or(0.0)
            .total_cmp(&a.estimated_clicks.unwrap_or(0.0))
    });
}

fn aggregate_query_rows(
    data: Option<&PerformanceDataset>,
    project: &OpportunityProject,
    request: &AnalysisRequest,
    cancel: &dyn Fn() -> bool,
) -> Vec<PerformanceRow> {
    let mut aggregated: BTreeMap<_, Totals> = BTreeMap::new();
    for row in data
        .into_iter()
        .flat_map(|d| &d.rows)
        .filter(|r| valid_row(r) && matches(r, project, request))
        .take_while(|_| !cancel())
    {
        aggregated
            .entry((
                row.url.clone(),
                row.query.clone(),
                row.country.clone(),
                row.device.clone(),
            ))
            .or_default()
            .add(row);
    }
    aggregated
        .into_iter()
        .map(|((url, query, country, device), total)| {
            let m = total.metrics();
            PerformanceRow {
                url,
                query,
                country,
                device,
                clicks: Some(m.clicks),
                impressions: Some(m.impressions),
                position: m.position,
                ..Default::default()
            }
        })
        .collect()
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)] // Keeps the two query rules adjacent to their shared observed metrics.
fn query_opportunities(
    output: &mut BTreeMap<String, Opportunity>,
    project: &OpportunityProject,
    request: &AnalysisRequest,
    data: Option<&PerformanceDataset>,
    pages: &BTreeMap<String, Metrics>,
    previous: &BTreeMap<String, Metrics>,
    queries: Option<&PerformanceDataset>,
    old_queries: Option<&PerformanceDataset>,
    peers: &Peers,
    cancel: &dyn Fn() -> bool,
) {
    let mut old: BTreeMap<_, Totals> = BTreeMap::new();
    for row in old_queries.into_iter().flat_map(|d| &d.rows) {
        old.entry((
            row.url.clone(),
            row.query.clone(),
            row.country.clone(),
            row.device.clone(),
        ))
        .or_default()
        .add(row);
    }
    let rows = aggregate_query_rows(queries.or(data), project, request, cancel).into_iter();
    for row in rows.take_while(|_| !cancel()) {
        let row = &row;
        let Some(url) = row.url.as_deref().and_then(normalized_url) else {
            continue;
        };
        let Some(page) = pages.get(&url) else {
            continue;
        };
        let mut total = Totals::default();
        total.add(row);
        let metrics = total.metrics();
        let Some(position) = metrics.position else {
            continue;
        };
        let (reference, count) = benchmark(peers, row, project, bucket(position));
        let previous_query = old
            .get(&(
                row.url.clone(),
                row.query.clone(),
                row.country.clone(),
                row.device.clone(),
            ))
            .map(Totals::metrics);
        if metrics.impressions >= 1000.0
            && position <= 20.0
            && metrics
                .ctr
                .is_some_and(|ctr| reference.is_none_or(|b| b > 0.0 && ctr < b * 0.75))
        {
            let gain = reference
                .zip(metrics.ctr)
                .map(|(b, ctr)| ((b - ctr) * metrics.impressions).max(0.0));
            let signal = OpportunitySignal {
                kind: "low-ctr".into(),
                query: row.query.clone(),
                current: metrics.clone(),
                previous: previous_query.clone(),
                benchmark_ctr: reference,
                peer_urls: count,
                estimated_clicks: gain,
                review_only: reference.is_none(),
            };
            entry(output, &url, page, previous.get(&url))
                .signals
                .push(signal);
        }
        if row.query.is_some() && metrics.impressions >= 200.0 && (4.0..=20.0).contains(&position) {
            let (target, count) =
                benchmark(peers, row, project, bucket(position).saturating_sub(1));
            let gain = target
                .zip(metrics.ctr)
                .and_then(|(b, ctr)| (b > ctr).then_some((b - ctr) * metrics.impressions));
            entry(output, &url, page, previous.get(&url))
                .signals
                .push(OpportunitySignal {
                    kind: "near-win".into(),
                    query: row.query.clone(),
                    current: metrics,
                    previous: previous_query,
                    benchmark_ctr: target,
                    peer_urls: count,
                    estimated_clicks: gain,
                    review_only: gain.is_none(),
                });
        }
    }
}

fn attach_queries(
    opportunities: &mut [Opportunity],
    project: &OpportunityProject,
    request: &AnalysisRequest,
    current: Option<&PerformanceDataset>,
    previous: Option<&PerformanceDataset>,
    cancel: &dyn Fn() -> bool,
) {
    let aggregate = |data: Option<&PerformanceDataset>| {
        let mut groups = BTreeMap::<(String, String), Totals>::new();
        for row in data
            .into_iter()
            .flat_map(|d| &d.rows)
            .filter(|r| valid_row(r) && matches(r, project, request))
            .take_while(|_| !cancel())
        {
            if let (Some(url), Some(query)) = (
                row.url.as_deref().and_then(normalized_url),
                row.query.as_ref(),
            ) {
                groups.entry((url, query.clone())).or_default().add(row);
            }
        }
        groups
    };
    let old = aggregate(previous);
    let mut related = BTreeMap::<String, Vec<QueryComparison>>::new();
    for ((url, query), metrics) in aggregate(current) {
        let previous = old.get(&(url.clone(), query.clone())).map(Totals::metrics);
        related.entry(url).or_default().push(QueryComparison {
            query,
            current: metrics.metrics(),
            previous,
        });
    }
    for item in opportunities {
        item.queries = related.remove(&item.url).unwrap_or_default();
        item.queries
            .sort_by(|a, b| b.current.impressions.total_cmp(&a.current.impressions));
    }
}

fn ga4_totals(
    project: &OpportunityProject,
    request: &AnalysisRequest,
    data: Option<&PerformanceDataset>,
) -> Option<GoalTotals> {
    let data = data.filter(|d| {
        d.coverage.google_organic
            && d.coverage.goal_key == project.goal_key()
            && d.coverage.origin_confirmed
            && project.ga4_origin_confirmed
            && request.country.is_empty()
            && request.device.is_empty()
            && matches!(request.brand.as_str(), "" | "all")
    })?;
    if data
        .coverage
        .warnings
        .iter()
        .any(|w| matches!(w.as_str(), "ga4-incomplete" | "row-limit"))
    {
        return None;
    }
    let sum = |metric: fn(&PerformanceRow) -> Option<f64>| {
        let rows: Option<Vec<_>> = data
            .rows
            .iter()
            .filter(|r| matches(r, project, request))
            .map(|r| metric(r).filter(|v| valid(*v)))
            .collect();
        rows.filter(|v| !v.is_empty()).map(|v| v.iter().sum())
    };
    Some(GoalTotals {
        sessions: sum(|r| r.sessions),
        key_events: sum(|r| r.key_events),
        revenue: sum(|r| r.revenue),
        currency: data.coverage.currency.clone(),
    })
}
