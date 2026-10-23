//! Keyword Explorer: the numbers of keywords, ideas, related keywords, suggestions (and the
//! questions among them), the live result page, Yandex Wordstat, free autocomplete, keyword
//! lists and their grouping into topics.
//!
//! Paid reports go through `keywords_estimate`, `get_keywords` and `fetch_keywords` with a
//! `report` argument, like the domain overview.

use octopuson_dataforseo::keywords::{self as labs, KeywordData, KeywordList};
use octopuson_dataforseo::query::{FieldSpec, KEYWORD_FIELDS, ListQuery, RELATED_KEYWORD_FIELDS};
use octopuson_dataforseo::{Client, Market, prices, serp};
use octopuson_keyword_sources::suggest::FreeSuggest;
use octopuson_keyword_sources::wordstat::Wordstat;
use octopuson_keyword_sources::{FoundKeyword, SourceContext, SourceError};
use octopuson_keywords::{Input, LexicalOptions, cluster_lexical};
use octopuson_platform::{CacheClass, KeywordListRow, ListKeyword, ProjectRow};
use octopuson_text::Language;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::fetch::{Estimate, Fetched, Paid, Reply};
use crate::router::{Context, Router};
use crate::{AppError, Workspace, now};

/// Module id, in spending records.
const MODULE: &str = "keywords";

/// Rows an idea report returns unless asked otherwise.
const DEFAULT_ROWS: u32 = 100;

/// Phrases Wordstat returns.
const WORDSTAT_PHRASES: u32 = 500;

/// Endpoint name of Wordstat's top requests, for kept answers and spending.
const WORDSTAT_TOP: &str = "wordstat/topRequests";

impl From<SourceError> for AppError {
    fn from(error: SourceError) -> Self {
        let text = error.to_string();
        match error {
            SourceError::Config(message) => Self::NoSource(message),
            SourceError::Auth(_) => Self::Auth(text),
            SourceError::Quota { .. } => Self::Quota(text),
            SourceError::RateLimited { retry_after } => Self::RateLimited { retry_after },
            SourceError::Network(_) => Self::Unavailable(text),
            SourceError::Rejected(_) | SourceError::Parse(_) => Self::Provider(text),
        }
    }
}

/// A Keyword Explorer report.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Report {
    /// The numbers of given keywords (up to 700).
    #[default]
    Overview,
    /// Ideas from seed keywords.
    Ideas,
    /// Keywords related to a seed.
    Related,
    /// Keywords containing a seed.
    Suggestions,
    /// The live result page of a keyword.
    Serp,
    /// Yandex Wordstat's most searched phrases with a seed.
    Wordstat,
}

#[derive(Debug, Deserialize)]
struct Ask {
    project_id: i64,
    #[serde(default)]
    report: Report,
    /// Keywords to measure, or seeds for ideas.
    #[serde(default)]
    keywords: Vec<String>,
    /// The seed of related keywords, suggestions, the result page and Wordstat.
    #[serde(default)]
    seed: Option<String>,
    /// Rows of an idea report (10–1000).
    #[serde(default)]
    limit: Option<u32>,
    /// Levels of related keywords (1–4).
    #[serde(default)]
    depth: Option<u32>,
    /// The mobile result page.
    #[serde(default)]
    mobile: bool,
    /// Suggestions holding the seed as the whole phrase, in its order (exact match).
    #[serde(default)]
    exact: bool,
    /// Another market than the project's: `country-language`.
    #[serde(default)]
    market: Option<String>,
    /// Filters, order and the first row of an idea report, asked of DataForSEO.
    #[serde(default)]
    query: Option<ListQuery>,
    /// Pay for fresh data even while the kept answer is fresh.
    #[serde(default)]
    force: bool,
}

impl Report {
    /// The fields an idea report filters and sorts by on DataForSEO's side.
    fn fields(self) -> Option<&'static [FieldSpec]> {
        match self {
            Self::Ideas | Self::Suggestions => Some(KEYWORD_FIELDS),
            Self::Related => Some(RELATED_KEYWORD_FIELDS),
            Self::Overview | Self::Serp | Self::Wordstat => None,
        }
    }
}

struct Question {
    report: Report,
    market: Market,
    seed: String,
    paid: Paid<'static>,
}

fn seed_of(ask: &Ask) -> Result<String, AppError> {
    ask.seed
        .as_deref()
        .or_else(|| ask.keywords.first().map(String::as_str))
        .map(|seed| seed.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|seed| !seed.is_empty())
        .ok_or_else(|| AppError::Invalid("seed: enter a keyword".to_owned()))
}

#[allow(clippy::too_many_lines)] // one arm per report
fn question(workspace: &Workspace, ask: &Ask) -> Result<Question, AppError> {
    let project: ProjectRow = workspace.store()?.project(ask.project_id)?;
    let market = super::market_of(&project, ask.market.as_deref())?;
    let rows = octopuson_dataforseo::site::rows(ask.limit.unwrap_or(DEFAULT_ROWS));
    let words = if ask.keywords.is_empty() {
        vec![seed_of(ask)?]
    } else {
        ask.keywords.clone()
    };
    let dataforseo = |endpoint, class, task, estimate_usd| Paid {
        travels: true,
        module: MODULE,
        provider: octopuson_dataforseo::ID,
        endpoint,
        class,
        project_id: Some(ask.project_id),
        task,
        estimate_usd,
    };
    let (seed, mut paid) = match ask.report {
        Report::Overview => {
            let task = labs::overview_task(&words, &market)?;
            let count = task["keywords"].as_array().map_or(0, Vec::len);
            let seed = words.first().cloned().unwrap_or_default();
            (
                seed,
                dataforseo(
                    labs::OVERVIEW,
                    CacheClass::Keywords,
                    task,
                    labs::overview_estimate(count),
                ),
            )
        }
        Report::Ideas => {
            let task = labs::ideas_task(&words, &market, rows)?;
            let seed = words.first().cloned().unwrap_or_default();
            (
                seed,
                dataforseo(labs::IDEAS, CacheClass::Keywords, task, prices::labs(rows)),
            )
        }
        Report::Related => {
            let seed = seed_of(ask)?;
            let task = labs::related_task(&seed, &market, ask.depth.unwrap_or(2), rows)?;
            (
                seed,
                dataforseo(
                    labs::RELATED,
                    CacheClass::Keywords,
                    task,
                    prices::labs(rows),
                ),
            )
        }
        Report::Suggestions => {
            let seed = seed_of(ask)?;
            let task = labs::suggestions_match_task(&seed, &market, rows, ask.exact)?;
            (
                seed,
                dataforseo(
                    labs::SUGGESTIONS,
                    CacheClass::Keywords,
                    task,
                    prices::labs(rows),
                ),
            )
        }
        Report::Serp => {
            let seed = seed_of(ask)?;
            let task = serp::organic_task(&seed, &market, 10, ask.mobile)?;
            (
                seed,
                dataforseo(serp::ORGANIC, CacheClass::Serp, task, serp::LIVE_PAGE),
            )
        }
        Report::Wordstat => {
            let seed = seed_of(ask)?;
            let task = json!({
                "phrase": seed,
                "country": market.country,
                "phrases": WORDSTAT_PHRASES,
            });
            let paid = Paid {
                travels: true,
                module: MODULE,
                provider: Wordstat::ID,
                endpoint: WORDSTAT_TOP,
                class: CacheClass::Keywords,
                project_id: Some(ask.project_id),
                task,
                estimate_usd: Wordstat::COST_PER_REQUEST_USD,
            };
            (seed, paid)
        }
    };
    paid.travels = super::queried(&mut paid.task, ask.query.as_ref(), ask.report.fields())?;
    Ok(Question {
        report: ask.report,
        market,
        seed,
        paid,
    })
}

/// A Wordstat keyword in the explorer's shape.
fn from_wordstat(found: &FoundKeyword) -> KeywordData {
    KeywordData {
        keyword: found.keyword.clone(),
        volume: found.volume,
        monthly: found
            .monthly
            .iter()
            .filter_map(|month| {
                let (year, number) = month.month.split_once('-')?;
                Some(labs::Monthly {
                    year: year.parse().ok()?,
                    month: number.parse().ok()?,
                    volume: month.volume,
                })
            })
            .collect(),
        ..KeywordData::default()
    }
}

/// The report read from a result list, as pages get it.
fn read(report: Report, result: &[Value]) -> Value {
    let value = match report {
        Report::Overview | Report::Ideas | Report::Related | Report::Suggestions => {
            serde_json::to_value(labs::list_from_result(result))
        }
        Report::Serp => serde_json::to_value(serp::organic_from_result(result)),
        // Wordstat answers are kept already in the explorer's shape.
        Report::Wordstat => Ok(result.first().cloned().unwrap_or(Value::Null)),
    };
    value.unwrap_or(Value::Null)
}

fn keywords_estimate(context: &Context<'_>, ask: Ask) -> Result<Estimate, AppError> {
    let question = question(context.workspace, &ask)?;
    context.workspace.estimate(&question.paid)
}

fn get_keywords(context: &Context<'_>, ask: Ask) -> Result<Option<Fetched<Value>>, AppError> {
    let question = question(context.workspace, &ask)?;
    let report = question.report;
    context
        .workspace
        .kept(&question.paid, |result| read(report, result))
}

fn fetch_keywords(context: &Context<'_>, ask: Ask) -> Result<Fetched<Value>, AppError> {
    let Question {
        report,
        market,
        seed,
        paid,
    } = question(context.workspace, &ask)?;
    context.workspace.fetch(
        &paid,
        ask.force,
        |route| {
            if report == Report::Wordstat {
                let secrets = |_: &str, name: &str| route.key(name).ok();
                let source = SourceContext {
                    transport: route.transport,
                    secrets: &secrets,
                };
                let market =
                    octopuson_keyword_sources::Market::new(&market.country, &market.language);
                let found = Wordstat::top_requests(&source, &seed, &market, WORDSTAT_PHRASES)?;
                let charged_usd = route.transport.take_charge_usd();
                let keywords: Vec<KeywordData> = found.keywords.iter().map(from_wordstat).collect();
                let list = KeywordList {
                    total: keywords.len() as u64,
                    seed: None,
                    keywords,
                };
                return Ok(Reply {
                    result: vec![serde_json::to_value(list).unwrap_or(Value::Null)],
                    cost_usd: found.cost_usd,
                    charged_usd,
                });
            }
            let credentials = route.key(octopuson_dataforseo::SECRET)?;
            let client = Client::new(route.transport, &credentials)?;
            Ok(client.post(paid.endpoint, &paid.task)?.into())
        },
        |result| read(report, result),
    )
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
enum Engine {
    Google,
    Yandex,
}

#[derive(Debug, Deserialize)]
struct Autocomplete {
    project_id: i64,
    seed: String,
    engine: Engine,
    /// Another market than the project's: `country-language`.
    #[serde(default)]
    market: Option<String>,
}

/// Free autocomplete suggestions of Google or Yandex in the project's (or the asked) market.
fn keyword_autocomplete(context: &Context<'_>, ask: Autocomplete) -> Result<Vec<String>, AppError> {
    let project = context.workspace.store()?.project(ask.project_id)?;
    let seed = ask.seed.split_whitespace().collect::<Vec<_>>().join(" ");
    if seed.is_empty() {
        return Err(AppError::Invalid("seed: enter a keyword".to_owned()));
    }
    let transport = context.workspace.free_transport()?;
    let no_keys = |_: &str, _: &str| None;
    let source = SourceContext {
        transport: &transport,
        secrets: &no_keys,
    };
    let market = super::market_of(&project, ask.market.as_deref())?;
    let market = octopuson_keyword_sources::Market::new(&market.country, &market.language);
    let found = match ask.engine {
        Engine::Google => FreeSuggest::google(&source, &seed, &market)?,
        Engine::Yandex => FreeSuggest::yandex(&source, &seed, &market)?,
    };
    Ok(found
        .keywords
        .into_iter()
        .map(|keyword| keyword.keyword)
        .collect())
}

// ------------------------------------------------------------------------------------- Lists

#[derive(Debug, Deserialize)]
struct ProjectLists {
    project_id: i64,
}

fn keyword_lists(
    context: &Context<'_>,
    ask: ProjectLists,
) -> Result<Vec<KeywordListRow>, AppError> {
    Ok(context.workspace.store()?.keyword_lists(ask.project_id)?)
}

#[derive(Debug, Deserialize)]
struct NewList {
    project_id: i64,
    name: String,
}

fn create_keyword_list(context: &Context<'_>, ask: NewList) -> Result<KeywordListRow, AppError> {
    Ok(context
        .workspace
        .store()?
        .create_keyword_list(ask.project_id, &ask.name, &now())?)
}

#[derive(Debug, Deserialize)]
struct Rename {
    list_id: i64,
    name: String,
}

fn rename_keyword_list(context: &Context<'_>, ask: Rename) -> Result<KeywordListRow, AppError> {
    Ok(context
        .workspace
        .store()?
        .rename_keyword_list(ask.list_id, &ask.name)?)
}

#[derive(Debug, Deserialize)]
struct ListId {
    list_id: i64,
}

fn delete_keyword_list(context: &Context<'_>, ask: ListId) -> Result<(), AppError> {
    Ok(context
        .workspace
        .store()?
        .delete_keyword_list(ask.list_id)?)
}

fn list_keywords(context: &Context<'_>, ask: ListId) -> Result<Vec<ListKeyword>, AppError> {
    Ok(context.workspace.store()?.list_keywords(ask.list_id)?)
}

#[derive(Debug, Deserialize)]
struct AddKeywords {
    list_id: i64,
    /// Keywords with their numbers, as the explorer shows them (`keyword` is required).
    keywords: Vec<Value>,
}

/// Adds keywords to a list; how many it holds now.
fn add_to_keyword_list(context: &Context<'_>, ask: AddKeywords) -> Result<u64, AppError> {
    let entries: Vec<(String, Value)> = ask
        .keywords
        .into_iter()
        .filter_map(|data| {
            let keyword = data.get("keyword")?.as_str()?.trim().to_owned();
            (!keyword.is_empty()).then_some((keyword, data))
        })
        .collect();
    if entries.is_empty() {
        return Err(AppError::Invalid("keywords: nothing to add".to_owned()));
    }
    Ok(context
        .workspace
        .store()?
        .add_to_keyword_list(ask.list_id, &entries, &now())?)
}

#[derive(Debug, Deserialize)]
struct RemoveKeywords {
    list_id: i64,
    keywords: Vec<String>,
}

fn remove_from_keyword_list(context: &Context<'_>, ask: RemoveKeywords) -> Result<u64, AppError> {
    Ok(context
        .workspace
        .store()?
        .remove_from_keyword_list(ask.list_id, &ask.keywords)?)
}

#[derive(Debug, Deserialize)]
struct ClusterAsk {
    list_id: i64,
    /// Least similarity to the pillar, 0.2–0.95.
    #[serde(default)]
    threshold: Option<f64>,
}

/// A keyword of a topic.
#[derive(Debug, Clone, PartialEq, Serialize)]
struct TopicMember {
    keyword: String,
    volume: Option<u64>,
    score: f64,
}

/// Keywords one page can target, led by the most searched.
#[derive(Debug, Clone, PartialEq, Serialize)]
struct Topic {
    pillar: String,
    volume: u64,
    keywords: Vec<TopicMember>,
}

/// Groups a list's keywords by wording (free, offline): each topic is what one page targets.
fn cluster_keyword_list(context: &Context<'_>, ask: ClusterAsk) -> Result<Vec<Topic>, AppError> {
    let store = context.workspace.store()?;
    let list = store.keyword_list(ask.list_id)?;
    let project = store.project(list.project_id)?;
    let entries = store.list_keywords(ask.list_id)?;
    drop(store);
    let volume = |entry: &ListKeyword| entry.data.get("volume").and_then(Value::as_u64);
    let inputs: Vec<Input<'_>> = entries
        .iter()
        .map(|entry| Input {
            keyword: &entry.keyword,
            volume: volume(entry),
        })
        .collect();
    let options = LexicalOptions {
        threshold: ask.threshold.unwrap_or(0.6).clamp(0.2, 0.95),
        language: Language::from_code(&project.language),
    };
    let clustering = cluster_lexical(&inputs, &options);
    let mut topics: Vec<Topic> = clustering
        .clusters
        .iter()
        .filter_map(|cluster| {
            let pillar = entries.get(cluster.pillar)?;
            let keywords: Vec<TopicMember> = cluster
                .members
                .iter()
                .filter_map(|member| {
                    let entry = entries.get(member.index)?;
                    Some(TopicMember {
                        keyword: entry.keyword.clone(),
                        volume: volume(entry),
                        score: member.score,
                    })
                })
                .collect();
            Some(Topic {
                pillar: pillar.keyword.clone(),
                volume: keywords.iter().filter_map(|member| member.volume).sum(),
                keywords,
            })
        })
        .collect();
    topics.sort_by_key(|topic| std::cmp::Reverse(topic.volume));
    Ok(topics)
}

/// Adds the commands.
pub fn register(router: &mut Router) {
    router.add("keywords_estimate", keywords_estimate);
    router.add("get_keywords", get_keywords);
    router.add("fetch_keywords", fetch_keywords);
    router.add("keyword_autocomplete", keyword_autocomplete);
    router.add("keyword_lists", keyword_lists);
    router.add("create_keyword_list", create_keyword_list);
    router.add("rename_keyword_list", rename_keyword_list);
    router.add("delete_keyword_list", delete_keyword_list);
    router.add("list_keywords", list_keywords);
    router.add("add_to_keyword_list", add_to_keyword_list);
    router.add("remove_from_keyword_list", remove_from_keyword_list);
    router.add("cluster_keyword_list", cluster_keyword_list);
}
