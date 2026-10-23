//! Core Web Vitals with the user's Google API key: what real Chrome users experienced (the
//! Chrome UX Report, for a whole site or one page, now and over the last weeks) and a lab run
//! of one page (PageSpeed Insights: the performance score, its metrics and what would save the
//! most time). Both are free within Google's daily quota.
//!
//! Field metrics are the 75th percentile of 28 days of visits: LCP, INP, FCP and TTFB in
//! milliseconds, CLS unitless; each with the share of visits that were good, needed
//! improvement or were poor.

use octopuson_core::http::{HttpRequest, HttpTransport};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use url::form_urlencoded::byte_serialize;

use crate::{GoogleError, post, send_keyed};

/// The Chrome UX Report API.
pub const CRUX: &str = "https://chromeuxreport.googleapis.com/v1";

/// PageSpeed Insights.
pub const PAGESPEED: &str = "https://www.googleapis.com/pagespeedonline/v5/runPagespeed";

/// Weeks of history asked for (the API keeps 40).
pub const HISTORY_WEEKS: u32 = 25;

/// The visits a record covers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FormFactor {
    /// Phones.
    Phone,
    /// Desktop computers.
    Desktop,
}

impl FormFactor {
    const fn api(self) -> &'static str {
        match self {
            Self::Phone => "PHONE",
            Self::Desktop => "DESKTOP",
        }
    }

    /// PageSpeed Insights' strategy for it.
    pub const fn strategy(self) -> &'static str {
        match self {
            Self::Phone => "mobile",
            Self::Desktop => "desktop",
        }
    }
}

/// What a record is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target<'a> {
    /// A whole site (`https://www.example.com`).
    Origin(&'a str),
    /// One page.
    Url(&'a str),
}

/// One metric: its 75th percentile and how visits were spread.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct Metric {
    /// Milliseconds (CLS: unitless).
    pub p75: Option<f64>,
    /// Share of good visits, 0–1.
    pub good: Option<f64>,
    /// Share of visits needing improvement, 0–1.
    pub needs: Option<f64>,
    /// Share of poor visits, 0–1.
    pub poor: Option<f64>,
}

/// What real users experienced over 28 days.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct FieldData {
    /// Largest Contentful Paint.
    pub lcp: Option<Metric>,
    /// Interaction to Next Paint.
    pub inp: Option<Metric>,
    /// Cumulative Layout Shift.
    pub cls: Option<Metric>,
    /// First Contentful Paint.
    pub fcp: Option<Metric>,
    /// Time to First Byte.
    pub ttfb: Option<Metric>,
    /// The 28 days, `YYYY-MM-DD`.
    pub first_date: Option<String>,
    /// The last of the 28 days.
    pub last_date: Option<String>,
    /// The page had too few visits: the numbers are the whole site's.
    #[serde(default)]
    pub origin_fallback: bool,
}

/// The 75th percentiles week by week, oldest first.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct FieldHistory {
    /// The last day of each 28-day window, `YYYY-MM-DD`.
    pub dates: Vec<String>,
    /// Largest Contentful Paint, milliseconds.
    pub lcp: Vec<Option<f64>>,
    /// Interaction to Next Paint, milliseconds.
    pub inp: Vec<Option<f64>>,
    /// Cumulative Layout Shift.
    pub cls: Vec<Option<f64>>,
    /// First Contentful Paint, milliseconds.
    pub fcp: Vec<Option<f64>>,
    /// Time to First Byte, milliseconds.
    pub ttfb: Vec<Option<f64>>,
}

/// Something that would make the page load faster, with the time it would save.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Opportunity {
    /// Lighthouse audit id (`render-blocking-resources`).
    pub id: String,
    /// In the language asked for.
    pub title: String,
    /// Milliseconds it would save.
    pub savings_ms: f64,
}

/// A PageSpeed Insights run of one page.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct LabReport {
    /// The page after redirects.
    pub url: String,
    /// Performance score, 0–100.
    pub score: Option<f64>,
    /// Largest Contentful Paint, milliseconds.
    pub lcp_ms: Option<f64>,
    /// Cumulative Layout Shift.
    pub cls: Option<f64>,
    /// Total blocking time, the lab's stand-in for INP.
    pub tbt_ms: Option<f64>,
    /// First Contentful Paint, milliseconds.
    pub fcp_ms: Option<f64>,
    /// Speed Index, milliseconds.
    pub speed_index_ms: Option<f64>,
    /// Server response time, milliseconds.
    pub ttfb_ms: Option<f64>,
    /// Most time saved first.
    pub opportunities: Vec<Opportunity>,
    /// The page's (or, with too few visits, the site's) field data PageSpeed showed with it.
    pub field: Option<FieldData>,
}

/// Opportunities kept from a run.
const OPPORTUNITIES: usize = 8;

/// How a metric's 75th percentile reads: `good`, `needs` (improvement) or `poor`, by Google's
/// thresholds.
pub fn rating(metric: &str, p75: f64) -> &'static str {
    let (good, poor) = match metric {
        "lcp" => (2_500.0, 4_000.0),
        "inp" => (200.0, 500.0),
        "cls" => (0.1, 0.25),
        "fcp" => (1_800.0, 3_000.0),
        "ttfb" => (800.0, 1_800.0),
        "tbt" => (200.0, 600.0),
        _ => return "good",
    };
    if p75 <= good {
        "good"
    } else if p75 <= poor {
        "needs"
    } else {
        "poor"
    }
}

/// Whether field data passes the Core Web Vitals assessment: LCP, INP and CLS good (INP is
/// left out when there is none). `None` without LCP or CLS.
pub fn passes(field: &FieldData) -> Option<bool> {
    let good = |name: &str, metric: Option<&Metric>| {
        metric
            .and_then(|metric| metric.p75)
            .map(|p75| rating(name, p75) == "good")
    };
    let lcp = good("lcp", field.lcp.as_ref())?;
    let cls = good("cls", field.cls.as_ref())?;
    let inp = good("inp", field.inp.as_ref()).unwrap_or(true);
    Some(lcp && cls && inp)
}

fn number(value: &Value) -> Option<f64> {
    value
        .as_f64()
        .or_else(|| value.as_str().and_then(|text| text.parse().ok()))
}

fn date(value: Option<&Value>) -> Option<String> {
    let value = value?;
    let part = |name: &str| value.get(name).and_then(Value::as_u64);
    Some(format!(
        "{:04}-{:02}-{:02}",
        part("year")?,
        part("month")?,
        part("day")?
    ))
}

/// The record's metric named by any of `names` (TTFB was renamed over time).
fn named<'a>(metrics: &'a Value, names: &[&str]) -> Option<&'a Value> {
    names.iter().find_map(|name| metrics.get(*name))
}

const LCP: &[&str] = &["largest_contentful_paint"];
const INP: &[&str] = &["interaction_to_next_paint"];
const CLS: &[&str] = &["cumulative_layout_shift"];
const FCP: &[&str] = &["first_contentful_paint"];
const TTFB: &[&str] = &["experimental_time_to_first_byte", "time_to_first_byte"];

fn crux_metric(metric: &Value) -> Metric {
    let bins: Vec<f64> = metric
        .get("histogram")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|bin| bin.get("density").and_then(number))
        .collect();
    Metric {
        p75: metric
            .get("percentiles")
            .and_then(|percentiles| percentiles.get("p75"))
            .and_then(number),
        good: bins.first().copied(),
        needs: bins.get(1).copied(),
        poor: bins.get(2).copied(),
    }
}

/// Field data from a Chrome UX Report record.
pub fn field_from_record(value: &Value) -> Option<FieldData> {
    let record = value.get("record")?;
    let metrics = record.get("metrics")?;
    let read = |names: &[&str]| named(metrics, names).map(crux_metric);
    let period = record.get("collectionPeriod");
    Some(FieldData {
        lcp: read(LCP),
        inp: read(INP),
        cls: read(CLS),
        fcp: read(FCP),
        ttfb: read(TTFB),
        first_date: date(period.and_then(|period| period.get("firstDate"))),
        last_date: date(period.and_then(|period| period.get("lastDate"))),
        origin_fallback: false,
    })
}

/// The weekly 75th percentiles from a Chrome UX Report history record.
pub fn history_from_record(value: &Value) -> Option<FieldHistory> {
    let record = value.get("record")?;
    let metrics = record.get("metrics")?;
    let series = |names: &[&str]| -> Vec<Option<f64>> {
        named(metrics, names)
            .and_then(|metric| metric.get("percentilesTimeseries"))
            .and_then(|series| series.get("p75s"))
            .and_then(Value::as_array)
            .map(|values| values.iter().map(number).collect())
            .unwrap_or_default()
    };
    let dates: Vec<String> = record
        .get("collectionPeriods")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|period| date(period.get("lastDate")))
        .collect();
    Some(FieldHistory {
        lcp: series(LCP),
        inp: series(INP),
        cls: series(CLS),
        fcp: series(FCP),
        ttfb: series(TTFB),
        dates,
    })
}

fn crux_body(target: Target<'_>, form_factor: FormFactor) -> Value {
    let mut body = json!({ "formFactor": form_factor.api() });
    match target {
        Target::Origin(origin) => body["origin"] = json!(origin.trim_end_matches('/')),
        Target::Url(url) => body["url"] = json!(url),
    }
    body
}

/// What real users experienced on a site or page over the last 28 days; `None` when Chrome
/// has too few visits to tell.
///
/// # Errors
/// [`GoogleError`] when the key is refused or Google cannot be reached.
pub fn field(
    transport: &dyn HttpTransport,
    key: &str,
    target: Target<'_>,
    form_factor: FormFactor,
) -> Result<Option<FieldData>, GoogleError> {
    let request = post(
        &format!("{CRUX}/records:queryRecord"),
        &crux_body(target, form_factor),
    );
    match send_keyed(transport, key, request) {
        Ok(value) => Ok(field_from_record(&value)),
        Err(GoogleError::NotFound(_)) => Ok(None),
        Err(error) => Err(error),
    }
}

/// The 75th percentiles of the last [`HISTORY_WEEKS`] weeks; `None` without enough visits.
///
/// # Errors
/// [`GoogleError`] when the key is refused or Google cannot be reached.
pub fn history(
    transport: &dyn HttpTransport,
    key: &str,
    target: Target<'_>,
    form_factor: FormFactor,
) -> Result<Option<FieldHistory>, GoogleError> {
    let mut body = crux_body(target, form_factor);
    body["collectionPeriodCount"] = json!(HISTORY_WEEKS);
    let request = post(&format!("{CRUX}/records:queryHistoryRecord"), &body);
    match send_keyed(transport, key, request) {
        Ok(value) => Ok(history_from_record(&value)),
        Err(GoogleError::NotFound(_)) => Ok(None),
        Err(error) => Err(error),
    }
}

fn psi_metric(metric: &Value, scale: f64) -> Metric {
    let shares: Vec<f64> = metric
        .get("distributions")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|bin| bin.get("proportion").and_then(number))
        .collect();
    Metric {
        p75: metric
            .get("percentile")
            .and_then(number)
            .map(|value| value / scale),
        good: shares.first().copied(),
        needs: shares.get(1).copied(),
        poor: shares.get(2).copied(),
    }
}

/// A PageSpeed Insights answer read.
pub fn lab_from_answer(value: &Value) -> LabReport {
    let lighthouse = value.get("lighthouseResult").unwrap_or(&Value::Null);
    let audits = lighthouse.get("audits").unwrap_or(&Value::Null);
    let numeric = |id: &str| {
        audits
            .get(id)
            .and_then(|audit| audit.get("numericValue"))
            .and_then(number)
    };
    let mut opportunities: Vec<Opportunity> = audits
        .as_object()
        .into_iter()
        .flatten()
        .filter_map(|(id, audit)| {
            let details = audit.get("details")?;
            if details.get("type").and_then(Value::as_str) != Some("opportunity") {
                return None;
            }
            let savings = details.get("overallSavingsMs").and_then(number)?;
            (savings >= 50.0).then(|| Opportunity {
                id: id.clone(),
                title: audit
                    .get("title")
                    .and_then(Value::as_str)
                    .unwrap_or(id)
                    .to_owned(),
                savings_ms: savings.round(),
            })
        })
        .collect();
    opportunities.sort_by(|a, b| {
        b.savings_ms
            .total_cmp(&a.savings_ms)
            .then_with(|| a.id.cmp(&b.id))
    });
    opportunities.truncate(OPPORTUNITIES);
    let experience = value.get("loadingExperience");
    let field = experience
        .and_then(|experience| experience.get("metrics"))
        .and_then(Value::as_object)
        .filter(|metrics| !metrics.is_empty())
        .map(|metrics| {
            let read = |name: &str, scale: f64| metrics.get(name).map(|m| psi_metric(m, scale));
            FieldData {
                lcp: read("LARGEST_CONTENTFUL_PAINT_MS", 1.0),
                inp: read("INTERACTION_TO_NEXT_PAINT", 1.0),
                cls: read("CUMULATIVE_LAYOUT_SHIFT_SCORE", 100.0),
                fcp: read("FIRST_CONTENTFUL_PAINT_MS", 1.0),
                ttfb: read("EXPERIMENTAL_TIME_TO_FIRST_BYTE", 1.0),
                first_date: None,
                last_date: None,
                origin_fallback: experience
                    .and_then(|experience| experience.get("origin_fallback"))
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
            }
        });
    LabReport {
        url: lighthouse
            .get("finalDisplayedUrl")
            .or_else(|| lighthouse.get("finalUrl"))
            .or_else(|| value.get("id"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned(),
        score: lighthouse
            .get("categories")
            .and_then(|categories| categories.get("performance"))
            .and_then(|performance| performance.get("score"))
            .and_then(number)
            .map(|score| (score * 100.0).round()),
        lcp_ms: numeric("largest-contentful-paint").map(f64::round),
        cls: numeric("cumulative-layout-shift").map(|cls| (cls * 1_000.0).round() / 1_000.0),
        tbt_ms: numeric("total-blocking-time").map(f64::round),
        fcp_ms: numeric("first-contentful-paint").map(f64::round),
        speed_index_ms: numeric("speed-index").map(f64::round),
        ttfb_ms: numeric("server-response-time").map(f64::round),
        opportunities,
        field,
    }
}

/// Runs PageSpeed Insights' performance check of a page (it takes 10–30 seconds); titles in
/// `locale` (`tr`, `en` …).
///
/// # Errors
/// [`GoogleError`] when the key is refused, the page cannot be loaded or Google cannot be
/// reached.
pub fn lab(
    transport: &dyn HttpTransport,
    key: &str,
    url: &str,
    form_factor: FormFactor,
    locale: &str,
) -> Result<LabReport, GoogleError> {
    let encoded: String = byte_serialize(url.as_bytes()).collect();
    let locale: String = byte_serialize(locale.as_bytes()).collect();
    let request = HttpRequest::get(format!(
        "{PAGESPEED}?url={encoded}&strategy={}&category=performance&locale={locale}",
        form_factor.strategy()
    ));
    Ok(lab_from_answer(&send_keyed(transport, key, request)?))
}
