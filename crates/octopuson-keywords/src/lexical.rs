//! Grouping by wording.
//!
//! Each keyword becomes the set of its word stems (Turkish suffixes removed, so `kahvesi`,
//! `kahveler` and `kahve` agree). Two keywords are as similar as the stems they share,
//! weighted by how rare each stem is in the list (a word in every keyword says little):
//! the weighted Jaccard index. Pillars are taken by search volume; each takes the keywords
//! still free whose similarity reaches the threshold. Keywords with the same stems (word
//! order, inflection) always group.
//!
//! Wording groups are a fast first pass, free and offline; result pages decide what one page
//! can rank for ([`crate::serp`]).

use std::collections::HashMap;

use octopuson_text::Language;
use serde::{Deserialize, Serialize};

use crate::cluster::{Clustering, Member, finish, pillar_order};
use crate::normalize::normalize;

/// Words that join others and carry no topic.
const CONNECTORS: &[&str] = &[
    "ve", "ile", "icin", "de", "da", "ki", "veya", "ya", "and", "or", "the", "of", "for", "to",
    "in", "a", "an", "on", "with",
];

/// Settings of a wording clustering.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LexicalOptions {
    /// Least similarity to the pillar (0–1).
    pub threshold: f64,
    /// Language of the keywords.
    pub language: Language,
}

impl Default for LexicalOptions {
    fn default() -> Self {
        Self {
            threshold: 0.6,
            language: Language::Turkish,
        }
    }
}

/// A keyword to group.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Input<'a> {
    /// The keyword.
    pub keyword: &'a str,
    /// Monthly searches, if known.
    pub volume: Option<u64>,
}

/// Stems as ids into a weight table, sorted and without duplicates.
struct Stems {
    ids: Vec<Vec<u32>>,
    weights: Vec<f64>,
    postings: Vec<Vec<usize>>,
}

#[allow(clippy::cast_precision_loss)] // keyword counts are far below 2^52
fn stems(inputs: &[Input<'_>], language: Language) -> Stems {
    let mut vocabulary: HashMap<String, u32> = HashMap::new();
    let mut ids: Vec<Vec<u32>> = Vec::with_capacity(inputs.len());
    for input in inputs {
        let mut own: Vec<u32> = normalize(input.keyword, language)
            .map(|normalized| normalized.stems)
            .unwrap_or_default()
            .into_iter()
            .filter(|stem| !CONNECTORS.contains(&stem.as_str()))
            .map(|stem| {
                let next = u32::try_from(vocabulary.len()).unwrap_or(u32::MAX);
                *vocabulary.entry(stem).or_insert(next)
            })
            .collect();
        own.sort_unstable();
        own.dedup();
        ids.push(own);
    }
    let mut postings: Vec<Vec<usize>> = vec![Vec::new(); vocabulary.len()];
    for (index, own) in ids.iter().enumerate() {
        for &id in own {
            postings[id as usize].push(index);
        }
    }
    let total = inputs.len() as f64;
    let weights = postings
        .iter()
        .map(|posting| (1.0 + total / posting.len().max(1) as f64).ln())
        .collect();
    Stems {
        ids,
        weights,
        postings,
    }
}

/// Weighted Jaccard index of two sorted id lists: the weight of the shared stems over the
/// weight of all stems of either.
fn similarity(left: &[u32], right: &[u32], weights: &[f64]) -> f64 {
    let weight = |id: &u32| weights[*id as usize];
    let mut shared = 0.0;
    let mut rest = right.iter().peekable();
    for id in left {
        while rest.next_if(|other| *other < id).is_some() {}
        if rest.next_if_eq(&id).is_some() {
            shared += weight(id);
        }
    }
    let union =
        left.iter().map(weight).sum::<f64>() + right.iter().map(weight).sum::<f64>() - shared;
    if union > 0.0 { shared / union } else { 0.0 }
}

/// Groups keywords by wording. Every input is in exactly one cluster; keywords without stems
/// stay alone.
pub fn cluster_lexical(inputs: &[Input<'_>], options: &LexicalOptions) -> Clustering {
    let stems = stems(inputs, options.language);
    let texts: Vec<&str> = inputs.iter().map(|input| input.keyword).collect();
    let volumes: Vec<Option<u64>> = inputs.iter().map(|input| input.volume).collect();
    // Common stems lead to many candidates and weigh little: a keyword that shares only
    // common stems with the pillar is rarely similar enough. They are skipped as a source of
    // candidates, except the pillar's rarest stem, which is always followed (so keywords with
    // the same stems always meet).
    let cap = (inputs.len() / 20).max(500);
    let mut taken = vec![false; inputs.len()];
    let mut seen = vec![usize::MAX; inputs.len()];
    let mut clusters = Vec::new();
    for pillar in pillar_order(&volumes, &texts) {
        if taken[pillar] {
            continue;
        }
        taken[pillar] = true;
        let own = &stems.ids[pillar];
        let rarest = own
            .iter()
            .min_by_key(|id| stems.postings[**id as usize].len())
            .copied();
        let mut members = Vec::new();
        for &id in own {
            let posting = &stems.postings[id as usize];
            if posting.len() > cap && Some(id) != rarest {
                continue;
            }
            for &candidate in posting {
                if taken[candidate] || seen[candidate] == pillar {
                    continue;
                }
                seen[candidate] = pillar;
                let score = similarity(own, &stems.ids[candidate], &stems.weights);
                if score + 1e-12 >= options.threshold {
                    taken[candidate] = true;
                    members.push(Member {
                        index: candidate,
                        score: (score * 1000.0).round() / 1000.0,
                        shared: None,
                    });
                }
            }
        }
        clusters.push(finish(pillar, members, &texts));
    }
    Clustering { clusters }
}

#[cfg(test)]
mod tests;
