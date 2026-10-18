use super::files::ImportCell;
use super::{
    DataCoverage, DataGrain, DataSource, DateRange, OpportunityProject, PerformanceDataset,
    PerformanceRow, digest, files, normalized_url, now_ms,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

pub type ColumnMapping = BTreeMap<String, usize>;
#[derive(Debug, Clone, Deserialize)]
pub struct ImportRequest {
    pub project_id: String,
    pub path: String,
    pub sheet: Option<String>,
    pub source: DataSource,
    pub grain: DataGrain,
    pub range: DateRange,
    #[serde(default)]
    pub columns: ColumnMapping,
    #[serde(default = "locale")]
    pub locale: String,
    #[serde(default)]
    pub timezone: String,
    pub currency: Option<String>,
    #[serde(default)]
    pub google_organic: bool,
    #[serde(default)]
    pub origin_confirmed: bool,
}
fn locale() -> String {
    "tr".into()
}
#[derive(Debug, Serialize)]
pub struct ImportSample {
    pub row: usize,
    pub reason: String,
}
#[derive(Debug, Serialize)]
pub struct ImportPreview {
    pub digest: String,
    pub sheets: Vec<String>,
    pub headers: Vec<String>,
    pub columns: ColumnMapping,
    pub rows: usize,
    pub valid: usize,
    pub invalid: usize,
    pub duplicates: usize,
    pub errors: Vec<ImportSample>,
    pub samples: Vec<PerformanceRow>,
    pub missing: Vec<String>,
    pub warnings: Vec<String>,
}
fn header(raw: &str) -> String {
    raw.trim()
        .to_lowercase()
        .replace('ı', "i")
        .replace('ş', "s")
        .replace('ğ', "g")
        .replace('ü', "u")
        .replace('ö', "o")
        .replace('ç', "c")
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect()
}
fn recognized(raw: &str) -> Option<&'static str> {
    match header(raw).as_str() {
        "url"
        | "page"
        | "pages"
        | "toppages"
        | "sayfa"
        | "sayfalar"
        | "eniyisayfalar"
        | "landingpage"
        | "landingpagequerystring"
        | "landingpageplusquerystring"
        | "acilissayfasi"
        | "acilissayfasisorgudizesi" => Some("url"),
        "query" | "queries" | "topqueries" | "sorgu" | "sorgular" | "eniyisorgular" => {
            Some("query")
        }
        "date" | "dates" | "tarih" => Some("date"),
        "clicks" | "tiklamalar" | "tiklama" => Some("clicks"),
        "impressions" | "gosterimler" | "gosterim" => Some("impressions"),
        "position" | "averageposition" | "konum" | "ortalamakonum" => Some("position"),
        "country" | "countryid" | "ulke" => Some("country"),
        "device" | "devicecategory" | "cihaz" | "cihazkategorisi" => Some("device"),
        "sessions" | "oturumlar" | "oturum" => Some("sessions"),
        "keyevents" | "conversions" | "onemliolaylar" | "donusumler" => Some("key_events"),
        "sessionkeyeventrate" | "oturumonemliolayorani" | "sessionconversionrate" => {
            Some("goal_rate")
        }
        "purchaserevenue" | "totalrevenue" | "revenue" | "satinalmageliri" | "toplamgelir"
        | "gelir" => Some("revenue"),
        _ => None,
    }
}
pub(super) fn preview(
    request: &ImportRequest,
    project: &OpportunityProject,
) -> Result<ImportPreview, String> {
    parse(request, project).map(|(preview, _)| preview)
}
#[allow(clippy::too_many_lines)] // Validates one preview transaction, retaining no source files.
pub(super) fn parse(
    request: &ImportRequest,
    project: &OpportunityProject,
) -> Result<(ImportPreview, Option<PerformanceDataset>), String> {
    if !request.range.valid() {
        return Err("Choose a valid period of at most 366 days".into());
    }
    if !matches!(
        (request.source, request.grain),
        (
            DataSource::Gsc,
            DataGrain::Pages | DataGrain::QueryPages | DataGrain::Queries | DataGrain::Daily
        ) | (DataSource::Ga4, DataGrain::LandingPages)
    ) {
        return Err("Report grain does not match its data source".into());
    }
    if !matches!(request.locale.as_str(), "tr" | "en") {
        return Err("Choose TR or EN numeric format".into());
    }
    let file = files::read(Path::new(&request.path), request.sheet.as_deref())?;
    let header_at = file
        .rows
        .iter()
        .take(30)
        .position(|row| row.iter().filter(|c| recognized(&c.text).is_some()).count() >= 2)
        .or_else(|| (!file.rows.is_empty()).then_some(0))
        .ok_or("The import file contains no header row")?;
    let headers: Vec<_> = file.rows[header_at]
        .iter()
        .map(|c| c.text.clone())
        .collect();
    let mut columns = if request.columns.is_empty() {
        headers
            .iter()
            .enumerate()
            .filter_map(|(i, name)| recognized(name).map(|name| (name.into(), i)))
            .collect()
    } else {
        request.columns.clone()
    };
    columns.retain(|key, index| *index < headers.len() && FIELDS.contains(&key.as_str()));
    if request.source == DataSource::Gsc
        && request.grain != DataGrain::QueryPages
        && columns.contains_key("url")
        && columns.contains_key("query")
    {
        return Err("This file contains query + page dimensions; choose the Query + page report type before importing".into());
    }
    let mut required = match request.source {
        DataSource::Gsc => vec!["clicks", "impressions"],
        DataSource::Ga4 => vec!["url", "sessions"],
    };
    match request.grain {
        DataGrain::Pages | DataGrain::LandingPages => required.push("url"),
        DataGrain::QueryPages => {
            required.push("url");
            required.push("query");
        }
        DataGrain::Queries => required.push("query"),
        DataGrain::Daily => required.push("date"),
    }
    let missing: Vec<String> = required
        .iter()
        .filter(|name| !columns.contains_key(**name))
        .map(|s| (*s).into())
        .collect();
    let sheet = request
        .sheet
        .as_deref()
        .or_else(|| file.sheets.first().map(String::as_str));
    let preview_digest = digest(
        &serde_json::to_vec(&(
            &file.digest,
            request.source,
            request.grain,
            request.range,
            &columns,
            &request.locale,
            &request.timezone,
            &request.currency,
            request.google_organic,
            request.origin_confirmed,
            sheet,
            project,
        ))
        .map_err(|e| e.to_string())?,
    );
    let mut preview = ImportPreview {
        digest: preview_digest.clone(),
        sheets: file.sheets,
        headers,
        columns: columns.clone(),
        rows: file.rows.len().saturating_sub(header_at + 1),
        valid: 0,
        invalid: 0,
        duplicates: 0,
        errors: Vec::new(),
        samples: Vec::new(),
        missing,
        warnings: vec!["imported-coverage-unknown".into()],
    };
    if !preview.missing.is_empty() {
        return Ok((preview, None));
    }
    let mut rows = Vec::new();
    let mut seen = BTreeSet::new();
    for (index, cells) in file.rows.iter().enumerate().skip(header_at + 1) {
        match row(cells, &columns, request, project) {
            Ok(row) => {
                let key = serde_json::to_string(&row).map_err(|e| e.to_string())?;
                if seen.insert(key) {
                    if preview.samples.len() < 10 {
                        preview.samples.push(row.clone());
                    }
                    rows.push(row);
                    preview.valid += 1;
                } else {
                    preview.duplicates += 1;
                }
            }
            Err(reason) => {
                preview.invalid += 1;
                if preview.errors.len() < 10 {
                    preview.errors.push(ImportSample {
                        row: index + 1,
                        reason,
                    });
                }
            }
        }
    }
    if rows.is_empty() {
        preview.warnings.push("no-valid-rows".into());
    }
    let dimensions = columns.keys().cloned().collect();
    let dataset = PerformanceDataset {
        id: digest(
            format!(
                "{}:{}:{:?}:{:?}:{:?}:{}",
                project.id,
                file.digest,
                request.source,
                request.grain,
                request.range,
                project.goal_key()
            )
            .as_bytes(),
        ),
        project_id: project.id.clone(),
        source: request.source,
        grain: request.grain,
        range: request.range,
        coverage: DataCoverage {
            limited: true,
            warnings: preview.warnings.clone(),
            timezone: if request.timezone.is_empty() {
                if request.source == DataSource::Gsc {
                    "America/Los_Angeles"
                } else {
                    "unknown"
                }
                .into()
            } else {
                request.timezone.clone()
            },
            currency: request.currency.clone(),
            google_organic: request.google_organic,
            origin_confirmed: request.origin_confirmed,
            goal_key: project.goal_key(),
            dimensions,
            site_url: project.site_url.clone(),
            property: if request.source == DataSource::Gsc {
                project.gsc_property.clone()
            } else {
                project.ga4_property.clone()
            },
        },
        rows,
        origin: "file-import".into(),
        created_at_ms: now_ms(),
    };
    Ok((preview, Some(dataset)))
}
const FIELDS: &[&str] = &[
    "url",
    "query",
    "date",
    "country",
    "device",
    "clicks",
    "impressions",
    "position",
    "sessions",
    "key_events",
    "goal_rate",
    "revenue",
];

fn number(cell: &ImportCell, locale: &str, rate: bool) -> Result<Option<f64>, String> {
    let text = cell.text.trim();
    if text.is_empty() || matches!(text, "—" | "-" | "(not set)") {
        return Ok(None);
    }
    let percent = text.contains('%');
    if percent
        && (!rate
            || text.matches('%').count() != 1
            || !(text.starts_with('%') || text.ends_with('%')))
    {
        return Err(
            "Percent values are only valid for rate columns; check the column mapping".into(),
        );
    }
    let mut text = text.replace(['%', ' ', '\u{a0}', '\u{202f}'], "");
    if !cell.numeric {
        text = if locale == "tr" {
            text.replace('.', "").replace(',', ".")
        } else {
            text.replace(',', "")
        };
    }
    let mut value = text
        .parse::<f64>()
        .map_err(|_| "Invalid number; check TR/EN number format")?;
    if percent {
        value /= 100.0;
    }
    if !value.is_finite() || value < 0.0 || rate && value > 1.0 {
        return Err("Metric is outside its valid range".into());
    }
    Ok(Some(value))
}
fn row(
    cells: &[ImportCell],
    columns: &ColumnMapping,
    request: &ImportRequest,
    project: &OpportunityProject,
) -> Result<PerformanceRow, String> {
    let cell = |name: &str| columns.get(name).and_then(|i| cells.get(*i));
    let text = |name: &str| {
        cell(name)
            .map(|c| c.text.trim())
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
    };
    let metric =
        |name: &str, rate: bool| cell(name).map_or(Ok(None), |c| number(c, &request.locale, rate));
    let mut row = PerformanceRow {
        url: text("url"),
        query: text("query"),
        country: text("country"),
        device: text("device"),
        clicks: metric("clicks", false)?,
        impressions: metric("impressions", false)?,
        position: metric("position", false)?,
        sessions: metric("sessions", false)?,
        key_events: metric("key_events", false)?,
        goal_rate: metric("goal_rate", true)?,
        revenue: metric("revenue", false)?,
        ..Default::default()
    };
    if let Some(raw) = text("date") {
        let raw = if cell("date").is_some_and(|c| c.numeric) {
            raw.parse::<i32>()
                .ok()
                .filter(|n| (1..=100_000).contains(n))
                .map_or(raw.clone(), |serial| {
                    seonexus_core::Date::from_days_since_epoch(serial - 25_569)
                        .map_or(raw.clone(), |date| date.to_string())
                })
        } else {
            raw
        };
        let normalized = if raw.len() == 8 && raw.bytes().all(|b| b.is_ascii_digit()) {
            format!("{}-{}-{}", &raw[..4], &raw[4..6], &raw[6..])
        } else {
            raw
        };
        let date = seonexus_core::Date::parse(&normalized).ok_or("Date must be YYYY-MM-DD")?;
        if date < request.range.start || date > request.range.end {
            return Err("Row date is outside the confirmed period".into());
        }
        row.date = Some(date);
    }
    if let Some(raw) = row.url.as_deref() {
        let absolute = if raw.starts_with('/')
            && !raw.starts_with("//")
            && request.source == DataSource::Ga4
        {
            let base = url::Url::parse(&project.site_url).map_err(|_| "Invalid project URL")?;
            base.join(raw)
                .map_err(|_| "Invalid landing page")?
                .to_string()
        } else {
            raw.into()
        };
        row.url = Some(normalized_url(&absolute).ok_or("Page is not a valid HTTP(S) URL")?);
    }
    if request.grain == DataGrain::Daily && row.date.is_none() {
        return Err("Daily report row is missing its date".into());
    }
    if request.source == DataSource::Gsc
        && (row.clicks.is_none() || row.impressions.is_none() || row.clicks > row.impressions)
    {
        return Err("GSC clicks/impressions are missing or inconsistent".into());
    }
    if row.position.is_some_and(|p| p < 1.0) && row.impressions.is_some_and(|n| n > 0.0) {
        return Err("Average position must be at least 1".into());
    }
    if request.source == DataSource::Ga4 && row.sessions.is_none() {
        return Err("GA4 sessions are missing".into());
    }
    if matches!(
        request.grain,
        DataGrain::Pages | DataGrain::QueryPages | DataGrain::LandingPages
    ) && row.url.is_none()
        || matches!(request.grain, DataGrain::Queries | DataGrain::QueryPages)
            && row.query.is_none()
    {
        return Err("The selected dimensions are missing in this row".into());
    }
    Ok(row)
}
#[cfg(test)]
#[path = "tests/imports.rs"]
mod tests;
