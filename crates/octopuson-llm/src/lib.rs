//! Answers of AI assistants asked with the user's own API keys, and how a brand appears in them.
//!
//! [`ask`] puts one question to ChatGPT (OpenAI Responses API with web search), Claude
//! (Anthropic Messages API with the server-side web search tool), Gemini (grounding with
//! Google Search) or Perplexity (Agent API with web search) and returns an [`Answer`]: the
//! text, the sources it cites, the tokens and searches used and what that cost. Every client
//! talks to the network only through [`HttpTransport`], so tests answer from memory.
//!
//! [`analysis`] reads an answer for a set of brands: which are mentioned and in what order,
//! which of their sites are cited, and the tone around each mention.

pub mod analysis;
mod anthropic;
mod gemini;
mod openai;
mod perplexity;
#[cfg(test)]
mod tests;

use std::time::Duration;

use octopuson_core::http::{HttpResponse, HttpTransport};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// How long one answer may take: assistants that search the web need a while.
const ANSWER_TIMEOUT: Duration = Duration::from_secs(180);

/// An AI assistant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Engine {
    /// OpenAI's ChatGPT.
    ChatGpt,
    /// Anthropic's Claude.
    Claude,
    /// Google's Gemini.
    Gemini,
    /// Perplexity.
    Perplexity,
}

impl Engine {
    /// Every assistant, in display order.
    pub const ALL: [Self; 4] = [Self::ChatGpt, Self::Claude, Self::Gemini, Self::Perplexity];

    /// Lowercase id (`chatgpt`).
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ChatGpt => "chatgpt",
            Self::Claude => "claude",
            Self::Gemini => "gemini",
            Self::Perplexity => "perplexity",
        }
    }

    /// The engine of an id.
    pub fn parse(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|engine| engine.as_str() == id)
    }

    /// The source whose key asks it directly (`openai`).
    pub const fn provider(self) -> &'static str {
        match self {
            Self::ChatGpt => "openai",
            Self::Claude => "anthropic",
            Self::Gemini => "google-gemini",
            Self::Perplexity => "perplexity",
        }
    }

    /// The address its key is sent to.
    pub const fn origin(self) -> &'static str {
        match self {
            Self::ChatGpt => "https://api.openai.com",
            Self::Claude => "https://api.anthropic.com",
            Self::Gemini => "https://generativelanguage.googleapis.com",
            Self::Perplexity => "https://api.perplexity.ai",
        }
    }

    /// The model asked unless the user picks another (for Perplexity, a preset).
    pub const fn default_model(self) -> &'static str {
        match self {
            Self::ChatGpt => "gpt-5.5",
            Self::Claude => "claude-opus-5-5",
            Self::Gemini => "gemini-3.8-flash",
            Self::Perplexity => "fast",
        }
    }

    /// A rough list price of one answer with web search, US dollars, for estimates and
    /// budgets before the answer reports what it used.
    pub fn estimate_usd(self, model: &str) -> f64 {
        match self {
            Self::ChatGpt => 0.04,
            Self::Claude => anthropic::estimate_usd(model),
            Self::Gemini => 0.03,
            Self::Perplexity => 0.012,
        }
    }
}

/// A source an answer cites.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Citation {
    /// The page.
    pub url: String,
    /// Its title, when given.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Its host, lowercase, without `www.`.
    pub domain: String,
}

impl Citation {
    /// A citation of `url`, its domain read from the address (or from `title` when the
    /// address is a redirect of the assistant's own, as Gemini's are).
    pub fn new(url: &str, title: Option<&str>) -> Self {
        let mut domain = domain_of(url);
        if domain.ends_with("vertexaisearch.cloud.google.com")
            && let Some(title) = title.filter(|title| title.contains('.') && !title.contains(' '))
        {
            domain = domain_of(title);
        }
        Self {
            url: url.to_owned(),
            title: title.filter(|title| !title.is_empty()).map(str::to_owned),
            domain,
        }
    }
}

/// A host of an address, lowercase, without `www.`.
pub fn domain_of(url: &str) -> String {
    let rest = url.split_once("://").map_or(url, |(_, rest)| rest);
    let host = rest
        .split(['/', '?', '#'])
        .next()
        .unwrap_or_default()
        .rsplit('@')
        .next()
        .unwrap_or_default()
        .split(':')
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    host.strip_prefix("www.").unwrap_or(&host).to_owned()
}

/// One answer.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Answer {
    /// The text, as the assistant wrote it (Markdown).
    pub text: String,
    /// The sources it cites, each once, in order.
    pub citations: Vec<Citation>,
    /// The model that answered.
    pub model: String,
    /// Tokens read.
    pub input_tokens: u64,
    /// Tokens written.
    pub output_tokens: u64,
    /// Web searches it ran.
    pub searches: u32,
    /// What it cost, US dollars: reported by the provider, else priced from the tokens.
    pub cost_usd: f64,
}

impl Answer {
    /// Adds a citation unless its address is already there.
    pub(crate) fn cite(&mut self, url: &str, title: Option<&str>) {
        if !url.is_empty() && !self.citations.iter().any(|known| known.url == url) {
            self.citations.push(Citation::new(url, title));
        }
    }
}

/// A question.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Question<'a> {
    /// What is asked.
    pub prompt: &'a str,
    /// The model (Perplexity: a preset such as `fast`, or `provider/model`).
    pub model: &'a str,
    /// Two-letter country the searches should favour (`TR`), when the assistant takes one.
    pub country: Option<&'a str>,
}

/// Why there is no answer.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LlmError {
    /// The key was refused.
    #[error("the API key was refused: {0}")]
    Auth(String),
    /// The provider account has no balance or quota left.
    #[error("no balance or quota left: {0}")]
    Quota(String),
    /// Too many requests.
    #[error("too many requests")]
    RateLimited {
        /// Seconds to wait, when known.
        retry_after: Option<u64>,
    },
    /// The assistant declined to answer.
    #[error("the assistant declined to answer: {0}")]
    Refused(String),
    /// No answer arrived.
    #[error("the assistant is unavailable: {0}")]
    Unavailable(String),
    /// The provider refused the request or answered something unreadable.
    #[error("{0}")]
    Invalid(String),
}

/// Asks `engine` the question with `key`.
///
/// # Errors
/// [`LlmError`] describing why there is no answer.
pub fn ask(
    engine: Engine,
    transport: &dyn HttpTransport,
    key: &str,
    question: &Question<'_>,
) -> Result<Answer, LlmError> {
    let prompt = question.prompt.trim();
    if prompt.is_empty() {
        return Err(LlmError::Invalid("the question is empty".to_owned()));
    }
    let question = Question {
        prompt,
        model: if question.model.trim().is_empty() {
            engine.default_model()
        } else {
            question.model.trim()
        },
        country: question.country,
    };
    match engine {
        Engine::ChatGpt => openai::ask(transport, key, &question),
        Engine::Claude => anthropic::ask(transport, key, &question),
        Engine::Gemini => gemini::ask(transport, key, &question),
        Engine::Perplexity => perplexity::ask(transport, key, &question),
    }
}

/// Sends a JSON request and reads the JSON answer, mapping failures.
pub(crate) fn exchange(
    transport: &dyn HttpTransport,
    request: octopuson_core::http::HttpRequest,
) -> Result<Value, LlmError> {
    let mut request = request.header("content-type", "application/json");
    request.timeout = Some(ANSWER_TIMEOUT);
    let response = transport
        .send(&request)
        .map_err(|error| LlmError::Unavailable(format!("{error:?}")))?;
    if !response.is_success() {
        return Err(status_error(&response));
    }
    serde_json::from_slice(&response.body)
        .map_err(|error| LlmError::Invalid(format!("unreadable answer: {error}")))
}

/// The provider's error message of a failed response, at most 300 characters.
fn message_of(response: &HttpResponse) -> String {
    let body: Value = serde_json::from_slice(&response.body).unwrap_or(Value::Null);
    let error = body.get("error").unwrap_or(&body);
    let message = error
        .get("message")
        .and_then(Value::as_str)
        .map_or_else(|| response.text(), str::to_owned);
    message.chars().take(300).collect()
}

fn status_error(response: &HttpResponse) -> LlmError {
    let message = message_of(response);
    match response.status {
        401 | 403 => LlmError::Auth(message),
        402 => LlmError::Quota(message),
        429 => LlmError::RateLimited {
            retry_after: response.retry_after(),
        },
        500..=599 => LlmError::Unavailable(format!("HTTP {}: {message}", response.status)),
        status => LlmError::Invalid(format!("HTTP {status}: {message}")),
    }
}

/// A count in a JSON field.
pub(crate) fn count(value: &Value, name: &str) -> u64 {
    value.get(name).and_then(Value::as_u64).unwrap_or_default()
}

/// US dollars of tokens at prices per million.
#[allow(clippy::cast_precision_loss)] // token counts
pub(crate) fn token_cost(input: u64, output: u64, per_million: (f64, f64)) -> f64 {
    (input as f64 * per_million.0 + output as f64 * per_million.1) / 1_000_000.0
}
