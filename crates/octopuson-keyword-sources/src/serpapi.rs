//! Google autocomplete through SerpApi, with the `api_key` secret.

use octopuson_core::http::HttpRequest;
use serde_json::Value;
use url::Url;

use crate::{Fetched, FoundKeyword, Market, SourceContext, SourceError, parse, redact};

/// SerpApi.
#[derive(Debug, Clone, Copy, Default)]
pub struct SerpApi;

impl SerpApi {
    /// Source id.
    pub const ID: &'static str = "serpapi";
    /// Address the secret is bound to.
    pub const ORIGIN: &'static str = "https://serpapi.com";
    /// Estimated price of one search, US dollars (plans sell searches at 0.01–0.02).
    pub const COST_PER_REQUEST_USD: f64 = 0.015;

    /// Google autocomplete suggestions for a phrase.
    ///
    /// # Errors
    /// [`SourceError`] when SerpApi cannot be asked.
    pub fn autocomplete(
        context: &SourceContext<'_>,
        phrase: &str,
        market: &Market,
    ) -> Result<Fetched, SourceError> {
        let key = context.secret(Self::ID, "api_key")?;
        let mut url = Url::parse(&format!("{}/search.json", Self::ORIGIN))
            .map_err(|error| SourceError::Config(error.to_string()))?;
        url.query_pairs_mut()
            .append_pair("engine", "google_autocomplete")
            .append_pair("q", phrase)
            .append_pair("gl", &market.country)
            .append_pair("hl", &market.language)
            .append_pair("api_key", &key);
        let response = context.send(&HttpRequest::get(url.as_str()))?;
        let hide = |text: &str| redact(text, std::slice::from_ref(&key));
        if !response.is_success() {
            return Err(SourceError::from_status(&response, &hide));
        }
        let value = parse(&response, "SerpApi")?;
        if let Some(error) = value.get("error").and_then(Value::as_str) {
            let message = hide(error);
            return Err(
                if message.to_ascii_lowercase().contains("run out of searches") {
                    SourceError::Quota {
                        message,
                        retry_after: None,
                    }
                } else {
                    SourceError::Rejected(message)
                },
            );
        }
        let keywords = value
            .get("suggestions")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|item| Some(FoundKeyword::named(item.get("value")?.as_str()?)))
            .collect();
        Ok(Fetched {
            keywords,
            requests: 1,
            cost_usd: Self::COST_PER_REQUEST_USD,
        })
    }
}
