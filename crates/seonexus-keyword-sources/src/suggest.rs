//! The free Google and Yandex suggestion endpoints.
//!
//! They are not official APIs: the SEO Keyword Research keeps them off until the user switches them
//! on after a terms warning (Yandex's terms forbid automated queries without permission).
//! Requests carry the program's own user agent, go one at a time at a fixed slow pace with a
//! daily limit (kept by the caller), and any refusal (429, 403, a check page) ends their use
//! for the day: it is reported as [`SourceError::Quota`], which the caller never retries.

use seonexus_core::http::HttpRequest;
use serde_json::Value;
use url::Url;

use crate::{Fetched, FoundKeyword, Market, SourceContext, SourceError};

/// The free endpoints.
#[derive(Debug, Clone, Copy, Default)]
pub struct FreeSuggest;

impl FreeSuggest {
    /// Source id of Google suggestions.
    pub const GOOGLE: &'static str = "google-suggest";
    /// Source id of Yandex suggestions.
    pub const YANDEX: &'static str = "yandex-suggest";

    /// Google suggestions for a phrase.
    ///
    /// # Errors
    /// [`SourceError::Quota`] when Google refuses or shows a check page.
    pub fn google(
        context: &SourceContext<'_>,
        phrase: &str,
        market: &Market,
    ) -> Result<Fetched, SourceError> {
        let mut url = Url::parse("https://suggestqueries.google.com/complete/search")
            .map_err(|error| SourceError::Config(error.to_string()))?;
        url.query_pairs_mut()
            .append_pair("client", "firefox")
            .append_pair("hl", &market.language)
            .append_pair("gl", &market.country)
            .append_pair("q", phrase);
        Self::ask(context, url.as_str(), "Google")
    }

    /// Yandex suggestions for a phrase, from the Yandex site of the country.
    ///
    /// # Errors
    /// [`SourceError::Quota`] when Yandex refuses or shows a check page.
    pub fn yandex(
        context: &SourceContext<'_>,
        phrase: &str,
        market: &Market,
    ) -> Result<Fetched, SourceError> {
        let host = match market.country.as_str() {
            "tr" => "suggest.yandex.com.tr",
            "ru" => "suggest.yandex.ru",
            "kz" => "suggest.yandex.kz",
            "by" => "suggest.yandex.by",
            _ => "suggest.yandex.com",
        };
        let mut url = Url::parse(&format!("https://{host}/suggest-ff.cgi"))
            .map_err(|error| SourceError::Config(error.to_string()))?;
        url.query_pairs_mut()
            .append_pair("part", phrase)
            .append_pair("uil", &market.language);
        Self::ask(context, url.as_str(), "Yandex")
    }

    fn ask(context: &SourceContext<'_>, url: &str, engine: &str) -> Result<Fetched, SourceError> {
        let response = context.send(&HttpRequest::get(url))?;
        let refused = |reason: String| SourceError::Quota {
            message: format!(
                "{engine} refused the suggestion request ({reason}); stopped for today"
            ),
            retry_after: None,
        };
        if matches!(response.status, 403 | 429) || response.url.contains("/sorry/") {
            return Err(refused(format!("HTTP {}", response.status)));
        }
        if !response.is_success() {
            return Err(SourceError::Rejected(format!("HTTP {}", response.status)));
        }
        // A check page arrives as HTML instead of the JSON array.
        let value: Value = serde_json::from_slice(&response.body)
            .map_err(|_| refused("a check page instead of suggestions".to_owned()))?;
        let keywords = value
            .get(1)
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|item| {
                // Yandex sometimes nests entries as arrays of [type, text, …].
                item.as_str().or_else(|| {
                    item.as_array()
                        .and_then(|entry| entry.get(1))
                        .and_then(Value::as_str)
                })
            })
            .map(FoundKeyword::named)
            .collect();
        Ok(Fetched {
            keywords,
            requests: 1,
            cost_usd: 0.0,
        })
    }
}
