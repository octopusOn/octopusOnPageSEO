//! Claude through the Anthropic Messages API, searching the web with the server-side web
//! search tool. The cited sources are the text blocks' `web_search_result_location`
//! citations. Models that support it get server-side refusal fallbacks (`fallbacks:
//! "default"`), so a declined question is answered by the model Anthropic recommends.

use octopuson_core::http::{HttpRequest, HttpTransport};
use serde_json::{Value, json};

use crate::{Answer, LlmError, Question, Writing, count, exchange, token_cost};

const URL: &str = "https://api.anthropic.com/v1/messages";
const VERSION: &str = "2023-06-01";
const FALLBACK_BETA: &str = "server-side-fallback-2026-07-01";

/// Times a paused server-side search loop is resumed.
const RESUMES: usize = 3;

/// Searches one answer may run.
const MAX_SEARCHES: u32 = 5;

/// Web search with dynamic filtering, on the models that have it.
const DYNAMIC_SEARCH_MODELS: &[&str] = &[
    "claude-opus-5-5",
    "claude-opus-5",
    "claude-opus-4-8",
    "claude-opus-4-7",
    "claude-opus-4-6",
    "claude-sonnet-5-5",
    "claude-sonnet-5",
    "claude-sonnet-4-6",
];

/// Models that take `fallbacks: "default"`.
const FALLBACK_MODELS: &[&str] = &[
    "claude-fable-5-1",
    "claude-opus-5-5",
    "claude-opus-5",
    "claude-sonnet-5-5",
];

/// Models that think adaptively and take an effort level (thinking is set by effort alone).
const ADAPTIVE_MODELS: &[&str] = &[
    "claude-fable",
    "claude-mythos",
    "claude-opus-5",
    "claude-opus-4-8",
    "claude-opus-4-7",
    "claude-opus-4-6",
    "claude-sonnet-5",
    "claude-sonnet-4-6",
    "claude-haiku-5",
];

/// Effort of a written answer: thorough enough for a brief, without the cost of the top
/// levels.
const WRITING_EFFORT: &str = "medium";

/// Input and output prices per million tokens.
pub(crate) fn prices(model: &str) -> (f64, f64) {
    if model.starts_with("claude-fable") || model.starts_with("claude-mythos") {
        (10.0, 50.0)
    } else if model.starts_with("claude-opus-5-5") {
        (4.0, 20.0)
    } else if model.starts_with("claude-opus") {
        (5.0, 25.0)
    } else if model.starts_with("claude-sonnet-5") {
        (2.0, 10.0)
    } else if model.starts_with("claude-sonnet") {
        (3.0, 15.0)
    } else if model.starts_with("claude-haiku-5") {
        (0.1, 0.5)
    } else {
        (1.0, 5.0)
    }
}

/// One web search, US dollars.
const SEARCH_USD: f64 = 0.01;

/// A rough price of one answer: a few thousand tokens and two searches.
pub(crate) fn estimate_usd(model: &str) -> f64 {
    token_cost(6_000, 1_500, prices(model)) + 2.0 * SEARCH_USD
}

pub(crate) fn ask(
    transport: &dyn HttpTransport,
    key: &str,
    question: &Question<'_>,
) -> Result<Answer, LlmError> {
    let model = question.model;
    let search_type = if DYNAMIC_SEARCH_MODELS.contains(&model) {
        "web_search_20260209"
    } else {
        "web_search_20250305"
    };
    let mut tool = json!({ "type": search_type, "name": "web_search", "max_uses": MAX_SEARCHES });
    if let Some(country) = question.country {
        tool["user_location"] = json!({ "type": "approximate", "country": country });
    }
    let fallbacks = FALLBACK_MODELS.contains(&model);
    let mut messages = vec![json!({ "role": "user", "content": question.prompt })];
    let mut answer = Answer {
        model: model.to_owned(),
        ..Answer::default()
    };
    let mut paid_searches = 0_u64;
    for _ in 0..=RESUMES {
        let mut body = json!({
            "model": model,
            "max_tokens": 16_000,
            "tools": [tool],
            "messages": messages,
        });
        let mut request = HttpRequest::post(URL, Vec::new())
            .header("x-api-key", key)
            .header("anthropic-version", VERSION);
        if fallbacks {
            body["fallbacks"] = json!("default");
            request = request.header("anthropic-beta", FALLBACK_BETA);
        }
        request.body = Some(body.to_string().into_bytes());
        let response = exchange(transport, request)?;
        if let Some(served) = response.get("model").and_then(Value::as_str) {
            served.clone_into(&mut answer.model);
        }
        let usage = response.get("usage").unwrap_or(&Value::Null);
        answer.input_tokens += count(usage, "input_tokens");
        answer.output_tokens += count(usage, "output_tokens");
        paid_searches += usage
            .get("server_tool_use")
            .map_or(0, |tools| count(tools, "web_search_requests"));
        let content = response
            .get("content")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        for block in &content {
            if block.get("type").and_then(Value::as_str) != Some("text") {
                continue;
            }
            if let Some(text) = block.get("text").and_then(Value::as_str) {
                answer.text.push_str(text);
            }
            for citation in block
                .get("citations")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                if citation.get("type").and_then(Value::as_str)
                    == Some("web_search_result_location")
                {
                    answer.cite(
                        citation
                            .get("url")
                            .and_then(Value::as_str)
                            .unwrap_or_default(),
                        citation.get("title").and_then(Value::as_str),
                    );
                }
            }
        }
        match response.get("stop_reason").and_then(Value::as_str) {
            Some("refusal") => {
                let category = response
                    .get("stop_details")
                    .and_then(|details| details.get("category"))
                    .and_then(Value::as_str)
                    .unwrap_or("policy");
                return Err(LlmError::Refused(category.to_owned()));
            }
            // The server paused its search loop: send the turn back and it resumes.
            Some("pause_turn") => {
                messages.push(json!({ "role": "assistant", "content": content }));
            }
            _ => break,
        }
    }
    answer.searches = u32::try_from(paid_searches).unwrap_or(u32::MAX);
    #[allow(clippy::cast_precision_loss)] // a few searches
    let searches_usd = paid_searches as f64 * SEARCH_USD;
    answer.cost_usd = token_cost(
        answer.input_tokens,
        answer.output_tokens,
        prices(&answer.model),
    ) + searches_usd;
    Ok(answer)
}

/// Writes from the material with the instructions as the system prompt, thinking adaptively
/// on the models that do; no tools. A refusal is an error; the text blocks are the answer
/// (thinking blocks are left out).
pub(crate) fn write(
    transport: &dyn HttpTransport,
    key: &str,
    writing: &Writing<'_>,
) -> Result<Answer, LlmError> {
    let model = writing.model;
    let mut body = json!({
        "model": model,
        "max_tokens": writing.max_tokens,
        "messages": [{ "role": "user", "content": writing.material }],
    });
    if !writing.instructions.trim().is_empty() {
        body["system"] = json!(writing.instructions);
    }
    if ADAPTIVE_MODELS
        .iter()
        .any(|family| model.starts_with(family))
    {
        body["thinking"] = json!({ "type": "adaptive" });
        body["output_config"] = json!({ "effort": WRITING_EFFORT });
    }
    let mut request = HttpRequest::post(URL, Vec::new())
        .header("x-api-key", key)
        .header("anthropic-version", VERSION);
    if FALLBACK_MODELS.contains(&model) {
        body["fallbacks"] = json!("default");
        request = request.header("anthropic-beta", FALLBACK_BETA);
    }
    request.body = Some(body.to_string().into_bytes());
    let response = exchange(transport, request)?;
    if response.get("stop_reason").and_then(Value::as_str) == Some("refusal") {
        let category = response
            .get("stop_details")
            .and_then(|details| details.get("category"))
            .and_then(Value::as_str)
            .unwrap_or("policy");
        return Err(LlmError::Refused(category.to_owned()));
    }
    let mut answer = Answer {
        model: response
            .get("model")
            .and_then(Value::as_str)
            .unwrap_or(model)
            .to_owned(),
        ..Answer::default()
    };
    for block in response
        .get("content")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        if block.get("type").and_then(Value::as_str) == Some("text")
            && let Some(text) = block.get("text").and_then(Value::as_str)
        {
            answer.text.push_str(text);
        }
    }
    let usage = response.get("usage").unwrap_or(&Value::Null);
    answer.input_tokens = count(usage, "input_tokens");
    answer.output_tokens = count(usage, "output_tokens");
    answer.cost_usd = token_cost(
        answer.input_tokens,
        answer.output_tokens,
        prices(&answer.model),
    );
    Ok(answer)
}
