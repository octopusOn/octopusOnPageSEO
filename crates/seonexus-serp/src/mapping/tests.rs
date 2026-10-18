use serde_json::json;
use url::Url;

use super::*;
use crate::challenge::ChallengeKind;
use crate::profile::builtin;
use crate::{Engine, FeatureKind, SerpQuery, SnapshotBuilder};

const GOOGLE: &str = include_str!("../../tests/fixtures/google-tr.html");
const BING: &str = include_str!("../../tests/fixtures/bing-tr.html");
const YANDEX: &str = include_str!("../../tests/fixtures/yandex-tr.html");

fn parse_with(engine: Engine, html: &str, url: &str) -> ParsedPage {
    let mapping = builtin(engine).unwrap().compile().unwrap();
    mapping.parse(html, &Url::parse(url).unwrap(), 1).unwrap()
}

fn kinds(page: &ParsedPage) -> Vec<FeatureKind> {
    page.features.iter().map(|feature| feature.kind).collect()
}

#[test]
fn reads_a_google_page() {
    let page = parse_with(
        Engine::Google,
        GOOGLE,
        "https://www.google.com/search?q=seo",
    );
    let urls: Vec<&str> = page
        .organic
        .iter()
        .map(|result| result.url.as_str())
        .collect();
    assert_eq!(
        urls,
        [
            "https://www.ornek-ajans.com.tr/",
            "https://blog.example.net/seo-rehberi",
            "https://forum.example.org/t/1"
        ]
    );
    let first = &page.organic[0];
    assert_eq!(first.position, 1);
    assert_eq!(first.title, "Örnek Ajans | SEO Hizmeti");
    assert_eq!(first.domain, "ornek-ajans.com.tr");
    assert_eq!(
        first.snippet.as_deref(),
        Some("İstanbul merkezli SEO ajansı.")
    );
    assert_eq!(
        first.displayed_url.as_deref(),
        Some("https://www.ornek-ajans.com.tr")
    );
    assert_eq!(first.sitelinks.len(), 2);
    assert_eq!(
        first.sitelinks[0].url,
        "https://www.ornek-ajans.com.tr/hizmetler"
    );
    assert_eq!(page.organic[1].date.as_deref(), Some("3 gün önce"));
    assert_eq!(page.total_results, Some(1_230_000));
    assert!(page.challenge.is_none());
    assert_eq!(page.coverage.matched, 5);
    assert_eq!(page.coverage.usable, 5);

    assert_eq!(
        kinds(&page),
        [
            FeatureKind::AdsTop,
            FeatureKind::AiOverview,
            FeatureKind::FeaturedSnippet,
            FeatureKind::PeopleAlsoAsk,
            FeatureKind::Videos,
            FeatureKind::RelatedSearches,
            FeatureKind::AdsBottom,
        ]
    );
    let ads = &page.features[0];
    assert_eq!(ads.items.len(), 2);
    assert_eq!(ads.absolute_position, Some(1));
    let snippet = &page.features[2];
    assert_eq!(
        snippet.items[0].url.as_deref(),
        Some("https://snippet.example.com/nedir")
    );
    let questions: Vec<&str> = page.features[3]
        .items
        .iter()
        .filter_map(|item| item.text.as_deref())
        .collect();
    assert_eq!(
        questions,
        ["SEO ajansı ne iş yapar?", "SEO fiyatları ne kadar?"]
    );
    let related: Vec<&str> = page.features[5]
        .items
        .iter()
        .filter_map(|item| item.text.as_deref())
        .collect();
    assert_eq!(related, ["seo ajansı istanbul", "en iyi seo ajansı"]);
    assert_eq!(
        page.organic
            .iter()
            .map(|result| result.absolute_position)
            .collect::<Vec<_>>(),
        [Some(4), Some(6), Some(8)]
    );
}

#[test]
fn reads_a_bing_page_and_unwraps_click_links() {
    let page = parse_with(Engine::Bing, BING, "https://www.bing.com/search?q=kahve");
    assert_eq!(page.organic.len(), 2);
    assert_eq!(page.organic[0].url, "https://www.example.com.tr/kahve");
    assert_eq!(
        page.organic[0].snippet.as_deref(),
        Some("Taze kahve çeşitleri.")
    );
    assert_eq!(page.total_results, Some(4500));
    assert_eq!(
        kinds(&page),
        [
            FeatureKind::AdsTop,
            FeatureKind::PeopleAlsoAsk,
            FeatureKind::RelatedSearches
        ]
    );
    assert_eq!(page.features[1].items.len(), 2);
    assert_eq!(
        page.features[2].items[0].url.as_deref(),
        Some("https://www.bing.com/search?q=t%C3%BCrk+kahvesi")
    );
}

#[test]
fn reads_a_yandex_page_without_ads_or_wizards() {
    let page = parse_with(
        Engine::Yandex,
        YANDEX,
        "https://yandex.com.tr/search/?text=kahve",
    );
    let domains: Vec<&str> = page
        .organic
        .iter()
        .map(|result| result.domain.as_str())
        .collect();
    assert_eq!(domains, ["kahve.com.tr", "wikipedia.org"]);
    assert_eq!(page.organic[0].snippet.as_deref(), Some("En iyi kahveler."));
    assert_eq!(
        kinds(&page),
        [
            FeatureKind::AdsTop,
            FeatureKind::Videos,
            FeatureKind::RelatedSearches
        ]
    );
}

#[test]
fn detects_challenge_pages() {
    let sorry =
        r#"<html><body><form id="captcha-form"><div id="recaptcha"></div></form></body></html>"#;
    let page = parse_with(
        Engine::Google,
        sorry,
        "https://www.google.com/sorry/index?continue=x",
    );
    assert_eq!(
        page.challenge.map(|challenge| challenge.kind),
        Some(ChallengeKind::Captcha)
    );
    assert_eq!(page.organic.len(), 0);

    let yandex = r#"<html><body><form id="checkbox-captcha-form"></form></body></html>"#;
    let page = parse_with(
        Engine::Yandex,
        yandex,
        "https://yandex.com.tr/showcaptcha?x=1",
    );
    assert_eq!(
        page.challenge.map(|challenge| challenge.kind),
        Some(ChallengeKind::Captcha)
    );

    let consent = parse_with(
        Engine::Google,
        "<html></html>",
        "https://consent.google.com/m?x",
    );
    assert_eq!(
        consent.challenge.map(|challenge| challenge.kind),
        Some(ChallengeKind::Consent)
    );
}

#[test]
fn bing_check_pages_wait_and_its_cookie_banner_does_not() {
    // Bing's Turnstile check, as served in Turkish (2026-10).
    let turnstile = r#"<html><head><title>kahve makinesi - Arama</title></head><body>
        <div class="captcha"><div class="captcha_header">Son bir adım...</div>
        <div class="captcha_text">Devam etmek için lütfen aşağıdaki sınamayı çözümleyin</div>
        <div class="checkbox"><div id="turnstile-widget"><div><div></div>
        <input type="hidden" name="cf-turnstile-response" id="cf-chl-widget-x_response"></div></div></div>
        </div></body></html>"#;
    let page = parse_with(
        Engine::Bing,
        turnstile,
        "https://www.bing.com/search?q=kahve+makinesi",
    );
    assert_eq!(
        page.challenge.map(|challenge| challenge.kind),
        Some(ChallengeKind::Captcha)
    );

    // The cookie banner sits on the result page; the results are read and nothing waits.
    let banner = BING.replace(
        "<body>",
        r#"<body><div id="bnp_container"><button id="bnp_btn_accept">Kabul et</button></div>"#,
    );
    assert_ne!(banner, BING);
    let page = parse_with(
        Engine::Bing,
        &banner,
        "https://www.bing.com/search?q=t%C3%BCrk+kahvesi",
    );
    assert!(page.challenge.is_none());
    assert_ne!(page.organic.len(), 0);
    // Alone, without results, it is still a consent page to clear.
    let page = parse_with(
        Engine::Bing,
        r#"<html><body><div id="bnp_container"><button id="bnp_btn_accept">Kabul et</button></div></body></html>"#,
        "https://www.bing.com/search?q=x",
    );
    assert_eq!(
        page.challenge.map(|challenge| challenge.kind),
        Some(ChallengeKind::Consent)
    );
}

#[test]
fn google_hidden_links_keep_the_site_of_each_result() {
    // Google's 2026 result links are encrypted (`/goto?url=…`); only `cite` names the site.
    let result = |token: &str, title: &str, cite: &str| {
        format!(
            r#"<div class="MjjYud"><div class="tF2Cxc" data-hveid="x"><div class="yuRUbf">
            <a jsname="UWckNb" href="/goto?url={token}" data-ved="v"><h3>{title}</h3>
            <cite>{cite}</cite></a></div><div class="VwiC3b">Özet</div></div></div>"#
        )
    };
    let html = format!(
        r#"<html><body><div id="rso">{}{}{}{}</div></body></html>"#,
        result(
            "CAESa1",
            "Bol Köpüklü Türk Kahvesi",
            "https://www.nefisyemektarifleri.com › ..."
        ),
        result(
            "CAESb2",
            "Türk Kahvesi Tarifi",
            "https://www.nefisyemektarifleri.com › tarif"
        ),
        result(
            "CAESc3",
            "Kahve Makineleri",
            "shop.delonghi.com.tr › kahve-makineleri"
        ),
        r#"<div class="MjjYud"><div class="tF2Cxc"><div class="yuRUbf"><a jsname="UWckNb" href="https://www.karaca.com/blog/turk-kahvesi"><h3>Karaca</h3></a></div></div></div>"#,
    );
    let page = parse_with(
        Engine::Google,
        &html,
        "https://www.google.com/search?q=kahve",
    );
    let read: Vec<(&str, bool)> = page
        .organic
        .iter()
        .map(|result| (result.url.as_str(), result.site_only))
        .collect();
    assert_eq!(
        read,
        [
            ("https://www.nefisyemektarifleri.com/", true),
            ("https://www.nefisyemektarifleri.com/", true),
            ("https://shop.delonghi.com.tr/", true),
            ("https://www.karaca.com/blog/turk-kahvesi", false),
        ]
    );
    assert_eq!(page.organic[2].domain, "delonghi.com.tr");

    let mut builder = SnapshotBuilder::new(
        SerpQuery::new(Engine::Google, "kahve"),
        "browser",
        "2026-10-18T08:00:00Z",
    );
    builder.add_page(page);
    let snapshot = builder.finish();
    // Two pages of one site stay two results.
    assert_eq!(snapshot.organic.len(), 4);
    assert!(
        snapshot
            .warnings
            .iter()
            .any(|warning| warning.contains("hid the page addresses of 3"))
    );
}

#[test]
fn feature_headings_ignore_style_sheets() {
    // Bing starts its answer blocks with a style element longer than the heading window.
    let style = format!(
        "<style>{}</style>",
        "#b_results .b_vidAns{padding:0}".repeat(30)
    );
    let html = format!(
        r#"<html><body><ol id="b_results">
        <li class="b_algo"><h2><a href="https://www.trendyol.com/kahve">Kahve</a></h2></li>
        <li class="b_ans b_vidAns">{style}<h2>Kahve Makinesi Videoları</h2>
        <div class="mc_vtvc"><a href="https://www.youtube.com/watch?v=1"><div class="mc_vtvc_title">İnceleme</div></a></div></li>
        <li class="b_ans b_mop">{style}<h2>Kahve Makinesi resimleri</h2></li>
        </ol></body></html>"#
    );
    let page = parse_with(Engine::Bing, &html, "https://www.bing.com/search?q=kahve");
    let kinds = kinds(&page);
    assert!(kinds.contains(&FeatureKind::Videos), "{kinds:?}");
    assert!(kinds.contains(&FeatureKind::Images), "{kinds:?}");
}

#[test]
fn snapshots_continue_positions_across_pages_and_stop_at_depth() {
    let first = parse_with(
        Engine::Google,
        GOOGLE,
        "https://www.google.com/search?q=seo",
    );
    let mut second = first.clone();
    second.page = 2;
    for result in &mut second.organic {
        result.page = 2;
    }
    for feature in &mut second.features {
        feature.page = 2;
    }
    second.organic[1].url = "https://new.example.com/".to_owned();

    let query = SerpQuery::new(Engine::Google, "seo ajansı").with_depth(5);
    let mut builder = SnapshotBuilder::new(query, "browser", "2026-10-18T10:00:00Z");
    assert_eq!(builder.add_page(first), 3);
    assert_eq!(builder.add_page(second), 1);
    let snapshot = builder.parser_rev("google-html@1").finish();
    let positions: Vec<(u16, u16)> = snapshot
        .organic
        .iter()
        .map(|result| (result.position, result.page))
        .collect();
    assert_eq!(positions, [(1, 1), (2, 1), (3, 1), (4, 2)]);
    assert_eq!(snapshot.total_results, Some(1_230_000));
    assert_eq!(snapshot.top_ad_count(), 2);
    assert!(snapshot.has_feature(FeatureKind::PeopleAlsoAsk));
}

#[test]
fn keeps_the_outermost_of_nested_feature_blocks() {
    let mapping: Mapping = serde_json::from_value(json!({
        "format": "html",
        "results": {"items": "li.r", "fields": {"url": "a @href", "title": "a"}},
        "features": [{"kind": "paa", "container": "div.block", "anchors": ["People also ask"],
            "items": "p.q", "fields": {"text": "::text"}}]
    }))
    .unwrap();
    let html = r#"<div class="block"><h2>People also ask</h2><div class="block"><p class="q">A?</p></div><p class="q">B?</p></div>
        <div class="block"><h2>Other</h2></div>"#;
    let page = mapping
        .compile()
        .unwrap()
        .parse(html, &Url::parse("https://e.example/").unwrap(), 1)
        .unwrap();
    assert_eq!(page.features.len(), 1);
    assert_eq!(page.features[0].items.len(), 2);
    assert_eq!(page.organic.len(), 0);
    assert_eq!(page.coverage.ratio(), 1.0);
}

#[test]
fn reads_json_container_features_and_text_blocks() {
    let mapping: Mapping = serde_json::from_value(json!({
        "format": "json",
        "results": {"items": "$.results", "fields": {"url": "/u", "title": "/t"}},
        "features": [
            {"kind": "featured-snippet", "container": "$.answer_box", "fields": {"text": "$.title", "url": "$.link"}},
            {"kind": "local-pack", "container": "$.local"}
        ]
    }))
    .unwrap();
    let body = json!({"results": [{"u": "https://a.example/", "t": "A"}],
        "answer_box": {"title": "Cevap", "link": "https://b.example/"}});
    let page = mapping
        .compile()
        .unwrap()
        .parse(
            &body.to_string(),
            &Url::parse("https://api.example/").unwrap(),
            1,
        )
        .unwrap();
    assert_eq!(page.organic.len(), 1);
    assert_eq!(page.features.len(), 1);
    assert_eq!(page.features[0].items[0].text.as_deref(), Some("Cevap"));
    assert!(
        mapping
            .compile()
            .unwrap()
            .parse("not json", &Url::parse("https://api.example/").unwrap(), 1)
            .is_err()
    );

    let text: Mapping = serde_json::from_value(json!({
        "format": "text",
        "results": {"items": "(?s)<r>.*?</r>", "fields": {"url": "u=(\\S+)", "title": "t=(?P<v>[^;]+)"}},
        "total": "found (\\d+)"
    }))
    .unwrap();
    let page = text
        .compile()
        .unwrap()
        .parse(
            "found 42 <r>u=https://x.example/ t=X;</r><r>u=https://y.example/ t=Y;</r>",
            &Url::parse("https://api.example/").unwrap(),
            1,
        )
        .unwrap();
    assert_eq!(page.total_results, Some(42));
    assert_eq!(page.organic.len(), 2);
}

#[test]
fn rejects_invalid_mappings() {
    let invalid = [
        json!({"format": "html", "results": {"fields": {"url": "a @href", "title": "a"}}}),
        json!({"format": "html", "results": {"items": "li", "fields": {"url": "a @href"}}}),
        json!({"format": "html", "results": {"items": "li", "fields": {"url": "a @href", "title": "a", "colour": "b"}}}),
        json!({"format": "json", "results": {"items": "li", "fields": {"url": "/u", "title": "/t"}}}),
        json!({"format": "json", "results": {"items": "$.r", "fields": {"url": "/u", "title": "/t"}, "exclude_within": ["x"]}}),
        json!({"format": "text", "results": {"regex": "(?P<url>x)"}}),
        json!({"format": "html", "results": {"items": "li", "fields": {"url": "a @href", "title": "a"}},
            "features": [{"kind": "paa", "items": "p"}]}),
        json!({"format": "html", "results": {"items": "li", "fields": {"url": "a @href", "title": "a"}},
            "features": [{"kind": "nonsense", "container": "p"}]}),
    ];
    for mapping in invalid {
        let parsed = serde_json::from_value::<Mapping>(mapping.clone());
        assert!(
            parsed.map_or(true, |mapping| mapping.compile().is_err()),
            "{mapping} should be rejected"
        );
    }
}
