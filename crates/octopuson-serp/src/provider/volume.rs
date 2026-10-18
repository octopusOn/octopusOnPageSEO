//! Monthly search volumes: Yandex Wordstat and Google Ads data through DataForSEO.

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use octopuson_core::http::HttpRequest;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::{ProviderError, SearchContext, YandexCloud};
use crate::Engine;
use crate::locale::{country_name, yandex_country_region};
use crate::template::redact;

/// Monthly searches of one keyword.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KeywordVolume {
    /// The keyword as asked.
    pub keyword: String,
    /// Searches for exactly this phrase per month, when the source reports it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub monthly: Option<u64>,
    /// Searches containing all its words in any order (Wordstat's total).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub broad: Option<u64>,
    /// Average cost per click in US dollars (Google Ads).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cpc_usd: Option<f64>,
    /// Advertiser competition from 0 to 1 (Google Ads).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub competition: Option<f64>,
    /// Phrases containing the keyword with their monthly searches, most searched first.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub related: Vec<(String, u64)>,
    /// Source id (`wordstat`, `dataforseo-ads`).
    pub source: String,
}

impl KeywordVolume {
    /// The best single monthly figure: exact, else broad.
    pub fn best(&self) -> Option<u64> {
        self.monthly.or(self.broad)
    }
}

/// Where volumes are asked for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VolumeQuery<'a> {
    /// Keywords.
    pub keywords: &'a [String],
    /// ISO country code.
    pub country: &'a str,
    /// ISO language code.
    pub language: &'a str,
    /// Yandex region id narrowing Wordstat, if any.
    pub yandex_region: Option<u32>,
}

/// A source of search volumes.
pub trait VolumeProvider: Send + Sync {
    /// Identifier.
    fn id(&self) -> &str;

    /// Engine whose searches it counts.
    fn engine(&self) -> Engine;

    /// Volumes of the keywords, in the order asked; keywords the source does not know are
    /// left out.
    ///
    /// # Errors
    /// [`ProviderError`] when the source cannot be asked.
    fn volumes(
        &self,
        query: &VolumeQuery<'_>,
        context: &SearchContext<'_>,
    ) -> Result<Vec<KeywordVolume>, ProviderError>;
}

/// Yandex Wordstat through the Yandex Search API (`topRequests`), with the Yandex Cloud
/// provider's `api_key` and `folder_id` secrets. One request per keyword.
#[derive(Debug, Clone, Copy, Default)]
pub struct Wordstat;

impl Wordstat {
    /// Provider id.
    pub const ID: &'static str = "wordstat";
    const ENDPOINT: &'static str = "https://searchapi.api.cloud.yandex.net/v2/wordstat/topRequests";
}

impl VolumeProvider for Wordstat {
    fn id(&self) -> &'static str {
        Self::ID
    }

    fn engine(&self) -> Engine {
        Engine::Yandex
    }

    fn volumes(
        &self,
        query: &VolumeQuery<'_>,
        context: &SearchContext<'_>,
    ) -> Result<Vec<KeywordVolume>, ProviderError> {
        let secret = |name: &str| {
            (context.secrets)(YandexCloud::ID, name)
                .filter(|value| !value.trim().is_empty())
                .ok_or_else(|| ProviderError::Config(format!("secret `{name}` is not set")))
        };
        let key = secret("api_key")?;
        let folder = secret("folder_id")?;
        let region = query
            .yandex_region
            .or_else(|| yandex_country_region(query.country));
        let mut volumes = Vec::with_capacity(query.keywords.len());
        for keyword in query.keywords {
            if (context.cancelled)() {
                return Err(ProviderError::Cancelled);
            }
            let mut body = json!({
                "phrase": keyword.trim(),
                "numPhrases": "50",
                "devices": ["DEVICE_ALL"],
                "folderId": folder,
            });
            if let Some(region) = region {
                body["regions"] = json!([region.to_string()]);
            }
            let request = HttpRequest::post(Self::ENDPOINT, body.to_string().into_bytes())
                .header("Authorization", format!("Api-Key {key}"))
                .header("Content-Type", "application/json");
            let response = context
                .transport
                .send(&request)
                .map_err(|error| ProviderError::Network(error.to_string()))?;
            let hide = |text: &str| redact(text, std::slice::from_ref(&key));
            if !response.is_success() {
                return Err(ProviderError::from_status(&response, &hide));
            }
            let value: Value =
                serde_json::from_slice(&response.body).map_err(|error| ProviderError::Parse {
                    message: format!("Wordstat: {error}"),
                    coverage: None,
                })?;
            volumes.push(read_wordstat(keyword, &value));
        }
        Ok(volumes)
    }
}

fn count(value: &Value) -> Option<u64> {
    value
        .as_u64()
        .or_else(|| value.as_str().and_then(|text| text.trim().parse().ok()))
}

fn read_wordstat(keyword: &str, value: &Value) -> KeywordVolume {
    let wanted = crate::text::squash(keyword).to_lowercase();
    let related: Vec<(String, u64)> = value
        .get("results")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|result| {
            let phrase = result.get("phrase")?.as_str()?;
            Some((phrase.to_owned(), count(result.get("count")?)?))
        })
        .collect();
    let monthly = related
        .iter()
        .find(|(phrase, _)| crate::text::squash(phrase).to_lowercase() == wanted)
        .map(|(_, count)| *count);
    KeywordVolume {
        keyword: keyword.to_owned(),
        monthly,
        broad: value.get("totalCount").and_then(count),
        cpc_usd: None,
        competition: None,
        related,
        source: Wordstat::ID.to_owned(),
    }
}

/// Google Ads volumes through DataForSEO (`keywords_data/google_ads/search_volume/live`),
/// with the DataForSEO provider's `credentials` secret. Up to 1,000 keywords per request.
#[derive(Debug, Clone, Copy, Default)]
pub struct DataForSeoVolume;

impl DataForSeoVolume {
    /// Provider id.
    pub const ID: &'static str = "dataforseo-ads";
    const ENDPOINT: &'static str =
        "https://api.dataforseo.com/v3/keywords_data/google_ads/search_volume/live";
    const BATCH: usize = 1000;
}

impl VolumeProvider for DataForSeoVolume {
    fn id(&self) -> &'static str {
        Self::ID
    }

    fn engine(&self) -> Engine {
        Engine::Google
    }

    fn volumes(
        &self,
        query: &VolumeQuery<'_>,
        context: &SearchContext<'_>,
    ) -> Result<Vec<KeywordVolume>, ProviderError> {
        let credentials = (context.secrets)("dataforseo", "credentials")
            .filter(|value| value.contains(':'))
            .ok_or_else(|| {
                ProviderError::Config("secret `credentials` (login:password) is not set".to_owned())
            })?;
        let authorization = format!("Basic {}", STANDARD.encode(&credentials));
        let hide = |text: &str| redact(text, &[credentials.clone(), authorization.clone()]);
        let location = country_name(query.country).unwrap_or("Turkey");
        let mut volumes = Vec::with_capacity(query.keywords.len());
        for batch in query.keywords.chunks(Self::BATCH) {
            if (context.cancelled)() {
                return Err(ProviderError::Cancelled);
            }
            let body = json!([{
                "keywords": batch,
                "location_name": location,
                "language_code": query.language,
            }]);
            let request = HttpRequest::post(Self::ENDPOINT, body.to_string().into_bytes())
                .header("Authorization", authorization.clone())
                .header("Content-Type", "application/json");
            let response = context
                .transport
                .send(&request)
                .map_err(|error| ProviderError::Network(error.to_string()))?;
            if !response.is_success() {
                return Err(ProviderError::from_status(&response, &hide));
            }
            let value: Value =
                serde_json::from_slice(&response.body).map_err(|error| ProviderError::Parse {
                    message: format!("DataForSEO: {error}"),
                    coverage: None,
                })?;
            let status = value
                .pointer("/tasks/0/status_code")
                .or_else(|| value.get("status_code"))
                .and_then(count);
            if let Some(status) = status.filter(|status| *status != 20_000) {
                let message = value
                    .pointer("/tasks/0/status_message")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                let message = hide(&format!("{status}: {message}"));
                return Err(match status {
                    40_100..=40_104 => ProviderError::Auth(message),
                    40_200 | 40_210 => ProviderError::Quota {
                        message,
                        retry_after: None,
                    },
                    _ => ProviderError::Rejected(message),
                });
            }
            for item in value
                .pointer("/tasks/0/result")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                let Some(keyword) = item.get("keyword").and_then(Value::as_str) else {
                    continue;
                };
                volumes.push(KeywordVolume {
                    keyword: keyword.to_owned(),
                    monthly: item.get("search_volume").and_then(count),
                    broad: None,
                    cpc_usd: item.get("cpc").and_then(Value::as_f64),
                    competition: item
                        .get("competition_index")
                        .and_then(Value::as_f64)
                        .map(|index| index / 100.0),
                    related: Vec::new(),
                    source: Self::ID.to_owned(),
                });
            }
        }
        Ok(volumes)
    }
}

#[cfg(test)]
mod tests {
    use octopuson_core::http::testing::FakeTransport;

    use super::*;

    fn context(transport: &FakeTransport) -> SearchContext<'_> {
        SearchContext {
            transport,
            secrets: &|provider, name| match (provider, name) {
                ("yandex-cloud", "api_key") => Some("key-1234".to_owned()),
                ("yandex-cloud", "folder_id") => Some("folder".to_owned()),
                ("dataforseo", "credentials") => Some("me:pw".to_owned()),
                _ => None,
            },
            fetched_at: "2026-10-18T10:00:00Z".to_owned(),
            cancelled: &|| false,
            events: &|_| {},
        }
    }

    #[test]
    fn reads_wordstat_totals_exact_counts_and_related_phrases() {
        let transport = FakeTransport::new().reply(
            200,
            json!({"totalCount": "52000", "results": [
                {"phrase": "türk kahvesi", "count": "30000"},
                {"phrase": "türk kahvesi nasıl yapılır", "count": "9000"}
            ]})
            .to_string(),
        );
        let keywords = vec!["Türk Kahvesi".to_owned()];
        let volumes = Wordstat
            .volumes(
                &VolumeQuery {
                    keywords: &keywords,
                    country: "tr",
                    language: "tr",
                    yandex_region: None,
                },
                &context(&transport),
            )
            .unwrap();
        assert_eq!(volumes[0].broad, Some(52_000));
        assert_eq!(volumes[0].monthly, Some(30_000));
        assert_eq!(volumes[0].best(), Some(30_000));
        assert_eq!(volumes[0].related.len(), 2);
        let body: Value =
            serde_json::from_slice(transport.requests()[0].body.as_deref().unwrap()).unwrap();
        assert_eq!(body["regions"], json!(["983"]));
        assert_eq!(body["folderId"], "folder");
    }

    #[test]
    fn reads_google_ads_volumes_and_errors() {
        let ok = json!({"status_code": 20000, "tasks": [{"status_code": 20000, "result": [
            {"keyword": "kahve", "search_volume": 74000, "cpc": 0.12, "competition_index": 35}
        ]}]});
        let transport = FakeTransport::new().reply(200, ok.to_string()).reply(
            200,
            json!({"tasks": [{"status_code": 40210, "status_message": "Insufficient"}]})
                .to_string(),
        );
        let keywords = vec!["kahve".to_owned()];
        let query = VolumeQuery {
            keywords: &keywords,
            country: "tr",
            language: "tr",
            yandex_region: None,
        };
        let volumes = DataForSeoVolume
            .volumes(&query, &context(&transport))
            .unwrap();
        assert_eq!(volumes[0].monthly, Some(74_000));
        assert_eq!(volumes[0].competition, Some(0.35));
        assert!(matches!(
            DataForSeoVolume.volumes(&query, &context(&transport)),
            Err(ProviderError::Quota { .. })
        ));
        let body: Value =
            serde_json::from_slice(transport.requests()[0].body.as_deref().unwrap()).unwrap();
        assert_eq!(body[0]["location_name"], "Turkey");
    }
}
