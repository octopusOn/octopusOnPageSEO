//! Clusters: groups of keywords one page can target, each led by a pillar keyword.

use serde::{Deserialize, Serialize};

/// A keyword of a cluster.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Member {
    /// Index of the keyword in the clustered list.
    pub index: usize,
    /// Similarity to the pillar (0–1; the pillar itself is 1).
    pub score: f64,
    /// Result-page URLs shared with the pillar, for clusters made from result pages.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shared: Option<u16>,
}

/// A group of keywords.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Cluster {
    /// Index of the keyword that leads it: the one with the most searches.
    pub pillar: usize,
    /// Its keywords, the pillar first, then by similarity.
    pub members: Vec<Member>,
    /// Whether result pages confirmed it (its pillar was compared by result page).
    #[serde(default)]
    pub verified: bool,
}

impl Cluster {
    /// Number of keywords.
    pub fn len(&self) -> usize {
        self.members.len()
    }

    /// Whether it has no keyword (never true for clusters made here).
    pub fn is_empty(&self) -> bool {
        self.members.is_empty()
    }

    /// Indices of its keywords.
    pub fn indices(&self) -> impl Iterator<Item = usize> + '_ {
        self.members.iter().map(|member| member.index)
    }
}

/// Every keyword of a list in exactly one cluster.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Clustering {
    /// Clusters, in the order their pillars were taken (most searched first).
    pub clusters: Vec<Cluster>,
}

impl Clustering {
    /// Cluster number of each keyword (`None` for keywords left out).
    pub fn assignment(&self, keywords: usize) -> Vec<Option<usize>> {
        let mut assignment = vec![None; keywords];
        for (number, cluster) in self.clusters.iter().enumerate() {
            for index in cluster.indices() {
                if let Some(slot) = assignment.get_mut(index) {
                    *slot = Some(number);
                }
            }
        }
        assignment
    }
}

/// The order pillars are taken in: most searches first (unknown volumes last), then the
/// keyword's text, so the result does not depend on the input order.
pub(crate) fn pillar_order(volumes: &[Option<u64>], texts: &[&str]) -> Vec<usize> {
    let mut order: Vec<usize> = (0..volumes.len()).collect();
    order.sort_by(|&a, &b| {
        volumes[b]
            .cmp(&volumes[a])
            .then_with(|| texts[a].cmp(texts[b]))
            .then_with(|| a.cmp(&b))
    });
    order
}

/// Finishes a cluster: the pillar first, then members by score and text.
pub(crate) fn finish(pillar: usize, mut members: Vec<Member>, texts: &[&str]) -> Cluster {
    members.sort_by(|a, b| {
        b.score
            .total_cmp(&a.score)
            .then_with(|| texts[a.index].cmp(texts[b.index]))
    });
    members.insert(
        0,
        Member {
            index: pillar,
            score: 1.0,
            shared: None,
        },
    );
    Cluster {
        pillar,
        members,
        verified: false,
    }
}
