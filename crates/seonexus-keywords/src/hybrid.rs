//! Wording first, result pages where they were fetched.
//!
//! Keywords with a result page are grouped by result pages ([`crate::serp`]). Keywords
//! without one follow the wording cluster they fall in: they join the result-page cluster
//! that most of their wording neighbours with result pages joined, or stay together as an
//! unverified cluster when none of them has a result page. Members placed by wording keep
//! `shared: None`, so the interface can mark them as unverified.

use std::collections::HashMap;

use crate::cluster::{Clustering, Member, finish};
use crate::lexical::{Input, LexicalOptions, cluster_lexical};
use crate::serp::{SerpInput, SerpOptions, cluster_serp};

/// Groups keywords by result pages where they were fetched and by wording elsewhere. Every
/// input is in exactly one cluster.
pub fn cluster_hybrid(
    inputs: &[SerpInput<'_>],
    lexical: &LexicalOptions,
    serp: &SerpOptions,
) -> Clustering {
    let words: Vec<Input<'_>> = inputs
        .iter()
        .map(|input| Input {
            keyword: input.keyword,
            volume: input.volume,
        })
        .collect();
    let wording = cluster_lexical(&words, lexical);

    // Result-page clusters of the keywords that have result pages.
    let with_pages: Vec<usize> = (0..inputs.len())
        .filter(|index| inputs[*index].results.is_some())
        .collect();
    let subset: Vec<SerpInput<'_>> = with_pages.iter().map(|index| inputs[*index]).collect();
    let pages = cluster_serp(&subset, serp);
    let mut clusters: Vec<crate::cluster::Cluster> = pages
        .clusters
        .into_iter()
        .map(|mut cluster| {
            cluster.pillar = with_pages[cluster.pillar];
            for member in &mut cluster.members {
                member.index = with_pages[member.index];
            }
            cluster
        })
        .collect();
    let mut home: HashMap<usize, usize> = HashMap::new();
    for (number, cluster) in clusters.iter().enumerate() {
        for index in cluster.indices() {
            home.insert(index, number);
        }
    }

    let texts: Vec<&str> = inputs.iter().map(|input| input.keyword).collect();
    for group in &wording.clusters {
        let without: Vec<&Member> = group
            .members
            .iter()
            .filter(|member| inputs[member.index].results.is_none())
            .collect();
        if without.is_empty() {
            continue;
        }
        // The result-page cluster most of this wording group's fetched keywords joined.
        let mut votes: HashMap<usize, usize> = HashMap::new();
        for member in &group.members {
            if let Some(number) = home.get(&member.index) {
                *votes.entry(*number).or_default() += 1;
            }
        }
        let winner = votes
            .into_iter()
            .max_by(|a, b| a.1.cmp(&b.1).then_with(|| b.0.cmp(&a.0)))
            .map(|(number, _)| number);
        let placed: Vec<Member> = without
            .iter()
            .map(|member| Member {
                index: member.index,
                score: member.score,
                shared: None,
            })
            .collect();
        if let Some(number) = winner {
            let cluster = &mut clusters[number];
            cluster.members.extend(placed);
            let pillar = cluster.members.remove(0);
            let mut rest = std::mem::take(&mut cluster.members);
            rest.sort_by(|a, b| {
                b.shared
                    .is_some()
                    .cmp(&a.shared.is_some())
                    .then_with(|| b.score.total_cmp(&a.score))
                    .then_with(|| texts[a.index].cmp(texts[b.index]))
            });
            rest.insert(0, pillar);
            cluster.members = rest;
        } else {
            // No result page in the group: it stays a wording cluster, led by its most
            // searched keyword.
            let pillar = placed[0].index;
            let others = placed.into_iter().skip(1).collect();
            clusters.push(finish(pillar, others, &texts));
        }
    }
    Clustering { clusters }
}
