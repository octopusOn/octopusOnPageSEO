//! What a cluster adds up to.

use std::collections::BTreeMap;

use seonexus_serp_score::IntentClass;
use serde::{Deserialize, Serialize};

use crate::cluster::Cluster;

/// What is known about one keyword.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct KeywordFacts {
    /// Monthly searches.
    pub volume: Option<u64>,
    /// Keyword difficulty (0–100).
    pub difficulty: Option<f64>,
    /// Cost per click (USD).
    pub cpc: Option<f64>,
    /// Search intent.
    pub intent: Option<IntentClass>,
}

/// Totals of a cluster.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClusterSummary {
    /// Keywords.
    pub keywords: usize,
    /// Monthly searches of all keywords with a known volume.
    pub volume: u64,
    /// The stated intent with the most searches, unless more searches state none.
    pub intent: Option<IntentClass>,
    /// Another intent has at least a quarter of its searches: one page may not serve both.
    pub mixed_intent: bool,
    /// Difficulty weighted by searches (plain mean where volumes are unknown).
    pub difficulty: Option<f64>,
    /// Cost per click weighted by searches.
    pub cpc: Option<f64>,
}

#[allow(clippy::cast_precision_loss)] // volumes and counts
fn weighted(values: &[(f64, f64)]) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    let weight: f64 = values.iter().map(|(_, weight)| weight).sum();
    let value = if weight > 0.0 {
        values
            .iter()
            .map(|(value, weight)| value * weight)
            .sum::<f64>()
            / weight
    } else {
        values.iter().map(|(value, _)| value).sum::<f64>() / values.len() as f64
    };
    Some((value * 10.0).round() / 10.0)
}

/// Totals of a cluster from the facts of each keyword (indexed like the clustered list).
#[allow(clippy::cast_precision_loss)] // volumes
pub fn summarize(cluster: &Cluster, facts: &[KeywordFacts]) -> ClusterSummary {
    let members: Vec<&KeywordFacts> = cluster
        .indices()
        .filter_map(|index| facts.get(index))
        .collect();
    let volume = members.iter().filter_map(|fact| fact.volume).sum();
    let weight = |fact: &KeywordFacts| fact.volume.map_or(0.0, |volume| volume as f64);
    // Intent shares by searches; keywords without a volume count as one search.
    let mut shares: BTreeMap<IntentClass, f64> = BTreeMap::new();
    let mut unstated = 0.0;
    for fact in &members {
        match fact.intent {
            Some(intent) => *shares.entry(intent).or_default() += weight(fact).max(1.0),
            None => unstated += weight(fact).max(1.0),
        }
    }
    let total: f64 = shares.values().sum();
    let mut ranked: Vec<(IntentClass, f64)> = shares.into_iter().collect();
    ranked.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    // A modifier on a few long-tail keywords does not decide for a pillar that states none.
    let intent = ranked
        .first()
        .filter(|(_, share)| *share >= unstated)
        .map(|(class, _)| *class);
    let mixed_intent = intent.is_some()
        && total > 0.0
        && ranked
            .get(1)
            .is_some_and(|(_, share)| share / total >= 0.25);
    let difficulty = weighted(
        &members
            .iter()
            .filter_map(|fact| fact.difficulty.map(|value| (value, weight(fact))))
            .collect::<Vec<_>>(),
    );
    let cpc = weighted(
        &members
            .iter()
            .filter_map(|fact| fact.cpc.map(|value| (value, weight(fact))))
            .collect::<Vec<_>>(),
    );
    ClusterSummary {
        keywords: members.len(),
        volume,
        intent,
        mixed_intent,
        difficulty,
        cpc,
    }
}
