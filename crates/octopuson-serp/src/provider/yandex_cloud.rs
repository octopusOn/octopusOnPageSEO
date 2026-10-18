//! The Yandex Search API (Yandex Cloud): `POST /v2/web/search`, results as base64 XML.

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use octopuson_core::http::HttpRequest;
use serde_json::{Value, json};

use super::{
    Capabilities, CostEstimate, ProviderError, RawPage, SearchContext, SerpFetch, SerpProvider,
};
use crate::locale::{yandex_localization, yandex_region, yandex_search_type};
use crate::template::redact;
use crate::{Device, Engine, MAX_DEPTH, SerpQuery, SnapshotBuilder, yandex_xml};

/// Endpoint of synchronous web searches.
const ENDPOINT: &str = "https://searchapi.api.cloud.yandex.net/v2/web/search";

/// Daytime price of one synchronous request, US dollars (October 2026 price list).
const PER_REQUEST_USD: f64 = 0.004;

/// User agent passed to the API (its documented `userAgent` field) for mobile results.
const MOBILE_AGENT: &str = "Mozilla/5.0 (Linux; Android 14; Pixel 8) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0 Mobile Safari/537.36";

/// The Yandex Search API provider. Needs the secrets `api_key` (an API key with the
/// `yc.search-api.execute` scope) and `folder_id`.
#[derive(Debug, Clone, Copy, Default)]
pub struct YandexCloud;

impl YandexCloud {
    /// Provider id.
    pub const ID: &'static str = "yandex-cloud";

    fn body(query: &SerpQuery, page: u16, folder: &str) -> Value {
        let mut body = json!({
            "query": {
                "searchType": yandex_search_type(&query.country),
                "queryText": query.keyword.trim(),
                "page": (page - 1).to_string(),
                "familyMode": "FAMILY_MODE_MODERATE",
                "fixTypoMode": "FIX_TYPO_MODE_OFF"
            },
            "groupSpec": {
                "groupMode": "GROUP_MODE_FLAT",
                "groupsOnPage": groups_on_page(query).to_string(),
                "docsInGroup": "1"
            },
            "maxPassages": "2",
            "l10n": yandex_localization(&query.language),
            "folderId": folder,
            "responseFormat": "FORMAT_XML"
        });
        if let Some(region) = yandex_region(query) {
            body["region"] = Value::String(region.to_string());
        }
        if query.device == Device::Mobile {
            body["userAgent"] = Value::String(MOBILE_AGENT.to_owned());
        }
        body
    }
}

impl SerpProvider for YandexCloud {
    fn id(&self) -> &str {
        Self::ID
    }

    fn name(&self) -> &'static str {
        "Yandex Search API"
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            engines: vec![Engine::Yandex],
            devices: vec![Device::Desktop, Device::Mobile],
            max_depth: MAX_DEPTH,
            interactive: false,
        }
    }

    fn estimate(&self, query: &SerpQuery) -> CostEstimate {
        let requests = u32::from(requests_for(query));
        CostEstimate {
            requests,
            usd: PER_REQUEST_USD * f64::from(requests),
        }
    }

    fn search(
        &self,
        query: &SerpQuery,
        context: &SearchContext<'_>,
    ) -> Result<SerpFetch, ProviderError> {
        if query.engine != Engine::Yandex {
            return Err(ProviderError::Unsupported(
                "the Yandex Search API only returns Yandex results".to_owned(),
            ));
        }
        query.validate()?;
        let secret = |name: &str| {
            (context.secrets)(Self::ID, name)
                .filter(|value| !value.trim().is_empty())
                .ok_or_else(|| ProviderError::Config(format!("secret `{name}` is not set")))
        };
        let key = secret("api_key")?;
        let folder = secret("folder_id")?;
        let hide = |text: &str| redact(text, std::slice::from_ref(&key));
        let mut builder = SnapshotBuilder::new(query.clone(), Self::ID, context.fetched_at.clone())
            .provider_rev("yandex-cloud@v2")
            .parser_rev(yandex_xml::PARSER_REV);
        let mut pages = Vec::new();
        let mut requests = 0;
        for page in 1..=requests_for(query) {
            if (context.cancelled)() {
                return Err(ProviderError::Cancelled);
            }
            let request = HttpRequest::post(
                ENDPOINT,
                Self::body(query, page, &folder).to_string().into_bytes(),
            )
            .header("Authorization", format!("Api-Key {key}"))
            .header("Content-Type", "application/json");
            let response = match context.transport.send(&request) {
                Ok(response) => response,
                Err(error) if page > 1 => {
                    builder.warn(format!("page {page}: {error}"));
                    break;
                }
                Err(error) => return Err(ProviderError::Network(error.to_string())),
            };
            requests += 1;
            if !response.is_success() {
                let error = classify(&response, &hide);
                if page > 1 {
                    builder.warn(format!("page {page}: {error}"));
                    break;
                }
                return Err(error);
            }
            let envelope: Value =
                serde_json::from_slice(&response.body).map_err(|error| ProviderError::Parse {
                    message: format!("the response is not JSON: {error}"),
                    coverage: None,
                })?;
            let xml = envelope
                .get("rawData")
                .and_then(Value::as_str)
                .and_then(|data| STANDARD.decode(data.trim()).ok())
                .and_then(|bytes| String::from_utf8(bytes).ok())
                .ok_or_else(|| ProviderError::Parse {
                    message: "the response has no base64 `rawData`".to_owned(),
                    coverage: None,
                })?;
            pages.push(RawPage {
                page,
                url: ENDPOINT.to_owned(),
                status: response.status,
                content_type: Some("application/xml".to_owned()),
                body: xml.clone().into_bytes(),
            });
            let parsed = yandex_xml::parse(&xml, page)
                .map_err(|error| ProviderError::Rejected(hide(&error.to_string())))?;
            if builder.add_page(parsed) == 0 || builder.is_full() {
                break;
            }
        }
        Ok(SerpFetch {
            snapshot: builder.finish(),
            pages,
            requests,
            reported_cost_usd: None,
            estimated_cost_usd: PER_REQUEST_USD * f64::from(requests),
        })
    }
}

/// gRPC status codes Yandex Cloud returns in error bodies.
/// Results the API returns per request: up to 100 in XML responses, so a search up to depth
/// 100 is one request (Google, by contrast, serves ten results per page).
const MAX_GROUPS_ON_PAGE: u16 = 100;

fn groups_on_page(query: &SerpQuery) -> u16 {
    query.depth.clamp(1, MAX_GROUPS_ON_PAGE)
}

fn requests_for(query: &SerpQuery) -> u16 {
    query
        .depth
        .clamp(1, MAX_DEPTH)
        .div_ceil(groups_on_page(query))
}

fn classify(
    response: &octopuson_core::http::HttpResponse,
    hide: &dyn Fn(&str) -> String,
) -> ProviderError {
    let body: Option<Value> = serde_json::from_slice(&response.body).ok();
    let code = body
        .as_ref()
        .and_then(|body| body.get("code"))
        .and_then(Value::as_i64);
    let message = body
        .as_ref()
        .and_then(|body| body.get("message"))
        .and_then(Value::as_str)
        .map_or_else(|| hide(&response.text()), hide);
    match (response.status, code) {
        (429, _) => ProviderError::RateLimited {
            message,
            retry_after: response.retry_after(),
        },
        (_, Some(7 | 16)) | (401 | 403, _) => ProviderError::Auth(message),
        (_, Some(8)) => ProviderError::Quota {
            message,
            retry_after: response.retry_after(),
        },
        _ => ProviderError::from_status(response, hide),
    }
}

#[cfg(test)]
mod tests {
    use octopuson_core::http::testing::FakeTransport;

    use super::*;

    const XML: &str = r#"<yandexsearch><response><found priority="all">40</found><results><grouping>
        <group><doc><url>https://a.example.com.tr/</url><title>A</title></doc></group>
        <group><doc><url>https://b.example.com.tr/</url><title>B</title></doc></group>
    </grouping></results></response></yandexsearch>"#;

    fn secrets(provider: &str, name: &str) -> Option<String> {
        (provider == YandexCloud::ID).then(|| format!("{name}-value"))
    }

    fn run(transport: &FakeTransport, query: &SerpQuery) -> Result<SerpFetch, ProviderError> {
        YandexCloud.search(
            query,
            &SearchContext {
                transport,
                secrets: &secrets,
                fetched_at: "2026-10-18T10:00:00Z".to_owned(),
                cancelled: &|| false,
                events: &|_| {},
            },
        )
    }

    #[test]
    fn decodes_base64_xml_and_sends_turkish_parameters() {
        let envelope = json!({"rawData": STANDARD.encode(XML)}).to_string();
        let transport = FakeTransport::new().reply(200, envelope);
        let query = SerpQuery {
            device: Device::Mobile,
            ..SerpQuery::new(Engine::Yandex, "kahve")
        };
        let fetch = run(&transport, &query).unwrap();
        assert_eq!(fetch.snapshot.organic.len(), 2);
        assert_eq!(fetch.snapshot.total_results, Some(40));
        assert_eq!(fetch.snapshot.parser_rev.as_deref(), Some("yandex-xml@1"));
        assert_eq!(fetch.pages[0].body, XML.as_bytes());

        let request = &transport.requests()[0];
        assert!(request.headers.contains(&(
            "Authorization".to_owned(),
            "Api-Key api_key-value".to_owned()
        )));
        let body: Value = serde_json::from_slice(request.body.as_deref().unwrap()).unwrap();
        assert_eq!(body["query"]["searchType"], "SEARCH_TYPE_TR");
        assert_eq!(body["region"], "983");
        assert_eq!(body["l10n"], "LOCALIZATION_TR");
        assert_eq!(body["folderId"], "folder_id-value");
        assert!(body["userAgent"].as_str().unwrap().contains("Mobile"));
    }

    #[test]
    fn asks_for_the_whole_depth_in_one_request() {
        let envelope = json!({"rawData": STANDARD.encode(XML)}).to_string();
        let transport = FakeTransport::new().reply(200, envelope);
        let query = SerpQuery {
            depth: 50,
            ..SerpQuery::new(Engine::Yandex, "kahve")
        };
        assert_eq!(YandexCloud.estimate(&query).requests, 1);
        let fetch = run(&transport, &query).unwrap();
        assert_eq!(fetch.requests, 1);
        let requests = transport.requests();
        assert_eq!(requests.len(), 1);
        let body: Value = serde_json::from_slice(requests[0].body.as_deref().unwrap()).unwrap();
        assert_eq!(body["groupSpec"]["groupsOnPage"], "50");
        assert_eq!(body["query"]["page"], "0");
    }

    #[test]
    fn classifies_cloud_errors_and_hides_the_key() {
        let quota = FakeTransport::new().reply(
            400,
            json!({"code": 8, "message": "quota api_key-value exceeded"}).to_string(),
        );
        match run(&quota, &SerpQuery::new(Engine::Yandex, "x")) {
            Err(ProviderError::Quota { message, .. }) => {
                assert!(!message.contains("api_key-value"));
            }
            other => panic!("expected a quota error, got {other:?}"),
        }
        let auth =
            FakeTransport::new().reply(401, json!({"code": 16, "message": "no"}).to_string());
        assert!(matches!(
            run(&auth, &SerpQuery::new(Engine::Yandex, "x")),
            Err(ProviderError::Auth(_))
        ));
        let wrong = run(&FakeTransport::new(), &SerpQuery::new(Engine::Google, "x"));
        assert!(matches!(wrong, Err(ProviderError::Unsupported(_))));
    }
}
