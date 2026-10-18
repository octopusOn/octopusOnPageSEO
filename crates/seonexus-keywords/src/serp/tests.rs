use super::*;

fn pages(domains: &[&str]) -> Vec<String> {
    domains
        .iter()
        .map(|path| format!("https://www.{path}"))
        .collect()
}

#[test]
fn compares_addresses_without_scheme_www_query_or_slash() {
    assert_eq!(
        result_key("https://www.Karaca.com/kahve/?utm=x#a", Level::Page).as_deref(),
        Some("karaca.com/kahve")
    );
    assert_eq!(
        result_key("http://karaca.com/kahve", Level::Page).as_deref(),
        Some("karaca.com/kahve")
    );
    assert_eq!(
        result_key("https://shop.delonghi.com.tr/x", Level::Site).as_deref(),
        Some("delonghi.com.tr")
    );
    assert_eq!(result_key("not a url", Level::Page), None);
}

#[test]
fn soft_joins_on_the_pillar_and_hard_on_every_member() {
    // a shares 3 with b and 3 with c; b and c share none.
    let a = pages(&[
        "x.com/1", "x.com/2", "x.com/3", "y.com/1", "y.com/2", "y.com/3",
    ]);
    let b = pages(&["x.com/1", "x.com/2", "x.com/3", "z.com/1"]);
    let c = pages(&["y.com/1", "y.com/2", "y.com/3", "w.com/1"]);
    let d = pages(&["q.com/1", "q.com/2"]);
    let inputs = [
        SerpInput {
            keyword: "a",
            volume: Some(100),
            results: Some(&a),
        },
        SerpInput {
            keyword: "b",
            volume: Some(50),
            results: Some(&b),
        },
        SerpInput {
            keyword: "c",
            volume: Some(40),
            results: Some(&c),
        },
        SerpInput {
            keyword: "d",
            volume: Some(30),
            results: Some(&d),
        },
        SerpInput {
            keyword: "e",
            volume: Some(20),
            results: None,
        },
    ];
    let soft = cluster_serp(&inputs, &SerpOptions::default());
    let first: Vec<usize> = soft.clusters[0].indices().collect();
    assert_eq!(first, [0, 1, 2]);
    assert!(soft.clusters[0].verified);
    assert_eq!(soft.clusters[0].members[1].shared, Some(3));
    assert_eq!(soft.clusters.len(), 3);

    let hard = cluster_serp(
        &inputs,
        &SerpOptions {
            mode: Mode::Hard,
            ..SerpOptions::default()
        },
    );
    // b joins a; c shares too little with b.
    let first: Vec<usize> = hard.clusters[0].indices().collect();
    assert_eq!(first.len(), 2);
    assert_eq!(hard.clusters.len(), 4);
    // Keywords without a result page stay alone and unverified.
    let alone = hard
        .clusters
        .iter()
        .find(|cluster| cluster.pillar == 4)
        .unwrap();
    assert!(!alone.verified);
    assert_eq!(alone.len(), 1);
}

#[test]
fn site_level_groups_hidden_google_addresses() {
    // Google in the browser mode gives only sites.
    let a = pages(&[
        "karaca.com/",
        "trendyol.com/",
        "hepsiburada.com/",
        "arzum.com.tr/",
    ]);
    let b = pages(&[
        "trendyol.com/",
        "karaca.com/",
        "beymen.com/",
        "arzum.com.tr/",
    ]);
    let inputs = [
        SerpInput {
            keyword: "kahve makinesi",
            volume: Some(90),
            results: Some(&a),
        },
        SerpInput {
            keyword: "kahve makineleri",
            volume: Some(12),
            results: Some(&b),
        },
    ];
    let sites = cluster_serp(
        &inputs,
        &SerpOptions {
            level: Level::Site,
            ..SerpOptions::default()
        },
    );
    assert_eq!(sites.clusters.len(), 1);
    assert_eq!(sites.clusters[0].members[1].shared, Some(3));
}

#[test]
fn a_higher_threshold_never_merges_more() {
    let a = pages(&["x.com/1", "x.com/2", "x.com/3", "x.com/4"]);
    let b = pages(&["x.com/1", "x.com/2", "x.com/3", "z.com/9"]);
    let c = pages(&["x.com/1", "x.com/2", "q.com/1", "q.com/2"]);
    let inputs = [
        SerpInput {
            keyword: "a",
            volume: Some(3),
            results: Some(&a),
        },
        SerpInput {
            keyword: "b",
            volume: Some(2),
            results: Some(&b),
        },
        SerpInput {
            keyword: "c",
            volume: Some(1),
            results: Some(&c),
        },
    ];
    let count = |threshold| {
        cluster_serp(
            &inputs,
            &SerpOptions {
                threshold,
                ..SerpOptions::default()
            },
        )
        .clusters
        .len()
    };
    assert_eq!(count(2), 1);
    assert_eq!(count(3), 2);
    assert_eq!(count(4), 3);
}
