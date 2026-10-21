//! AI visibility through DataForSEO: answers of ChatGPT, Claude, Gemini and Perplexity (AI
//! Optimization, LLM Responses, with web search), Google AI Mode (SERP), and the LLM Mentions
//! database of questions whose AI answers mention or cite a site.
//!
//! LLM Responses cost $0.0006 a task plus what the model charges (reported back); AI Mode
//! about $0.004; LLM Mentions $0.10 a task plus $0.001 a row.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::{DfsError, Market, count, items_of, number, target_domain};

/// Endpoint of Google AI Mode answers.
pub const AI_MODE: &str = "serp/google/ai_mode/live/advanced";
/// Endpoint of a site's mention metrics.
pub const MENTIONS_METRICS: &str = "ai_optimization/llm_mentions/target_metrics/live";
/// Endpoint of the questions whose answers mention a site.
pub const MENTIONS_SEARCH: &str = "ai_optimization/llm_mentions/search_mentions/live";

/// Longest question LLM Responses take.
pub const MAX_PROMPT: usize = 500;

/// Base price of an LLM Responses task, US dollars.
const LLM_TASK_USD: f64 = 0.0006;
/// Price of an AI Mode task, US dollars.
const AI_MODE_USD: f64 = 0.004;
/// LLM Mentions: a task, then a row, US dollars.
const MENTIONS_TASK_USD: f64 = 0.10;
const MENTIONS_ROW_USD: f64 = 0.001;

/// The LLM Responses endpoint of an assistant (`chatgpt`, `claude`, `gemini`, `perplexity`).
pub fn llm_endpoint(engine: &str) -> Option<&'static str> {
    Some(match engine {
        "chatgpt" => "ai_optimization/chat_gpt/llm_responses/live",
        "claude" => "ai_optimization/claude/llm_responses/live",
        "gemini" => "ai_optimization/gemini/llm_responses/live",
        "perplexity" => "ai_optimization/perplexity/llm_responses/live",
        _ => return None,
    })
}

/// The model asked through DataForSEO unless the user picks another.
pub fn default_model(engine: &str) -> &'static str {
    match engine {
        "chatgpt" => "gpt-5",
        "claude" => "claude-opus-5",
        "gemini" => "gemini-2.5-flash",
        _ => "sonar",
    }
}

/// A rough price of one answer through DataForSEO: the task and the model's charge.
pub fn llm_estimate(engine: &str) -> f64 {
    LLM_TASK_USD
        + match engine {
            "chatgpt" => 0.03,
            "claude" => 0.04,
            "gemini" => 0.02,
            _ => 0.01,
        }
}

/// The price of an AI Mode answer.
pub const fn ai_mode_estimate() -> f64 {
    AI_MODE_USD
}

/// The price of a mentions question returning `rows` rows.
pub fn mentions_estimate(rows: u32) -> f64 {
    MENTIONS_TASK_USD + MENTIONS_ROW_USD * f64::from(rows)
}

/// The task of one answer with web search, favouring `country` (two letters).
///
/// # Errors
/// [`DfsError::Config`] for an empty question or one over 500 characters.
pub fn llm_task(prompt: &str, model: &str, country: Option<&str>) -> Result<Value, DfsError> {
    let prompt = prompt.trim();
    if prompt.is_empty() {
        return Err(DfsError::Config("the question is empty".to_owned()));
    }
    if prompt.chars().count() > MAX_PROMPT {
        return Err(DfsError::Config(
            "questions are at most 500 characters".to_owned(),
        ));
    }
    let mut task = json!({
        "user_prompt": prompt,
        "model_name": model,
        "web_search": true,
        "max_output_tokens": 2_048,
    });
    if let Some(country) = country {
        task["web_search_country_iso_code"] = json!(country.to_ascii_uppercase());
    }
    Ok(task)
}

/// An answer read from DataForSEO.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct LlmAnswer {
    /// The text (Markdown).
    pub text: String,
    /// Cited sources: address and title, each once.
    pub citations: Vec<(String, Option<String>)>,
    /// The model that answered.
    pub model: Option<String>,
}

impl LlmAnswer {
    fn cite(&mut self, url: Option<&str>, title: Option<&str>) {
        if let Some(url) = url.filter(|url| !url.is_empty())
            && !self.citations.iter().any(|(known, _)| known == url)
        {
            self.citations.push((
                url.to_owned(),
                title.filter(|t| !t.is_empty()).map(str::to_owned),
            ));
        }
    }
}

/// Reads an LLM Responses answer: the `message` items' text sections and their annotations.
pub fn llm_from_result(result: &[Value]) -> LlmAnswer {
    let first = result.first().unwrap_or(&Value::Null);
    let mut answer = LlmAnswer {
        model: first
            .get("model_name")
            .and_then(Value::as_str)
            .map(str::to_owned),
        ..LlmAnswer::default()
    };
    for item in items_of(result) {
        if item.get("type").and_then(Value::as_str) != Some("message") {
            continue;
        }
        for section in item
            .get("sections")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            if let Some(text) = section.get("text").and_then(Value::as_str) {
                answer.text.push_str(text);
            }
            for annotation in section
                .get("annotations")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                answer.cite(
                    annotation.get("url").and_then(Value::as_str),
                    annotation.get("title").and_then(Value::as_str),
                );
            }
        }
    }
    answer
}

/// The task of a Google AI Mode answer in the market.
///
/// # Errors
/// [`DfsError::Config`] for an empty question or an unknown country.
pub fn ai_mode_task(prompt: &str, market: &Market) -> Result<Value, DfsError> {
    let prompt = prompt.split_whitespace().collect::<Vec<_>>().join(" ");
    if prompt.is_empty() {
        return Err(DfsError::Config("the question is empty".to_owned()));
    }
    let mut task = market.fields()?;
    task["keyword"] = json!(prompt);
    Ok(task)
}

fn walk_references(item: &Value, answer: &mut LlmAnswer) {
    for name in ["references", "links"] {
        for reference in item
            .get(name)
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            answer.cite(
                reference.get("url").and_then(Value::as_str),
                reference
                    .get("title")
                    .or_else(|| reference.get("source"))
                    .and_then(Value::as_str),
            );
        }
    }
    for child in item
        .get("items")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        walk_references(child, answer);
    }
}

/// Reads an AI Mode answer: the AI overview's Markdown and its references.
pub fn ai_mode_from_result(result: &[Value]) -> LlmAnswer {
    let mut answer = LlmAnswer {
        model: Some("google-ai-mode".to_owned()),
        ..LlmAnswer::default()
    };
    for item in items_of(result) {
        if item.get("type").and_then(Value::as_str) != Some("ai_overview") {
            continue;
        }
        if let Some(markdown) = item.get("markdown").and_then(Value::as_str) {
            if !answer.text.is_empty() {
                answer.text.push_str("\n\n");
            }
            answer.text.push_str(markdown);
        }
        walk_references(item, &mut answer);
    }
    answer
}

fn mentions_target(domain: &str) -> Result<Value, DfsError> {
    let domain = target_domain(domain);
    if domain.is_empty() {
        return Err(DfsError::Config("the domain is empty".to_owned()));
    }
    Ok(json!([{ "domain": domain, "search_filter": "include", "include_subdomains": true }]))
}

/// The task of a site's mention metrics in the market.
///
/// # Errors
/// [`DfsError::Config`] for an empty domain or an unknown country.
pub fn mentions_metrics_task(domain: &str, market: &Market) -> Result<Value, DfsError> {
    let mut task = market.fields()?;
    task["target"] = mentions_target(domain)?;
    task["internal_list_limit"] = json!(10);
    Ok(task)
}

/// The task of the questions whose answers mention or cite a site, most searched first.
///
/// # Errors
/// [`DfsError::Config`] for an empty domain or an unknown country.
pub fn mentions_search_task(domain: &str, market: &Market, limit: u32) -> Result<Value, DfsError> {
    let mut task = market.fields()?;
    task["target"] = mentions_target(domain)?;
    task["order_by"] = json!(["ai_search_volume,desc"]);
    task["limit"] = json!(limit.clamp(10, 1000));
    Ok(task)
}

/// A group of mentions (a platform, a cited domain, a brand).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct MentionGroup {
    /// The platform, domain or brand.
    pub key: String,
    /// Answers mentioning the site.
    pub mentions: u64,
    /// Monthly AI searches of those questions.
    pub ai_search_volume: u64,
}

/// A site's mention metrics.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct MentionMetrics {
    /// The site.
    pub target: String,
    /// Answers mentioning or citing it.
    pub mentions: u64,
    /// Monthly AI searches of those questions.
    pub ai_search_volume: u64,
    /// By platform (`chat_gpt`, `google`).
    pub platforms: Vec<MentionGroup>,
    /// Domains cited alongside.
    pub sources: Vec<MentionGroup>,
    /// Brands named alongside (ChatGPT).
    pub brands: Vec<MentionGroup>,
}

fn groups(metrics: &Value, name: &str) -> Vec<MentionGroup> {
    metrics
        .get(name)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|group| {
            Some(MentionGroup {
                key: group.get("key")?.as_str()?.to_owned(),
                mentions: group.get("mentions").and_then(count).unwrap_or_default(),
                ai_search_volume: group
                    .get("ai_search_volume")
                    .and_then(count)
                    .unwrap_or_default(),
            })
        })
        .collect()
}

/// Reads mention metrics of `target`.
pub fn metrics_from_result(target: &str, result: &[Value]) -> MentionMetrics {
    let first = result.first().unwrap_or(&Value::Null);
    let metrics = first.get("aggregated_metrics").unwrap_or(&Value::Null);
    let total = metrics.get("total").unwrap_or(&Value::Null);
    MentionMetrics {
        target: target_domain(target),
        mentions: total.get("mentions").and_then(count).unwrap_or_default(),
        ai_search_volume: total
            .get("ai_search_volume")
            .and_then(count)
            .unwrap_or_default(),
        platforms: groups(metrics, "platform"),
        sources: groups(metrics, "sources_domain"),
        brands: groups(metrics, "brand_entities_title"),
    }
}

/// A source of a mentioned answer.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct MentionSource {
    /// Its domain.
    pub domain: String,
    /// The page.
    pub url: Option<String>,
    /// Its title.
    pub title: Option<String>,
}

/// A question whose AI answer mentions or cites the site.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct MentionRow {
    /// `chat_gpt` or `google`.
    pub platform: String,
    /// The question.
    pub question: String,
    /// The start of the answer.
    pub answer: String,
    /// Its sources, by rank.
    pub sources: Vec<MentionSource>,
    /// Monthly AI searches of the question.
    pub ai_search_volume: Option<u64>,
    /// When the answer was last seen.
    pub last_response_at: Option<String>,
}

/// Longest answer start kept per row.
const ANSWER_START: usize = 600;

/// Reads the questions of a mentions search.
pub fn mentions_from_result(result: &[Value]) -> Vec<MentionRow> {
    items_of(result)
        .iter()
        .map(|item| {
            let text = |name: &str| {
                item.get(name)
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned()
            };
            MentionRow {
                platform: text("platform"),
                question: text("question"),
                answer: text("answer").chars().take(ANSWER_START).collect(),
                sources: item
                    .get("sources")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .map(|source| MentionSource {
                        domain: source
                            .get("domain")
                            .and_then(Value::as_str)
                            .map(target_domain)
                            .unwrap_or_default(),
                        url: source.get("url").and_then(Value::as_str).map(str::to_owned),
                        title: source
                            .get("title")
                            .and_then(Value::as_str)
                            .map(str::to_owned),
                    })
                    .collect(),
                ai_search_volume: item.get("ai_search_volume").and_then(count),
                last_response_at: item
                    .get("last_response_at")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
            }
        })
        .collect()
}

/// What an LLM Responses answer cost the model's provider, as DataForSEO reports it.
pub fn money_spent(result: &[Value]) -> Option<f64> {
    result.first()?.get("money_spent").and_then(number)
}
