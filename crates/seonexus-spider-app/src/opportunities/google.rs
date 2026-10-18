//! Read-only Google APIs, bounded pagination, metadata and cancellable retry/backoff.
use super::{
    AnalysisRequest, BusinessGoal, DataCoverage, DataGrain, DataSource, DateRange,
    MAX_DATASET_ROWS, OpportunityProject, OpportunityWorkspace, PerformanceDataset, PerformanceRow,
    new_id, normalized_url, now_ms, oauth,
};
use seonexus_core::http::{HttpRequest, HttpTransport};
use serde::Serialize;
use serde_json::{Value, json};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Serialize)]
pub struct GoogleProperty {
    pub id: String,
    pub name: String,
}
#[derive(Debug, Serialize)]
pub struct GoogleCatalog {
    pub gsc: Vec<GoogleProperty>,
    pub ga4: Vec<GoogleProperty>,
    pub warnings: Vec<String>,
}
#[derive(Debug, Clone, Default, Serialize)]
pub struct SyncProgress {
    pub phase: String,
    pub rows: usize,
    pub requests: usize,
    pub period: Option<DateRange>,
    pub retry_after_seconds: Option<u64>,
}

type TokenProvider<'a> = &'a dyn Fn(&str, &dyn HttpTransport) -> Result<String, String>;
struct Client<'a> {
    token: TokenProvider<'a>,
    invalidate: &'a dyn Fn(&str),
    http: &'a dyn HttpTransport,
    connection: &'a str,
    cancel: &'a AtomicBool,
    requests: usize,
    progress: Option<&'a mut dyn FnMut(SyncProgress)>,
    progress_state: SyncProgress,
}
impl Client<'_> {
    fn emit(&mut self) {
        self.progress_state.requests = self.requests;
        if let Some(callback) = &mut self.progress {
            callback(self.progress_state.clone());
        }
    }
    fn phase(&mut self, phase: &str, period: Option<DateRange>) {
        self.progress_state.phase = phase.into();
        self.progress_state.period = period;
        self.progress_state.retry_after_seconds = None;
        self.emit();
    }
    fn advance(&mut self, rows: usize) {
        self.progress_state.rows += rows;
        self.emit();
    }

    fn send(&mut self, url: &str, body: Option<&Value>) -> Result<Value, String> {
        let parsed = url::Url::parse(url).map_err(|_| "Invalid Google endpoint")?;
        if parsed.scheme() != "https"
            || !matches!(
                parsed.host_str(),
                Some(
                    "www.googleapis.com"
                        | "analyticsdata.googleapis.com"
                        | "analyticsadmin.googleapis.com"
                )
            )
        {
            return Err("Unsupported Google API endpoint".into());
        }
        for attempt in 0_u32..=3 {
            if self.cancel.load(Ordering::Relaxed) {
                return Err("Operation cancelled".into());
            }
            let token = (self.token)(self.connection, self.http)?;
            let request = match body {
                Some(body) => {
                    HttpRequest::post(url, serde_json::to_vec(body).map_err(|e| e.to_string())?)
                        .header("content-type", "application/json")
                }
                None => HttpRequest::get(url),
            }
            .header("authorization", format!("Bearer {token}"));
            self.requests += 1;
            self.emit();
            let response = self
                .http
                .send(&request)
                .map_err(|_| "Google API could not be reached; existing data was preserved")?;
            if response.status == 401 && attempt < 1 {
                (self.invalidate)(self.connection);
                continue;
            }
            if response.status == 429 || (500..600).contains(&response.status) {
                if attempt < 3 {
                    let seconds = response
                        .header("retry-after")
                        .and_then(|s| s.parse::<u64>().ok())
                        .unwrap_or(1_u64 << attempt)
                        .min(30);
                    self.progress_state.retry_after_seconds = Some(seconds);
                    self.emit();
                    wait(seconds, self.cancel)?;
                    self.progress_state.retry_after_seconds = None;
                    continue;
                }
                return Err("Google API quota or temporary server error; retry later. Existing data was preserved.".into());
            }
            if response.status != 200 {
                return Err(format!(
                    "Google API HTTP {}: {}",
                    response.status,
                    match response.status {
                        401 => "reconnect your Google account",
                        403 => "check property permission and enable the required Google API",
                        400 => "the selected dimensions, metrics or property are unsupported",
                        _ => "request failed; existing data was preserved",
                    }
                ));
            }
            return serde_json::from_slice(&response.body)
                .map_err(|_| "Invalid Google API response".into());
        }
        Err("Google API request failed".into())
    }
}
fn wait(seconds: u64, cancel: &AtomicBool) -> Result<(), String> {
    let end = Instant::now() + Duration::from_secs(seconds);
    while Instant::now() < end {
        if cancel.load(Ordering::Relaxed) {
            return Err("Operation cancelled".into());
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    Ok(())
}

impl OpportunityWorkspace {
    /// # Errors
    /// Returns an error for an unknown connection or unavailable Google authentication.
    pub fn google_catalog(&self, connection: &str) -> Result<GoogleCatalog, String> {
        let data = self.read()?;
        if !data.connections.iter().any(|c| c.id == connection) {
            return Err("Choose a connected Google account".into());
        }
        let http = oauth::transport()?;
        let cancel = AtomicBool::new(false);
        let mut client = Client {
            token: &oauth::access_token,
            invalidate: &oauth::invalidate_access,
            http: &http,
            connection,
            cancel: &cancel,
            requests: 0,
            progress: None,
            progress_state: SyncProgress::default(),
        };
        catalog(&mut client)
    }
    /// # Errors
    /// Returns an error for invalid periods, authorization, transport, cancellation or atomic storage failure.
    pub fn sync_google(
        &self,
        request: &AnalysisRequest,
        cancel: &AtomicBool,
        mut progress: impl FnMut(SyncProgress),
    ) -> Result<(), String> {
        let project = self.project(&request.project_id)?;
        if !request.current.valid()
            || !request.previous.valid()
            || request.previous.end >= request.current.start
        {
            return Err("Choose valid non-overlapping periods".into());
        }
        if project.google_connection.is_empty() || project.gsc_property.is_empty() {
            return Err("Choose a Google connection and Search Console property".into());
        }
        let http = oauth::transport()?;
        let mut client = Client {
            token: &oauth::access_token,
            invalidate: &oauth::invalidate_access,
            http: &http,
            connection: &project.google_connection,
            cancel,
            requests: 0,
            progress: Some(&mut progress),
            progress_state: SyncProgress::default(),
        };
        let mut datasets = Vec::new();
        for range in [request.current, request.previous] {
            for grain in [DataGrain::Pages, DataGrain::Queries, DataGrain::QueryPages] {
                datasets.push(gsc(&mut client, &project, range, grain)?);
            }
        }
        let daily_range = DateRange {
            start: request.previous.start,
            end: request.current.end,
        };
        if daily_range.valid() {
            datasets.push(gsc(&mut client, &project, daily_range, DataGrain::Daily)?);
        }
        if !project.ga4_property.is_empty() {
            for range in [request.current, request.previous] {
                match ga4(&mut client, &project, range) {
                    Ok(dataset) => datasets.push(dataset),
                    Err(error) if !cancel.load(Ordering::Relaxed) => {
                        // Fresh GSC data remains useful when GA4 access/measurement is unavailable.
                        for dataset in &mut datasets {
                            dataset
                                .coverage
                                .warnings
                                .push(format!("ga4-sync-unavailable: {error}"));
                        }
                        break;
                    }
                    Err(error) => return Err(error),
                }
            }
        }
        if cancel.load(Ordering::Relaxed) {
            return Err("Operation cancelled".into());
        }
        client.phase("saving", None);
        self.commit_datasets_with_cancel(&project, datasets, &|| cancel.load(Ordering::Relaxed))
    }
}
fn catalog(client: &mut Client<'_>) -> Result<GoogleCatalog, String> {
    let mut output = GoogleCatalog {
        gsc: Vec::new(),
        ga4: Vec::new(),
        warnings: Vec::new(),
    };
    match client.send("https://www.googleapis.com/webmasters/v3/sites", None) {
        Ok(value) => {
            for site in value
                .get("siteEntry")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                if site.get("permissionLevel").and_then(Value::as_str) == Some("siteUnverifiedUser")
                {
                    continue;
                }
                if let Some(id) = site.get("siteUrl").and_then(Value::as_str) {
                    output.gsc.push(GoogleProperty {
                        id: id.into(),
                        name: id.into(),
                    });
                }
            }
        }
        Err(error) => output.warnings.push(error),
    }
    let mut page = String::new();
    for _ in 0..20 {
        let mut url =
            url::Url::parse("https://analyticsadmin.googleapis.com/v1beta/accountSummaries")
                .map_err(|e| e.to_string())?;
        url.query_pairs_mut().append_pair("pageSize", "200");
        if !page.is_empty() {
            url.query_pairs_mut().append_pair("pageToken", &page);
        }
        let value = match client.send(url.as_str(), None) {
            Ok(value) => value,
            Err(error) => {
                output.warnings.push(error);
                break;
            }
        };
        for account in value
            .get("accountSummaries")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            for property in account
                .get("propertySummaries")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                if let Some(id) = property.get("property").and_then(Value::as_str) {
                    output.ga4.push(GoogleProperty {
                        id: id.into(),
                        name: property
                            .get("displayName")
                            .and_then(Value::as_str)
                            .unwrap_or(id)
                            .into(),
                    });
                }
            }
        }
        page = value
            .get("nextPageToken")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .into();
        if page.is_empty() {
            break;
        }
    }
    output.ga4.sort_by(|a, b| a.id.cmp(&b.id));
    output.ga4.dedup_by(|a, b| a.id == b.id);
    Ok(output)
}

fn base_dataset(
    project: &OpportunityProject,
    range: DateRange,
    source: DataSource,
    grain: DataGrain,
) -> Result<PerformanceDataset, String> {
    Ok(PerformanceDataset {
        id: new_id()?,
        project_id: project.id.clone(),
        source,
        grain,
        range,
        coverage: DataCoverage {
            site_url: project.site_url.clone(),
            property: if source == DataSource::Gsc {
                project.gsc_property.clone()
            } else {
                project.ga4_property.clone()
            },
            ..Default::default()
        },
        rows: Vec::new(),
        origin: "google-api".into(),
        created_at_ms: now_ms(),
    })
}
fn gsc(
    client: &mut Client<'_>,
    project: &OpportunityProject,
    range: DateRange,
    grain: DataGrain,
) -> Result<PerformanceDataset, String> {
    let dimensions = match grain {
        DataGrain::Pages => vec!["page"],
        DataGrain::QueryPages => vec!["page", "query", "country", "device"],
        DataGrain::Queries => vec!["query", "country", "device"],
        DataGrain::Daily => vec!["date"],
        DataGrain::LandingPages => return Err("Unsupported GSC grain".into()),
    };
    client.phase(
        match grain {
            DataGrain::Pages => "gsc-pages",
            DataGrain::Queries => "gsc-queries",
            DataGrain::QueryPages => "gsc-query-pages",
            DataGrain::Daily => "gsc-daily",
            DataGrain::LandingPages => return Err("Unsupported GSC grain".into()),
        },
        Some(range),
    );
    let mut output = base_dataset(project, range, DataSource::Gsc, grain)?;
    output.coverage.timezone = "America/Los_Angeles".into();
    output.coverage.dimensions = dimensions.iter().map(|s| (*s).into()).collect();
    output.coverage.warnings.push("gsc-top-rows".into());
    let mut endpoint = url::Url::parse("https://www.googleapis.com/webmasters/v3/sites/")
        .map_err(|e| e.to_string())?;
    endpoint
        .path_segments_mut()
        .map_err(|()| "Invalid GSC endpoint")?
        .pop_if_empty()
        .push(&project.gsc_property)
        .push("searchAnalytics")
        .push("query");
    loop {
        let body = json!({"startDate":range.start.to_string(),"endDate":range.end.to_string(),"dimensions":dimensions,"type":"web","dataState":"final","aggregationType":if matches!(grain,DataGrain::Daily|DataGrain::Queries){"byProperty"}else{"auto"},"rowLimit":25000,"startRow":output.rows.len()});
        let value = client.send(endpoint.as_str(), Some(&body))?;
        let rows = value
            .get("rows")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        for raw in &rows {
            let keys = raw
                .get("keys")
                .and_then(Value::as_array)
                .ok_or("GSC row dimensions are missing")?;
            if keys.len() != dimensions.len() {
                return Err("GSC dimensions did not match the requested report".into());
            }
            let mut row = PerformanceRow {
                clicks: raw.get("clicks").and_then(Value::as_f64),
                impressions: raw.get("impressions").and_then(Value::as_f64),
                position: raw.get("position").and_then(Value::as_f64),
                ..Default::default()
            };
            for (dimension, key) in dimensions.iter().zip(keys) {
                let key = key.as_str().ok_or("Invalid GSC dimension")?;
                match *dimension {
                    "page" => row.url = Some(key.into()),
                    "query" => row.query = Some(key.into()),
                    "country" => row.country = Some(key.into()),
                    "device" => row.device = Some(key.into()),
                    "date" => row.date = seonexus_core::Date::parse(key),
                    _ => {}
                }
            }
            if row.clicks.is_none() || row.impressions.is_none() {
                return Err("GSC response metrics are missing".into());
            }
            output.rows.push(row);
        }
        client.advance(rows.len());
        if rows.len() < 25000 {
            break;
        }
        if output.rows.len() >= MAX_DATASET_ROWS {
            output.coverage.limited = true;
            output.coverage.warnings.push("row-limit".into());
            break;
        }
    }
    Ok(output)
}

pub(super) fn property_id(raw: &str) -> Result<String, String> {
    let id = raw.strip_prefix("properties/").unwrap_or(raw);
    if id.is_empty() || !id.bytes().all(|b| b.is_ascii_digit()) {
        return Err("GA4 property ID must be numeric".into());
    }
    Ok(format!("properties/{id}"))
}
#[allow(clippy::too_many_lines)] // Keeps GA4 metadata validation beside its bounded report pagination.
fn ga4(
    client: &mut Client<'_>,
    project: &OpportunityProject,
    range: DateRange,
) -> Result<PerformanceDataset, String> {
    client.phase("ga4", Some(range));
    let property = property_id(&project.ga4_property)?;
    let info = client.send(
        &format!("https://analyticsadmin.googleapis.com/v1beta/{property}"),
        None,
    )?;
    let metadata = client.send(
        &format!("https://analyticsdata.googleapis.com/v1beta/{property}/metadata"),
        None,
    )?;
    let rate = if project.goal == BusinessGoal::Event {
        format!("sessionKeyEventRate:{}", project.event_name)
    } else {
        "sessionKeyEventRate".into()
    };
    if !metadata
        .get("metrics")
        .and_then(Value::as_array)
        .is_some_and(|metrics| {
            metrics
                .iter()
                .any(|m| m.get("apiName").and_then(Value::as_str) == Some(&rate))
        })
    {
        return Err(
            "Selected GA4 key-event metric is unavailable; check the project's event name".into(),
        );
    }
    let mut output = base_dataset(project, range, DataSource::Ga4, DataGrain::LandingPages)?;
    output.coverage = DataCoverage {
        timezone: info
            .get("timeZone")
            .and_then(Value::as_str)
            .unwrap_or("unknown")
            .into(),
        currency: info
            .get("currencyCode")
            .and_then(Value::as_str)
            .map(str::to_owned),
        google_organic: true,
        origin_confirmed: project.ga4_origin_confirmed,
        goal_key: project.goal_key(),
        site_url: project.site_url.clone(),
        property: project.ga4_property.clone(),
        dimensions: vec!["landingPagePlusQueryString".into()],
        ..Default::default()
    };
    let metrics = [
        "sessions".to_owned(),
        "keyEvents".to_owned(),
        rate,
        "purchaseRevenue".into(),
    ];
    let compatibility=client.send(&format!("https://analyticsdata.googleapis.com/v1beta/{property}:checkCompatibility"),Some(&json!({"dimensions":[{"name":"landingPagePlusQueryString"}],"metrics":metrics.iter().map(|name|json!({"name":name})).collect::<Vec<_>>()})))?;
    if compatibility
        .get("metricCompatibilities")
        .and_then(Value::as_array)
        .is_some_and(|v| {
            v.iter()
                .any(|m| m.get("compatibility").and_then(Value::as_str) == Some("INCOMPATIBLE"))
        })
    {
        return Err("Selected GA4 metrics are incompatible with landing-page reporting".into());
    }
    let base = url::Url::parse(&project.site_url).map_err(|_| "Invalid project site URL")?;
    let mut fetched = 0;
    loop {
        let body = json!({"dateRanges":[{"startDate":range.start.to_string(),"endDate":range.end.to_string()}],"dimensions":[{"name":"landingPagePlusQueryString"}],"metrics":metrics.iter().map(|name|json!({"name":name})).collect::<Vec<_>>(),"dimensionFilter":{"andGroup":{"expressions":[{"filter":{"fieldName":"sessionSource","stringFilter":{"matchType":"EXACT","value":"google"}}},{"filter":{"fieldName":"sessionMedium","stringFilter":{"matchType":"EXACT","value":"organic"}}}]}},"offset":fetched.to_string(),"limit":"10000","orderBys":[{"dimension":{"dimensionName":"landingPagePlusQueryString"}}],"returnPropertyQuota":true});
        let value = client.send(
            &format!("https://analyticsdata.googleapis.com/v1beta/{property}:runReport"),
            Some(&body),
        )?;
        if value
            .pointer("/metadata/subjectToThresholding")
            .and_then(Value::as_bool)
            == Some(true)
            || value
                .pointer("/metadata/dataLossFromOtherRow")
                .and_then(Value::as_bool)
                == Some(true)
            || value
                .pointer("/metadata/samplingMetadatas")
                .and_then(Value::as_array)
                .is_some_and(|v| !v.is_empty())
        {
            output.coverage.limited = true;
            output.coverage.warnings.push("ga4-limited".into());
        }
        let rows = value
            .get("rows")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        fetched += rows.len();
        for raw in &rows {
            let Some(path) = raw
                .pointer("/dimensionValues/0/value")
                .and_then(Value::as_str)
                .filter(|s| s.starts_with('/') && !s.starts_with("//"))
            else {
                output.coverage.warnings.push("ga4-unmatched-url".into());
                continue;
            };
            let url = base
                .join(path)
                .ok()
                .and_then(|u| normalized_url(u.as_str()));
            let values = raw
                .get("metricValues")
                .and_then(Value::as_array)
                .ok_or("GA4 metric values are missing")?;
            let metric = |index: usize| {
                values
                    .get(index)
                    .and_then(|v| v.get("value"))
                    .and_then(Value::as_str)
                    .and_then(|v| v.parse::<f64>().ok())
                    .filter(|v| v.is_finite() && *v >= 0.0)
            };
            if metric(0).is_none() {
                return Err("GA4 sessions are missing or invalid".into());
            }
            output.rows.push(PerformanceRow {
                url,
                sessions: metric(0),
                key_events: if project.goal == BusinessGoal::Event {
                    None
                } else {
                    metric(1)
                },
                goal_rate: metric(2),
                revenue: metric(3),
                ..Default::default()
            });
        }
        client.advance(rows.len());
        if !ga4_has_more(&value, rows.len(), fetched, &mut output.coverage) {
            break;
        }
        if fetched >= MAX_DATASET_ROWS {
            output.coverage.limited = true;
            output.coverage.warnings.push("row-limit".into());
            break;
        }
    }
    if project.goal == BusinessGoal::Event {
        event_counts(client, project, range, &property, &base, &mut output)?;
    }
    output.coverage.warnings.sort();
    output.coverage.warnings.dedup();
    Ok(output)
}
fn event_counts(
    client: &mut Client<'_>,
    project: &OpportunityProject,
    range: DateRange,
    property: &str,
    base: &url::Url,
    output: &mut PerformanceDataset,
) -> Result<(), String> {
    client.phase("ga4-events", Some(range));
    let mut offset = 0;
    let mut counts = std::collections::BTreeMap::<String, f64>::new();
    loop {
        let body = json!({"dateRanges":[{"startDate":range.start.to_string(),"endDate":range.end.to_string()}],"dimensions":[{"name":"landingPagePlusQueryString"}],"metrics":[{"name":"keyEvents"}],"dimensionFilter":{"andGroup":{"expressions":[{"filter":{"fieldName":"sessionSource","stringFilter":{"matchType":"EXACT","value":"google"}}},{"filter":{"fieldName":"sessionMedium","stringFilter":{"matchType":"EXACT","value":"organic"}}},{"filter":{"fieldName":"eventName","stringFilter":{"matchType":"EXACT","value":project.event_name}}}]}},"offset":offset.to_string(),"limit":"10000","orderBys":[{"dimension":{"dimensionName":"landingPagePlusQueryString"}}]});
        let value = client.send(
            &format!("https://analyticsdata.googleapis.com/v1beta/{property}:runReport"),
            Some(&body),
        )?;
        if value
            .pointer("/metadata/subjectToThresholding")
            .and_then(Value::as_bool)
            == Some(true)
            || value
                .pointer("/metadata/dataLossFromOtherRow")
                .and_then(Value::as_bool)
                == Some(true)
            || value
                .pointer("/metadata/samplingMetadatas")
                .and_then(Value::as_array)
                .is_some_and(|v| !v.is_empty())
        {
            output.coverage.limited = true;
            output.coverage.warnings.push("ga4-limited".into());
        }
        let rows = value
            .get("rows")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        offset += rows.len();
        for row in &rows {
            let Some(path) = row
                .pointer("/dimensionValues/0/value")
                .and_then(Value::as_str)
                .filter(|s| s.starts_with('/') && !s.starts_with("//"))
            else {
                continue;
            };
            let Some(url) = base
                .join(path)
                .ok()
                .and_then(|u| normalized_url(u.as_str()))
            else {
                continue;
            };
            if let Some(count) = row
                .pointer("/metricValues/0/value")
                .and_then(Value::as_str)
                .and_then(|s| s.parse::<f64>().ok())
                .filter(|v| v.is_finite() && *v >= 0.0)
            {
                counts.insert(url, count);
            }
        }
        client.advance(rows.len());
        if !ga4_has_more(&value, rows.len(), offset, &mut output.coverage) {
            break;
        }
        if offset >= MAX_DATASET_ROWS {
            output.coverage.limited = true;
            output.coverage.warnings.push("row-limit".into());
            break;
        }
    }
    for row in &mut output.rows {
        row.key_events = row.url.as_ref().and_then(|url| counts.get(url)).copied();
    }
    Ok(())
}
fn ga4_has_more(
    response: &Value,
    received: usize,
    fetched: usize,
    coverage: &mut DataCoverage,
) -> bool {
    let total = response.get("rowCount").and_then(Value::as_u64);
    if received == 0 {
        if total.is_some_and(|total| total > fetched as u64) {
            coverage.limited = true;
            coverage.warnings.push("ga4-incomplete".into());
        }
        return false;
    }
    total.map_or(received >= 10_000, |total| total > fetched as u64)
}
#[cfg(test)]
#[path = "tests/google.rs"]
mod tests;
