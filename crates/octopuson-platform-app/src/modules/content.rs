//! Content optimisation: briefs made from the pages ranking for a keyword, pages and drafts
//! scored against them, the project's pages worth checking, and writing by an assistant with
//! the user's own key.
//!
//! A brief is a job (`start_content_brief`): the keyword's result page (the one the Keyword
//! Explorer keeps, paid only when it is not fresh), then its first ten results read with the
//! program's own crawler, measured as the SERP Checker measures them. Pages and drafts are
//! scored against what was read (`score_content`), free. Writing a brief or improvements
//! (`start_content_writing`) and drafting a reply to a review (`draft_review_reply`) ask
//! Claude, ChatGPT or Gemini with the user's own key; credits do not write yet.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use octopuson_cloud::Via;
use octopuson_crawl::{AgentPreset, Fetcher};
use octopuson_dataforseo::{Client, serp};
use octopuson_llm::{Engine, Writing};
use octopuson_net::CrawlFetcher;
use octopuson_platform::{
    CacheClass, ContentBriefRow, ContentScoreRow, MAX_DRAFT_CHARS, PageCandidate, ScoreToSave,
};
use octopuson_serp_score::PageSignals;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::content::{
    Brief, BriefSource, ContentScore, brief_instructions, brief_material, build_brief,
    improvement_instructions, improvement_material, internal_links, read_page, reply_instructions,
    reply_material, score_page, signals_of_draft,
};
use crate::fetch::{Estimate, Paid, Reply};
use crate::jobs::{JobControl, JobDone, JobStatus};
use crate::router::{Context, Router};
use crate::sources::LLM_KEY;
use crate::{AppError, Host, Workspace, now};

/// Module id, in spending records.
const MODULE: &str = "content";

/// Kind of the job making a brief.
pub const BRIEF_JOB: &str = "content-brief";

/// Kind of the job of an assistant writing.
pub const WRITING_JOB: &str = "content-writing";

/// Results of the result page read for a brief.
const RESULTS: usize = 10;

/// Ranking pages read at once.
const PARALLEL: usize = 4;

/// How long a ranking page is waited for.
const PAGE_TIMEOUT: Duration = Duration::from_secs(20);

/// Pages of the site suggested as internal links.
const LINKS: usize = 8;

/// Pages of the latest audit's crawl looked through for internal links.
const LINK_PAGES: u32 = 5_000;

/// Pages listed in the page checker.
const CANDIDATES: u32 = 200;

/// Longest answer of an assistant writing a brief or improvements, tokens (thinking included).
const WRITING_TOKENS: u32 = 16_000;

/// Longest answer of an assistant drafting a reply, tokens (thinking included).
const REPLY_TOKENS: u32 = 4_000;

/// Assistants that write, in the order offered.
const WRITERS: [Engine; 3] = [Engine::Claude, Engine::ChatGpt, Engine::Gemini];

#[derive(Debug, Deserialize)]
struct ProjectAsk {
    project_id: i64,
}

#[derive(Debug, Deserialize)]
struct BriefId {
    brief_id: i64,
}

#[derive(Debug, Clone, Deserialize)]
struct BriefAsk {
    project_id: i64,
    keyword: String,
    /// Another market than the project's: `country-language`.
    #[serde(default)]
    market: Option<String>,
    #[serde(default)]
    mobile: bool,
    /// A page of the project to score against the brief once made (the page checker).
    #[serde(default)]
    page: Option<String>,
}

/// The result page a brief reads: the Keyword Explorer's question, so its kept answer is used.
fn serp_paid(
    workspace: &Workspace,
    ask: &BriefAsk,
) -> Result<(Paid<'static>, String, String), AppError> {
    let project = workspace.store()?.project(ask.project_id)?;
    let market = super::market_of(&project, ask.market.as_deref())?;
    let keyword = ask.keyword.split_whitespace().collect::<Vec<_>>().join(" ");
    if keyword.is_empty() {
        return Err(AppError::Invalid("keyword: enter a keyword".to_owned()));
    }
    let paid = Paid {
        travels: true,
        module: MODULE,
        provider: octopuson_dataforseo::ID,
        endpoint: serp::ORGANIC,
        class: CacheClass::Serp,
        project_id: Some(ask.project_id),
        task: serp::organic_task(&keyword, &market, 10, ask.mobile)?,
        estimate_usd: serp::LIVE_PAGE,
    };
    Ok((paid, market.country.clone(), market.language.clone()))
}

fn content_brief_estimate(context: &Context<'_>, ask: BriefAsk) -> Result<Estimate, AppError> {
    let (paid, ..) = serp_paid(context.workspace, &ask)?;
    context.workspace.estimate(&paid)
}

/// A brief started.
#[derive(Debug, Serialize)]
struct Started {
    job_id: String,
    brief_id: i64,
}

fn start_content_brief(context: &Context<'_>, ask: BriefAsk) -> Result<Started, AppError> {
    let workspace = context.workspace;
    if context
        .jobs
        .running_for(BRIEF_JOB, Some(ask.project_id))
        .is_some()
    {
        return Err(AppError::Busy(
            "a brief of this project is being made; wait for it to finish".to_owned(),
        ));
    }
    let (paid, country, language) = serp_paid(workspace, &ask)?;
    let estimate = workspace.estimate(&paid)?;
    if let Some(blocked) = estimate.blocked
        && !estimate.kept.as_ref().is_some_and(|kept| kept.fresh)
    {
        return Err(match blocked.code {
            crate::router::ErrorCode::Budget => AppError::Budget(blocked.message),
            _ => AppError::NoSource(blocked.message),
        });
    }
    if let Some(page) = &ask.page {
        url::Url::parse(page)
            .map_err(|_| AppError::Invalid(format!("page: `{page}` is not an address")))?;
    }
    let store = workspace.store()?;
    let project = store.project(ask.project_id)?;
    let brief_id = store.begin_content_brief(
        ask.project_id,
        &ask.keyword.split_whitespace().collect::<Vec<_>>().join(" "),
        (&country, &language),
        ask.mobile,
        &now(),
    )?;
    drop(store);
    let shared = Arc::clone(workspace);
    let label = ask.keyword.clone();
    let job_id = context
        .jobs
        .start(BRIEF_JOB, &label, Some(project.id), move |control| {
            run_brief(&shared, brief_id, &ask, paid, control)
        })?;
    Ok(Started { job_id, brief_id })
}

/// The fetcher ranking pages are read with: the program's own crawler, which on the web reads
/// only public addresses.
fn page_fetcher(workspace: &Workspace, language: &str) -> Arc<dyn Fetcher> {
    if let Some(fetcher) = workspace.test_fetcher() {
        return Arc::clone(fetcher);
    }
    let agent = AgentPreset::Octopus.user_agent();
    let accept = Some(format!("{language},en;q=0.5"));
    match workspace.host() {
        Host::Web => Arc::new(CrawlFetcher::public_only(agent, PAGE_TIMEOUT, accept)),
        Host::Desktop => Arc::new(CrawlFetcher::new(agent, PAGE_TIMEOUT, accept)),
    }
}

/// Reads `urls` (with their positions) a few at once, in order.
fn read_pages(
    fetcher: &dyn Fetcher,
    urls: &[(u16, String)],
    control: &JobControl,
) -> Vec<PageSignals> {
    let next = AtomicUsize::new(0);
    let done = AtomicUsize::new(0);
    let slots: Vec<std::sync::Mutex<Option<PageSignals>>> =
        urls.iter().map(|_| std::sync::Mutex::new(None)).collect();
    std::thread::scope(|scope| {
        for _ in 0..PARALLEL.min(urls.len()) {
            scope.spawn(|| {
                loop {
                    let index = next.fetch_add(1, Ordering::SeqCst);
                    if index >= urls.len() || control.cancelled() {
                        break;
                    }
                    let (position, url) = &urls[index];
                    let page = read_page(fetcher, url, *position);
                    if let Ok(mut slot) = slots[index].lock() {
                        *slot = Some(page);
                    }
                    let finished = done.fetch_add(1, Ordering::SeqCst) + 1;
                    control.progress(url, finished as u64, urls.len() as u64);
                }
            });
        }
    });
    slots
        .into_iter()
        .filter_map(|slot| slot.into_inner().ok().flatten())
        .collect()
}

/// The site's pages from the latest audit's crawl: address, title and H1.
fn site_pages(workspace: &Workspace, project_id: i64) -> Vec<(String, String, String)> {
    use octopuson_core::table::{Cell, TableBuffer};
    let Some(audit) = workspace.store().ok().and_then(|store| {
        store.audits(project_id, 10).ok()?.into_iter().find(|row| {
            row.crawl_id.is_some() && !matches!(row.status.as_str(), "running" | "failed")
        })
    }) else {
        return Vec::new();
    };
    let Some(reader) = super::audit::open(workspace, &audit) else {
        return Vec::new();
    };
    let mut buffer = TableBuffer::default();
    let sql = format!(
        "SELECT url, coalesce(title, ''), coalesce(h1, '') FROM urls
         WHERE internal AND status = 200 AND indexability = 'indexable' LIMIT {LINK_PAGES}"
    );
    if reader.query(&sql, &mut buffer).is_err() {
        return Vec::new();
    }
    let text = |cell: Option<&Cell>| match cell {
        Some(Cell::Text(text)) => text.clone(),
        _ => String::new(),
    };
    buffer
        .rows
        .iter()
        .map(|row| (text(row.first()), text(row.get(1)), text(row.get(2))))
        .collect()
}

/// What a brief job did, as its result.
#[derive(Debug, Serialize)]
struct BriefDone {
    brief_id: i64,
    read: usize,
    /// The page scored with it (the page checker).
    score: Option<f64>,
    cost_usd: f64,
}

fn run_brief(
    workspace: &Workspace,
    brief_id: i64,
    ask: &BriefAsk,
    paid: Paid<'static>,
    control: &JobControl,
) -> Result<JobDone, AppError> {
    let mut spent = 0.0;
    let outcome = make_brief(workspace, brief_id, ask, &paid, control, &mut spent);
    match outcome {
        Ok(done) => Ok(JobDone {
            status: JobStatus::Done,
            result: serde_json::to_value(&done).ok(),
        }),
        Err(error) => {
            let message = if control.cancelled() {
                "stopped".to_owned()
            } else {
                error.to_string()
            };
            workspace
                .store()?
                .fail_content_brief(brief_id, &message, spent, &now())?;
            Err(error)
        }
    }
}

fn make_brief(
    workspace: &Workspace,
    brief_id: i64,
    ask: &BriefAsk,
    paid: &Paid<'_>,
    control: &JobControl,
    spent: &mut f64,
) -> Result<BriefDone, AppError> {
    let row = workspace.store()?.content_brief(brief_id)?;
    control.progress("serp", 0, 1);
    let fetched = workspace.fetch(
        paid,
        false,
        |route| {
            let credentials = route.key(octopuson_dataforseo::SECRET)?;
            let client = Client::new(route.transport, &credentials)?;
            Ok(client.post(paid.endpoint, &paid.task)?.into())
        },
        <[Value]>::to_vec,
    )?;
    *spent += fetched.spent_usd;
    control.spent(fetched.spent_usd);
    let market = octopuson_dataforseo::Market::new(&row.country, &row.language);
    let snapshot = serp::snapshot_of(
        &fetched.data,
        &row.keyword,
        &market,
        row.mobile,
        &fetched.fetched_at,
    );
    let mut organic: Vec<(u16, String)> = snapshot
        .organic
        .iter()
        .map(|result| (result.position, result.url.clone()))
        .collect();
    organic.sort_by_key(|(position, _)| *position);
    organic.truncate(RESULTS);
    if organic.is_empty() {
        return Err(AppError::Invalid(
            "the result page has no organic results to learn from".to_owned(),
        ));
    }
    let fetcher = page_fetcher(workspace, &row.language);
    let pages = read_pages(fetcher.as_ref(), &organic, control);
    if control.cancelled() {
        return Err(AppError::Invalid("stopped".to_owned()));
    }
    let read = pages.iter().filter(|page| page.is_live()).count();
    if read == 0 {
        return Err(AppError::Unavailable(
            "none of the ranking pages could be read".to_owned(),
        ));
    }
    let (mut brief, source) = build_brief(snapshot, pages);
    let site = site_pages(workspace, ask.project_id);
    brief.internal_links = internal_links(&brief, &source, &site, ask.page.as_deref(), LINKS);
    let mut score = None;
    let page = ask
        .page
        .as_deref()
        .map(|url| read_page(fetcher.as_ref(), url, 0));
    workspace.store()?.finish_content_brief(
        brief_id,
        &serde_json::to_value(&brief).unwrap_or(Value::Null),
        &serde_json::to_value(&source).unwrap_or(Value::Null),
        *spent,
        &now(),
    )?;
    if let Some(page) = page
        && page.status.is_some()
    {
        let scored = score_page(&source, &page);
        score = Some(scored.breakdown.score);
        save_score(workspace, &row, &page, &scored, "url")?;
    }
    Ok(BriefDone {
        brief_id,
        read,
        score,
        cost_usd: *spent,
    })
}

fn save_score(
    workspace: &Workspace,
    brief: &ContentBriefRow,
    page: &PageSignals,
    scored: &ContentScore,
    kind: &str,
) -> Result<i64, AppError> {
    let result = serde_json::to_value(scored).unwrap_or(Value::Null);
    let draft = kind == "draft";
    Ok(workspace.store()?.save_content_score(
        brief.id,
        brief.project_id,
        ScoreToSave {
            kind,
            url: (!draft).then_some(page.url.as_str()),
            title: if draft { page.title.as_deref() } else { None },
            description: if draft {
                page.meta_description.as_deref()
            } else {
                None
            },
            body: if draft {
                Some(page.text.main.as_str())
            } else {
                None
            },
            score: scored.breakdown.score,
            result: &result,
        },
        &now(),
    )?)
}

/// A brief in the list.
#[derive(Debug, Serialize)]
struct BriefSummary {
    id: i64,
    keyword: String,
    country: String,
    language: String,
    mobile: bool,
    status: String,
    error: Option<String>,
    created_at: String,
    updated_at: String,
    /// Median words of the ranking pages.
    words: Option<f64>,
    /// Ranking pages read.
    pages: usize,
    /// An assistant wrote from it.
    written: bool,
    cost_usd: f64,
}

fn content_briefs(context: &Context<'_>, ask: ProjectAsk) -> Result<Vec<BriefSummary>, AppError> {
    let store = context.workspace.store()?;
    store.project(ask.project_id)?;
    Ok(store
        .content_briefs(ask.project_id)?
        .into_iter()
        .map(|row| {
            let brief: Option<Brief> = row
                .brief
                .clone()
                .and_then(|value| serde_json::from_value(value).ok());
            BriefSummary {
                words: brief
                    .as_ref()
                    .and_then(|brief| brief.words.map(|words| words.median)),
                pages: brief.as_ref().map_or(0, |brief| {
                    brief
                        .competitors
                        .iter()
                        .filter(|page| page.score.is_some())
                        .count()
                }),
                written: row.ai_text.is_some(),
                id: row.id,
                keyword: row.keyword,
                country: row.country,
                language: row.language,
                mobile: row.mobile,
                status: row.status,
                error: row.error,
                created_at: row.created_at,
                updated_at: row.updated_at,
                cost_usd: row.cost_usd,
            }
        })
        .collect())
}

/// A brief with the pages and drafts scored against it.
#[derive(Debug, Serialize)]
struct BriefView {
    #[serde(flatten)]
    row: ContentBriefRow,
    scores: Vec<ContentScoreRow>,
}

fn content_brief(context: &Context<'_>, ask: BriefId) -> Result<BriefView, AppError> {
    let store = context.workspace.store()?;
    Ok(BriefView {
        row: store.content_brief(ask.brief_id)?,
        scores: store.content_scores(ask.brief_id)?,
    })
}

fn delete_content_brief(context: &Context<'_>, ask: BriefId) -> Result<(), AppError> {
    let store = context.workspace.store()?;
    if store.content_brief(ask.brief_id)?.status == "running" {
        return Err(AppError::Busy(
            "the brief is being made; wait for it to finish".to_owned(),
        ));
    }
    Ok(store.delete_content_brief(ask.brief_id)?)
}

/// A brief made, with what it was made from.
fn made(
    workspace: &Workspace,
    brief_id: i64,
) -> Result<(ContentBriefRow, Brief, BriefSource), AppError> {
    let store = workspace.store()?;
    let row = store.content_brief(brief_id)?;
    let not_made = || AppError::Invalid("the brief is not made yet".to_owned());
    let brief: Brief = row
        .brief
        .clone()
        .and_then(|value| serde_json::from_value(value).ok())
        .ok_or_else(not_made)?;
    let source: BriefSource = store
        .content_brief_source(brief_id)?
        .and_then(|value| serde_json::from_value(value).ok())
        .ok_or_else(not_made)?;
    Ok((row, brief, source))
}

#[derive(Debug, Deserialize)]
struct ScoreAsk {
    brief_id: i64,
    /// A published page to read and score.
    #[serde(default)]
    url: Option<String>,
    /// A draft: its title, meta description and text (Markdown).
    #[serde(default)]
    title: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    body: String,
    /// Keep the score (a draft is kept as the brief's draft).
    #[serde(default)]
    save: bool,
}

/// A page or draft scored.
#[derive(Debug, Serialize)]
struct Scored {
    /// The kept score, when kept.
    id: Option<i64>,
    #[serde(flatten)]
    score: ContentScore,
}

fn score_content(context: &Context<'_>, ask: ScoreAsk) -> Result<Scored, AppError> {
    let workspace = context.workspace;
    let (row, _, source) = made(workspace, ask.brief_id)?;
    let address = ask
        .url
        .as_deref()
        .map(str::trim)
        .filter(|url| !url.is_empty());
    let (page, kind) = if let Some(url) = address {
        let parsed = url::Url::parse(url)
            .ok()
            .filter(|parsed| matches!(parsed.scheme(), "http" | "https"))
            .ok_or_else(|| AppError::Invalid(format!("url: `{url}` is not an address")))?;
        let fetcher = page_fetcher(workspace, &row.language);
        let page = read_page(fetcher.as_ref(), parsed.as_str(), 0);
        if page.status.is_none() {
            return Err(AppError::Unavailable(format!("{url} did not answer")));
        }
        (page, "url")
    } else {
        if ask.body.trim().is_empty() {
            return Err(AppError::Invalid(
                "write or paste the draft, or give a page's address".to_owned(),
            ));
        }
        if ask.body.chars().count() > MAX_DRAFT_CHARS {
            return Err(AppError::Invalid(format!(
                "a draft has at most {MAX_DRAFT_CHARS} characters"
            )));
        }
        (
            signals_of_draft(&ask.title, &ask.description, &ask.body),
            "draft",
        )
    };
    let score = score_page(&source, &page);
    let id = if ask.save || kind == "url" {
        let body_page = if kind == "draft" {
            PageSignals {
                text: octopuson_text::PageText {
                    main: ask.body.clone(),
                    ..page.text.clone()
                },
                ..page.clone()
            }
        } else {
            page
        };
        Some(save_score(workspace, &row, &body_page, &score, kind)?)
    } else {
        None
    };
    Ok(Scored { id, score })
}

/// A page of the project in the page checker.
#[derive(Debug, Serialize)]
struct CheckerRow {
    #[serde(flatten)]
    page: PageCandidate,
    /// Its latest score against a brief.
    score: Option<f64>,
    /// The keyword of that brief.
    scored_for: Option<String>,
    scored_at: Option<String>,
}

fn content_pages(context: &Context<'_>, ask: ProjectAsk) -> Result<Vec<CheckerRow>, AppError> {
    let store = context.workspace.store()?;
    store.project(ask.project_id)?;
    let scores = store.content_page_scores(ask.project_id)?;
    Ok(store
        .content_candidates(ask.project_id, CANDIDATES)?
        .into_iter()
        .map(|page| {
            let scored = scores.iter().find(|(url, ..)| *url == page.url);
            CheckerRow {
                score: scored.map(|(_, _, score, _)| *score),
                scored_for: scored.map(|(_, keyword, ..)| keyword.clone()),
                scored_at: scored.map(|(.., at)| at.clone()),
                page,
            }
        })
        .collect())
}

// ----- Writing ------------------------------------------------------------------------------

/// What an assistant writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
enum Task {
    /// A brief for a writer.
    Brief,
    /// Improvements of a scored page or draft.
    Improve,
}

#[derive(Debug, Clone, Deserialize)]
struct WriteAsk {
    brief_id: i64,
    task: Task,
    /// The scored page or draft to improve.
    #[serde(default)]
    score_id: Option<i64>,
    /// `claude`, `chatgpt` or `gemini`; the first ready when absent.
    #[serde(default)]
    engine: Option<String>,
}

/// An assistant that can write, as the page offers it.
#[derive(Debug, Serialize)]
struct Writer {
    engine: &'static str,
    model: String,
    /// Its key is entered.
    ready: bool,
    /// Rough list price of this writing, US dollars.
    usd: f64,
}

/// Who can write and what it would cost.
#[derive(Debug, Serialize)]
struct WritingEstimate {
    writers: Vec<Writer>,
    /// Characters of material the assistant gets.
    material_chars: usize,
}

/// The model the project's AI settings chose for an assistant, or its default.
fn model_for(workspace: &Workspace, project_id: i64, engine: Engine) -> String {
    workspace
        .store()
        .ok()
        .and_then(|store| store.ai_settings(project_id).ok())
        .and_then(|settings| settings.models.get(engine.as_str()).cloned())
        .filter(|model| !model.trim().is_empty())
        .unwrap_or_else(|| engine.default_model().to_owned())
}

fn writers(workspace: &Workspace, project_id: i64, material_chars: usize) -> Vec<Writer> {
    WRITERS
        .iter()
        .map(|&engine| {
            let model = model_for(workspace, project_id, engine);
            Writer {
                engine: engine.as_str(),
                usd: engine.writing_estimate_usd(&model, material_chars),
                ready: workspace.own_ready(engine.provider()),
                model,
            }
        })
        .collect()
}

/// The material and instructions of a writing task.
fn writing_of(
    workspace: &Workspace,
    ask: &WriteAsk,
) -> Result<(ContentBriefRow, String, String), AppError> {
    let (row, brief, source) = made(workspace, ask.brief_id)?;
    let (material, instructions) = match ask.task {
        Task::Brief => (
            brief_material(&brief, &source),
            brief_instructions(&row.language),
        ),
        Task::Improve => {
            let id = ask.score_id.ok_or_else(|| {
                AppError::Invalid("score_id: choose the page or draft".to_owned())
            })?;
            let scored = workspace.store()?.content_score(id)?;
            if scored.brief_id != row.id {
                return Err(AppError::Invalid(
                    "the page was scored against another brief".to_owned(),
                ));
            }
            let score: ContentScore = serde_json::from_value(scored.result.clone())
                .map_err(|error| AppError::Internal(format!("stored score: {error}")))?;
            let page = match scored.url.as_deref() {
                Some(url) => {
                    let fetcher = page_fetcher(workspace, &row.language);
                    read_page(fetcher.as_ref(), url, 0)
                }
                None => signals_of_draft(
                    scored.title.as_deref().unwrap_or_default(),
                    scored.description.as_deref().unwrap_or_default(),
                    scored.body.as_deref().unwrap_or_default(),
                ),
            };
            (
                improvement_material(&brief, &score, &page),
                improvement_instructions(&row.language),
            )
        }
    };
    Ok((row, material, instructions))
}

fn content_writing_estimate(
    context: &Context<'_>,
    ask: WriteAsk,
) -> Result<WritingEstimate, AppError> {
    let workspace = context.workspace;
    let (row, brief, source) = made(workspace, ask.brief_id)?;
    // The page itself is read only when writing; its share of the material is guessed here.
    let material_chars = match ask.task {
        Task::Brief => brief_material(&brief, &source).chars().count(),
        Task::Improve => 24_000,
    };
    Ok(WritingEstimate {
        writers: writers(workspace, row.project_id, material_chars),
        material_chars,
    })
}

/// The assistant asked: the one asked for, or the first whose key is entered.
fn engine_for(workspace: &Workspace, asked: Option<&str>) -> Result<Engine, AppError> {
    if let Some(name) = asked.filter(|name| !name.is_empty()) {
        let engine = Engine::parse(name)
            .filter(|engine| engine.writes())
            .ok_or_else(|| {
                AppError::Invalid(format!(
                    "`{name}` does not write; choose Claude, ChatGPT or Gemini"
                ))
            })?;
        if !workspace.own_ready(engine.provider()) {
            return Err(AppError::NoSource(not_ready()));
        }
        return Ok(engine);
    }
    WRITERS
        .into_iter()
        .find(|engine| workspace.own_ready(engine.provider()))
        .ok_or_else(|| AppError::NoSource(not_ready()))
}

fn not_ready() -> String {
    "enter your own Claude, ChatGPT or Gemini API key to write with an assistant; writing on credits is not available yet".to_owned()
}

/// Asks `engine` to write: paid within the month's budget and recorded. The text, the model
/// that wrote it and what it cost.
fn write_with(
    workspace: &Workspace,
    project_id: i64,
    engine: Engine,
    instructions: &str,
    material: &str,
    max_tokens: u32,
) -> Result<(String, String, f64), AppError> {
    let model = model_for(workspace, project_id, engine);
    let session = workspace.paid_run(engine.provider())?;
    if session.via() != Via::Own {
        return Err(AppError::NoSource(not_ready()));
    }
    let paid = Paid {
        travels: false,
        module: MODULE,
        provider: engine.provider(),
        endpoint: engine.as_str(),
        class: CacheClass::Ai,
        project_id: Some(project_id),
        task: json!({ "model": model }),
        estimate_usd: engine.writing_estimate_usd(&model, material.chars().count()),
    };
    let answered = session.ask(&paid, |route| {
        let key = route.key(LLM_KEY)?;
        let answer = octopuson_llm::write(
            engine,
            route.transport,
            &key,
            &Writing {
                instructions,
                material,
                model: &model,
                max_tokens,
            },
        )
        .map_err(super::ai::llm_error)?;
        let cost = answer.cost_usd;
        Ok(Reply {
            result: vec![serde_json::to_value(answer).unwrap_or(Value::Null)],
            cost_usd: cost,
            charged_usd: None,
        })
    })?;
    let answer: octopuson_llm::Answer = answered
        .result
        .first()
        .cloned()
        .and_then(|value| serde_json::from_value(value).ok())
        .unwrap_or_default();
    if answer.text.trim().is_empty() {
        return Err(AppError::Provider(format!(
            "{} wrote nothing",
            engine.as_str()
        )));
    }
    Ok((
        answer.text,
        format!("{}:{}", engine.as_str(), answer.model),
        answered.spent_usd,
    ))
}

fn start_content_writing(context: &Context<'_>, ask: WriteAsk) -> Result<String, AppError> {
    let workspace = context.workspace;
    let (row, ..) = made(workspace, ask.brief_id)?;
    let engine = engine_for(workspace, ask.engine.as_deref())?;
    if ask.task == Task::Improve && ask.score_id.is_none() {
        return Err(AppError::Invalid(
            "score_id: choose the page or draft".to_owned(),
        ));
    }
    let shared = Arc::clone(workspace);
    context.jobs.start(
        WRITING_JOB,
        &row.keyword,
        Some(row.project_id),
        move |control| {
            control.progress(engine.as_str(), 0, 1);
            let (row, material, instructions) = writing_of(&shared, &ask)?;
            let (text, model, spent) = write_with(
                &shared,
                row.project_id,
                engine,
                &instructions,
                &material,
                WRITING_TOKENS,
            )?;
            control.spent(spent);
            let store = shared.store()?;
            match ask.task {
                Task::Brief => store.note_brief_writing(row.id, &text, &model, spent, &now())?,
                Task::Improve => {
                    let id = ask.score_id.unwrap_or_default();
                    store.note_score_suggestions(id, &text)?;
                    store.add_content_cost(row.id, spent, &now())?;
                }
            }
            control.progress(engine.as_str(), 1, 1);
            Ok(JobDone {
                status: JobStatus::Done,
                result: Some(json!({ "brief_id": row.id, "task": ask.task, "cost_usd": spent })),
            })
        },
    )
}

#[derive(Debug, Deserialize)]
struct ReplyAsk {
    project_id: i64,
    review_id: String,
    #[serde(default)]
    engine: Option<String>,
}

/// A reply an assistant drafted.
#[derive(Debug, Serialize)]
struct Drafted {
    text: String,
    model: String,
    cost_usd: f64,
}

fn reply_estimate(context: &Context<'_>, ask: ProjectAsk) -> Result<Vec<Writer>, AppError> {
    context.workspace.store()?.project(ask.project_id)?;
    Ok(writers(context.workspace, ask.project_id, 1_500))
}

fn draft_review_reply(context: &Context<'_>, ask: ReplyAsk) -> Result<Drafted, AppError> {
    let workspace = context.workspace;
    let (project, review, business) = {
        let store = workspace.store()?;
        let project = store.project(ask.project_id)?;
        let review = store
            .local_reviews(ask.project_id, u32::MAX)?
            .into_iter()
            .find(|review| review.review_id == ask.review_id)
            .ok_or_else(|| AppError::NotFound(format!("review {}", ask.review_id)))?;
        let business = store.local_settings(ask.project_id)?.business;
        (project, review, business)
    };
    let engine = engine_for(workspace, ask.engine.as_deref())?;
    let name = business
        .as_ref()
        .and_then(|business| business["title"].as_str())
        .unwrap_or(&project.name)
        .to_owned();
    let category = business
        .as_ref()
        .and_then(|business| business["category"].as_str())
        .map(str::to_owned);
    let material = reply_material(
        &name,
        category.as_deref(),
        review.rating,
        review.author.as_deref(),
        review.text.as_deref(),
    );
    let (text, model, cost_usd) = write_with(
        workspace,
        project.id,
        engine,
        &reply_instructions(&project.language),
        &material,
        REPLY_TOKENS,
    )?;
    Ok(Drafted {
        text: text.trim().to_owned(),
        model,
        cost_usd,
    })
}

/// Adds the module's commands.
pub fn register(router: &mut Router) {
    router.add("content_brief_estimate", content_brief_estimate);
    router.add("start_content_brief", start_content_brief);
    router.add("content_briefs", content_briefs);
    router.add("content_brief", content_brief);
    router.add("delete_content_brief", delete_content_brief);
    router.add("score_content", score_content);
    router.add("content_pages", content_pages);
    router.add("content_writing_estimate", content_writing_estimate);
    router.add("start_content_writing", start_content_writing);
    router.add("reply_estimate", reply_estimate);
    router.add("draft_review_reply", draft_review_reply);
}
