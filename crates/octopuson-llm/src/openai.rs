//! ChatGPT through the OpenAI Responses API with the `web_search` tool. The answer is the
//! `output_text` of the `message` items; its sources are their `url_citation` annotations.

use octopuson_core::http::{HttpRequest, HttpTransport};
use serde_json::{Value, json};

use crate::{Answer, LlmError, Question, Writing, count, exchange, token_cost};

const URL: &str = "https://api.openai.com/v1/responses";

/// Input and output prices per million tokens (an estimate; the API reports tokens only).
pub(crate) const PRICES: (f64, f64) = (2.5, 15.0);

/// One web search, US dollars.
const SEARCH_USD: f64 = 0.01;

pub(crate) fn ask(
    transport: &dyn HttpTransport,
    key: &str,
    question: &Question<'_>,
) -> Result<Answer, LlmError> {
    let mut tool = json!({ "type": "web_search" });
    if let Some(country) = question.country {
        tool["user_location"] = json!({ "type": "approximate", "country": country });
    }
    let body = json!({ "model": question.model, "input": question.prompt, "tools": [tool] });
    let request = HttpRequest::post(URL, body.to_string().into_bytes())
        .header("authorization", format!("Bearer {key}"));
    let response = exchange(transport, request)?;
    if let Some(error) = response.get("error").filter(|error| !error.is_null()) {
        let message = error
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("the answer failed");
        return Err(LlmError::Invalid(message.to_owned()));
    }
    let mut answer = Answer {
        model: response
            .get("model")
            .and_then(Value::as_str)
            .unwrap_or(question.model)
            .to_owned(),
        ..Answer::default()
    };
    for item in response
        .get("output")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        match item.get("type").and_then(Value::as_str) {
            Some("web_search_call") => answer.searches += 1,
            Some("message") => {
                for part in item
                    .get("content")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                {
                    match part.get("type").and_then(Value::as_str) {
                        Some("output_text") => {
                            if let Some(text) = part.get("text").and_then(Value::as_str) {
                                answer.text.push_str(text);
                            }
                            for annotation in part
                                .get("annotations")
                                .and_then(Value::as_array)
                                .into_iter()
                                .flatten()
                            {
                                if annotation.get("type").and_then(Value::as_str)
                                    == Some("url_citation")
                                {
                                    answer.cite(
                                        annotation
                                            .get("url")
                                            .and_then(Value::as_str)
                                            .unwrap_or_default(),
                                        annotation.get("title").and_then(Value::as_str),
                                    );
                                }
                            }
                        }
                        Some("refusal") => {
                            let reason = part
                                .get("refusal")
                                .and_then(Value::as_str)
                                .unwrap_or("policy");
                            return Err(LlmError::Refused(reason.to_owned()));
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
    let usage = response.get("usage").unwrap_or(&Value::Null);
    answer.input_tokens = count(usage, "input_tokens");
    answer.output_tokens = count(usage, "output_tokens");
    answer.cost_usd = token_cost(answer.input_tokens, answer.output_tokens, PRICES)
        + f64::from(answer.searches) * SEARCH_USD;
    Ok(answer)
}

/// Writes from the material with the instructions, without tools.
pub(crate) fn write(
    transport: &dyn HttpTransport,
    key: &str,
    writing: &Writing<'_>,
) -> Result<Answer, LlmError> {
    let body = json!({
        "model": writing.model,
        "instructions": writing.instructions,
        "input": writing.material,
        "max_output_tokens": writing.max_tokens,
    });
    let request = HttpRequest::post(URL, body.to_string().into_bytes())
        .header("authorization", format!("Bearer {key}"));
    let response = exchange(transport, request)?;
    if let Some(error) = response.get("error").filter(|error| !error.is_null()) {
        let message = error
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("the answer failed");
        return Err(LlmError::Invalid(message.to_owned()));
    }
    let mut answer = Answer {
        model: response
            .get("model")
            .and_then(Value::as_str)
            .unwrap_or(writing.model)
            .to_owned(),
        ..Answer::default()
    };
    for part in response
        .get("output")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|item| item.get("type").and_then(Value::as_str) == Some("message"))
        .filter_map(|item| item.get("content").and_then(Value::as_array))
        .flatten()
    {
        match part.get("type").and_then(Value::as_str) {
            Some("output_text") => {
                if let Some(text) = part.get("text").and_then(Value::as_str) {
                    answer.text.push_str(text);
                }
            }
            Some("refusal") => {
                let reason = part
                    .get("refusal")
                    .and_then(Value::as_str)
                    .unwrap_or("policy");
                return Err(LlmError::Refused(reason.to_owned()));
            }
            _ => {}
        }
    }
    let usage = response.get("usage").unwrap_or(&Value::Null);
    answer.input_tokens = count(usage, "input_tokens");
    answer.output_tokens = count(usage, "output_tokens");
    answer.cost_usd = token_cost(answer.input_tokens, answer.output_tokens, PRICES);
    Ok(answer)
}
