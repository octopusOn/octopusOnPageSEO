//! Grouping by result pages.
//!
//! Two keywords belong on one page when search engines answer them with the same pages: the
//! number of top-ten addresses their result pages share is the measure, as in SE Ranking,
//! Serpstat and Keyword Insights. Pillars are taken by search volume. In **soft** mode a
//! keyword joins when it shares at least the threshold with the pillar; in **hard** mode it
//! must share that much with every keyword already in the cluster (tighter groups, more of
//! them).

use std::collections::HashMap;

use seonexus_serp::registrable_domain;
use serde::{Deserialize, Serialize};
use url::Url;

use crate::cluster::{Cluster, Clustering, Member, finish, pillar_order};

/// How keywords are compared with a cluster.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    /// With its pillar.
    #[default]
    Soft,
    /// With every member.
    Hard,
}

/// What two result pages compare.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    /// Page addresses (the usual choice).
    #[default]
    Page,
    /// Sites: for result pages whose addresses are hidden (Google in the browser mode) or for
    /// coarser topics.
    Site,
}

/// Settings of a result-page clustering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct SerpOptions {
    /// Least shared results (1–9).
    pub threshold: u16,
    /// Soft or hard.
    pub mode: Mode,
    /// Pages or sites.
    pub level: Level,
    /// Results compared from the top of each page.
    pub depth: usize,
}

impl Default for SerpOptions {
    fn default() -> Self {
        Self {
            threshold: 3,
            mode: Mode::Soft,
            level: Level::Page,
            depth: 10,
        }
    }
}

/// A keyword and its result page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SerpInput<'a> {
    /// The keyword.
    pub keyword: &'a str,
    /// Monthly searches, if known.
    pub volume: Option<u64>,
    /// Addresses of its results from the top; `None` when no result page was fetched.
    pub results: Option<&'a [String]>,
}

/// What a result is compared by: an address without scheme, `www.`, query, fragment and
/// trailing slash, or its registrable domain.
pub fn result_key(address: &str, level: Level) -> Option<String> {
    let url = Url::parse(address.trim()).ok()?;
    let host = url.host_str()?.trim_end_matches('.').to_ascii_lowercase();
    let host = host.strip_prefix("www.").unwrap_or(&host).to_owned();
    match level {
        Level::Site => Some(registrable_domain(&host)),
        Level::Page => {
            let path = url.path().trim_end_matches('/');
            Some(format!("{host}{path}"))
        }
    }
}

/// Result keys of every keyword (sorted, without duplicates), and which keywords each key
/// appears in.
fn index(
    inputs: &[SerpInput<'_>],
    options: &SerpOptions,
) -> (Vec<Option<Vec<u32>>>, Vec<Vec<usize>>) {
    let mut ids: HashMap<String, u32> = HashMap::new();
    let keys: Vec<Option<Vec<u32>>> = inputs
        .iter()
        .map(|input| {
            input.results.map(|results| {
                let mut own: Vec<u32> = results
                    .iter()
                    .take(options.depth)
                    .filter_map(|address| result_key(address, options.level))
                    .map(|key| {
                        let next = u32::try_from(ids.len()).unwrap_or(u32::MAX);
                        *ids.entry(key).or_insert(next)
                    })
                    .collect();
                own.sort_unstable();
                own.dedup();
                own
            })
        })
        .collect();
    let mut postings = vec![Vec::new(); ids.len()];
    for (index, own) in keys.iter().enumerate() {
        for &id in own.iter().flatten() {
            postings[id as usize].push(index);
        }
    }
    (keys, postings)
}

/// Results two sorted id lists share.
pub(crate) fn shared(left: &[u32], right: &[u32]) -> u16 {
    let mut count = 0u16;
    let mut rest = right.iter().peekable();
    for id in left {
        while rest.next_if(|other| *other < id).is_some() {}
        if rest.next_if_eq(&id).is_some() {
            count = count.saturating_add(1);
        }
    }
    count
}

/// Groups keywords by result pages. Every input is in exactly one cluster; keywords without
/// a result page stay alone.
pub fn cluster_serp(inputs: &[SerpInput<'_>], options: &SerpOptions) -> Clustering {
    let (keys, postings) = index(inputs, options);
    let texts: Vec<&str> = inputs.iter().map(|input| input.keyword).collect();
    let volumes: Vec<Option<u64>> = inputs.iter().map(|input| input.volume).collect();
    let threshold = options.threshold.max(1);
    let mut taken = vec![false; inputs.len()];
    let mut clusters = Vec::new();
    for pillar in pillar_order(&volumes, &texts) {
        if taken[pillar] {
            continue;
        }
        taken[pillar] = true;
        let Some(own) = &keys[pillar] else {
            clusters.push(finish(pillar, Vec::new(), &texts));
            continue;
        };
        // Candidates share at least one result; count what they share with the pillar.
        let mut counts: HashMap<usize, u16> = HashMap::new();
        for &id in own {
            for &candidate in &postings[id as usize] {
                if candidate != pillar && !taken[candidate] {
                    *counts.entry(candidate).or_default() += 1;
                }
            }
        }
        let mut candidates: Vec<(usize, u16)> = counts
            .into_iter()
            .filter(|(_, count)| *count >= threshold)
            .collect();
        // Closest first, then by the pillar order, so hard mode is deterministic.
        candidates.sort_by(|a, b| {
            b.1.cmp(&a.1)
                .then_with(|| volumes[b.0].cmp(&volumes[a.0]))
                .then_with(|| texts[a.0].cmp(texts[b.0]))
        });
        let mut members: Vec<Member> = Vec::new();
        for (candidate, count) in candidates {
            let Some(theirs) = &keys[candidate] else {
                continue;
            };
            if options.mode == Mode::Hard
                && members.iter().any(|member| {
                    keys[member.index]
                        .as_ref()
                        .is_none_or(|other| shared(theirs, other) < threshold)
                })
            {
                continue;
            }
            taken[candidate] = true;
            members.push(Member {
                index: candidate,
                score: score(count, own.len().min(theirs.len())),
                shared: Some(count),
            });
        }
        let mut cluster: Cluster = finish(pillar, members, &texts);
        cluster.members[0].shared = Some(u16::try_from(own.len()).unwrap_or(u16::MAX));
        cluster.verified = true;
        clusters.push(cluster);
    }
    Clustering { clusters }
}

/// Shared results as a share of the shorter result list.
fn score(shared: u16, shorter: usize) -> f64 {
    if shorter == 0 {
        return 0.0;
    }
    let share = f64::from(shared) / f64::from(u16::try_from(shorter).unwrap_or(u16::MAX));
    (share.min(1.0) * 1000.0).round() / 1000.0
}

#[cfg(test)]
mod tests;
