//! Core Web Vitals: what real Chrome users experience on the project's site (the Chrome UX
//! Report, on phones and desktops, with the last 25 weeks) and PageSpeed Insights runs of its
//! key pages (the home page, the pages Search Console shows most, the tracked keywords'
//! pages). Free with the user's own Google API key; checked as a job and kept day by day.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, PoisonError};

use octopuson_core::{Date, Timestamp};
use octopuson_google::GoogleError;
use octopuson_google::vitals::{self, FieldData, FieldHistory, FormFactor, LabReport, Target};
use octopuson_platform::{Period, ProjectRow, SearchKind, VitalCheck};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::jobs::{JobControl, JobDone, JobStatus};
use crate::router::{Context, Router};
use crate::sources::LLM_KEY;
use crate::{AppError, Workspace, now};

/// Kind of the job of a check.
pub const JOB_KIND: &str = "web-vitals";

/// The source whose key asks Google.
pub const PROVIDER: &str = "google-api";

/// Pages checked at most.
const MAX_PAGES: usize = 10;

/// PageSpeed runs at once.
const PARALLEL: usize = 3;

/// Longest address checked.
const MAX_URL: usize = 2_000;

#[derive(Debug, Deserialize)]
struct ProjectAsk {
    project_id: i64,
}

/// What the page offers before a check.
#[derive(Debug, Serialize)]
struct VitalsSetup {
    /// The Google API key is entered.
    key_ready: bool,
    /// Pages checked unless others are chosen.
    pages: Vec<String>,
    /// When the site was last checked, RFC 3339.
    last_checked: Option<String>,
}

const fn device_name(device: FormFactor) -> &'static str {
    match device {
        FormFactor::Phone => "phone",
        FormFactor::Desktop => "desktop",
    }
}

fn today() -> String {
    Date::of(Timestamp::now()).to_string()
}

/// `https://host` of an address.
fn origin_of(url: &str) -> Option<String> {
    let parsed = url::Url::parse(url).ok()?;
    Some(format!("{}://{}", parsed.scheme(), parsed.host_str()?))
}

/// Pages worth checking: the home page, the pages Search Console shows most, then the pages
/// ranking for tracked keywords.
fn suggested_pages(workspace: &Workspace, project: &ProjectRow) -> Result<Vec<String>, AppError> {
    let store = workspace.store()?;
    let audit_home = store
        .audits(project.id, 5)?
        .into_iter()
        .map(|audit| audit.start_url)
        .find(|url| !url.is_empty());
    let mut pages: Vec<String> =
        vec![audit_home.unwrap_or_else(|| format!("https://{}/", project.domain))];
    pages.extend(
        store
            .gsc_rows(project.id, SearchKind::Page, Period::Now, 30)?
            .into_iter()
            .map(|row| row.key),
    );
    pages.extend(
        store
            .content_candidates(project.id, 30)?
            .into_iter()
            .filter(|page| page.source == "rank")
            .map(|page| page.url),
    );
    let mut seen = std::collections::HashSet::new();
    pages.retain(|page| seen.insert(page.trim_end_matches('/').to_owned()));
    pages.truncate(MAX_PAGES);
    Ok(pages)
}

fn vitals_setup(context: &Context<'_>, ask: ProjectAsk) -> Result<VitalsSetup, AppError> {
    let workspace = context.workspace;
    let project = workspace.store()?.project(ask.project_id)?;
    Ok(VitalsSetup {
        key_ready: workspace.own_ready(PROVIDER),
        pages: suggested_pages(workspace, &project)?,
        last_checked: workspace
            .store()?
            .latest_vitals(ask.project_id)?
            .into_iter()
            .map(|check| check.checked_at)
            .max(),
    })
}

#[derive(Debug, Deserialize)]
struct StartAsk {
    project_id: i64,
    /// Pages to run PageSpeed on; the suggested ones when empty.
    #[serde(default)]
    pages: Vec<String>,
    /// Run the pages on a desktop too.
    #[serde(default)]
    desktop: bool,
}

/// What a Google refusal of the key means.
fn key_error(error: GoogleError) -> AppError {
    match error {
        GoogleError::Auth(message) | GoogleError::Forbidden(message) => AppError::Auth(format!(
            "Google refused the API key ({message}); check that the PageSpeed Insights API and the Chrome UX Report API are enabled for it"
        )),
        other => crate::google::google_error(other),
    }
}

/// Whether a failure concerns every request (the key), so the check stops.
const fn stops(error: &AppError) -> bool {
    matches!(error, AppError::Auth(_) | AppError::RateLimited { .. })
}

fn start_vitals(context: &Context<'_>, ask: StartAsk) -> Result<String, AppError> {
    let workspace = context.workspace;
    if !workspace.own_ready(PROVIDER) {
        return Err(AppError::NoSource(
            "enter a Google API key (PageSpeed Insights and Chrome UX Report) on the Sources page"
                .to_owned(),
        ));
    }
    let project = workspace.store()?.project(ask.project_id)?;
    let mut pages: Vec<String> = ask
        .pages
        .iter()
        .map(|page| page.trim().to_owned())
        .filter(|page| !page.is_empty())
        .collect();
    for page in &pages {
        let parsed = url::Url::parse(page)
            .ok()
            .filter(|parsed| matches!(parsed.scheme(), "http" | "https") && page.len() <= MAX_URL);
        if parsed.is_none() {
            return Err(AppError::Invalid(format!(
                "pages: `{page}` is not an address"
            )));
        }
    }
    if pages.is_empty() {
        pages = suggested_pages(workspace, &project)?;
    }
    pages.truncate(MAX_PAGES);
    let shared = Arc::clone(workspace);
    let desktop = ask.desktop;
    let (label, id) = (project.name.clone(), project.id);
    context
        .jobs
        .start(JOB_KIND, &label, Some(id), move |control| {
            run_vitals(&shared, &project, &pages, desktop, control)
        })
}

/// How a check went, as its job's result.
#[derive(Debug, Default, Serialize)]
struct Checked {
    /// Sites with field data (phone and desktop each).
    origins: u32,
    pages: u32,
    failed: u32,
}

fn as_value<T: Serialize>(value: Option<&T>) -> Option<Value> {
    value.and_then(|value| serde_json::to_value(value).ok())
}

/// The site's field data on one device: its own origin, else the other `www.` form.
fn origin_field(
    transport: &dyn octopuson_core::http::HttpTransport,
    key: &str,
    origins: &[String],
    device: FormFactor,
) -> Result<Option<(String, FieldData, Option<FieldHistory>)>, GoogleError> {
    for origin in origins {
        if let Some(field) = vitals::field(transport, key, Target::Origin(origin), device)? {
            let history = vitals::history(transport, key, Target::Origin(origin), device)?;
            return Ok(Some((origin.clone(), field, history)));
        }
    }
    Ok(None)
}

#[allow(clippy::too_many_lines)] // one check with its bookkeeping
fn run_vitals(
    workspace: &Workspace,
    project: &ProjectRow,
    pages: &[String],
    desktop: bool,
    control: &JobControl,
) -> Result<JobDone, AppError> {
    let key = workspace
        .own_secret(PROVIDER, LLM_KEY)
        .ok_or_else(|| AppError::NoSource("the Google API key is not entered".to_owned()))?;
    let transport = workspace.free_transport()?;
    let day = today();
    let locale = project.language.clone();
    let devices: Vec<FormFactor> = if desktop {
        vec![FormFactor::Phone, FormFactor::Desktop]
    } else {
        vec![FormFactor::Phone]
    };
    let total = 2 + (pages.len() * devices.len()) as u64;
    let mut summary = Checked::default();
    let mut origins: Vec<String> = pages
        .iter()
        .filter_map(|page| origin_of(page))
        .take(1)
        .collect();
    if let Some(first) = origins.first().cloned() {
        let other = if let Some(rest) = first.strip_prefix("https://www.") {
            format!("https://{rest}")
        } else {
            first.replacen("https://", "https://www.", 1)
        };
        origins.push(other);
    }
    for (index, device) in [FormFactor::Phone, FormFactor::Desktop]
        .into_iter()
        .enumerate()
    {
        control.progress(device.strategy(), index as u64, total);
        match origin_field(&transport, &key, &origins, device) {
            Ok(Some((origin, field, history))) => {
                workspace.store()?.save_vital_check(
                    project.id,
                    &VitalCheck {
                        target: origin,
                        scope: "origin".to_owned(),
                        device: device_name(device).to_owned(),
                        day: day.clone(),
                        field: as_value(Some(&field)),
                        lab: None,
                        history: as_value(history.as_ref()),
                        checked_at: now(),
                    },
                )?;
                summary.origins += 1;
            }
            Ok(None) => {}
            Err(error) => {
                let error = key_error(error);
                if stops(&error) {
                    return Err(error);
                }
                summary.failed += 1;
            }
        }
    }
    let jobs: Vec<(&String, FormFactor)> = pages
        .iter()
        .flat_map(|page| devices.iter().map(move |device| (page, *device)))
        .collect();
    let next = AtomicUsize::new(0);
    let done = AtomicUsize::new(2);
    let stopped: Mutex<Option<AppError>> = Mutex::new(None);
    let results: Mutex<Vec<(String, FormFactor, LabReport)>> = Mutex::new(Vec::new());
    let failed = AtomicUsize::new(0);
    std::thread::scope(|scope| {
        for _ in 0..PARALLEL.min(jobs.len()) {
            scope.spawn(|| {
                loop {
                    let index = next.fetch_add(1, Ordering::SeqCst);
                    let halted = stopped
                        .lock()
                        .unwrap_or_else(PoisonError::into_inner)
                        .is_some();
                    if index >= jobs.len() || halted || control.cancelled() {
                        break;
                    }
                    let (page, device) = jobs[index];
                    match vitals::lab(&transport, &key, page, device, &locale) {
                        Ok(report) => results
                            .lock()
                            .unwrap_or_else(PoisonError::into_inner)
                            .push((page.clone(), device, report)),
                        Err(error) => {
                            let error = key_error(error);
                            if stops(&error) {
                                stopped
                                    .lock()
                                    .unwrap_or_else(PoisonError::into_inner)
                                    .get_or_insert(error);
                            } else {
                                failed.fetch_add(1, Ordering::SeqCst);
                            }
                        }
                    }
                    let finished = done.fetch_add(1, Ordering::SeqCst) + 1;
                    control.progress(page, finished as u64, total);
                }
            });
        }
    });
    if let Some(error) = stopped.into_inner().unwrap_or_else(PoisonError::into_inner) {
        return Err(error);
    }
    summary.failed += u32::try_from(failed.load(Ordering::SeqCst)).unwrap_or(u32::MAX);
    let store = workspace.store()?;
    for (page, device, report) in results.into_inner().unwrap_or_else(PoisonError::into_inner) {
        store.save_vital_check(
            project.id,
            &VitalCheck {
                target: page,
                scope: "page".to_owned(),
                device: device_name(device).to_owned(),
                day: day.clone(),
                field: as_value(report.field.as_ref().filter(|field| !field.origin_fallback)),
                lab: as_value(Some(&report)),
                history: None,
                checked_at: now(),
            },
        )?;
        summary.pages += 1;
    }
    Ok(JobDone {
        status: if summary.failed == 0 {
            JobStatus::Done
        } else {
            JobStatus::Partial
        },
        result: serde_json::to_value(&summary).ok(),
    })
}

/// The site on one device.
#[derive(Debug, Serialize)]
struct OriginVitals {
    origin: String,
    device: String,
    day: String,
    field: Option<FieldData>,
    history: Option<FieldHistory>,
    /// The Core Web Vitals assessment: LCP, INP and CLS good.
    passes: Option<bool>,
}

/// A page on one device.
#[derive(Debug, Serialize)]
struct PageVitals {
    url: String,
    device: String,
    day: String,
    lab: Option<LabReport>,
    field: Option<FieldData>,
    passes: Option<bool>,
}

#[derive(Debug, Serialize)]
struct VitalsReport {
    origins: Vec<OriginVitals>,
    pages: Vec<PageVitals>,
    /// The PageSpeed score of each page day by day, oldest first: (day, url, device, score).
    scores: Vec<(String, String, String, f64)>,
}

fn vitals_report(context: &Context<'_>, ask: ProjectAsk) -> Result<VitalsReport, AppError> {
    let store = context.workspace.store()?;
    store.project(ask.project_id)?;
    let latest = store.latest_vitals(ask.project_id)?;
    let all = store.vital_checks(ask.project_id, None)?;
    let read = |value: &Option<Value>| -> Option<FieldData> {
        value
            .clone()
            .and_then(|value| serde_json::from_value(value).ok())
    };
    let mut origins = Vec::new();
    let mut pages = Vec::new();
    for check in latest {
        let field = read(&check.field);
        let passes = field.as_ref().and_then(vitals::passes);
        if check.scope == "origin" {
            origins.push(OriginVitals {
                history: check
                    .history
                    .clone()
                    .and_then(|value| serde_json::from_value(value).ok()),
                origin: check.target,
                device: check.device,
                day: check.day,
                field,
                passes,
            });
        } else {
            pages.push(PageVitals {
                lab: check
                    .lab
                    .clone()
                    .and_then(|value| serde_json::from_value(value).ok()),
                url: check.target,
                device: check.device,
                day: check.day,
                field,
                passes,
            });
        }
    }
    let scores = all
        .into_iter()
        .filter(|check| check.scope == "page")
        .filter_map(|check| {
            let score = check.lab.as_ref()?.get("score")?.as_f64()?;
            Some((check.day, check.target, check.device, score))
        })
        .collect();
    Ok(VitalsReport {
        origins,
        pages,
        scores,
    })
}

/// Adds the module's commands.
pub fn register(router: &mut Router) {
    router.add("vitals_setup", vitals_setup);
    router.add("start_vitals", start_vitals);
    router.add("vitals_report", vitals_report);
}
