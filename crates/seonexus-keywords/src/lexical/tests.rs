use std::collections::BTreeSet;

use proptest::prelude::*;

use super::*;

const COFFEE: &[(&str, u64)] = &[
    ("kahve makinesi", 90_500),
    ("kahve makineleri", 12_100),
    ("makinesi kahve", 40),
    ("kahve makinesi fiyatları", 22_200),
    ("kahve makinesi fiyat", 2_900),
    ("kahve makinesi nasıl temizlenir", 1_300),
    ("kahve makinesi temizleme", 880),
    ("türk kahvesi", 74_000),
    ("türk kahvesi nasıl yapılır", 49_500),
    ("türk kahvesinin yapılışı", 1_900),
    ("filtre kahve", 33_100),
    ("filtre kahve makinesi", 18_100),
    ("filtre kahve nasıl yapılır", 6_600),
    ("espresso makinesi", 27_100),
    ("çay makinesi", 14_800),
];

fn inputs(list: &[(&'static str, u64)]) -> Vec<Input<'static>> {
    list.iter()
        .map(|(keyword, volume)| Input {
            keyword,
            volume: Some(*volume),
        })
        .collect()
}

/// Clusters as sets of keywords, for comparisons that ignore order.
fn groups(inputs: &[Input<'_>], clustering: &Clustering) -> BTreeSet<BTreeSet<String>> {
    clustering
        .clusters
        .iter()
        .map(|cluster| {
            cluster
                .indices()
                .map(|index| inputs[index].keyword.to_owned())
                .collect()
        })
        .collect()
}

fn together(inputs: &[Input<'_>], clustering: &Clustering, a: &str, b: &str) -> bool {
    let assignment = clustering.assignment(inputs.len());
    let find = |text: &str| {
        let index = inputs
            .iter()
            .position(|input| input.keyword == text)
            .unwrap();
        assignment[index].unwrap()
    };
    find(a) == find(b)
}

#[test]
fn groups_inflections_and_word_order_and_keeps_intents_apart() {
    let inputs = inputs(COFFEE);
    let clustering = cluster_lexical(&inputs, &LexicalOptions::default());
    let pairs_together = [
        ("kahve makinesi", "kahve makineleri"),
        ("kahve makinesi", "makinesi kahve"),
        ("kahve makinesi fiyatları", "kahve makinesi fiyat"),
    ];
    for (a, b) in pairs_together {
        assert!(together(&inputs, &clustering, a, b), "{a} / {b}");
    }
    let pairs_apart = [
        (
            "kahve makinesi fiyatları",
            "kahve makinesi nasıl temizlenir",
        ),
        ("türk kahvesi", "filtre kahve"),
        ("espresso makinesi", "çay makinesi"),
        ("türk kahvesi nasıl yapılır", "filtre kahve nasıl yapılır"),
    ];
    for (a, b) in pairs_apart {
        assert!(!together(&inputs, &clustering, a, b), "{a} / {b}");
    }
    // The most searched keyword leads its cluster.
    let first = &clustering.clusters[0];
    assert_eq!(inputs[first.pillar].keyword, "kahve makinesi");
    assert_eq!(first.members[0].index, first.pillar);
    assert!((first.members[0].score - 1.0).abs() < f64::EPSILON);
}

#[test]
fn a_strict_threshold_groups_only_the_same_stems() {
    let inputs = inputs(COFFEE);
    let strict = cluster_lexical(
        &inputs,
        &LexicalOptions {
            threshold: 1.0,
            ..LexicalOptions::default()
        },
    );
    assert!(together(
        &inputs,
        &strict,
        "kahve makinesi",
        "makinesi kahve"
    ));
    assert!(!together(
        &inputs,
        &strict,
        "kahve makinesi",
        "kahve makinesi fiyat"
    ));
}

#[test]
fn unknown_volumes_and_empty_keywords_are_still_placed() {
    let inputs = vec![
        Input {
            keyword: "kahve",
            volume: None,
        },
        Input {
            keyword: "--",
            volume: Some(10),
        },
        Input {
            keyword: "kahve",
            volume: Some(5),
        },
    ];
    let clustering = cluster_lexical(&inputs, &LexicalOptions::default());
    let assignment = clustering.assignment(inputs.len());
    assert!(assignment.iter().all(Option::is_some));
    // Known volumes lead.
    assert_eq!(clustering.clusters[0].pillar, 1);
}

const WORDS: &[&str] = &[
    "kahve",
    "kahvesi",
    "makinesi",
    "makineleri",
    "türk",
    "filtre",
    "fiyat",
    "fiyatları",
    "nasıl",
    "yapılır",
    "en",
    "iyi",
    "ucuz",
    "espresso",
    "çay",
    "demlik",
    "öğütücü",
    "çekirdek",
];

fn keyword() -> impl Strategy<Value = String> {
    prop::collection::vec(prop::sample::select(WORDS), 1..4).prop_map(|words| words.join(" "))
}

proptest! {
    #[test]
    fn every_keyword_is_in_exactly_one_cluster_whatever_the_order(
        mut list in prop::collection::btree_map(keyword(), 0u64..10_000, 1..60),
        threshold in 0.2f64..1.0,
        seed in any::<u64>(),
    ) {
        let entries: Vec<(String, u64)> = std::mem::take(&mut list).into_iter().collect();
        let inputs: Vec<Input<'_>> = entries
            .iter()
            .map(|(keyword, volume)| Input { keyword, volume: Some(*volume) })
            .collect();
        let options = LexicalOptions { threshold, ..LexicalOptions::default() };
        let clustering = cluster_lexical(&inputs, &options);
        let mut counts = vec![0usize; inputs.len()];
        for cluster in &clustering.clusters {
            prop_assert!(!cluster.is_empty());
            prop_assert_eq!(cluster.members[0].index, cluster.pillar);
            for member in &cluster.members {
                counts[member.index] += 1;
                prop_assert!(member.score >= threshold - 1e-3 || member.index == cluster.pillar);
            }
        }
        prop_assert!(counts.iter().all(|count| *count == 1));

        // The same keywords in another order give the same groups.
        let mut shuffled = inputs.clone();
        let len = shuffled.len();
        for step in 0..len {
            let other = usize::try_from(seed.wrapping_mul(step as u64 + 7) % len as u64).unwrap();
            shuffled.swap(step, other);
        }
        let again = cluster_lexical(&shuffled, &options);
        prop_assert_eq!(groups(&inputs, &clustering), groups(&shuffled, &again));
    }
}
