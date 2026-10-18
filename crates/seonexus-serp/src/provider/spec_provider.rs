//! Providers described by a [`ProviderSpec`]: the built-in API specs and the user's own.

use seonexus_core::http::{HttpMethod, HttpRequest};
use serde_json::Value;
use url::Url;

use super::{
    Capabilities, CostEstimate, ProviderError, RawPage, SearchContext, SerpFetch, SerpProvider,
};
use crate::mapping::Format;
use crate::mapping::ParsedPage;
use crate::snapshot::LOW_COVERAGE;
use crate::spec::{CompiledSpec, Method, ProviderSpec, RenderedRequest, StatusOutcome};
use crate::{Device, MAX_DEPTH, SerpError, SerpQuery, SnapshotBuilder};

/// Specs of the built-in API providers: Serper, `SerpApi` and `DataForSEO`.
///
/// # Errors
/// [`SerpError::InvalidSpec`] if an embedded spec is invalid (a build defect the tests catch).
pub fn builtin_specs() -> Result<Vec<ProviderSpec>, SerpError> {
    [
        include_str!("specs/serper.json"),
        include_str!("specs/serpapi.json"),
        include_str!("specs/dataforseo.json"),
    ]
    .into_iter()
    .map(|source| {
        serde_json::from_str(source)
            .map_err(|error| SerpError::InvalidSpec(format!("built-in provider spec: {error}")))
    })
    .collect()
}

/// A provider running a compiled spec.
#[derive(Debug, Clone)]
pub struct SpecProvider {
    spec: CompiledSpec,
}

impl SpecProvider {
    /// Wraps a compiled spec.
    pub fn new(spec: CompiledSpec) -> Self {
        Self { spec }
    }

    /// The compiled spec.
    pub fn spec(&self) -> &CompiledSpec {
        &self.spec
    }
}

impl SerpProvider for SpecProvider {
    fn id(&self) -> &str {
        &self.spec.spec().id
    }

    fn name(&self) -> &str {
        &self.spec.spec().name
    }

    fn capabilities(&self) -> Capabilities {
        let spec = self.spec.spec();
        let paging = spec.paging;
        Capabilities {
            engines: spec.engines.clone(),
            devices: if spec.devices.is_empty() {
                vec![Device::Desktop, Device::Mobile]
            } else {
                spec.devices.clone()
            },
            max_depth: paging.max_pages.map_or(MAX_DEPTH, |pages| {
                paging.per_page.saturating_mul(pages).min(MAX_DEPTH)
            }),
            interactive: false,
        }
    }

    fn estimate(&self, query: &SerpQuery) -> CostEstimate {
        CostEstimate {
            requests: u32::from(self.spec.pages(query)),
            usd: self.spec.estimate_usd(query),
        }
    }

    fn search(
        &self,
        query: &SerpQuery,
        context: &SearchContext<'_>,
    ) -> Result<SerpFetch, ProviderError> {
        if !self.spec.supports(query) {
            return Err(ProviderError::Unsupported(format!(
                "{} does not return {} {} results",
                self.name(),
                query.device.as_str(),
                query.engine
            )));
        }
        let id = self.id().to_owned();
        let secrets = |name: &str| (context.secrets)(&id, name);
        let mut builder =
            SnapshotBuilder::new(query.clone(), id.clone(), context.fetched_at.clone())
                .provider_rev(self.spec.revision());
        let mut pages = Vec::new();
        let mut reported: Option<f64> = None;
        let mut requests = 0;

        for page in 1..=self.spec.pages(query) {
            if (context.cancelled)() {
                return Err(ProviderError::Cancelled);
            }
            let request = self.spec.render(query, page, &secrets)?;
            let http = HttpRequest {
                method: match request.method {
                    Method::Get => HttpMethod::Get,
                    Method::Post => HttpMethod::Post,
                },
                url: request.url.to_string(),
                headers: request.headers.clone(),
                body: request.body.clone(),
                timeout: None,
            };
            let response = match context.transport.send(&http) {
                Ok(response) => response,
                Err(error) if page > 1 => {
                    builder.warn(format!("page {page}: {error}"));
                    break;
                }
                Err(error) => return Err(ProviderError::Network(error.to_string())),
            };
            requests += 1;
            pages.push(RawPage {
                page,
                url: request.redacted_url(),
                status: response.status,
                content_type: response.header("content-type").map(str::to_owned),
                body: response.body.clone(),
            });
            if !response.is_success() {
                let error = ProviderError::from_status(&response, &|text| request.redact(text));
                if page > 1 {
                    builder.warn(format!("page {page}: {error}"));
                    break;
                }
                return Err(error);
            }
            let url = Url::parse(&response.url).unwrap_or_else(|_| request.url.clone());
            let (parsed, cost) = self.read_page(query, page, &response.text(), &url, &request)?;
            if let Some(cost) = cost {
                *reported.get_or_insert(0.0) += cost;
            }
            let added = builder.add_page(parsed);
            if added == 0 || builder.is_full() {
                break;
            }
        }
        let estimated = self
            .spec
            .spec()
            .cost
            .map_or(0.0, |cost| cost.per_request_usd * f64::from(requests));
        Ok(SerpFetch {
            snapshot: builder.finish(),
            pages,
            requests,
            reported_cost_usd: reported,
            estimated_cost_usd: estimated,
        })
    }
}

impl SpecProvider {
    /// Reads one successful response: status codes and cost inside JSON bodies, results,
    /// challenges and errors of an empty first page.
    fn read_page(
        &self,
        query: &SerpQuery,
        page: u16,
        text: &str,
        url: &Url,
        request: &RenderedRequest,
    ) -> Result<(ParsedPage, Option<f64>), ProviderError> {
        let json = self
            .spec
            .mapping(query.engine)
            .is_some_and(|mapping| mapping.format() == Format::Json);
        let mut cost = None;
        if json && let Ok(value) = serde_json::from_str::<Value>(text) {
            check_status(&self.spec, &value, request)?;
            cost = self.spec.usage_of(&value).cost_usd;
        }
        let parsed = self.spec.parse(query.engine, text, url, page)?;
        if let Some(challenge) = parsed.challenge {
            return Err(ProviderError::Blocked(challenge));
        }
        if page == 1 && parsed.organic.is_empty() {
            if let Some(error) = &parsed.error {
                return Err(ProviderError::Rejected(request.redact(error)));
            }
            if parsed.coverage.matched > 0 && parsed.coverage.ratio() < LOW_COVERAGE {
                return Err(ProviderError::Parse {
                    message: "no matched result could be read; the mapping may be outdated"
                        .to_owned(),
                    coverage: Some(parsed.coverage.ratio()),
                });
            }
        }
        Ok((parsed, cost))
    }
}

fn check_status(
    spec: &CompiledSpec,
    body: &Value,
    request: &RenderedRequest,
) -> Result<(), ProviderError> {
    match spec.status_of(body) {
        StatusOutcome::Ok => Ok(()),
        StatusOutcome::Auth(code) => Err(ProviderError::Auth(request.redact(&code))),
        StatusOutcome::Quota(code) => Err(ProviderError::Quota {
            message: request.redact(&code),
            retry_after: None,
        }),
        StatusOutcome::RateLimited(code) => Err(ProviderError::RateLimited {
            message: request.redact(&code),
            retry_after: None,
        }),
        StatusOutcome::Error(code) => Err(ProviderError::Rejected(request.redact(&code))),
    }
}

#[cfg(test)]
mod tests {
    use seonexus_core::http::TransportError;
    use seonexus_core::http::testing::FakeTransport;
    use serde_json::json;

    use super::*;
    use crate::{Engine, FeatureKind};

    fn context<'a>(
        transport: &'a FakeTransport,
        secrets: &'a dyn Fn(&str, &str) -> Option<String>,
    ) -> SearchContext<'a> {
        SearchContext {
            transport,
            secrets,
            fetched_at: "2026-10-18T10:00:00Z".to_owned(),
            cancelled: &|| false,
            events: &|_| {},
        }
    }

    fn builtin(id: &str) -> SpecProvider {
        let spec = builtin_specs()
            .unwrap()
            .into_iter()
            .find(|spec| spec.id == id)
            .unwrap();
        SpecProvider::new(spec.compile().unwrap())
    }

    fn keys(provider: &str, name: &str) -> Option<String> {
        (!name.is_empty()).then(|| format!("{provider}-{name}-value"))
    }

    #[test]
    fn builtin_specs_compile() {
        let specs = builtin_specs().unwrap();
        assert_eq!(specs.len(), 3);
        for spec in specs {
            spec.compile().unwrap();
        }
    }

    #[test]
    fn serper_pages_until_the_depth_and_reports_credits() {
        let page = |offset: u16| {
            json!({
                "organic": (1..=10).map(|i| json!({
                    "title": format!("R{}", offset + i), "link": format!("https://r{}.example/", offset + i),
                    "position": i
                })).collect::<Vec<_>>(),
                "peopleAlsoAsk": [{"question": "Ne?"}],
                "credits": 1
            })
            .to_string()
        };
        let transport = FakeTransport::new()
            .reply(200, page(0))
            .reply(200, page(10));
        let provider = builtin("serper");
        let query = SerpQuery::new(Engine::Google, "kahve").with_depth(20);
        assert_eq!(provider.estimate(&query).requests, 2);
        let fetch = provider
            .search(&query, &context(&transport, &keys))
            .unwrap();
        assert_eq!(fetch.snapshot.organic.len(), 20);
        assert_eq!(fetch.snapshot.organic[19].position, 20);
        assert_eq!(fetch.snapshot.organic[19].page, 2);
        assert_eq!(fetch.requests, 2);
        assert_eq!(fetch.reported_cost_usd, Some(0.002));
        assert!(fetch.snapshot.has_feature(FeatureKind::PeopleAlsoAsk));
        assert_eq!(fetch.snapshot.provider_rev.as_deref(), Some("serper@v1"));

        let requests = transport.requests();
        assert_eq!(requests[0].url, "https://google.serper.dev/search");
        assert!(
            requests[0]
                .headers
                .contains(&("X-API-KEY".to_owned(), "serper-api_key-value".to_owned()))
        );
        let body: Value = serde_json::from_slice(requests[1].body.as_deref().unwrap()).unwrap();
        assert_eq!(body["page"], json!(2));
        assert_eq!(body.get("location"), None);
    }

    #[test]
    fn serpapi_builds_engine_specific_queries_and_redacts_keys() {
        let body = json!({"organic_results": [{"position": 1, "title": "A", "link": "https://a.example/"}],
            "ads": [{"block_position": "top", "title": "Ad", "link": "https://ad.example/"}],
            "search_information": {"total_results": 5000}});
        let transport = FakeTransport::new().reply(200, body.to_string());
        let provider = builtin("serpapi");
        let fetch = provider
            .search(
                &SerpQuery::new(Engine::Yandex, "kahve"),
                &context(&transport, &keys),
            )
            .unwrap();
        assert_eq!(fetch.snapshot.organic.len(), 1);
        assert_eq!(fetch.snapshot.top_ad_count(), 1);
        assert_eq!(fetch.snapshot.total_results, Some(5000));
        let url = Url::parse(&transport.requests()[0].url).unwrap();
        let pairs: Vec<(String, String)> = url.query_pairs().into_owned().collect();
        let has = |key: &str, value: &str| pairs.contains(&(key.to_owned(), value.to_owned()));
        assert!(has("engine", "yandex") && has("text", "kahve") && has("lr", "983"));
        assert!(has("yandex_domain", "yandex.com.tr") && has("p", "0"));
        assert!(!pairs.iter().any(|(key, _)| key == "q" || key == "location"));
        assert!(fetch.pages[0].url.contains("api_key=***"));
    }

    #[test]
    fn classifies_failures() {
        let provider = builtin("dataforseo");
        let query = SerpQuery::new(Engine::Google, "kahve");
        let run = |transport: FakeTransport| provider.search(&query, &context(&transport, &keys));

        let unauthorised = run(FakeTransport::new().reply(401, "nope"));
        assert!(matches!(unauthorised, Err(ProviderError::Auth(_))));
        let limited =
            run(FakeTransport::new().reply_with_headers(429, vec![("Retry-After", "9")], ""));
        assert!(matches!(
            limited,
            Err(ProviderError::RateLimited {
                retry_after: Some(9),
                ..
            })
        ));
        let broke = run(FakeTransport::new().reply(
            200,
            json!({"status_code": 20000, "tasks": [{"status_code": 40210, "status_message": "Insufficient funds."}]})
                .to_string(),
        ));
        assert!(matches!(broke, Err(ProviderError::Quota { .. })));
        let down = run(FakeTransport::new().fail(TransportError::Timeout));
        assert!(matches!(down, Err(ProviderError::Network(_))));
        let missing = provider.search(&query, &context(&FakeTransport::new(), &|_, _| None));
        assert!(matches!(missing, Err(ProviderError::Config(_))));
        let mobile_bing = SerpQuery {
            device: Device::Mobile,
            ..SerpQuery::new(Engine::Bing, "x")
        };
        assert!(
            builtin("serper")
                .search(&mobile_bing, &context(&FakeTransport::new(), &keys))
                .is_err()
        );
    }

    #[test]
    fn dataforseo_reads_typed_items_and_reported_cost() {
        let body = json!({"status_code": 20000, "cost": 0.003, "tasks": [{"status_code": 20000, "result": [{
        "se_results_count": 12000,
        "items": [
            {"type": "paid", "title": "Ad", "url": "https://ad.example/"},
            {"type": "organic", "rank_group": 1, "rank_absolute": 2, "title": "A", "url": "https://a.example/",
                "description": "desc", "links": [{"title": "Alt", "url": "https://a.example/alt"}]},
            {"type": "people_also_ask", "items": [{"title": "Ne?"}, {"title": "Nasıl?"}]},
            {"type": "organic", "rank_group": 2, "rank_absolute": 4, "title": "B", "url": "https://b.example/"},
            {"type": "related_searches", "items": ["kahve fiyat", "türk kahvesi"]}
        ]}]}]});
        let transport = FakeTransport::new().reply(200, body.to_string());
        let provider = builtin("dataforseo");
        let fetch = provider
            .search(
                &SerpQuery::new(Engine::Google, "kahve").with_depth(20),
                &context(&transport, &keys),
            )
            .unwrap();
        assert_eq!(fetch.requests, 1);
        assert_eq!(fetch.reported_cost_usd, Some(0.003));
        let snapshot = &fetch.snapshot;
        assert_eq!(snapshot.organic.len(), 2);
        assert_eq!(snapshot.organic[0].absolute_position, Some(2));
        assert_eq!(snapshot.organic[0].sitelinks.len(), 1);
        let related: Vec<_> = snapshot
            .features_of(FeatureKind::RelatedSearches)
            .flat_map(|feature| feature.items.iter().filter_map(|item| item.text.clone()))
            .collect();
        assert_eq!(related, ["kahve fiyat", "türk kahvesi"]);
        let paa = snapshot
            .features_of(FeatureKind::PeopleAlsoAsk)
            .next()
            .unwrap();
        assert_eq!(paa.items.len(), 2);
        let request = &transport.requests()[0];
        assert_eq!(
            request.url,
            "https://api.dataforseo.com/v3/serp/google/organic/live/advanced"
        );
        assert!(
            request
                .headers
                .iter()
                .any(|(name, value)| name == "Authorization" && value.starts_with("Basic "))
        );
    }
}
