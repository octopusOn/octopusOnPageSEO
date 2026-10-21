//! DataForSEO for the OctopusOn GEO-SEO Platform.
//!
//! One [`Client`] sends live tasks (no task polling) with the `credentials` secret
//! (`login:password`) and reads the answer envelope: the task's status, its results and what it
//! cost. In credit mode the client gets the placeholder login and the transport sends the
//! request through the OctopusOn SEO gateway, which swaps in the platform's own account and
//! reports the charge; [`Answer::charged_usd`] carries it.
//!
//! Typed endpoints live in modules: [`labs`] for a domain's overview, [`site`] for the Site
//! Explorer reports (history, keywords, pages, competitors, subdomains, comparisons),
//! [`keywords`] for the Keyword Explorer, [`serp`] for live result pages, [`backlinks`] for
//! link profiles, [`gap`] for keyword and link gaps, [`rank`] for rank checks on Google, Bing
//! and Yandex, and [`ai`] for AI visibility (assistants' answers, AI Mode, LLM Mentions).

pub mod ai;
pub mod backlinks;
pub mod gap;
pub mod keywords;
pub mod labs;
pub mod prices;
pub mod rank;
pub mod serp;
pub mod site;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use octopuson_core::http::{HttpRequest, HttpResponse, HttpTransport};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// Source id, as the secret store and the gateway know it.
pub const ID: &str = "dataforseo";

/// Address the secret is bound to and requests go to.
pub const ORIGIN: &str = "https://api.dataforseo.com";

/// Name of the secret holding `login:password`.
pub const SECRET: &str = "credentials";

/// Status code of a successful task.
const OK: u64 = 20_000;

/// Why DataForSEO gave no answer.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum DfsError {
    /// The request cannot be made as configured (no key, unknown country).
    #[error("{0}")]
    Config(String),
    /// The key or the account token was refused.
    #[error("not authorised: {0}")]
    Auth(String),
    /// No money left: the DataForSEO balance or the OctopusOn SEO credits.
    #[error("no balance left: {0}")]
    Quota(String),
    /// Too many requests; try again after the given seconds when known.
    #[error("too many requests")]
    RateLimited {
        /// Seconds to wait, when the answer said.
        retry_after: Option<u64>,
    },
    /// The service is switched off or out of balance on the platform's side.
    #[error("the service is unavailable: {0}")]
    Unavailable(String),
    /// DataForSEO refused the task (invalid field, no data).
    #[error("refused: {0}")]
    Rejected(String),
    /// The answer could not be read.
    #[error("invalid answer: {0}")]
    Invalid(String),
    /// No answer arrived.
    #[error("cannot reach DataForSEO: {0}")]
    Transport(String),
}

/// The market a task is asked for.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Market {
    /// ISO 3166 country code, lowercase (`tr`).
    pub country: String,
    /// ISO 639 language code (`tr`).
    pub language: String,
}

impl Market {
    /// A market from codes, lowercased.
    pub fn new(country: &str, language: &str) -> Self {
        Self {
            country: country.trim().to_ascii_lowercase(),
            language: language.trim().to_ascii_lowercase(),
        }
    }

    /// The DataForSEO location name of the country (`Turkey`).
    ///
    /// # Errors
    /// [`DfsError::Config`] for a country DataForSEO is not asked for.
    pub fn location_name(&self) -> Result<&'static str, DfsError> {
        octopuson_serp::locale::country_name(&self.country).ok_or_else(|| {
            DfsError::Config(format!(
                "no DataForSEO location for country `{}`",
                self.country
            ))
        })
    }

    /// The location and language fields of a task.
    ///
    /// # Errors
    /// [`DfsError::Config`] for an unknown country.
    pub fn fields(&self) -> Result<Value, DfsError> {
        Ok(json!({
            "location_name": self.location_name()?,
            "language_code": self.language,
        }))
    }
}

/// One answered task.
#[derive(Debug, Clone, PartialEq)]
pub struct Answer {
    /// The task's result list (usually one element holding `items`).
    pub result: Vec<Value>,
    /// What DataForSEO says the task cost, US dollars.
    pub cost_usd: f64,
    /// What the OctopusOn SEO gateway charged in credits, US dollars, when it went through it.
    pub charged_usd: Option<f64>,
}

impl Answer {
    /// The `items` of the first result; empty when DataForSEO has no data.
    pub fn items(&self) -> &[Value] {
        items_of(&self.result)
    }

    /// The first result, `Value::Null` when there is none.
    pub fn first(&self) -> &Value {
        self.result.first().unwrap_or(&Value::Null)
    }

    /// What the task cost the user: the credit charge when there was one, else the provider cost.
    pub fn spent_usd(&self) -> f64 {
        self.charged_usd.unwrap_or(self.cost_usd)
    }
}

/// The `items` of the first element of a result list (as an answer or a kept copy holds it).
pub fn items_of(result: &[Value]) -> &[Value] {
    result
        .first()
        .and_then(|result| result.get("items"))
        .and_then(Value::as_array)
        .map_or(&[], Vec::as_slice)
}

/// Sends live tasks to DataForSEO.
pub struct Client<'a> {
    transport: &'a dyn HttpTransport,
    credentials: String,
    authorization: String,
}

impl std::fmt::Debug for Client<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Client").finish_non_exhaustive()
    }
}

impl<'a> Client<'a> {
    /// A client with `credentials` (`login:password`, or the credit placeholder).
    ///
    /// # Errors
    /// [`DfsError::Config`] when the credentials are not `login:password`.
    pub fn new(transport: &'a dyn HttpTransport, credentials: &str) -> Result<Self, DfsError> {
        let credentials = credentials.trim();
        let valid = credentials
            .split_once(':')
            .is_some_and(|(login, password)| !login.is_empty() && !password.is_empty());
        if !valid {
            return Err(DfsError::Config(
                "the DataForSEO key must be login:password".to_owned(),
            ));
        }
        Ok(Self {
            transport,
            credentials: credentials.to_owned(),
            authorization: format!("Basic {}", STANDARD.encode(credentials)),
        })
    }

    /// Hides the credentials in text that may echo them.
    fn hide(&self, text: &str) -> String {
        [self.credentials.as_str(), self.authorization.as_str()]
            .iter()
            .fold(text.to_owned(), |text, secret| text.replace(secret, "***"))
    }

    /// Sends one task to `/v3/{path}` and returns its results and cost.
    ///
    /// # Errors
    /// [`DfsError`] when the request fails or DataForSEO refuses the task.
    pub fn post(&self, path: &str, task: &Value) -> Result<Answer, DfsError> {
        let request = HttpRequest::post(
            format!("{ORIGIN}/v3/{}", path.trim_start_matches('/')),
            json!([task]).to_string().into_bytes(),
        )
        .header("Authorization", self.authorization.clone())
        .header("Content-Type", "application/json");
        let response = self
            .transport
            .send(&request)
            .map_err(|error| DfsError::Transport(self.hide(&error.to_string())))?;
        let charged_usd = self.transport.take_charge_usd();
        if !response.is_success() {
            return Err(self.status_error(&response));
        }
        let value: Value = serde_json::from_slice(&response.body)
            .map_err(|error| DfsError::Invalid(error.to_string()))?;
        if let Some(error) = self.task_error(&value) {
            return Err(error);
        }
        let cost_usd = value
            .pointer("/tasks/0/cost")
            .or_else(|| value.get("cost"))
            .and_then(Value::as_f64)
            .unwrap_or_default();
        let result = value
            .pointer("/tasks/0/result")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        Ok(Answer {
            result,
            cost_usd,
            charged_usd,
        })
    }

    /// The error of a non-2xx answer: DataForSEO's own or the gateway's.
    fn status_error(&self, response: &HttpResponse) -> DfsError {
        let body: String = self.hide(&message_of(response)).chars().take(300).collect();
        match response.status {
            401 | 403 => DfsError::Auth(body),
            402 => DfsError::Quota(body),
            429 => DfsError::RateLimited {
                retry_after: response.retry_after(),
            },
            503 => DfsError::Unavailable(body),
            status => DfsError::Rejected(format!("HTTP {status}: {body}")),
        }
    }

    /// The error of a task DataForSEO did not complete, if any.
    fn task_error(&self, value: &Value) -> Option<DfsError> {
        let status = value
            .pointer("/tasks/0/status_code")
            .or_else(|| value.get("status_code"))
            .and_then(Value::as_u64)?;
        if status == OK {
            return None;
        }
        let message = value
            .pointer("/tasks/0/status_message")
            .or_else(|| value.get("status_message"))
            .and_then(Value::as_str)
            .unwrap_or_default();
        let message = self.hide(&format!("{status}: {message}"));
        Some(match status {
            40_100..=40_104 => DfsError::Auth(message),
            40_200 | 40_210 => DfsError::Quota(message),
            40_202 => DfsError::RateLimited { retry_after: None },
            _ => DfsError::Rejected(message),
        })
    }
}

/// The message of an error answer: the gateway's `error` field, DataForSEO's
/// `status_message`, or the body.
fn message_of(response: &HttpResponse) -> String {
    let text = response.text();
    serde_json::from_str::<Value>(&text)
        .ok()
        .and_then(|value| {
            ["error", "message", "status_message"]
                .iter()
                .find_map(|field| value.get(field).and_then(Value::as_str).map(str::to_owned))
        })
        .unwrap_or(text)
}

/// A number written as a number or a string.
pub(crate) fn number(value: &Value) -> Option<f64> {
    match value {
        Value::Number(number) => number.as_f64(),
        Value::String(text) => text.trim().parse().ok(),
        _ => None,
    }
}

/// A count written as a number or a string.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub(crate) fn count(value: &Value) -> Option<u64> {
    value.as_u64().or_else(|| {
        number(value)
            .filter(|number| *number >= 0.0)
            .map(|n| n as u64)
    })
}

/// A domain written the way DataForSEO targets want it: host only, lowercase, without `www.`.
pub fn target_domain(input: &str) -> String {
    let text = input.trim().to_ascii_lowercase();
    let text = text
        .split_once("://")
        .map_or(text.as_str(), |(_, rest)| rest)
        .to_owned();
    let host = text
        .split(['/', '?', '#'])
        .next()
        .unwrap_or_default()
        .rsplit('@')
        .next()
        .unwrap_or_default();
    let host = host.split(':').next().unwrap_or_default();
    host.strip_prefix("www.")
        .unwrap_or(host)
        .trim_end_matches('.')
        .to_owned()
}

#[cfg(test)]
mod ai_tests;
#[cfg(test)]
mod backlinks_tests;
#[cfg(test)]
mod gap_tests;
#[cfg(test)]
mod keywords_tests;
#[cfg(test)]
mod rank_tests;
#[cfg(test)]
mod site_tests;
#[cfg(test)]
mod tests;
