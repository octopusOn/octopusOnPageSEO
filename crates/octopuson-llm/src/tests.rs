use octopuson_core::http::testing::FakeTransport;
use serde_json::{Value, json};

use crate::analysis::{Brand, analyze, fold};
use crate::{Citation, Engine, LlmError, Question, ask, domain_of};

fn question(model: &str) -> Question<'_> {
    Question {
        prompt: "  Türkiye'de en iyi spor ayakkabı mağazası hangisi? ",
        model,
        country: Some("TR"),
    }
}

fn sent(transport: &FakeTransport, index: usize) -> (String, Value, Vec<(String, String)>) {
    let request = &transport.requests()[index];
    (
        request.url.clone(),
        serde_json::from_slice(request.body.as_deref().unwrap()).unwrap(),
        request.headers.clone(),
    )
}

fn header<'a>(headers: &'a [(String, String)], name: &str) -> Option<&'a str> {
    headers
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case(name))
        .map(|(_, value)| value.as_str())
}

#[test]
fn engines_have_ids_providers_and_defaults() {
    assert_eq!(Engine::parse("chatgpt"), Some(Engine::ChatGpt));
    assert_eq!(Engine::parse("bard"), None);
    assert_eq!(Engine::Claude.provider(), "anthropic");
    assert_eq!(Engine::Claude.default_model(), "claude-opus-5-5");
    assert!(
        Engine::Claude.estimate_usd("claude-opus-5-5")
            > Engine::Claude.estimate_usd("claude-haiku-5-5")
    );
    assert_eq!(domain_of("https://WWW.Example.com:8443/a?b"), "example.com");
    let redirect = Citation::new(
        "https://vertexaisearch.cloud.google.com/grounding-api-redirect/abc",
        Some("rakip-magaza.com"),
    );
    assert_eq!(redirect.domain, "rakip-magaza.com");
}

#[test]
fn chatgpt_answers_with_url_citations() {
    let transport = FakeTransport::new().reply(
        200,
        json!({
            "model": "gpt-5.5-2026-08-01",
            "output": [
                { "type": "web_search_call", "status": "completed" },
                { "type": "message", "content": [{
                    "type": "output_text",
                    "text": "Örnek Mağaza öne çıkıyor.",
                    "annotations": [
                        { "type": "url_citation", "url": "https://www.ornekmagaza.com/spor", "title": "Örnek" },
                        { "type": "url_citation", "url": "https://www.ornekmagaza.com/spor", "title": "Örnek" }
                    ]
                }]}
            ],
            "usage": { "input_tokens": 2000, "output_tokens": 500 }
        })
        .to_string(),
    );
    let answer = ask(Engine::ChatGpt, &transport, "sk-test", &question("")).unwrap();
    assert_eq!(answer.text, "Örnek Mağaza öne çıkıyor.");
    assert_eq!(answer.citations.len(), 1, "each source once");
    assert_eq!(answer.citations[0].domain, "ornekmagaza.com");
    assert_eq!(answer.searches, 1);
    assert_eq!(answer.model, "gpt-5.5-2026-08-01");
    assert!(answer.cost_usd > 0.0);
    let (url, body, headers) = sent(&transport, 0);
    assert_eq!(url, "https://api.openai.com/v1/responses");
    assert_eq!(body["model"], "gpt-5.5");
    assert_eq!(
        body["input"],
        "Türkiye'de en iyi spor ayakkabı mağazası hangisi?"
    );
    assert_eq!(body["tools"][0]["type"], "web_search");
    assert_eq!(body["tools"][0]["user_location"]["country"], "TR");
    assert_eq!(header(&headers, "authorization"), Some("Bearer sk-test"));
}

#[test]
fn claude_searches_resumes_a_paused_turn_and_reads_citations() {
    let transport = FakeTransport::new()
        .reply(
            200,
            json!({
                "model": "claude-opus-5-5",
                "stop_reason": "pause_turn",
                "content": [
                    { "type": "server_tool_use", "id": "srvtoolu_1", "name": "web_search", "input": { "query": "spor ayakkabı" } },
                    { "type": "text", "text": "Arıyorum. " }
                ],
                "usage": { "input_tokens": 3000, "output_tokens": 100, "server_tool_use": { "web_search_requests": 1 } }
            })
            .to_string(),
        )
        .reply(
            200,
            json!({
                "model": "claude-opus-5-5",
                "stop_reason": "end_turn",
                "content": [
                    { "type": "web_search_tool_result", "tool_use_id": "srvtoolu_1", "content": [] },
                    { "type": "text", "text": "Rakip Mağaza ve Örnek Mağaza iyi seçenekler.", "citations": [
                        { "type": "web_search_result_location", "url": "https://rakip-magaza.com/x", "title": "Rakip", "cited_text": "..." }
                    ]}
                ],
                "usage": { "input_tokens": 4000, "output_tokens": 300, "server_tool_use": { "web_search_requests": 1 } }
            })
            .to_string(),
        );
    let answer = ask(Engine::Claude, &transport, "sk-ant-test", &question("")).unwrap();
    assert_eq!(
        answer.text,
        "Arıyorum. Rakip Mağaza ve Örnek Mağaza iyi seçenekler."
    );
    assert_eq!(answer.citations[0].domain, "rakip-magaza.com");
    assert_eq!(
        (answer.input_tokens, answer.output_tokens, answer.searches),
        (7000, 400, 2)
    );
    let expected = (7000.0 * 4.0 + 400.0 * 20.0) / 1_000_000.0 + 0.02;
    assert!((answer.cost_usd - expected).abs() < 1e-9);
    let (url, body, headers) = sent(&transport, 0);
    assert_eq!(url, "https://api.anthropic.com/v1/messages");
    assert_eq!(body["model"], "claude-opus-5-5");
    assert_eq!(body["tools"][0]["type"], "web_search_20260209");
    assert_eq!(body["fallbacks"], "default");
    assert!(body.get("thinking").is_none());
    assert_eq!(header(&headers, "anthropic-version"), Some("2023-06-01"));
    assert_eq!(
        header(&headers, "anthropic-beta"),
        Some("server-side-fallback-2026-07-01")
    );
    assert_eq!(header(&headers, "x-api-key"), Some("sk-ant-test"));
    // The paused turn goes back as the assistant's turn.
    let (_, resumed, _) = sent(&transport, 1);
    assert_eq!(resumed["messages"][1]["role"], "assistant");
    assert_eq!(
        resumed["messages"][1]["content"][0]["type"],
        "server_tool_use"
    );

    // An older model gets the basic search tool and no fallbacks.
    let transport = FakeTransport::new().reply(
        200,
        json!({ "model": "claude-haiku-4-5", "stop_reason": "end_turn", "content": [], "usage": {} }).to_string(),
    );
    ask(
        Engine::Claude,
        &transport,
        "k",
        &question("claude-haiku-4-5"),
    )
    .unwrap();
    let (_, body, headers) = sent(&transport, 0);
    assert_eq!(body["tools"][0]["type"], "web_search_20250305");
    assert!(body.get("fallbacks").is_none());
    assert_eq!(header(&headers, "anthropic-beta"), None);
}

#[test]
fn refusals_and_http_failures_become_errors() {
    let transport = FakeTransport::new()
        .reply(
            200,
            json!({ "stop_reason": "refusal", "stop_details": { "category": "cyber" }, "content": [], "usage": {} }).to_string(),
        )
        .reply(401, r#"{"error":{"type":"authentication_error","message":"invalid x-api-key"}}"#)
        .reply_with_headers(429, vec![("retry-after", "30")], "{}")
        .reply(529, r#"{"error":{"message":"Overloaded"}}"#);
    assert_eq!(
        ask(Engine::Claude, &transport, "k", &question("")),
        Err(LlmError::Refused("cyber".to_owned()))
    );
    assert_eq!(
        ask(Engine::Claude, &transport, "k", &question("")),
        Err(LlmError::Auth("invalid x-api-key".to_owned()))
    );
    assert_eq!(
        ask(Engine::Claude, &transport, "k", &question("")),
        Err(LlmError::RateLimited {
            retry_after: Some(30)
        })
    );
    assert!(matches!(
        ask(Engine::Claude, &transport, "k", &question("")),
        Err(LlmError::Unavailable(_))
    ));
    let empty = Question {
        prompt: "  ",
        ..question("")
    };
    assert!(matches!(
        ask(Engine::ChatGpt, &FakeTransport::new(), "k", &empty),
        Err(LlmError::Invalid(_))
    ));
}

#[test]
fn gemini_reads_grounding_chunks_without_thoughts() {
    let transport = FakeTransport::new().reply(
        200,
        json!({
            "modelVersion": "gemini-3.8-flash",
            "candidates": [{
                "finishReason": "STOP",
                "content": { "parts": [
                    { "text": "düşünüyorum", "thought": true },
                    { "text": "Örnek Mağaza güvenilir bir seçenek." }
                ]},
                "groundingMetadata": {
                    "webSearchQueries": ["en iyi spor ayakkabı mağazası"],
                    "groundingChunks": [
                        { "web": { "uri": "https://vertexaisearch.cloud.google.com/grounding-api-redirect/x", "title": "ornekmagaza.com" } }
                    ]
                }
            }],
            "usageMetadata": { "promptTokenCount": 100, "candidatesTokenCount": 50, "thoughtsTokenCount": 20 }
        })
        .to_string(),
    );
    let answer = ask(Engine::Gemini, &transport, "AIza-test", &question("")).unwrap();
    assert_eq!(answer.text, "Örnek Mağaza güvenilir bir seçenek.");
    assert_eq!(answer.citations[0].domain, "ornekmagaza.com");
    assert_eq!((answer.output_tokens, answer.searches), (70, 1));
    let (url, body, headers) = sent(&transport, 0);
    assert_eq!(
        url,
        "https://generativelanguage.googleapis.com/v1beta/models/gemini-3.8-flash:generateContent"
    );
    assert_eq!(body["tools"][0], json!({ "google_search": {} }));
    assert_eq!(header(&headers, "x-goog-api-key"), Some("AIza-test"));
    assert!(matches!(
        ask(
            Engine::Gemini,
            &FakeTransport::new(),
            "k",
            &question("../models")
        ),
        Err(LlmError::Invalid(_))
    ));
}

#[test]
fn perplexity_maps_markers_to_search_results_and_reports_cost() {
    let transport = FakeTransport::new().reply(
        200,
        json!({
            "status": "completed",
            "model": "perplexity/fast",
            "output": [
                { "type": "search_results", "queries": ["spor ayakkabı mağaza"], "results": [
                    { "id": 1, "url": "https://ornekmagaza.com/", "title": "Örnek", "snippet": "" },
                    { "id": 2, "url": "https://rakip-magaza.com/", "title": "Rakip", "snippet": "" }
                ]},
                { "type": "message", "role": "assistant", "content": [
                    { "type": "output_text", "text": "Örnek Mağaza öne çıkıyor [1].", "annotations": [] }
                ]}
            ],
            "usage": { "input_tokens": 50, "output_tokens": 80, "total_tokens": 130,
                       "cost": { "currency": "USD", "input_cost": 0.0, "output_cost": 0.0, "total_cost": 0.0061 } }
        })
        .to_string(),
    );
    let answer = ask(Engine::Perplexity, &transport, "pplx-test", &question("")).unwrap();
    assert_eq!(answer.citations.len(), 1, "only the result [1] points to");
    assert_eq!(answer.citations[0].domain, "ornekmagaza.com");
    assert!((answer.cost_usd - 0.0061).abs() < 1e-12);
    let (url, body, _) = sent(&transport, 0);
    assert_eq!(url, "https://api.perplexity.ai/v1/agent");
    assert_eq!(body["preset"], "fast");
    assert!(body.get("model").is_none());
    let named = FakeTransport::new().reply(200, r#"{"status":"completed","output":[],"usage":{}}"#);
    ask(Engine::Perplexity, &named, "k", &question("openai/gpt-5.5")).unwrap();
    assert_eq!(sent(&named, 0).1["model"], "openai/gpt-5.5");
}

fn brands() -> Vec<Brand> {
    vec![
        Brand::of_site("ornekmagaza.com", &["Örnek Mağaza".to_owned()], true),
        Brand::of_site("rakip-magaza.com", &[], false),
        Brand::of_site("diger.com.tr", &["Diğer Dükkan".to_owned()], false),
    ]
}

#[test]
fn folds_turkish_and_accents() {
    assert_eq!(
        fold("ÖRNEK MAĞAZA'nın İyi Fiyatları!"),
        "ornek magaza nin iyi fiyatlari"
    );
    assert_eq!(fold("Zuverlässig, más rápido"), "zuverlassig mas rapido");
    let brand = Brand::of_site("https://www.rakip-magaza.com/", &[], false);
    assert_eq!(brand.key, "rakip-magaza.com");
    assert_eq!(brand.names, ["rakip-magaza.com", "rakip-magaza"]);
}

#[test]
fn analysis_orders_brands_finds_citations_and_tone() {
    let text = "Rakip Mağaza'nın kargosu yavaş ve şikayetler çok. \
                En iyi seçenek ÖRNEK MAĞAZA: güvenilir ve hızlı. \
                Ayrıca ornekmagaza.com üzerinden sipariş verilebilir.";
    let citations = vec![Citation::new("https://shop.ornekmagaza.com/x", None)];
    let analysis = analyze(text, &citations, &brands());
    assert!(analysis.mentioned);
    assert_eq!(analysis.position, Some(2), "the rival comes first");
    assert!(analysis.cited, "a subdomain is the site");
    assert!(analysis.sentiment.unwrap() > 0.5);
    let rival = analysis
        .brands
        .iter()
        .find(|hit| hit.key == "rakip-magaza.com")
        .unwrap();
    assert_eq!(rival.position, Some(1));
    assert!(rival.sentiment.unwrap() < 0.0);
    assert!(!rival.cited);
    assert!(analysis.brands.iter().all(|hit| hit.key != "diger.com.tr"));

    // Cited but not named: listed without a place; no tone.
    let quiet = analyze("Birkaç mağaza var.", &citations, &brands());
    assert!(!quiet.mentioned && quiet.cited);
    assert_eq!(quiet.position, None);
    assert_eq!(quiet.sentiment, None);
    // A name inside another word does not count.
    let inside = analyze("Örnekmağazacılık başka bir şey.", &[], &brands());
    assert!(!inside.mentioned);
}

#[test]
fn a_ranked_list_gives_the_brands_their_numbers() {
    // The own brand is named first in the intro but listed third.
    let text = "Örnek Mağaza da sık önerilir. İşte en iyi mağazalar:\n\n\
                1. **Diğer Dükkan** – geniş ürün yelpazesi.\n\
                2. Bir başka site\n   Rakip Mağaza ile karşılaştırılır.\n\
                3. **Örnek Mağaza**: hızlı kargo.\n";
    let analysis = analyze(text, &[], &brands());
    assert_eq!(
        analysis.position,
        Some(3),
        "its item's number, not its first mention"
    );
    let place = |key: &str| {
        analysis
            .brands
            .iter()
            .find(|hit| hit.key == key)
            .unwrap()
            .position
    };
    assert_eq!(place("diger.com.tr"), Some(1));
    assert_eq!(
        place("rakip-magaza.com"),
        Some(2),
        "named only under item 2"
    );

    // Headings and bullets rank too; a paragraph starts a new run of bullets.
    let headings = "### 1. Rakip Mağaza\nAçıklama.\n### 2. Örnek Mağaza\nAçıklama.";
    assert_eq!(analyze(headings, &[], &brands()).position, Some(2));
    let bullets =
        "Seçenekler:\n- Diğer Dükkan\n- Rakip Mağaza\n- Örnek Mağaza\n\nDiğerleri:\n- Örnek Mağaza";
    assert_eq!(analyze(bullets, &[], &brands()).position, Some(3));
    // Without a list, the place among the brands named stays.
    let prose = "Rakip Mağaza ve Örnek Mağaza öne çıkar.";
    assert_eq!(analyze(prose, &[], &brands()).position, Some(2));
}
