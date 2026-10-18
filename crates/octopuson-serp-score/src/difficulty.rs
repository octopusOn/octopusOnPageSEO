//! How hard it is to reach the first page for a keyword.

use std::collections::BTreeMap;

use octopuson_serp::SerpSnapshot;
use serde::{Deserialize, Serialize};

use crate::breakdown::{Breakdown, Component, round};

/// Version of the keyword difficulty model.
pub const DIFFICULTY_MODEL: &str = "kd-1";

/// Share of clicks by organic position 1–10 (a typical desktop curve; versioned with the
/// model).
pub const CTR: [f64; 10] = [0.28, 0.15, 0.10, 0.07, 0.05, 0.04, 0.03, 0.025, 0.02, 0.018];

/// Forums and user-generated content sites, which rank on site strength more than on
/// matching content; their presence means a focused page has room.
pub const UGC: &[&str] = &[
    "reddit.com",
    "quora.com",
    "eksisozluk.com",
    "eksisozluk1923.com",
    "donanimhaber.com",
    "technopat.net",
    "sikayetvar.com",
    "r10.net",
    "kadinlarkulubu.com",
    "forum.shiftdelete.net",
    "uzmantv.com",
    "stackexchange.com",
    "stackoverflow.com",
    "medium.com",
    "pinterest.com",
    "tiktok.com",
    "facebook.com",
    "instagram.com",
    "x.com",
    "twitter.com",
    "youtube.com",
];

/// Difficulty bands, from very easy to extreme.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Band {
    /// 0–14.
    VeryEasy,
    /// 15–29.
    Easy,
    /// 30–49.
    Medium,
    /// 50–69.
    Hard,
    /// 70–84.
    VeryHard,
    /// 85–100.
    Extreme,
}

impl Band {
    /// The band of a score.
    pub fn of(score: f64) -> Self {
        match score {
            score if score < 15.0 => Self::VeryEasy,
            score if score < 30.0 => Self::Easy,
            score if score < 50.0 => Self::Medium,
            score if score < 70.0 => Self::Hard,
            score if score < 85.0 => Self::VeryHard,
            _ => Self::Extreme,
        }
    }
}

/// How strong one top result is.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResultStrength {
    /// Organic position.
    pub position: u16,
    /// Registrable domain.
    pub domain: String,
    /// Site score, when known.
    pub site: Option<f64>,
    /// Content score, when the page was read.
    pub content: Option<f64>,
    /// How well its title targets the keyword (0–1).
    pub targeting: Option<f64>,
    /// The blend: 0.55 site + 0.30 content + 0.15 targeting, over the parts known (0–1).
    pub strength: Option<f64>,
    /// Whether the site is a major brand (Tranco top 10,000 or Open PageRank ≥ 6.5).
    pub brand: bool,
}

impl ResultStrength {
    /// Blends the known parts.
    #[must_use]
    pub fn blend(mut self) -> Self {
        let parts = [
            (0.55, self.site.map(|site| site / 100.0)),
            (0.30, self.content.map(|content| content / 100.0)),
            (0.15, self.targeting),
        ];
        let weight: f64 = parts
            .iter()
            .filter(|(_, value)| value.is_some())
            .map(|(weight, _)| weight)
            .sum();
        self.strength = (weight > 0.0).then(|| {
            parts
                .iter()
                .filter_map(|(weight, value)| value.map(|value| weight * value))
                .sum::<f64>()
                / weight
        });
        self
    }
}

fn component(
    id: &str,
    value: Option<f64>,
    normalized: Option<f64>,
    contribution: f64,
    reason: &str,
) -> Component {
    Component {
        id: id.to_owned(),
        weight: if id == "competition" { 1.0 } else { 0.0 },
        value,
        normalized,
        contribution: round(contribution),
        reason: reason.to_owned(),
        params: BTreeMap::new(),
    }
}

/// Keyword difficulty from the strength of the top ten results, weighted by the clicks their
/// positions get, adjusted for brand dominance, forums and ads.
#[allow(clippy::cast_precision_loss)] // counts of ten
pub fn difficulty(results: &[ResultStrength], snapshot: &SerpSnapshot) -> Breakdown {
    let top: Vec<&ResultStrength> = results
        .iter()
        .filter(|result| (1..=10).contains(&result.position))
        .collect();
    let mut weighted = 0.0;
    let mut weights = 0.0;
    let mut known = 0usize;
    for result in &top {
        let ctr = CTR[usize::from(result.position - 1)];
        if let Some(strength) = result.strength {
            weighted += ctr * strength;
            weights += ctr;
            known += 1;
        }
    }
    let base = if weights > 0.0 {
        weighted / weights
    } else {
        0.5
    };
    let count = top.len().max(1) as f64;
    let brands = top.iter().filter(|result| result.brand).count() as f64 / count;
    let forums = top
        .iter()
        .filter(|result| {
            UGC.iter()
                .any(|ugc| result.domain == *ugc || result.domain.ends_with(&format!(".{ugc}")))
        })
        .count() as f64
        / count;
    let ads = snapshot.top_ad_count();
    let ads_points = ((ads as f64 - 2.0) / 2.0).clamp(0.0, 1.0) * 5.0;

    let mut components = vec![
        component(
            "competition",
            Some(base),
            Some(base),
            base * 100.0,
            "kd.competition",
        ),
        component("brands", Some(brands), None, brands * 10.0, "kd.brands"),
        component(
            "forums",
            Some(forums),
            None,
            0.0 - forums * 10.0,
            "kd.forums",
        ),
        component("ads", Some(ads as f64), None, ads_points, "kd.ads"),
    ];
    components[0]
        .params
        .insert("results".to_owned(), top.len().into());
    components[0]
        .params
        .insert("measured".to_owned(), known.into());
    components[1].params.insert(
        "domains".to_owned(),
        top.iter()
            .filter(|result| result.brand)
            .map(|result| result.domain.clone())
            .collect::<Vec<_>>()
            .into(),
    );
    let score = components
        .iter()
        .map(|component| component.contribution)
        .sum::<f64>()
        .clamp(0.0, 100.0);
    Breakdown {
        score: round(score),
        model: DIFFICULTY_MODEL.to_owned(),
        confidence: round(known as f64 / count),
        components,
    }
}

#[cfg(test)]
mod tests {
    use octopuson_serp::{Engine, SerpQuery, SnapshotBuilder};

    use super::*;

    fn result(position: u16, domain: &str, site: f64, content: f64, brand: bool) -> ResultStrength {
        ResultStrength {
            position,
            domain: domain.to_owned(),
            site: Some(site),
            content: Some(content),
            targeting: Some(1.0),
            strength: None,
            brand,
        }
        .blend()
    }

    #[test]
    fn strong_brands_make_keywords_hard_and_forums_easier() {
        let snapshot = SnapshotBuilder::new(
            SerpQuery::new(Engine::Google, "x"),
            "t",
            "2026-10-18T00:00:00Z",
        )
        .finish();
        let strong: Vec<_> = (1..=10)
            .map(|position| result(position, &format!("brand{position}.com"), 90.0, 85.0, true))
            .collect();
        let hard = difficulty(&strong, &snapshot);
        assert!(hard.score >= 85.0, "{hard:?}");
        assert_eq!(Band::of(hard.score), Band::Extreme);

        let weak: Vec<_> = (1..=10)
            .map(|position| {
                let domain = if position % 2 == 0 {
                    "reddit.com".to_owned()
                } else {
                    format!("small{position}.com.tr")
                };
                result(position, &domain, 15.0, 25.0, false)
            })
            .collect();
        let easy = difficulty(&weak, &snapshot);
        assert!(easy.score < 30.0, "{easy:?}");
        assert!(easy.component("forums").unwrap().contribution < 0.0);
        assert!((easy.confidence - 1.0).abs() < 1e-9);
    }

    #[test]
    fn unknown_strength_counts_as_medium_with_no_confidence() {
        let snapshot = SnapshotBuilder::new(
            SerpQuery::new(Engine::Google, "x"),
            "t",
            "2026-10-18T00:00:00Z",
        )
        .finish();
        let unknown = ResultStrength {
            position: 1,
            domain: "a.com".to_owned(),
            site: None,
            content: None,
            targeting: None,
            strength: None,
            brand: false,
        }
        .blend();
        let kd = difficulty(&[unknown], &snapshot);
        assert!((kd.score - 50.0).abs() < 1e-9);
        assert_eq!(kd.confidence, 0.0);
    }
}
