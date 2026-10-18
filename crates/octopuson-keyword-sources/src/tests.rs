use octopuson_core::http::testing::FakeTransport;
use serde_json::{Value, json};

use crate::dataforseo::DataForSeo;
use crate::serpapi::SerpApi;
use crate::wordstat::Wordstat;
use crate::{Market, SourceContext, SourceError};

fn secrets(provider: &str, name: &str) -> Option<String> {
    match (provider, name) {
        ("yandex-cloud", "api_key") => Some("yc-secret-key".to_owned()),
        ("yandex-cloud", "folder_id") => Some("b1gfolder".to_owned()),
        ("dataforseo", "credentials") => Some("me@example.com:pass-1234".to_owned()),
        ("serpapi", "api_key") => Some("serp-secret-key".to_owned()),
        _ => None,
    }
}

fn context(transport: &FakeTransport) -> SourceContext<'_> {
    SourceContext {
        transport,
        secrets: &secrets,
    }
}

fn body(transport: &FakeTransport, index: usize) -> Value {
    serde_json::from_slice(transport.requests()[index].body.as_deref().unwrap()).unwrap()
}

fn tr() -> Market {
    Market::new("tr", "tr")
}

#[test]
fn wordstat_returns_top_requests_and_associations() {
    let transport = FakeTransport::new().reply(
        200,
        json!({
            "totalCount": "52000",
            "results": [
                {"phrase": "türk kahvesi", "count": "30000"},
                {"phrase": "türk kahvesi nasıl yapılır", "count": 9000}
            ],
            "associations": [{"phrase": "dibek kahvesi", "count": "4100"}]
        })
        .to_string(),
    );
    let fetched =
        Wordstat::top_requests(&context(&transport), " türk kahvesi ", &tr(), 5_000).unwrap();
    let found: Vec<_> = fetched
        .keywords
        .iter()
        .map(|keyword| (keyword.keyword.as_str(), keyword.volume))
        .collect();
    assert_eq!(
        found,
        [
            ("türk kahvesi", Some(30_000)),
            ("türk kahvesi nasıl yapılır", Some(9_000)),
            ("dibek kahvesi", Some(4_100)),
        ]
    );
    assert_eq!(fetched.requests, 1);
    let request = &transport.requests()[0];
    assert_eq!(
        request.url,
        "https://searchapi.api.cloud.yandex.net/v2/wordstat/topRequests"
    );
    assert!(request.headers.contains(&(
        "Authorization".to_owned(),
        "Api-Key yc-secret-key".to_owned()
    )));
    let sent = body(&transport, 0);
    assert_eq!(sent["phrase"], "türk kahvesi");
    assert_eq!(sent["numPhrases"], "2000");
    assert_eq!(sent["regions"], json!(["983"]));
    assert_eq!(sent["folderId"], "b1gfolder");
}

#[test]
fn wordstat_reads_dynamics_regions_and_refusals() {
    let transport = FakeTransport::new()
        .reply(
            200,
            json!({"results": [
                {"date": "2026-08-01T00:00:00Z", "count": "800", "share": 0.001},
                {"date": "2026-09-01T00:00:00Z", "count": "950", "share": 0.001}
            ]})
            .to_string(),
        )
        .reply(
            200,
            json!({"regions": [{"id": "983", "label": "Türkiye", "children": [
                {"id": "11508", "label": "İstanbul"}
            ]}]})
            .to_string(),
        )
        .reply_with_headers(
            429,
            vec![("Retry-After", "60")],
            "RESOURCE_EXHAUSTED yc-secret-key",
        );
    let mut market = tr();
    market.yandex_region = Some(11_508);
    let (monthly, _) =
        Wordstat::dynamics(&context(&transport), "kahve", &market, "2026-08", "2026-09").unwrap();
    assert_eq!(monthly.len(), 2);
    assert_eq!(monthly[1].month, "2026-09");
    assert_eq!(monthly[1].volume, 950);
    let sent = body(&transport, 0);
    assert_eq!(sent["fromDate"], "2026-08-01T00:00:00Z");
    assert_eq!(sent["toDate"], "2026-09-30T23:59:59Z");
    assert_eq!(sent["regions"], json!(["11508"]));

    let regions = Wordstat::regions_tree(&context(&transport)).unwrap();
    let all: Vec<_> = regions[0].walk().iter().map(|region| region.id).collect();
    assert_eq!(all, [983, 11_508]);

    let error = Wordstat::top_requests(&context(&transport), "kahve", &tr(), 10).unwrap_err();
    assert_eq!(
        error,
        SourceError::RateLimited {
            retry_after: Some(60)
        }
    );
    assert!(error.is_temporary());

    let none = SourceContext {
        transport: &transport,
        secrets: &|_, _| None,
    };
    assert!(matches!(
        Wordstat::top_requests(&none, "kahve", &tr(), 10),
        Err(SourceError::Config(_))
    ));
}

#[test]
fn dataforseo_reads_google_ads_numbers_and_hides_credentials() {
    let transport = FakeTransport::new()
        .reply(
            200,
            json!({"status_code": 20000, "cost": 0.075, "tasks": [{"status_code": 20000, "cost": 0.075, "result": [{
                "keyword": "kahve makinesi", "search_volume": 90500, "cpc": 0.81, "competition_index": 100,
                "monthly_searches": [
                    {"year": 2026, "month": 9, "search_volume": 110_000},
                    {"year": 2026, "month": 8, "search_volume": 90500}
                ]
            }]}]})
            .to_string(),
        )
        .reply(
            200,
            json!({"status_code": 20000, "tasks": [{"status_code": 40210, "status_message": "Insufficient funds."}]})
                .to_string(),
        )
        .reply(401, "bad credentials me@example.com:pass-1234");
    let keywords = vec!["kahve makinesi".to_owned()];
    let fetched = DataForSeo::search_volume(&context(&transport), &keywords, &tr()).unwrap();
    let keyword = &fetched.keywords[0];
    assert_eq!(keyword.volume, Some(90_500));
    assert_eq!(keyword.competition, Some(1.0));
    assert_eq!(keyword.monthly[0].month, "2026-08");
    assert!((fetched.cost_usd - 0.075).abs() < 1e-9);
    let sent = body(&transport, 0);
    assert_eq!(sent[0]["location_name"], "Turkey");
    assert_eq!(sent[0]["language_code"], "tr");

    assert!(matches!(
        DataForSeo::search_volume(&context(&transport), &keywords, &tr()),
        Err(SourceError::Quota { .. })
    ));
    let error = DataForSeo::search_volume(&context(&transport), &keywords, &tr()).unwrap_err();
    assert!(matches!(&error, SourceError::Auth(message) if !message.contains("pass-1234")));
}

#[test]
fn dataforseo_reads_labs_items_rankings_difficulty_and_intent() {
    let item = |keyword: &str| {
        json!({
            "keyword": keyword,
            "keyword_info": {"search_volume": 1300, "cpc": 0.4, "competition": 0.62},
            "keyword_properties": {"keyword_difficulty": 37},
            "search_intent_info": {"main_intent": "commercial", "foreign_intent": ["transactional"]}
        })
    };
    let transport = FakeTransport::new()
        .reply(
            200,
            json!({"tasks": [{"status_code": 20000, "result": [{"items": [
                {"keyword_data": item("en iyi kahve makinesi"), "depth": 1}
            ]}]}]})
            .to_string(),
        )
        .reply(
            200,
            json!({"tasks": [{"status_code": 20000, "result": [{"items": [{
                "keyword_data": item("kahve makinesi"),
                "ranked_serp_element": {"serp_item": {"rank_group": 3, "url": "https://rakip.com/kahve"}}
            }]}]}]})
            .to_string(),
        )
        .reply(
            200,
            json!({"tasks": [{"status_code": 20000, "result": [{"items": [
                {"keyword": "kahve makinesi", "keyword_difficulty": 54},
                {"keyword": "bilinmeyen", "keyword_difficulty": null}
            ]}]}]})
            .to_string(),
        )
        .reply(
            200,
            json!({"tasks": [{"status_code": 20000, "result": [{"items": [{
                "keyword": "kahve makinesi",
                "keyword_intent": {"label": "commercial", "probability": 0.8},
                "secondary_keyword_intents": [{"label": "transactional", "probability": 0.3}]
            }]}]}]})
            .to_string(),
        );
    let related =
        DataForSeo::related_keywords(&context(&transport), "kahve makinesi", &tr(), 9, 50).unwrap();
    let keyword = &related.keywords[0];
    assert_eq!(keyword.keyword, "en iyi kahve makinesi");
    assert_eq!(keyword.difficulty, Some(37.0));
    assert_eq!(keyword.intent.as_deref(), Some("commercial"));
    assert_eq!(keyword.secondary_intent.as_deref(), Some("transactional"));
    assert!((related.cost_usd - DataForSeo::labs_estimate(1)).abs() < 1e-9);
    let sent = body(&transport, 0);
    assert_eq!(sent[0]["depth"], 4);
    assert_eq!(sent[0]["limit"], 50);
    assert!(
        transport.requests()[0]
            .url
            .ends_with("/v3/dataforseo_labs/google/related_keywords/live")
    );

    let ranked =
        DataForSeo::ranked_keywords(&context(&transport), "rakip.com", &tr(), 100).unwrap();
    assert_eq!(
        ranked.keywords[0].url.as_deref(),
        Some("https://rakip.com/kahve")
    );
    assert_eq!(ranked.keywords[0].position, Some(3));
    assert_eq!(body(&transport, 1)[0]["target"], "rakip.com");

    let keywords = vec!["kahve makinesi".to_owned(), "bilinmeyen".to_owned()];
    let difficulty =
        DataForSeo::keyword_difficulty(&context(&transport), &keywords, &tr()).unwrap();
    assert_eq!(difficulty.keywords.len(), 1);
    assert_eq!(difficulty.keywords[0].difficulty, Some(54.0));

    let intent = DataForSeo::search_intent(&context(&transport), &keywords, &tr()).unwrap();
    assert_eq!(intent.keywords[0].intent.as_deref(), Some("commercial"));
    assert_eq!(body(&transport, 3)[0].get("location_name"), None);
}

#[test]
fn autocomplete_through_dataforseo_and_serpapi() {
    let transport = FakeTransport::new()
        .reply(
            200,
            json!({"tasks": [{"status_code": 20000, "result": [{"items": [
                {"type": "autocomplete", "suggestion": "kahve makinesi fiyatları"},
                {"type": "autocomplete", "suggestion": "kahve makinesi tavsiye"}
            ]}]}]})
            .to_string(),
        )
        .reply(
            200,
            json!({"suggestions": [{"value": "kahve falı"}, {"value": "kahve makinesi"}]})
                .to_string(),
        )
        .reply(
            200,
            json!({"error": "Invalid API key serp-secret-key."}).to_string(),
        );
    let paid = DataForSeo::autocomplete(&context(&transport), "kahve makinesi", &tr()).unwrap();
    assert_eq!(paid.keywords.len(), 2);
    let serp = SerpApi::autocomplete(&context(&transport), "kahve", &tr()).unwrap();
    assert_eq!(serp.keywords[0].keyword, "kahve falı");
    let url = &transport.requests()[1].url;
    assert!(url.starts_with(
        "https://serpapi.com/search.json?engine=google_autocomplete&q=kahve&gl=tr&hl=tr"
    ));
    let error = SerpApi::autocomplete(&context(&transport), "kahve", &tr()).unwrap_err();
    assert!(
        matches!(&error, SourceError::Rejected(message) if !message.contains("serp-secret-key"))
    );
}

#[test]
fn knows_the_codes_google_apis_want() {
    use crate::countries::{alpha3, geo_target, language_constant};
    assert_eq!(geo_target("tr"), Some(2_792));
    assert_eq!(geo_target("us"), Some(2_840));
    assert_eq!(alpha3("de"), Some("deu"));
    assert_eq!(language_constant("tr"), Some(1_037));
    assert_eq!(language_constant("xx"), None);
}

#[test]
fn search_console_lists_properties_and_pages_of_queries() {
    use crate::search_console::{QueryRequest, SearchConsole};
    let transport = FakeTransport::new()
        .reply(
            200,
            json!({"siteEntry": [
                {"siteUrl": "sc-domain:ornek.com.tr", "permissionLevel": "siteOwner"},
                {"siteUrl": "https://baska.com/", "permissionLevel": "siteUnverifiedUser"}
            ]})
            .to_string(),
        )
        .reply(
            200,
            json!({"rows": [{"keys": ["türk kahvesi", "https://ornek.com.tr/kahve"],
                "clicks": 12, "impressions": 340, "ctr": 0.035, "position": 4.2}]})
            .to_string(),
        )
        .reply(401, "invalid token ya29.token");
    let properties = SearchConsole::properties(&context(&transport), "ya29.token").unwrap();
    assert_eq!(properties.len(), 1);
    assert_eq!(properties[0].site_url, "sc-domain:ornek.com.tr");
    let request = QueryRequest {
        site_url: "sc-domain:ornek.com.tr",
        start: "2026-07-17",
        end: "2026-10-15",
        country: Some("tr"),
        start_row: 25_000,
    };
    let rows = SearchConsole::query(&context(&transport), "ya29.token", &request).unwrap();
    assert_eq!(rows[0].query, "türk kahvesi");
    assert!((rows[0].position - 4.2).abs() < 1e-9);
    let sent = &transport.requests()[1];
    assert_eq!(
        sent.url,
        "https://www.googleapis.com/webmasters/v3/sites/sc-domain%3Aornek.com.tr/searchAnalytics/query"
    );
    let body = body(&transport, 1);
    assert_eq!(body["startRow"], 25_000);
    assert_eq!(body["rowLimit"], 25_000);
    assert_eq!(
        body["dimensionFilterGroups"][0]["filters"][0]["expression"],
        "tur"
    );
    let error = SearchConsole::query(&context(&transport), "ya29.token", &request).unwrap_err();
    assert!(matches!(&error, SourceError::Auth(message) if !message.contains("ya29")));
}

#[test]
fn google_ads_reads_ideas_with_their_metrics() {
    use crate::google_ads::{AdsAccount, GoogleAds};
    let transport = FakeTransport::new()
        .reply(
            200,
            json!({"results": [{
                "text": "türk kahvesi makinesi",
                "keywordIdeaMetrics": {
                    "avgMonthlySearches": "8100",
                    "competition": "HIGH",
                    "competitionIndex": "81",
                    "averageCpcMicros": "1500000",
                    "monthlySearchVolumes": [
                        {"month": "SEPTEMBER", "year": "2026", "monthlySearches": "9900"},
                        {"month": "AUGUST", "year": "2026", "monthlySearches": "8100"}
                    ]
                }
            }, {"text": "kahve"}]})
            .to_string(),
        )
        .reply(403, "PERMISSION_DENIED for ya29.token");
    let account = AdsAccount {
        token: "ya29.token",
        customer_id: "123-456-7890",
        login_customer_id: Some("999-000-1111"),
        developer_token: None,
    };
    let seeds = vec!["türk kahvesi".to_owned()];
    let fetched = GoogleAds::keyword_ideas(&context(&transport), &account, &seeds, &tr()).unwrap();
    let idea = &fetched.keywords[0];
    assert_eq!(idea.volume, Some(8_100));
    assert_eq!(idea.competition, Some(0.81));
    assert_eq!(idea.cpc_usd, Some(1.5));
    assert_eq!(idea.monthly[0].month, "2026-08");
    assert_eq!(fetched.keywords[1].volume, None);
    let request = &transport.requests()[0];
    assert_eq!(
        request.url,
        "https://googleads.googleapis.com/v26/customers/1234567890:generateKeywordIdeas"
    );
    assert!(
        request
            .headers
            .contains(&("login-customer-id".to_owned(), "9990001111".to_owned()))
    );
    assert!(
        !request
            .headers
            .iter()
            .any(|(name, _)| name == "developer-token")
    );
    let sent = body(&transport, 0);
    assert_eq!(
        sent["geoTargetConstants"],
        json!(["geoTargetConstants/2792"])
    );
    assert_eq!(sent["language"], "languageConstants/1037");
    let error =
        GoogleAds::keyword_ideas(&context(&transport), &account, &seeds, &tr()).unwrap_err();
    assert!(matches!(&error, SourceError::Auth(message) if !message.contains("ya29")));
    let bad = AdsAccount {
        customer_id: "12",
        ..account
    };
    assert!(matches!(
        GoogleAds::keyword_ideas(&context(&transport), &bad, &seeds, &tr()),
        Err(SourceError::Config(_))
    ));
}

#[test]
fn free_suggestions_stop_at_the_first_refusal() {
    use crate::suggest::FreeSuggest;
    let transport = FakeTransport::new()
        .reply(200, r#"["kahve",["kahve falı","kahve makinesi"]]"#)
        .reply(
            200,
            r#"["kahve",["kahve fincanı",["fulltext","kahve çekirdeği"]]]"#,
        )
        .reply(429, "Too many requests")
        .reply(200, "<html>unusual traffic</html>");
    let google = FreeSuggest::google(&context(&transport), "kahve", &tr()).unwrap();
    assert_eq!(google.keywords.len(), 2);
    assert_eq!(google.cost_usd, 0.0);
    let url = &transport.requests()[0].url;
    assert!(url.starts_with(
        "https://suggestqueries.google.com/complete/search?client=firefox&hl=tr&gl=tr&q=kahve"
    ));
    let yandex = FreeSuggest::yandex(&context(&transport), "kahve", &tr()).unwrap();
    assert_eq!(yandex.keywords[1].keyword, "kahve çekirdeği");
    assert!(
        transport.requests()[1]
            .url
            .starts_with("https://suggest.yandex.com.tr/suggest-ff.cgi?part=kahve")
    );
    for _ in 0..2 {
        let error = FreeSuggest::google(&context(&transport), "kahve", &tr()).unwrap_err();
        assert!(matches!(error, SourceError::Quota { .. }));
    }
}
