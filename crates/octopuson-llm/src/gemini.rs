//! Gemini through `generateContent` with grounding in Google Search. The answer is the text of
//! the first candidate (thoughts left out); its sources are the grounding chunks, whose
//! addresses are Google redirects titled with the source's domain.

use octopuson_core::http::{HttpRequest, HttpTransport};
use serde_json::{Value, json};

use crate::{Answer, LlmError, Question, count, exchange, token_cost};

const BASE: &str = "https://generativelanguage.googleapis.com/v1beta/models";

/// Input and output prices per million tokens of a Flash model (an estimate).
const PRICES: (f64, f64) = (0.5, 3.0);

/// One grounding search, US dollars.
const SEARCH_USD: f64 = 0.014;

pub(crate) fn ask(
    transport: &dyn HttpTransport,
    key: &str,
    question: &Question<'_>,
) -> Result<Answer, LlmError> {
    let model = question.model.trim_start_matches("models/");
    if !model
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '.' | '_'))
    {
        return Err(LlmError::Invalid(format!("unknown Gemini model `{model}`")));
    }
    let body = json!({
        "contents": [{ "role": "user", "parts": [{ "text": question.prompt }] }],
        "tools": [{ "google_search": {} }],
    });
    let request = HttpRequest::post(
        format!("{BASE}/{model}:generateContent"),
        body.to_string().into_bytes(),
    )
    .header("x-goog-api-key", key);
    let response = exchange(transport, request)?;
    if let Some(reason) = response
        .get("promptFeedback")
        .and_then(|feedback| feedback.get("blockReason"))
        .and_then(Value::as_str)
    {
        return Err(LlmError::Refused(reason.to_owned()));
    }
    let candidate = response
        .get("candidates")
        .and_then(Value::as_array)
        .and_then(|candidates| candidates.first())
        .ok_or_else(|| LlmError::Invalid("Gemini gave no answer".to_owned()))?;
    if let Some(reason @ ("SAFETY" | "PROHIBITED_CONTENT" | "BLOCKLIST" | "RECITATION")) =
        candidate.get("finishReason").and_then(Value::as_str)
    {
        return Err(LlmError::Refused(reason.to_owned()));
    }
    let mut answer = Answer {
        model: response
            .get("modelVersion")
            .and_then(Value::as_str)
            .unwrap_or(model)
            .to_owned(),
        ..Answer::default()
    };
    for part in candidate
        .get("content")
        .and_then(|content| content.get("parts"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        if part.get("thought").and_then(Value::as_bool) == Some(true) {
            continue;
        }
        if let Some(text) = part.get("text").and_then(Value::as_str) {
            answer.text.push_str(text);
        }
    }
    let grounding = candidate.get("groundingMetadata").unwrap_or(&Value::Null);
    for chunk in grounding
        .get("groundingChunks")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        if let Some(web) = chunk.get("web") {
            answer.cite(
                web.get("uri").and_then(Value::as_str).unwrap_or_default(),
                web.get("title").and_then(Value::as_str),
            );
        }
    }
    answer.searches = grounding
        .get("webSearchQueries")
        .and_then(Value::as_array)
        .map_or(0, |queries| {
            u32::try_from(queries.len()).unwrap_or(u32::MAX)
        });
    let usage = response.get("usageMetadata").unwrap_or(&Value::Null);
    answer.input_tokens = count(usage, "promptTokenCount");
    answer.output_tokens =
        count(usage, "candidatesTokenCount") + count(usage, "thoughtsTokenCount");
    answer.cost_usd = token_cost(answer.input_tokens, answer.output_tokens, PRICES)
        + f64::from(answer.searches) * SEARCH_USD;
    Ok(answer)
}
