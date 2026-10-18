//! Manual placements: keywords the user moved, kept when the list is clustered again.

use crate::cluster::{Cluster, Clustering, Member, pillar_order};

/// Keyword `keyword` belongs with keyword `pillar`; a keyword placed with itself leads a
/// cluster of its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Placement {
    /// Index of the moved keyword.
    pub keyword: usize,
    /// Index of the keyword whose cluster it joins.
    pub pillar: usize,
}

/// Applies placements to a clustering of keywords with `volumes` and `texts`.
///
/// Placements of a keyword with itself go first, so others can join the cluster it now
/// leads. A keyword joins whichever cluster its pillar keyword ends up in; placements with
/// keywords that are not clustered are skipped. A moved keyword gets score 0 (its similarity
/// to the new pillar was not measured). A cluster that loses its pillar is led by its most
/// searched remaining keyword; clusters left empty are removed.
pub fn place(
    clustering: &Clustering,
    placements: &[Placement],
    volumes: &[Option<u64>],
    texts: &[&str],
) -> Clustering {
    let count = volumes.len().min(texts.len());
    let mut clusters = clustering.clusters.clone();
    let mut home = clustering.assignment(count);
    let (own, joining): (Vec<&Placement>, Vec<&Placement>) = placements
        .iter()
        .partition(|placement| placement.keyword == placement.pillar);
    for placement in own.into_iter().chain(joining) {
        let (keyword, pillar) = (placement.keyword, placement.pillar);
        if keyword >= count || pillar >= count {
            continue;
        }
        let Some(from) = home[keyword] else {
            continue;
        };
        let to = if keyword == pillar {
            if clusters[from].pillar == keyword {
                continue;
            }
            clusters.push(Cluster {
                pillar: keyword,
                members: Vec::new(),
                verified: false,
            });
            clusters.len() - 1
        } else {
            match home[pillar] {
                Some(to) if to != from => to,
                _ => continue,
            }
        };
        clusters[from]
            .members
            .retain(|member| member.index != keyword);
        clusters[to].members.push(Member {
            index: keyword,
            score: if keyword == pillar { 1.0 } else { 0.0 },
            shared: None,
        });
        home[keyword] = Some(to);
    }
    for cluster in &mut clusters {
        if cluster.members.is_empty() {
            continue;
        }
        if !cluster.indices().any(|index| index == cluster.pillar) {
            let indices: Vec<usize> = cluster.indices().collect();
            let member_volumes: Vec<Option<u64>> =
                indices.iter().map(|&index| volumes[index]).collect();
            let member_texts: Vec<&str> = indices.iter().map(|&index| texts[index]).collect();
            if let Some(&first) = pillar_order(&member_volumes, &member_texts).first() {
                cluster.pillar = indices[first];
            }
        }
        if let Some(position) = cluster
            .members
            .iter()
            .position(|member| member.index == cluster.pillar)
        {
            let mut leader = cluster.members.remove(position);
            leader.score = 1.0;
            cluster.members.insert(0, leader);
        }
    }
    clusters.retain(|cluster| !cluster.is_empty());
    Clustering { clusters }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn member(index: usize, score: f64) -> Member {
        Member {
            index,
            score,
            shared: None,
        }
    }

    /// Keywords 0–2 in one cluster led by 0, keyword 3 alone.
    fn start() -> Clustering {
        Clustering {
            clusters: vec![
                Cluster {
                    pillar: 0,
                    members: vec![member(0, 1.0), member(1, 0.8), member(2, 0.7)],
                    verified: true,
                },
                Cluster {
                    pillar: 3,
                    members: vec![member(3, 1.0)],
                    verified: false,
                },
            ],
        }
    }

    const VOLUMES: [Option<u64>; 4] = [Some(100), Some(50), Some(70), None];
    const TEXTS: [&str; 4] = ["a", "b", "c", "d"];

    #[test]
    fn moves_keywords_and_keeps_every_keyword_once() {
        let placements = [
            // 2 leads its own cluster, 1 joins it, 3 joins 0.
            Placement {
                keyword: 1,
                pillar: 2,
            },
            Placement {
                keyword: 2,
                pillar: 2,
            },
            Placement {
                keyword: 3,
                pillar: 0,
            },
        ];
        let placed = place(&start(), &placements, &VOLUMES, &TEXTS);
        let assignment = placed.assignment(4);
        assert_eq!(assignment, [Some(0), Some(1), Some(1), Some(0)]);
        assert_eq!(placed.clusters.len(), 2);
        assert_eq!(placed.clusters[1].pillar, 2);
        assert_eq!(placed.clusters[0].members[1], member(3, 0.0));
    }

    #[test]
    fn a_cluster_that_loses_its_pillar_is_led_by_its_most_searched_keyword() {
        let placements = [Placement {
            keyword: 0,
            pillar: 3,
        }];
        let placed = place(&start(), &placements, &VOLUMES, &TEXTS);
        assert_eq!(placed.clusters[0].pillar, 2);
        assert_eq!(placed.clusters[0].members[0], member(2, 1.0));
        assert_eq!(placed.clusters[1].pillar, 3);
        assert_eq!(placed.assignment(4), [Some(1), Some(0), Some(0), Some(1)]);
    }

    #[test]
    fn skips_placements_with_unknown_or_unclustered_keywords() {
        let placements = [
            Placement {
                keyword: 9,
                pillar: 0,
            },
            Placement {
                keyword: 1,
                pillar: 9,
            },
            // Already there; a pillar placed with itself stays.
            Placement {
                keyword: 1,
                pillar: 0,
            },
            Placement {
                keyword: 0,
                pillar: 0,
            },
        ];
        assert_eq!(place(&start(), &placements, &VOLUMES, &TEXTS), start());
    }
}
