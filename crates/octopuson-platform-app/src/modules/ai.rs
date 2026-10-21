//! AI Visibility (GEO): how ChatGPT, Claude, Gemini, Perplexity and Google AI Mode answer the
//! project's questions, and how its brand appears there: mention rate, place among the brands
//! named, citation of its site, tone, share of voice against rival brands, the sources the
//! answers cite, and all of it run by run. The LLM Mentions database adds the questions whose
//! AI answers mention the site across the web, its mention metrics and its rivals'.
//!
//! Each assistant is asked with the user's own key for it when there is one (OpenAI,
//! Anthropic, Google Gemini, Perplexity), otherwise through DataForSEO (own key or credits);
//! Google AI Mode only through DataForSEO. Answers are always asked fresh, assistants in
//! parallel, and every answer is paid and recorded on its source within the month's budget.

use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use octopuson_core::Timestamp;
use octopuson_dataforseo::ai as dfs;
use octopuson_dataforseo::{Client, Market};
use octopuson_llm::analysis::{Brand, analyze};
use octopuson_llm::{Answer, Citation, Engine, LlmError, Question};
use octopuson_platform::{
    AiAnswerRow, AiPrompt, AiRunRow, AiSettings, AnswerBrand, AnswerSource, CacheClass, ProjectRow,
    Rival, RunEnd, unix_now,
};
use octopuson_serp_score::rank::seconds_of;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::fetch::{Estimate, Fetched, Paid, Reply};
use crate::jobs::{JobControl, JobDone, JobManager, JobStatus};
use crate::router::{Context, Router};
use crate::sources::LLM_KEY;
use crate::{AppError, Workspace, now};

/// Module id, in spending records.
const MODULE: &str = "ai";

/// Kind of the job of a run.
pub const JOB_KIND: &str = "ai-visibility";

/// Google AI Mode, asked through DataForSEO only.
const AI_MODE: &str = "google_ai_mode";

/// Days of history a report shows unless asked otherwise.
const DEFAULT_DAYS: u32 = 90;

/// Runs listed.
const RUNS_SHOWN: u32 = 20;

/// Answers shown for one question and assistant.
const ANSWERS_SHOWN: usize = 10;

/// Domains and pages listed in a report.
const TOP_SOURCES: usize = 15;

/// Questions suggested at a time.
const SUGGESTIONS: usize = 12;

/// Commands after which an AI run may be due at another time.
const SCHEDULE_COMMANDS: &[&str] = &[
    "save_ai_settings",
    "add_ai_prompts",
    "remove_ai_prompts",
    "start_ai_run",
    "delete_project",
];

/// Whether a command may move when an AI run is due next.
pub fn changes_schedule(command: &str) -> bool {
    SCHEDULE_COMMANDS.contains(&command)
}

fn rfc3339(seconds: i64) -> String {
    Timestamp::from_unix_micros(seconds.saturating_mul(1_000_000)).to_string()
}

/// When a project's AI schedule is due, seconds since the Unix epoch.
fn due_at(settings: &AiSettings, prompts: u64) -> Option<i64> {
    let interval = match settings.schedule.as_str() {
        "weekly" => 6 * 86_400 + 12 * 3_600,
        "monthly" => 29 * 86_400,
        _ => return None,
    };
    if prompts == 0 || settings.engines.is_empty() {
        return None;
    }
    Some(
        settings
            .last_run_at
            .as_deref()
            .and_then(seconds_of)
            .map_or(0, |last| last + interval),
    )
}

// ----- Routing -----------------------------------------------------------------------------

/// How an assistant is asked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Path {
    /// Directly, with the user's own key for it.
    Direct(Engine),
    /// Through DataForSEO's LLM Responses.
    DataForSeo,
    /// Google AI Mode through DataForSEO's SERP API.
    AiMode,
}

/// An assistant ready to be asked.
#[derive(Debug, Clone)]
struct Route {
    engine: String,
    path: Path,
    /// `own`, `dataforseo` or `credit`, as answers record it.
    via: &'static str,
    model: String,
    estimate_usd: f64,
}

impl Route {
    fn provider(&self) -> &'static str {
        match self.path {
            Path::Direct(engine) => engine.provider(),
            Path::DataForSeo | Path::AiMode => octopuson_dataforseo::ID,
        }
    }
}

/// How an assistant would be asked now, or `None` when it cannot be.
fn route_of(workspace: &Workspace, settings: &AiSettings, engine: &str) -> Option<Route> {
    let app_settings = workspace.settings().ok()?;
    let chosen = settings
        .models
        .get(engine)
        .map(String::as_str)
        .filter(|model| !model.is_empty());
    let dataforseo = workspace.via(&app_settings, octopuson_dataforseo::ID);
    let dataforseo_via = |via: octopuson_cloud::Via| match via {
        octopuson_cloud::Via::Own => "dataforseo",
        octopuson_cloud::Via::Credit => "credit",
    };
    if engine == AI_MODE {
        return dataforseo.map(|via| Route {
            engine: engine.to_owned(),
            path: Path::AiMode,
            via: dataforseo_via(via),
            model: "google-ai-mode".to_owned(),
            estimate_usd: dfs::ai_mode_estimate(),
        });
    }
    let llm = Engine::parse(engine)?;
    if workspace.own_ready(llm.provider()) {
        let model = chosen.unwrap_or(llm.default_model()).to_owned();
        return Some(Route {
            engine: engine.to_owned(),
            path: Path::Direct(llm),
            via: "own",
            estimate_usd: llm.estimate_usd(&model),
            model,
        });
    }
    dataforseo.map(|via| Route {
        engine: engine.to_owned(),
        path: Path::DataForSeo,
        via: dataforseo_via(via),
        model: chosen.unwrap_or(dfs::default_model(engine)).to_owned(),
        estimate_usd: dfs::llm_estimate(engine),
    })
}

fn llm_error(error: LlmError) -> AppError {
    match error {
        LlmError::Auth(message) => AppError::Auth(message),
        LlmError::Quota(message) => AppError::Quota(message),
        LlmError::RateLimited { retry_after } => AppError::RateLimited { retry_after },
        LlmError::Unavailable(message) => AppError::Unavailable(message),
        LlmError::Refused(_) | LlmError::Invalid(_) => AppError::Provider(error.to_string()),
    }
}

/// The brands an answer is read for: the project's and its rivals'.
fn brands_of(project: &ProjectRow, settings: &AiSettings) -> Vec<Brand> {
    let mut brands = vec![Brand::of_site(&project.domain, &project.brands, true)];
    let rivals: Vec<Rival> = if settings.rivals.is_empty() {
        project
            .competitors
            .iter()
            .map(|domain| Rival {
                domain: domain.clone(),
                names: Vec::new(),
            })
            .collect()
    } else {
        settings.rivals.clone()
    };
    brands.extend(
        rivals
            .iter()
            .map(|rival| Brand::of_site(&rival.domain, &rival.names, false)),
    );
    brands
}

// ----- Setup -------------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct ProjectAsk {
    project_id: i64,
}

/// An assistant as the page shows it.
#[derive(Debug, Serialize)]
struct EngineInfo {
    engine: String,
    /// Asked in runs.
    enabled: bool,
    /// `own`, `dataforseo`, `credit`; `None` when it cannot be asked.
    via: Option<&'static str>,
    model: Option<String>,
    /// The source whose own key asks it directly.
    provider: Option<&'static str>,
}

#[derive(Debug, Serialize)]
struct AiSetup {
    settings: AiSettings,
    prompts: Vec<AiPrompt>,
    engines: Vec<EngineInfo>,
    /// The brands watched: the project's first.
    brands: Vec<Brand>,
    /// When the schedule runs next, RFC 3339; `None` when run only by hand.
    next_run_at: Option<String>,
}

fn setup(workspace: &Workspace, project_id: i64) -> Result<AiSetup, AppError> {
    let (project, settings, prompts) = {
        let store = workspace.store()?;
        (
            store.project(project_id)?,
            store.ai_settings(project_id)?,
            store.ai_prompts(project_id)?,
        )
    };
    let app_settings = workspace.settings()?;
    workspace.warm_catalog(&app_settings)?;
    let engines = octopuson_platform::AI_ENGINES
        .iter()
        .map(|&engine| {
            let route = route_of(workspace, &settings, engine);
            EngineInfo {
                engine: engine.to_owned(),
                enabled: settings.engines.iter().any(|known| known == engine),
                via: route.as_ref().map(|route| route.via),
                model: route.map(|route| route.model),
                provider: Engine::parse(engine).map(Engine::provider),
            }
        })
        .collect();
    Ok(AiSetup {
        next_run_at: due_at(&settings, prompts.len() as u64).map(|at| rfc3339(at.max(unix_now()))),
        brands: brands_of(&project, &settings),
        engines,
        prompts,
        settings,
    })
}

fn ai_setup(context: &Context<'_>, ask: ProjectAsk) -> Result<AiSetup, AppError> {
    setup(context.workspace, ask.project_id)
}

#[derive(Debug, Deserialize)]
struct SettingsAsk {
    project_id: i64,
    schedule: String,
    engines: Vec<String>,
    #[serde(default)]
    models: BTreeMap<String, String>,
    #[serde(default)]
    rivals: Vec<Rival>,
}

fn save_ai_settings(context: &Context<'_>, ask: SettingsAsk) -> Result<AiSetup, AppError> {
    context.workspace.store()?.save_ai_settings(
        ask.project_id,
        &AiSettings {
            schedule: ask.schedule,
            engines: ask.engines,
            models: ask.models,
            rivals: ask.rivals,
            last_run_at: None,
        },
    )?;
    setup(context.workspace, ask.project_id)
}

#[derive(Debug, Deserialize)]
struct PromptsAsk {
    project_id: i64,
    prompts: Vec<String>,
    #[serde(default)]
    tags: Vec<String>,
}

#[derive(Debug, Serialize)]
struct Added {
    added: usize,
    total: usize,
}

fn add_ai_prompts(context: &Context<'_>, ask: PromptsAsk) -> Result<Added, AppError> {
    let store = context.workspace.store()?;
    let added = store.add_ai_prompts(ask.project_id, &ask.prompts, &ask.tags, &now())?;
    Ok(Added {
        added,
        total: store.ai_prompts(ask.project_id)?.len(),
    })
}

#[derive(Debug, Deserialize)]
struct RemoveAsk {
    project_id: i64,
    prompt_ids: Vec<i64>,
}

fn remove_ai_prompts(context: &Context<'_>, ask: RemoveAsk) -> Result<usize, AppError> {
    Ok(context
        .workspace
        .store()?
        .remove_ai_prompts(ask.project_id, &ask.prompt_ids)?)
}

/// Question patterns by language: `{}` is a keyword.
fn patterns(language: &str) -> (&'static [&'static str], &'static [&'static str]) {
    match language {
        "tr" => (
            &[
                "{} için en iyi markalar hangileri?",
                "{} nereden alınır?",
                "En iyi {} önerileri neler?",
            ],
            &["{} güvenilir mi?", "{} yorumları nasıl?"],
        ),
        "de" => (
            &[
                "Welche Marken sind die besten für {}?",
                "Wo kann man {} kaufen?",
                "Was sind die besten {}-Empfehlungen?",
            ],
            &[
                "Ist {} vertrauenswürdig?",
                "Wie sind die Bewertungen von {}?",
            ],
        ),
        "es" => (
            &[
                "¿Cuáles son las mejores marcas de {}?",
                "¿Dónde comprar {}?",
                "¿Cuáles son las mejores recomendaciones de {}?",
            ],
            &["¿Es {} fiable?", "¿Qué opiniones tiene {}?"],
        ),
        _ => (
            &[
                "What are the best brands for {}?",
                "Where to buy {}?",
                "What are the best {} recommendations?",
            ],
            &["Is {} trustworthy?", "What do reviews say about {}?"],
        ),
    }
}

/// A question starting with a capital letter, the Turkish way for Turkish (`i` → `İ`).
fn capitalized(text: &str, language: &str) -> String {
    let mut chars = text.chars();
    let Some(first) = chars.next() else {
        return String::new();
    };
    let head = match (language, first) {
        ("tr", 'i') => "İ".to_owned(),
        ("tr", 'ı') => "I".to_owned(),
        _ => first.to_uppercase().collect(),
    };
    head + chars.as_str()
}

/// Free question suggestions: about the project's most searched tracked keywords and about
/// its brand, in its language, without the questions it already asks.
fn suggest_ai_prompts(context: &Context<'_>, ask: ProjectAsk) -> Result<Vec<String>, AppError> {
    let store = context.workspace.store()?;
    let project = store.project(ask.project_id)?;
    let known: Vec<String> = store
        .ai_prompts(project.id)?
        .into_iter()
        .map(|prompt| prompt.prompt)
        .collect();
    let mut keywords = store.tracked_keywords(project.id)?;
    keywords.sort_by_key(|keyword| std::cmp::Reverse(keyword.volume));
    let (about_topics, about_brand) = patterns(&project.language);
    let brand = project
        .brands
        .first()
        .cloned()
        .unwrap_or_else(|| project.domain.clone());
    let mut suggestions: Vec<String> = about_brand
        .iter()
        .map(|pattern| pattern.replace("{}", &brand))
        .collect();
    for keyword in keywords.iter().take(4) {
        for pattern in about_topics {
            suggestions.push(capitalized(
                &pattern.replace("{}", &keyword.keyword),
                &project.language,
            ));
        }
    }
    suggestions.retain(|suggestion| !known.contains(suggestion));
    suggestions.truncate(SUGGESTIONS);
    Ok(suggestions)
}

// ----- Runs --------------------------------------------------------------------------------

#[derive(Debug, Serialize)]
struct EngineCost {
    engine: String,
    via: &'static str,
    model: String,
    answers: u32,
    usd: f64,
    /// Credits, when it runs on credits.
    credits: Option<f64>,
}

#[derive(Debug, Serialize)]
struct AiEstimate {
    engines: Vec<EngineCost>,
    /// Answers of one run.
    answers: u32,
    /// List price of one run, US dollars.
    usd: f64,
    /// Credits of one run (the assistants on credits).
    credits: f64,
    runs_per_month: u32,
    month_usd: f64,
    month_credits: f64,
    /// Assistants chosen that cannot be asked now (no key, no credits).
    unavailable: Vec<String>,
}

/// The routes of the chosen assistants and those that cannot be asked.
fn routes(workspace: &Workspace, settings: &AiSettings) -> (Vec<Route>, Vec<String>) {
    let mut ready = Vec::new();
    let mut missing = Vec::new();
    for engine in &settings.engines {
        match route_of(workspace, settings, engine) {
            Some(route) => ready.push(route),
            None => missing.push(engine.clone()),
        }
    }
    (ready, missing)
}

fn ai_estimate(context: &Context<'_>, ask: ProjectAsk) -> Result<AiEstimate, AppError> {
    let workspace = context.workspace;
    let (settings, prompts) = {
        let store = workspace.store()?;
        store.project(ask.project_id)?;
        (
            store.ai_settings(ask.project_id)?,
            store.ai_prompts(ask.project_id)?.len(),
        )
    };
    let app_settings = workspace.settings()?;
    workspace.warm_catalog(&app_settings)?;
    let (ready, unavailable) = routes(workspace, &settings);
    let count = u32::try_from(prompts).unwrap_or(u32::MAX);
    let rate = workspace.credits_per_usd(octopuson_dataforseo::ID);
    let engines: Vec<EngineCost> = ready
        .iter()
        .map(|route| {
            let usd = route.estimate_usd * f64::from(count);
            EngineCost {
                engine: route.engine.clone(),
                via: route.via,
                model: route.model.clone(),
                answers: count,
                usd,
                credits: (route.via == "credit")
                    .then_some(rate)
                    .flatten()
                    .map(|rate| usd * rate),
            }
        })
        .collect();
    let usd: f64 = engines.iter().map(|engine| engine.usd).sum();
    let credits: f64 = engines.iter().filter_map(|engine| engine.credits).sum();
    let runs_per_month = match settings.schedule.as_str() {
        "weekly" => 4,
        "monthly" => 1,
        _ => 0,
    };
    Ok(AiEstimate {
        answers: count.saturating_mul(u32::try_from(engines.len()).unwrap_or(u32::MAX)),
        month_usd: usd * f64::from(runs_per_month),
        month_credits: credits * f64::from(runs_per_month),
        engines,
        usd,
        credits,
        runs_per_month,
        unavailable,
    })
}

/// Starts a run of a project as a job, or returns the one running.
fn start(
    workspace: &Arc<Workspace>,
    jobs: &JobManager,
    project: &ProjectRow,
    scheduled: bool,
) -> Result<String, AppError> {
    if let Some(id) = jobs.running_for(JOB_KIND, Some(project.id)) {
        return Ok(id);
    }
    let store = workspace.store()?;
    let settings = store.ai_settings(project.id)?;
    let prompts = store.ai_prompts(project.id)?.len();
    drop(store);
    if prompts == 0 {
        return Err(AppError::Invalid(
            "add questions before asking the assistants".to_owned(),
        ));
    }
    let app_settings = workspace.settings()?;
    workspace.warm_catalog(&app_settings)?;
    let (ready, _) = routes(workspace, &settings);
    if ready.is_empty() {
        return Err(AppError::NoSource(
            "no assistant can be asked: enter an assistant's API key or a DataForSEO key, or connect your OctopusOn SEO account to use credits".to_owned(),
        ));
    }
    let total = u32::try_from(prompts * ready.len()).unwrap_or(u32::MAX);
    let run_id = workspace
        .store()?
        .begin_ai_run(project.id, scheduled, total, &now())?;
    let workspace = Arc::clone(workspace);
    let project_id = project.id;
    jobs.start(JOB_KIND, &project.name, Some(project.id), move |control| {
        run_ai(&workspace, project_id, run_id, &ready, control)
    })
}

fn start_ai_run(context: &Context<'_>, ask: ProjectAsk) -> Result<String, AppError> {
    let project = context.workspace.store()?.project(ask.project_id)?;
    start(context.workspace, context.jobs, &project, false)
}

/// Whether a failure concerns every answer of the assistant's source: stop asking it.
const fn stops(error: &AppError) -> bool {
    !matches!(
        error,
        AppError::Provider(_) | AppError::Unavailable(_) | AppError::Busy(_)
    )
}

/// Asks one question on one route: the answer and what it cost.
fn ask_once(
    session: &crate::fetch::PaidRun<'_>,
    project: &ProjectRow,
    route: &Route,
    prompt: &str,
) -> Result<(Answer, f64), AppError> {
    let country = project.country.to_ascii_uppercase();
    let market = Market::new(&project.country, &project.language);
    let (endpoint, task) = match route.path {
        Path::Direct(engine) => (
            engine.as_str(),
            json!({ "prompt": prompt, "model": route.model }),
        ),
        Path::DataForSeo => (
            dfs::llm_endpoint(&route.engine).ok_or_else(|| {
                AppError::Invalid(format!("unknown assistant `{}`", route.engine))
            })?,
            dfs::llm_task(prompt, &route.model, Some(&country))?,
        ),
        Path::AiMode => (dfs::AI_MODE, dfs::ai_mode_task(prompt, &market)?),
    };
    let paid = Paid {
        module: MODULE,
        provider: route.provider(),
        endpoint,
        class: CacheClass::Ai,
        project_id: Some(project.id),
        task,
        estimate_usd: route.estimate_usd,
    };
    let answered = session.ask(&paid, |transport_route| match route.path {
        Path::Direct(engine) => {
            let key = transport_route.key(LLM_KEY)?;
            let answer = octopuson_llm::ask(
                engine,
                transport_route.transport,
                &key,
                &Question {
                    prompt,
                    model: &route.model,
                    country: Some(&country),
                },
            )
            .map_err(llm_error)?;
            let cost = answer.cost_usd;
            Ok(Reply {
                result: vec![serde_json::to_value(answer).unwrap_or(Value::Null)],
                cost_usd: cost,
                charged_usd: None,
            })
        }
        Path::DataForSeo | Path::AiMode => {
            let credentials = transport_route.key(octopuson_dataforseo::SECRET)?;
            let client = Client::new(transport_route.transport, &credentials)?;
            Ok(client.post(paid.endpoint, &paid.task)?.into())
        }
    })?;
    let answer = match route.path {
        Path::Direct(_) => answered
            .result
            .first()
            .cloned()
            .and_then(|value| serde_json::from_value::<Answer>(value).ok())
            .unwrap_or_default(),
        Path::DataForSeo | Path::AiMode => {
            let read = if route.path == Path::AiMode {
                dfs::ai_mode_from_result(&answered.result)
            } else {
                dfs::llm_from_result(&answered.result)
            };
            Answer {
                text: read.text,
                citations: read
                    .citations
                    .iter()
                    .map(|(url, title)| Citation::new(url, title.as_deref()))
                    .collect(),
                model: read.model.unwrap_or_else(|| route.model.clone()),
                ..Answer::default()
            }
        }
    };
    Ok((answer, answered.spent_usd))
}

/// How one assistant's part of a run went.
#[derive(Debug, Default)]
struct Tally {
    checked: u32,
    failed: u32,
    cost_usd: f64,
    stopped: Option<String>,
}

#[allow(clippy::too_many_arguments)] // one assistant's share of a run
fn run_engine(
    workspace: &Workspace,
    project: &ProjectRow,
    brands: &[Brand],
    prompts: &[AiPrompt],
    route: &Route,
    run_id: i64,
    control: &JobControl,
    done: &AtomicU64,
    total: u64,
) -> Tally {
    let mut tally = Tally::default();
    let session = match workspace.paid_run(route.provider()) {
        Ok(session) => session,
        Err(error) => {
            tally.failed = u32::try_from(prompts.len()).unwrap_or(u32::MAX);
            tally.stopped = Some(format!("{}: {error}", route.engine));
            return tally;
        }
    };
    for prompt in prompts {
        if control.cancelled() {
            break;
        }
        let outcome = ask_once(&session, project, route, &prompt.prompt);
        let mut row = AiAnswerRow {
            run_id,
            project_id: project.id,
            prompt_id: prompt.id,
            engine: route.engine.clone(),
            via: route.via.to_owned(),
            model: Some(route.model.clone()),
            checked_at: now(),
            text: String::new(),
            mentioned: false,
            position: None,
            cited: false,
            sentiment: None,
            brands: Vec::new(),
            citations: Vec::new(),
            error: None,
        };
        let mut stop = None;
        match outcome {
            Ok((answer, spent)) => {
                let analysis = analyze(&answer.text, &answer.citations, brands);
                row.model = Some(answer.model.clone()).filter(|model| !model.is_empty());
                row.mentioned = analysis.mentioned;
                row.position = analysis.position;
                row.cited = analysis.cited;
                row.sentiment = analysis.sentiment;
                row.brands = analysis
                    .brands
                    .into_iter()
                    .map(|hit| AnswerBrand {
                        key: hit.key,
                        own: hit.own,
                        position: hit.position,
                        cited: hit.cited,
                        sentiment: hit.sentiment,
                    })
                    .collect();
                row.citations = answer
                    .citations
                    .into_iter()
                    .map(|citation| AnswerSource {
                        url: citation.url,
                        title: citation.title,
                        domain: citation.domain,
                    })
                    .collect();
                row.text = answer.text;
                tally.checked += 1;
                tally.cost_usd += spent;
                control.spent(spent);
            }
            Err(error) => {
                row.error = Some(error.to_string());
                tally.failed += 1;
                if stops(&error) {
                    stop = Some(format!("{}: {error}", route.engine));
                }
            }
        }
        if let Err(error) = workspace
            .store()
            .and_then(|store| Ok(store.add_ai_answer(&row)?))
        {
            tracing::warn!(%error, "an AI answer was not kept");
        }
        let finished = done.fetch_add(1, Ordering::SeqCst) + 1;
        control.progress(&route.engine, finished, total);
        if stop.is_some() {
            tally.stopped = stop;
            break;
        }
    }
    tally
}

fn run_ai(
    workspace: &Workspace,
    project_id: i64,
    run_id: i64,
    routes: &[Route],
    control: &JobControl,
) -> Result<JobDone, AppError> {
    let (project, settings, prompts) = {
        let store = workspace.store()?;
        (
            store.project(project_id)?,
            store.ai_settings(project_id)?,
            store.ai_prompts(project_id)?,
        )
    };
    let brands = brands_of(&project, &settings);
    let total = (prompts.len() * routes.len()) as u64;
    let done = AtomicU64::new(0);
    control.progress("", 0, total);
    let tallies: Vec<Tally> = std::thread::scope(|scope| {
        let handles: Vec<_> = routes
            .iter()
            .map(|route| {
                let (project, brands, prompts, done) = (&project, &brands, &prompts, &done);
                scope.spawn(move || {
                    run_engine(
                        workspace, project, brands, prompts, route, run_id, control, done, total,
                    )
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|handle| handle.join().unwrap_or_default())
            .collect()
    });
    let checked: u32 = tallies.iter().map(|tally| tally.checked).sum();
    let failed: u32 = tallies.iter().map(|tally| tally.failed).sum();
    let cost_usd: f64 = tallies.iter().map(|tally| tally.cost_usd).sum();
    let stopped: Vec<String> = tallies
        .iter()
        .filter_map(|tally| tally.stopped.clone())
        .collect();
    let cancelled = control.cancelled();
    let status = if cancelled {
        "cancelled"
    } else if checked == 0 && failed > 0 {
        "failed"
    } else if failed > 0 {
        "partial"
    } else {
        "done"
    };
    let error = (!stopped.is_empty()).then(|| stopped.join("; "));
    workspace.store()?.finish_ai_run(
        run_id,
        &RunEnd {
            status,
            checked,
            failed,
            cost_usd,
            error: error.as_deref(),
        },
        &now(),
    )?;
    if status == "failed" {
        return Err(AppError::Provider(
            error.unwrap_or_else(|| "no assistant answered".to_owned()),
        ));
    }
    Ok(JobDone {
        status: if status == "partial" {
            JobStatus::Partial
        } else {
            JobStatus::Done
        },
        result: Some(
            json!({ "run_id": run_id, "checked": checked, "failed": failed, "cost_usd": cost_usd }),
        ),
    })
}

fn ai_runs(context: &Context<'_>, ask: ProjectAsk) -> Result<Vec<AiRunRow>, AppError> {
    Ok(context
        .workspace
        .store()?
        .ai_runs(ask.project_id, RUNS_SHOWN)?)
}

/// Starts the AI runs whose schedule is due; returns the jobs started (or already running).
///
/// # Errors
/// Storage errors.
pub fn run_due(workspace: &Arc<Workspace>, jobs: &JobManager) -> Result<Vec<String>, AppError> {
    let now_secs = unix_now();
    let store = workspace.store()?;
    let mut projects = Vec::new();
    for (project_id, settings, prompts) in store.ai_plans()? {
        if due_at(&settings, prompts).is_some_and(|at| at <= now_secs) {
            projects.push(store.project(project_id)?);
        }
    }
    drop(store);
    let mut started = Vec::new();
    for project in &projects {
        match start(workspace, jobs, project, true) {
            Ok(id) => started.push(id),
            Err(error) => {
                tracing::warn!(project = project.id, %error, "a scheduled AI run did not start");
            }
        }
    }
    Ok(started)
}

/// When the next scheduled AI run is due, seconds since the Unix epoch.
///
/// # Errors
/// Storage errors.
pub fn next_due(workspace: &Workspace) -> Result<Option<i64>, AppError> {
    Ok(workspace
        .store()?
        .ai_plans()?
        .iter()
        .filter_map(|(_, settings, prompts)| due_at(settings, *prompts))
        .min())
}

// ----- Report ------------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct ReportAsk {
    project_id: i64,
    #[serde(default)]
    days: Option<u32>,
}

/// How the brand fares in a set of answers.
#[derive(Debug, Clone, Default, Serialize)]
struct Visibility {
    /// Answers read.
    answers: u32,
    /// Answers mentioning the own brand.
    mentioned: u32,
    /// Answers citing its site.
    cited: u32,
    /// Share of answers mentioning it, 0–100.
    mention_rate: f64,
    /// Share of answers citing its site, 0–100.
    citation_rate: f64,
    /// Its average place among the brands named.
    average_position: Option<f64>,
    /// Average tone of its mentions, -1 to 1.
    sentiment: Option<f64>,
    /// Its share of every brand mention, 0–100.
    share_of_voice: f64,
}

#[allow(clippy::cast_precision_loss)] // answer counts
fn visibility(answers: &[&AiAnswerRow]) -> Visibility {
    let ok: Vec<&&AiAnswerRow> = answers.iter().filter(|row| row.error.is_none()).collect();
    let count = ok.len() as f64;
    let mentioned = ok.iter().filter(|row| row.mentioned).count();
    let cited = ok.iter().filter(|row| row.cited).count();
    let positions: Vec<f64> = ok
        .iter()
        .filter_map(|row| row.position)
        .map(f64::from)
        .collect();
    let tones: Vec<f64> = ok.iter().filter_map(|row| row.sentiment).collect();
    let all_mentions = ok
        .iter()
        .flat_map(|row| row.brands.iter())
        .filter(|brand| brand.position.is_some())
        .count();
    let round = |value: f64| (value * 10.0).round() / 10.0;
    Visibility {
        answers: u32::try_from(ok.len()).unwrap_or(u32::MAX),
        mentioned: u32::try_from(mentioned).unwrap_or(u32::MAX),
        cited: u32::try_from(cited).unwrap_or(u32::MAX),
        mention_rate: if count > 0.0 {
            round(mentioned as f64 / count * 100.0)
        } else {
            0.0
        },
        citation_rate: if count > 0.0 {
            round(cited as f64 / count * 100.0)
        } else {
            0.0
        },
        average_position: (!positions.is_empty())
            .then(|| round(positions.iter().sum::<f64>() / positions.len() as f64)),
        sentiment: (!tones.is_empty())
            .then(|| (tones.iter().sum::<f64>() / tones.len() as f64 * 100.0).round() / 100.0),
        share_of_voice: if all_mentions > 0 {
            round(mentioned as f64 / all_mentions as f64 * 100.0)
        } else {
            0.0
        },
    }
}

#[derive(Debug, Serialize)]
struct EngineVisibility {
    engine: String,
    #[serde(flatten)]
    visibility: Visibility,
}

/// A brand across the latest answers.
#[derive(Debug, Serialize)]
struct BrandShare {
    key: String,
    own: bool,
    /// Answers naming it.
    mentions: u32,
    /// Answers citing its site.
    citations: u32,
    /// Its share of every brand mention, 0–100.
    share: f64,
    average_position: Option<f64>,
}

/// Results of one run.
#[derive(Debug, Serialize)]
struct RunPoint {
    run_id: i64,
    /// `YYYY-MM-DD`.
    date: String,
    mention_rate: f64,
    citation_rate: f64,
    share_of_voice: f64,
    /// Mention rate by assistant.
    engines: BTreeMap<String, f64>,
}

/// The latest answer of a question on one assistant.
#[derive(Debug, Serialize)]
struct Cell {
    mentioned: bool,
    position: Option<u32>,
    cited: bool,
    sentiment: Option<f64>,
    error: Option<String>,
    checked_at: String,
}

#[derive(Debug, Serialize)]
struct PromptRow {
    prompt_id: i64,
    prompt: String,
    tags: Vec<String>,
    results: BTreeMap<String, Cell>,
}

#[derive(Debug, Serialize)]
struct SourceCount {
    /// A domain, or a page of the own site.
    key: String,
    title: Option<String>,
    own: bool,
    /// Answers citing it.
    answers: u32,
}

#[derive(Debug, Serialize)]
struct AiReport {
    /// Over the latest answer of every question on every assistant.
    summary: Visibility,
    engines: Vec<EngineVisibility>,
    brands: Vec<BrandShare>,
    history: Vec<RunPoint>,
    prompts: Vec<PromptRow>,
    /// Domains the latest answers cite most.
    top_domains: Vec<SourceCount>,
    /// The own site's pages the latest answers cite.
    own_pages: Vec<SourceCount>,
    last_run: Option<AiRunRow>,
    since: String,
}

fn belongs(domain: &str, site: &str) -> bool {
    domain == site
        || domain
            .strip_suffix(site)
            .is_some_and(|rest| rest.ends_with('.'))
}

#[allow(clippy::cast_precision_loss, clippy::too_many_lines)] // counts; one report
fn ai_report(context: &Context<'_>, ask: ReportAsk) -> Result<AiReport, AppError> {
    let days = ask.days.unwrap_or(DEFAULT_DAYS).clamp(7, 365);
    let since = rfc3339(unix_now() - i64::from(days) * 86_400);
    let store = context.workspace.store()?;
    let project = store.project(ask.project_id)?;
    let settings = store.ai_settings(project.id)?;
    let prompts = store.ai_prompts(project.id)?;
    let answers = store.ai_answers(project.id, Some(&since), None, None)?;
    let runs = store.ai_runs(project.id, RUNS_SHOWN)?;
    drop(store);
    let own = Brand::of_site(&project.domain, &[], true).key;

    // The latest answer of every question on every assistant.
    let mut latest: HashMap<(i64, &str), &AiAnswerRow> = HashMap::new();
    for answer in &answers {
        latest.insert((answer.prompt_id, answer.engine.as_str()), answer);
    }
    let current: Vec<&AiAnswerRow> = latest.values().copied().collect();
    let mut engines: Vec<EngineVisibility> = Vec::new();
    for engine in octopuson_platform::AI_ENGINES {
        let rows: Vec<&AiAnswerRow> = current
            .iter()
            .copied()
            .filter(|row| row.engine == *engine)
            .collect();
        if rows.is_empty() && !settings.engines.iter().any(|known| known == engine) {
            continue;
        }
        engines.push(EngineVisibility {
            engine: (*engine).to_owned(),
            visibility: visibility(&rows),
        });
    }

    let ok: Vec<&AiAnswerRow> = current
        .iter()
        .copied()
        .filter(|row| row.error.is_none())
        .collect();
    let all_mentions = ok
        .iter()
        .flat_map(|row| row.brands.iter())
        .filter(|brand| brand.position.is_some())
        .count();
    let mut brand_rows: BTreeMap<(bool, String), (u32, u32, Vec<u32>)> = BTreeMap::new();
    for brand in brands_of(&project, &settings) {
        brand_rows.insert((!brand.own, brand.key), (0, 0, Vec::new()));
    }
    for row in &ok {
        for brand in &row.brands {
            let entry = brand_rows
                .entry((!brand.own, brand.key.clone()))
                .or_default();
            if let Some(position) = brand.position {
                entry.0 += 1;
                entry.2.push(position);
            }
            if brand.cited {
                entry.1 += 1;
            }
        }
    }
    let mut brands: Vec<BrandShare> = brand_rows
        .into_iter()
        .map(
            |((not_own, key), (mentions, citations, positions))| BrandShare {
                own: !not_own,
                share: if all_mentions > 0 {
                    (f64::from(mentions) / all_mentions as f64 * 1000.0).round() / 10.0
                } else {
                    0.0
                },
                average_position: (!positions.is_empty()).then(|| {
                    (positions.iter().map(|p| f64::from(*p)).sum::<f64>() / positions.len() as f64
                        * 10.0)
                        .round()
                        / 10.0
                }),
                key,
                mentions,
                citations,
            },
        )
        .collect();
    brands.sort_by(|left, right| {
        right
            .own
            .cmp(&left.own)
            .then(right.mentions.cmp(&left.mentions))
    });

    // Run by run.
    let mut by_run: BTreeMap<i64, Vec<&AiAnswerRow>> = BTreeMap::new();
    for answer in &answers {
        by_run.entry(answer.run_id).or_default().push(answer);
    }
    let history: Vec<RunPoint> = by_run
        .into_iter()
        .map(|(run_id, rows)| {
            let overall = visibility(&rows);
            let mut engines = BTreeMap::new();
            for engine in octopuson_platform::AI_ENGINES {
                let own_rows: Vec<&AiAnswerRow> = rows
                    .iter()
                    .copied()
                    .filter(|row| row.engine == *engine)
                    .collect();
                if !own_rows.is_empty() {
                    engines.insert((*engine).to_owned(), visibility(&own_rows).mention_rate);
                }
            }
            RunPoint {
                run_id,
                date: rows
                    .first()
                    .map(|row| row.checked_at.get(..10).unwrap_or_default().to_owned())
                    .unwrap_or_default(),
                mention_rate: overall.mention_rate,
                citation_rate: overall.citation_rate,
                share_of_voice: overall.share_of_voice,
                engines,
            }
        })
        .collect();

    let prompt_rows: Vec<PromptRow> = prompts
        .iter()
        .map(|prompt| PromptRow {
            prompt_id: prompt.id,
            prompt: prompt.prompt.clone(),
            tags: prompt.tags.clone(),
            results: current
                .iter()
                .filter(|row| row.prompt_id == prompt.id)
                .map(|row| {
                    (
                        row.engine.clone(),
                        Cell {
                            mentioned: row.mentioned,
                            position: row.position,
                            cited: row.cited,
                            sentiment: row.sentiment,
                            error: row.error.clone(),
                            checked_at: row.checked_at.clone(),
                        },
                    )
                })
                .collect(),
        })
        .collect();

    let mut domains: HashMap<String, u32> = HashMap::new();
    let mut pages: HashMap<String, (Option<String>, u32)> = HashMap::new();
    for row in &ok {
        let mut seen = Vec::new();
        for citation in &row.citations {
            if !seen.contains(&citation.domain) {
                *domains.entry(citation.domain.clone()).or_default() += 1;
                seen.push(citation.domain.clone());
            }
            if belongs(&citation.domain, &own) {
                let entry = pages
                    .entry(citation.url.clone())
                    .or_insert((citation.title.clone(), 0));
                entry.1 += 1;
            }
        }
    }
    let mut top_domains: Vec<SourceCount> = domains
        .into_iter()
        .map(|(key, answers)| SourceCount {
            own: belongs(&key, &own),
            title: None,
            key,
            answers,
        })
        .collect();
    top_domains.sort_by(|left, right| {
        right
            .answers
            .cmp(&left.answers)
            .then(left.key.cmp(&right.key))
    });
    top_domains.truncate(TOP_SOURCES);
    let mut own_pages: Vec<SourceCount> = pages
        .into_iter()
        .map(|(key, (title, answers))| SourceCount {
            key,
            title,
            own: true,
            answers,
        })
        .collect();
    own_pages.sort_by(|left, right| {
        right
            .answers
            .cmp(&left.answers)
            .then(left.key.cmp(&right.key))
    });
    own_pages.truncate(TOP_SOURCES);

    Ok(AiReport {
        summary: visibility(&current),
        engines,
        brands,
        history,
        prompts: prompt_rows,
        top_domains,
        own_pages,
        last_run: runs.into_iter().next(),
        since,
    })
}

#[derive(Debug, Deserialize)]
struct AnswerAsk {
    project_id: i64,
    prompt_id: i64,
    engine: String,
}

#[derive(Debug, Serialize)]
struct AnswerDetail {
    prompt: AiPrompt,
    engine: String,
    /// The brands watched, with their names, for highlighting.
    brands: Vec<Brand>,
    /// The latest answers, newest first.
    answers: Vec<AiAnswerRow>,
}

fn ai_answer(context: &Context<'_>, ask: AnswerAsk) -> Result<AnswerDetail, AppError> {
    let store = context.workspace.store()?;
    let project = store.project(ask.project_id)?;
    let settings = store.ai_settings(project.id)?;
    let prompt = store
        .ai_prompts(project.id)?
        .into_iter()
        .find(|prompt| prompt.id == ask.prompt_id)
        .ok_or_else(|| AppError::NotFound(format!("question {}", ask.prompt_id)))?;
    let mut answers = store.ai_answers(project.id, None, Some(prompt.id), Some(&ask.engine))?;
    answers.reverse();
    answers.truncate(ANSWERS_SHOWN);
    Ok(AnswerDetail {
        brands: brands_of(&project, &settings),
        prompt,
        engine: ask.engine,
        answers,
    })
}

// ----- LLM Mentions ------------------------------------------------------------------------

/// A report of the LLM Mentions database.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
enum MentionsReport {
    /// The site's mention metrics.
    #[default]
    Summary,
    /// The questions whose answers mention or cite the site.
    Queries,
    /// The site's metrics beside its rivals'.
    Compare,
}

#[derive(Debug, Deserialize)]
struct MentionsAsk {
    project_id: i64,
    #[serde(default)]
    report: MentionsReport,
    /// Rows of the questions (10–1000).
    #[serde(default)]
    limit: Option<u32>,
    #[serde(default)]
    force: bool,
}

/// Most rivals compared.
const COMPARED: usize = 4;

struct MentionsQuestion {
    report: MentionsReport,
    targets: Vec<String>,
    parts: Vec<Paid<'static>>,
}

fn mentions_question(
    workspace: &Workspace,
    ask: &MentionsAsk,
) -> Result<MentionsQuestion, AppError> {
    let store = workspace.store()?;
    let project = store.project(ask.project_id)?;
    let settings = store.ai_settings(project.id)?;
    drop(store);
    let market = Market::new(&project.country, &project.language);
    let paid = |endpoint: &'static str, task: Value, estimate_usd: f64| Paid {
        module: MODULE,
        provider: octopuson_dataforseo::ID,
        endpoint,
        class: CacheClass::Domain,
        project_id: Some(project.id),
        task,
        estimate_usd,
    };
    let (targets, parts) = match ask.report {
        MentionsReport::Summary => (
            vec![project.domain.clone()],
            vec![paid(
                dfs::MENTIONS_METRICS,
                dfs::mentions_metrics_task(&project.domain, &market)?,
                dfs::mentions_estimate(30),
            )],
        ),
        MentionsReport::Queries => {
            let rows = ask.limit.unwrap_or(100).clamp(10, 1000);
            (
                vec![project.domain.clone()],
                vec![paid(
                    dfs::MENTIONS_SEARCH,
                    dfs::mentions_search_task(&project.domain, &market, rows)?,
                    dfs::mentions_estimate(rows),
                )],
            )
        }
        MentionsReport::Compare => {
            let mut targets = vec![project.domain.clone()];
            targets.extend(
                brands_of(&project, &settings)
                    .into_iter()
                    .filter(|brand| !brand.own)
                    .map(|brand| brand.key)
                    .take(COMPARED),
            );
            if targets.len() < 2 {
                return Err(AppError::Invalid(
                    "add competitors or rival brands to compare".to_owned(),
                ));
            }
            let parts = targets
                .iter()
                .map(|target| {
                    Ok(paid(
                        dfs::MENTIONS_METRICS,
                        dfs::mentions_metrics_task(target, &market)?,
                        dfs::mentions_estimate(30),
                    ))
                })
                .collect::<Result<Vec<_>, AppError>>()?;
            (targets, parts)
        }
    };
    Ok(MentionsQuestion {
        report: ask.report,
        targets,
        parts,
    })
}

fn read_mentions(question: &MentionsQuestion, results: &[Vec<Value>]) -> Value {
    let value = match question.report {
        MentionsReport::Summary => serde_json::to_value(dfs::metrics_from_result(
            &question.targets[0],
            results.first().map_or(&[][..], Vec::as_slice),
        )),
        MentionsReport::Queries => serde_json::to_value(dfs::mentions_from_result(
            results.first().map_or(&[][..], Vec::as_slice),
        )),
        MentionsReport::Compare => serde_json::to_value(
            question
                .targets
                .iter()
                .zip(results)
                .map(|(target, result)| dfs::metrics_from_result(target, result))
                .collect::<Vec<_>>(),
        ),
    };
    value.unwrap_or(Value::Null)
}

fn regrouped(fetched: Fetched<Vec<Vec<Value>>>, question: &MentionsQuestion) -> Fetched<Value> {
    Fetched {
        data: read_mentions(question, &fetched.data),
        fetched_at: fetched.fetched_at,
        fresh: fetched.fresh,
        cached: fetched.cached,
        spent_usd: fetched.spent_usd,
        via: fetched.via,
    }
}

fn ai_mentions_estimate(context: &Context<'_>, ask: MentionsAsk) -> Result<Estimate, AppError> {
    let question = mentions_question(context.workspace, &ask)?;
    context.workspace.estimate_many(&question.parts)
}

fn get_ai_mentions(
    context: &Context<'_>,
    ask: MentionsAsk,
) -> Result<Option<Fetched<Value>>, AppError> {
    let question = mentions_question(context.workspace, &ask)?;
    Ok(context
        .workspace
        .kept_many(&question.parts)?
        .map(|fetched| regrouped(fetched, &question)))
}

fn fetch_ai_mentions(context: &Context<'_>, ask: MentionsAsk) -> Result<Fetched<Value>, AppError> {
    let question = mentions_question(context.workspace, &ask)?;
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
    router.add("ai_setup", ai_setup);
    router.add("save_ai_settings", save_ai_settings);
    router.add("add_ai_prompts", add_ai_prompts);
    router.add("remove_ai_prompts", remove_ai_prompts);
    router.add("suggest_ai_prompts", suggest_ai_prompts);
    router.add("ai_estimate", ai_estimate);
    router.add("start_ai_run", start_ai_run);
    router.add("ai_runs", ai_runs);
    router.add("ai_report", ai_report);
    router.add("ai_answer", ai_answer);
    router.add("ai_mentions_estimate", ai_mentions_estimate);
    router.add("get_ai_mentions", get_ai_mentions);
    router.add("fetch_ai_mentions", fetch_ai_mentions);
}
