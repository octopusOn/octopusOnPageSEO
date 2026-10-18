//! Hybrid clustering, cluster totals and site mapping.

use octopuson_serp_score::IntentClass;
use octopuson_text::Language;

use crate::{
    ClusterSummary, KeywordFacts, LexicalOptions, SerpInput, SerpOptions, SitePage, cluster_hybrid,
    map_cluster, site_domain, summarize,
};

fn pages(paths: &[&str]) -> Vec<String> {
    paths.iter().map(|path| format!("https://{path}")).collect()
}

#[test]
fn hybrid_places_unfetched_keywords_with_their_wording_neighbours() {
    let machine = pages(&["a.com/1", "b.com/1", "c.com/1", "d.com/1"]);
    let machines = pages(&["a.com/1", "b.com/1", "c.com/1", "e.com/1"]);
    let turkish = pages(&["t.com/1", "u.com/1", "v.com/1"]);
    let inputs = [
        SerpInput {
            keyword: "kahve makinesi",
            volume: Some(90_500),
            results: Some(&machine),
        },
        SerpInput {
            keyword: "kahve makineleri",
            volume: Some(12_100),
            results: Some(&machines),
        },
        // Not fetched: follows its wording neighbours into the verified cluster.
        SerpInput {
            keyword: "makinesi kahve",
            volume: Some(40),
            results: None,
        },
        SerpInput {
            keyword: "türk kahvesi",
            volume: Some(74_000),
            results: Some(&turkish),
        },
        // Not fetched, no fetched neighbour: an unverified cluster of its own.
        SerpInput {
            keyword: "çay demliği",
            volume: Some(9_900),
            results: None,
        },
        SerpInput {
            keyword: "çaydanlık demlik",
            volume: Some(1_000),
            results: None,
        },
    ];
    let clustering = cluster_hybrid(&inputs, &LexicalOptions::default(), &SerpOptions::default());
    let assignment = clustering.assignment(inputs.len());
    assert!(assignment.iter().all(Option::is_some));
    assert_eq!(assignment[0], assignment[1]);
    assert_eq!(assignment[0], assignment[2]);
    assert_ne!(assignment[0], assignment[3]);
    let machines = &clustering.clusters[assignment[0].unwrap()];
    assert!(machines.verified);
    let placed = machines
        .members
        .iter()
        .find(|member| member.index == 2)
        .unwrap();
    assert_eq!(placed.shared, None);
    let tea = &clustering.clusters[assignment[4].unwrap()];
    assert!(!tea.verified);
}

#[test]
fn totals_weight_by_searches_and_flag_mixed_intent() {
    let facts = [
        KeywordFacts {
            volume: Some(1_000),
            difficulty: Some(60.0),
            cpc: Some(1.0),
            intent: Some(IntentClass::Transactional),
        },
        KeywordFacts {
            volume: Some(500),
            difficulty: Some(30.0),
            cpc: None,
            intent: Some(IntentClass::Informational),
        },
        KeywordFacts {
            volume: None,
            difficulty: None,
            cpc: None,
            intent: None,
        },
    ];
    let inputs = [
        SerpInput {
            keyword: "kahve makinesi fiyat",
            volume: Some(1_000),
            results: None,
        },
        SerpInput {
            keyword: "kahve makinesi fiyatı nasıl",
            volume: Some(500),
            results: None,
        },
        SerpInput {
            keyword: "kahve makinesi fiyatları",
            volume: None,
            results: None,
        },
    ];
    let clustering = cluster_hybrid(
        &inputs,
        &LexicalOptions {
            threshold: 0.3,
            language: Language::Turkish,
        },
        &SerpOptions::default(),
    );
    let cluster = &clustering.clusters[0];
    assert_eq!(cluster.len(), 3);
    let summary: ClusterSummary = summarize(cluster, &facts);
    assert_eq!(summary.volume, 1_500);
    assert_eq!(summary.intent, Some(IntentClass::Transactional));
    assert!(summary.mixed_intent);
    assert_eq!(summary.difficulty, Some(50.0));
    assert_eq!(summary.cpc, Some(1.0));
}

#[test]
fn a_long_tail_modifier_does_not_decide_the_intent_of_a_cluster() {
    let fact = |volume: u64, intent: Option<IntentClass>| KeywordFacts {
        volume: Some(volume),
        difficulty: None,
        cpc: None,
        intent,
    };
    let facts = [
        fact(74_000, None),
        fact(2_400, Some(IntentClass::Transactional)),
    ];
    let cluster = crate::Cluster {
        pillar: 0,
        members: vec![
            crate::Member {
                index: 0,
                score: 1.0,
                shared: None,
            },
            crate::Member {
                index: 1,
                score: 0.7,
                shared: None,
            },
        ],
        verified: false,
    };
    let summary = summarize(&cluster, &facts);
    assert_eq!(summary.intent, None);
    assert!(!summary.mixed_intent);
    assert_eq!(summary.volume, 76_400);
}

#[test]
fn maps_a_cluster_to_its_page_competition_and_competitors() {
    let first = pages(&["www.ornek.com.tr/kahve", "rakip.com/a", "diger.com/b"]);
    let second = pages(&["rakip.com/a", "www.ornek.com.tr/blog/kahve", "diger.com/d"]);
    let inputs = [
        SerpInput {
            keyword: "kahve makinesi",
            volume: Some(100),
            results: Some(&first),
        },
        SerpInput {
            keyword: "kahve makineleri",
            volume: Some(50),
            results: Some(&second),
        },
    ];
    let clustering = cluster_hybrid(
        &inputs,
        &LexicalOptions::default(),
        &SerpOptions {
            threshold: 1,
            ..SerpOptions::default()
        },
    );
    let cluster = &clustering.clusters[0];
    assert_eq!(cluster.len(), 2);
    let site = site_domain("https://www.Ornek.com.tr/").unwrap();
    assert_eq!(site, "ornek.com.tr");
    let site_pages = [
        SitePage {
            keyword: 0,
            url: "https://www.ornek.com.tr/kahve".to_owned(),
            impressions: Some(900.0),
            position: Some(4.0),
        },
        SitePage {
            keyword: 1,
            url: "https://www.ornek.com.tr/blog/kahve".to_owned(),
            impressions: Some(300.0),
            position: Some(9.0),
        },
    ];
    let results = [Some(first.as_slice()), Some(second.as_slice())];
    let map = map_cluster(cluster, &site_pages, &results, &site);
    assert_eq!(
        map.target.as_deref(),
        Some("https://www.ornek.com.tr/kahve")
    );
    assert_eq!(map.competing, ["https://www.ornek.com.tr/blog/kahve"]);
    assert!(!map.gap);
    assert_eq!(map.competitors[0], ("diger.com".to_owned(), 2));
    assert_eq!(map.competitors.len(), 2);

    let nothing = map_cluster(cluster, &[], &results, &site);
    assert!(nothing.gap);
    assert!(nothing.target.is_none());
}

/// 50,000 keywords, as a large project: `cargo test --release -p octopuson-keywords -- --ignored`.
#[test]
#[ignore = "timing check, run in release mode"]
fn clusters_fifty_thousand_keywords() {
    use std::time::Instant;

    let heads = [
        "kahve",
        "çay",
        "espresso",
        "filtre",
        "türk",
        "makine",
        "öğütücü",
        "fincan",
        "demlik",
        "termos",
        "kupa",
        "çekirdek",
        "kapsül",
        "süt",
        "köpürtücü",
        "bardak",
        "tabak",
        "tencere",
        "tava",
        "bıçak",
    ];
    let mods = [
        "fiyat",
        "fiyatları",
        "en iyi",
        "ucuz",
        "nasıl",
        "nedir",
        "yorum",
        "indirim",
        "marka",
        "model",
        "satın al",
        "tavsiye",
        "kullanımı",
        "temizleme",
        "tamiri",
        "parçaları",
        "seti",
        "hediye",
        "2026",
        "otomatik",
    ];
    let brands: Vec<String> = (0..125).map(|n| format!("marka{n}")).collect();
    let mut keywords = Vec::new();
    for head in heads {
        for modifier in mods {
            for brand in &brands {
                keywords.push(format!("{brand} {head} {modifier}"));
            }
        }
    }
    assert_eq!(keywords.len(), 50_000);
    let urls: Vec<Vec<String>> = keywords
        .iter()
        .enumerate()
        .map(|(index, _)| {
            (0..10)
                .map(|rank| {
                    format!(
                        "https://site{}.com/{}",
                        (index / 7 + rank) % 4_000,
                        index % 13
                    )
                })
                .collect()
        })
        .collect();
    let inputs: Vec<SerpInput<'_>> = keywords
        .iter()
        .zip(&urls)
        .enumerate()
        .map(|(index, (keyword, urls))| SerpInput {
            keyword,
            volume: Some(u64::try_from(index % 997).unwrap()),
            results: (index % 5 == 0).then_some(urls.as_slice()),
        })
        .collect();
    let words: Vec<crate::Input<'_>> = inputs
        .iter()
        .map(|input| crate::Input {
            keyword: input.keyword,
            volume: input.volume,
        })
        .collect();

    let started = Instant::now();
    let lexical = crate::cluster_lexical(&words, &LexicalOptions::default());
    let lexical_time = started.elapsed();
    let started = Instant::now();
    let hybrid = cluster_hybrid(&inputs, &LexicalOptions::default(), &SerpOptions::default());
    let hybrid_time = started.elapsed();
    eprintln!(
        "lexical: {} clusters in {lexical_time:?}; hybrid: {} clusters in {hybrid_time:?}",
        lexical.clusters.len(),
        hybrid.clusters.len()
    );
    assert!(hybrid.assignment(inputs.len()).iter().all(Option::is_some));
    assert!(lexical_time.as_secs() < 30 && hybrid_time.as_secs() < 60);
}
