//! Perplexity through the Agent API (`/v1/agent`, which replaced Sonar chat completions) with
//! the `web_search` tool. The answer is the `output_text` of the `message` items; its sources
//! are their `url_citation` annotations, or else the search results its `[n]` markers point
//! to. Perplexity reports what each answer cost.

use octopuson_core::http::{HttpRequest, HttpTransport};
use serde_json::{Value, json};

use crate::{Answer, LlmError, Question, count, exchange};

const URL: &str = "https://api.perplexity.ai/v1/agent";

/// Numbers of the `[n]` markers in a text.
fn markers(text: &str) -> Vec<u64> {
    let mut found = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find('[') {
        rest = &rest[start + 1..];
        let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
        if !digits.is_empty()
            && rest[digits.len()..].starts_with(']')
            && let Ok(number) = digits.parse()
            && !found.contains(&number)
        {
            found.push(number);
        }
    }
    found
}

/// The items of a JSON array field; none when it is missing.
fn items<'a>(value: &'a Value, name: &str) -> impl Iterator<Item = &'a Value> {
    value
        .get(name)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
}

pub(crate) fn ask(
    transport: &dyn HttpTransport,
    key: &str,
    question: &Question<'_>,
) -> Result<Answer, LlmError> {
    let mut body = json!({
        "input": question.prompt,
        "tools": [{ "type": "web_search" }],
        "max_output_tokens": 4_096,
    });
    // `provider/model` names a model; anything else is a preset such as `fast`.
    if question.model.contains('/') {
        body["model"] = json!(question.model);
    } else {
        body["preset"] = json!(question.model);
    }
    let request = HttpRequest::post(URL, body.to_string().into_bytes())
        .header("authorization", format!("Bearer {key}"));
    let response = exchange(transport, request)?;
    if response.get("status").and_then(Value::as_str) == Some("failed") {
        let message = response
            .get("error")
            .and_then(|error| error.get("message"))
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
    let mut results: Vec<(u64, String, Option<String>)> = Vec::new();
    for item in items(&response, "output") {
        match item.get("type").and_then(Value::as_str) {
            Some("message") => {
                for part in items(item, "content") {
                    if let Some(text) = part.get("text").and_then(Value::as_str) {
                        answer.text.push_str(text);
                    }
                    for annotation in items(part, "annotations") {
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
            Some("search_results") => {
                answer.searches += item
                    .get("queries")
                    .and_then(Value::as_array)
                    .map_or(1, |queries| {
                        u32::try_from(queries.len().max(1)).unwrap_or(1)
                    });
                for result in items(item, "results") {
                    if let Some(url) = result.get("url").and_then(Value::as_str) {
                        results.push((
                            count(result, "id"),
                            url.to_owned(),
                            result
                                .get("title")
                                .and_then(Value::as_str)
                                .map(str::to_owned),
                        ));
                    }
                }
            }
            _ => {}
        }
    }
    if answer.citations.is_empty() {
        for number in markers(&answer.text) {
            if let Some((_, url, title)) = results.iter().find(|(id, ..)| *id == number) {
                answer.cite(url, title.as_deref());
            }
        }
    }
    let usage = response.get("usage").unwrap_or(&Value::Null);
    answer.input_tokens = count(usage, "input_tokens");
    answer.output_tokens = count(usage, "output_tokens");
    answer.cost_usd = usage
        .get("cost")
        .and_then(|cost| cost.get("total_cost"))
        .and_then(Value::as_f64)
        .unwrap_or_default();
    Ok(answer)
}

#[cfg(test)]
mod tests {
    #[test]
    fn reads_citation_markers_once() {
        assert_eq!(super::markers("A [1] and B [3][1], not [x] or [2"), [1, 3]);
    }
}
