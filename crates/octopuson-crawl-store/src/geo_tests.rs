use std::collections::HashMap;

use octopuson_crawl::{
    CrawlConfig, CrawlControl, CrawlPlan, Crawler, FetchError, FetchRequest, FetchResponse, Fetcher,
};

use crate::geo::is_question;
use crate::{CrawlLibrary, GeoStatus};

struct Site(HashMap<&'static str, FetchResponse>);

impl Fetcher for Site {
    fn fetch(&self, request: &FetchRequest<'_>) -> Result<FetchResponse, FetchError> {
        Ok(self
            .0
            .get(request.url)
            .cloned()
            .unwrap_or_else(|| FetchResponse::new(404, &[("Content-Type", "text/html")], "")))
    }
}

fn words(count: usize) -> String {
    vec!["kahve"; count].join(" ")
}

fn site() -> Site {
    let html = |body: String| FetchResponse::new(200, &[("Content-Type", "text/html")], body);
    Site(HashMap::from([
        (
            "https://example.com/",
            html(format!(
                r#"<html lang="tr"><head><title>Kahve</title>
                <script type="application/ld+json">{{"@context":"https://schema.org","@type":"Organization","name":"Örnek"}}</script>
                </head><body><h1>Kahve</h1><h2>Türk kahvesi nasıl pişirilir</h2><p>{}</p>
                <a href="/a">A</a><a href="/b">B</a></body></html>"#,
                words(320)
            )),
        ),
        (
            "https://example.com/a",
            html(format!(
                r#"<html><head><title>A</title><meta name="robots" content="max-snippet:20"></head>
                <body><h1>A</h1><h2>Fiyatlar</h2><p>{}</p></body></html>"#,
                words(400)
            )),
        ),
        (
            "https://example.com/b",
            html(
                "<html><head><title>B</title></head><body><h1>B</h1><p>kısa</p></body></html>"
                    .to_owned(),
            ),
        ),
        (
            "https://example.com/robots.txt",
            FetchResponse::new(
                200,
                &[("Content-Type", "text/plain")],
                "User-agent: PerplexityBot\nDisallow: /\n\nUser-agent: *\nAllow: /\n",
            ),
        ),
    ]))
}

#[test]
fn questions_are_told_in_four_languages() {
    for question in [
        "Türk kahvesi nasıl pişirilir",
        "Kahve sağlıklı mı",
        "What is GEO?",
        "how to brew",
        "Wie lange hält Kaffee",
        "¿Cuánto cuesta?",
        "cuál es mejor",
    ] {
        assert!(is_question(question), "{question}");
    }
    for statement in ["Fiyatlar", "Our story", "Kaffeesorten", "Precios", ""] {
        assert!(!is_question(statement), "{statement}");
    }
}

#[test]
fn readiness_weighs_access_snippets_schema_questions_and_content() {
    let dir = tempfile::tempdir().unwrap();
    let library = CrawlLibrary::new(dir.path());
    let plan = CrawlPlan::new(CrawlConfig {
        start_urls: vec!["https://example.com/".into()],
        ..CrawlConfig::default()
    })
    .unwrap();
    let mut writer = library.create(&plan).unwrap();
    let summary = Crawler::new(&plan, &site())
        .run(&mut writer, &CrawlControl::new())
        .unwrap();
    let meta = writer.finish(&summary).unwrap();
    let geo = library.open(&meta.id).unwrap().geo_readiness().unwrap();

    assert_eq!(geo.pages, 3);
    assert_eq!(geo.robots.as_deref(), Some("found"));
    let signal = |id: &str| geo.signals.iter().find(|signal| signal.id == id).unwrap();

    let search = signal("ai-search-access");
    assert!(search.passed < search.total, "PerplexityBot is blocked");
    assert!(search.examples.contains(&"PerplexityBot".to_owned()));
    assert_eq!(search.status, GeoStatus::Warning);
    assert!(
        geo.crawlers
            .iter()
            .any(|crawler| crawler.name == "PerplexityBot" && !crawler.allowed)
    );

    let llms = signal("llms-txt");
    assert_eq!((llms.passed, llms.total), (0, 1));
    assert_eq!(llms.examples, ["https://example.com/llms.txt"]);

    let snippets = signal("snippets-allowed");
    assert_eq!((snippets.passed, snippets.total), (2, 3));
    assert_eq!(snippets.examples, ["https://example.com/a"]);

    let schema = signal("structured-data");
    assert_eq!((schema.passed, schema.total), (1, 3));
    let entity = signal("entity-schema");
    assert_eq!((entity.passed, entity.total), (1, 2));
    assert_eq!(entity.examples, ["WebSite"]);

    // Two pages have something to quote; the home page leads with a question.
    let questions = signal("question-headings");
    assert_eq!((questions.passed, questions.total), (1, 2));
    assert_eq!(questions.examples, ["https://example.com/a"]);
    let content = signal("substantial-content");
    assert_eq!((content.passed, content.total), (2, 3));

    assert_eq!(geo.schema_types, [("Organization".to_owned(), 1)]);
    let score = geo.score.unwrap();
    assert!(score > 30.0 && score < 90.0, "{score}");
    assert_eq!(geo.signals[0].id, "ai-search-access", "heaviest first");
}
