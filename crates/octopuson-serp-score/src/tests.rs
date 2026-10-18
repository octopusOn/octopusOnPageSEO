use std::collections::BTreeMap;

use octopuson_serp::{
    Engine, Feature, FeatureItem, FeatureKind, Organic, SerpQuery, SerpSnapshot, SnapshotBuilder,
};
use octopuson_text::extract;

use super::*;

fn organic(position: u16, domain: &str) -> Organic {
    Organic {
        position,
        absolute_position: None,
        page: 1,
        url: format!("https://www.{domain}/turk-kahvesi"),
        host: format!("www.{domain}"),
        domain: domain.to_owned(),
        title: format!("Sonuç {position}"),
        snippet: None,
        displayed_url: None,
        date: None,
        site_only: false,
        sitelinks: Vec::new(),
    }
}

fn snapshot(keyword: &str, domains: &[&str], features: Vec<Feature>) -> SerpSnapshot {
    let mut snapshot = SnapshotBuilder::new(
        SerpQuery::new(Engine::Google, keyword),
        "test",
        "2026-10-18T10:00:00Z",
    )
    .finish();
    snapshot.organic = domains
        .iter()
        .enumerate()
        .map(|(index, domain)| organic(u16::try_from(index + 1).unwrap(), domain))
        .collect();
    snapshot.features = features;
    snapshot
}

fn paa(questions: &[&str]) -> Feature {
    Feature {
        kind: FeatureKind::PeopleAlsoAsk,
        absolute_position: Some(3),
        page: 1,
        items: questions
            .iter()
            .map(|question| FeatureItem {
                text: Some((*question).to_owned()),
                url: None,
                domain: None,
            })
            .collect(),
    }
}

const STRONG: &str = r#"<html><head><title>Türk Kahvesi Nasıl Yapılır? Köpüklü Tarif</title></head><body><main>
<h1>Türk Kahvesi Nasıl Yapılır?</h1>
<p>Türk kahvesi cezvede, soğuk su ve ince çekilmiş kahve ile pişirilir. Köpük için ateşi kısık tutun.</p>
<h2>Malzemeler</h2><ul><li>Cezve</li><li>Kahve</li><li>Su</li><li>Şeker</li></ul>
<h2>Köpük nasıl olur?</h2><p>Köpüklü türk kahvesi için kahveyi suyla iyice karıştırın ve kaynatmayın.</p>
<h2>Türk kahvesi kaç dakikada pişer?</h2><p>Kısık ateşte yaklaşık üç dakikada pişer, cezve ve fincan seçimi önemlidir.</p>
<img src="a.jpg" alt="Cezvede türk kahvesi"></main></body></html>"#;

const MEDIUM: &str = r"<html><head><title>Türk kahvesi tarifi</title></head><body><main>
<h1>Türk kahvesi</h1><p>Cezve ile türk kahvesi pişirmek için kahve, su ve şeker gerekir. Köpük önemlidir.</p>
<h2>Malzemeler</h2><p>Cezve, kahve, su.</p></main></body></html>";

const WEAK: &str = r"<html><head><title>Espresso makineleri</title></head><body><main>
<h1>En iyi espresso makineleri</h1><p>Makine seçerken basınç ve bütçe önemlidir.</p></main></body></html>";

fn page(position: u16, domain: &str, html: &str) -> PageSignals {
    let text = extract(html);
    PageSignals {
        position,
        url: format!("https://www.{domain}/turk-kahvesi"),
        domain: domain.to_owned(),
        status: Some(200),
        indexable: true,
        title: text.title.clone(),
        meta_description: None,
        schema_types: Vec::new(),
        images: u32::from(html.contains("<img")),
        images_with_alt: u32::from(html.contains("alt=")),
        has_video: false,
        days_since_modified: Some(30),
        findings: Vec::new(),
        https: true,
        hsts: false,
        response_ms: 300,
        text,
    }
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // small positive counts
fn sites(strength: f64) -> BTreeMap<String, SiteSignals> {
    ["a.com", "b.com.tr", "c.net"]
        .into_iter()
        .map(|domain| {
            (
                domain.to_owned(),
                SiteSignals {
                    domain: domain.to_owned(),
                    open_page_rank: Some(strength),
                    referring_domains: Some((10f64.powf(strength / 2.0)) as u64),
                    tranco_rank: None,
                    age_years: Some(strength * 2.0),
                    https: true,
                    hsts: false,
                    technical: None,
                },
            )
        })
        .collect()
}

#[test]
fn content_scores_follow_how_well_pages_answer_the_keyword() {
    let snapshot = snapshot(
        "türk kahvesi nasıl yapılır",
        &["a.com", "b.com.tr", "c.net"],
        vec![paa(&[
            "Türk kahvesi kaç dakikada pişer?",
            "Köpüklü türk kahvesi nasıl olur?",
        ])],
    );
    let pages = [
        page(1, "a.com", STRONG),
        page(2, "b.com.tr", MEDIUM),
        page(3, "c.net", WEAK),
    ];
    let sites = sites(5.0);
    let analysis = analyze(&AnalysisInput {
        snapshot: &snapshot,
        pages: &pages,
        sites: &sites,
        tranco: false,
        volume: Some(12_000),
        target: None,
    });
    let content: Vec<f64> = analysis
        .pages
        .iter()
        .map(|page| page.content.as_ref().unwrap().score)
        .collect();
    assert!(
        content[0] > content[1] && content[1] > content[2],
        "{content:?}"
    );
    let strong = analysis.pages[0].content.as_ref().unwrap();
    assert_eq!(strong.model, "content-1");
    assert!(strong.component("placement").unwrap().normalized.unwrap() > 0.8);
    assert!(strong.component("questions").unwrap().normalized.unwrap() > 0.4);
    assert_eq!(analysis.intent.primary, IntentClass::Informational);
    assert!(analysis.recommendations.words.is_some());
    assert_eq!(analysis.recommendations.questions.len(), 2);
    assert!(analysis.difficulty.score > 0.0 && analysis.difficulty.score < 100.0);
    assert_eq!(analysis.band, Band::of(analysis.difficulty.score));
}

#[test]
fn stronger_sites_make_keywords_harder() {
    let snapshot = snapshot("türk kahvesi", &["a.com", "b.com.tr", "c.net"], Vec::new());
    let pages = [
        page(1, "a.com", STRONG),
        page(2, "b.com.tr", MEDIUM),
        page(3, "c.net", WEAK),
    ];
    let run = |strength: f64| {
        let sites = sites(strength);
        analyze(&AnalysisInput {
            snapshot: &snapshot,
            pages: &pages,
            sites: &sites,
            tranco: false,
            volume: None,
            target: None,
        })
        .difficulty
        .score
    };
    assert!(run(8.5) > run(4.0) + 15.0);
}

#[test]
fn reads_intent_from_a_keyword_alone() {
    let class = |keyword: &str| crate::keyword_intent(keyword, "tr").map(|intent| intent.primary);
    assert_eq!(
        class("türk kahvesi nasıl yapılır"),
        Some(IntentClass::Informational)
    );
    assert_eq!(
        class("kahve makinesi fiyatları"),
        Some(IntentClass::Transactional)
    );
    assert_eq!(
        class("en iyi kahve makinesi"),
        Some(IntentClass::Commercial)
    );
    assert_eq!(class("kadıköy kahveci"), Some(IntentClass::Local));
    // A keyword that states no intent says so instead of defaulting.
    assert_eq!(class("kahve makinesi"), None);
}

#[test]
fn reads_intents_from_modifiers_and_features() {
    let transactional = snapshot("türk kahvesi fiyatları", &["a.com"], Vec::new());
    assert_eq!(
        intent::intent(&transactional, &[]).primary,
        IntentClass::Transactional
    );
    let local = snapshot("kadıköy kahveci", &["a.com"], Vec::new());
    assert_eq!(intent::intent(&local, &[]).primary, IntentClass::Local);
    let commercial = snapshot("en iyi kahve makinesi", &["a.com"], Vec::new());
    assert_eq!(
        intent::intent(&commercial, &[]).primary,
        IntentClass::Commercial
    );
    let brand = snapshot("trendyol", &["trendyol.com"], Vec::new());
    assert_eq!(
        intent::intent(&brand, &[]).primary,
        IntentClass::Navigational
    );
    let plain = snapshot("kahve", &["a.com"], Vec::new());
    assert_eq!(
        intent::intent(&plain, &[]).primary,
        IntentClass::Informational
    );
}

#[test]
fn reports_opportunity_and_gaps_for_a_target_site() {
    let snapshot = snapshot(
        "türk kahvesi nasıl yapılır",
        &["a.com", "b.com.tr", "c.net"],
        Vec::new(),
    );
    let pages = [
        page(1, "a.com", STRONG),
        page(2, "b.com.tr", MEDIUM),
        page(3, "c.net", WEAK),
    ];
    let sites = sites(4.0);
    let target = TargetInput {
        domain: "c.net".to_owned(),
        site: sites["c.net"].clone(),
        page: Some(pages[2].clone()),
        position: Some(3),
    };
    let analysis = analyze(&AnalysisInput {
        snapshot: &snapshot,
        pages: &pages,
        sites: &sites,
        tranco: false,
        volume: Some(5_000),
        target: Some(target),
    });
    let target = analysis.target.unwrap();
    assert_ne!(target.gaps, [] as [std::string::String; 0]);
    assert_eq!(analysis.recommendations.terms, target.gaps);
    assert!(target.opportunity.score > 0.0);
    assert_eq!(target.opportunity.model, "opp-1");
}

proptest::proptest! {
    #[test]
    fn site_scores_stay_in_range_and_grow_with_pagerank(
        opr in 0.0f64..10.0, referring in 0u64..10_000_000, age in 0.0f64..40.0, rank in 1u32..2_000_000
    ) {
        let site = SiteSignals {
            domain: "x.com".to_owned(),
            open_page_rank: Some(opr),
            referring_domains: Some(referring),
            tranco_rank: Some(rank),
            age_years: Some(age),
            https: true,
            hsts: false,
            technical: Some(80.0),
        };
        let score = site::site_score(&site, true).score;
        proptest::prop_assert!((0.0..=100.0).contains(&score));
        let better = site::site_score(&SiteSignals { open_page_rank: Some((opr + 1.0).min(10.0)), ..site }, true).score;
        proptest::prop_assert!(better >= score);
    }
}
