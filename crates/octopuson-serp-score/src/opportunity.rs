//! Whether a keyword is worth going after for a given site.

use octopuson_serp::{FeatureKind, SerpSnapshot};

use crate::breakdown::{Breakdown, Builder, round};

/// Version of the opportunity model.
pub const OPPORTUNITY_MODEL: &str = "opp-1";

/// What is known about the site that wants to rank.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OpportunityInput {
    /// Strength of the site's page for the keyword, 0–100 (site, content and targeting
    /// blended like the results'), if known.
    pub strength: Option<f64>,
    /// Keyword difficulty.
    pub difficulty: f64,
    /// Monthly searches, if known.
    pub volume: Option<u64>,
    /// Current organic position, if the site ranks.
    pub position: Option<u16>,
    /// Domain of the site, to tell whether it owns the featured snippet.
    pub owns_snippet: bool,
}

/// Opportunity from 0 to 100: chance to win, demand, clicks left over by the result page's
/// own blocks, and how close the site already is.
#[allow(clippy::cast_precision_loss)] // volumes and counts
pub fn opportunity(input: &OpportunityInput, snapshot: &SerpSnapshot) -> Breakdown {
    let mut builder = Builder::new();
    let win = input
        .strength
        .map(|strength| 1.0 / (1.0 + (-(strength - input.difficulty) / 15.0).exp()));
    builder
        .add(
            "win",
            40.0,
            input.strength,
            win,
            if win.is_some() {
                "opportunity.win"
            } else {
                "opportunity.win.unknown"
            },
        )
        .with("difficulty", round(input.difficulty));
    let demand = input
        .volume
        .map(|volume| ((1.0 + volume as f64).log10() / 5.0).min(1.0));
    builder.add(
        "demand",
        25.0,
        input.volume.map(|volume| volume as f64),
        demand,
        if demand.is_some() {
            "opportunity.demand"
        } else {
            "opportunity.demand.unknown"
        },
    );
    let first_page =
        |kind: FeatureKind| snapshot.features_of(kind).any(|feature| feature.page == 1);
    let mut suppression = 0.0;
    let mut takers = Vec::new();
    for (kind, share) in [
        (FeatureKind::AiOverview, 0.25),
        (FeatureKind::LocalPack, 0.15),
    ] {
        if first_page(kind) {
            suppression += share;
            takers.push(kind.as_str());
        }
    }
    if first_page(FeatureKind::FeaturedSnippet) && !input.owns_snippet {
        suppression += 0.10;
        takers.push(FeatureKind::FeaturedSnippet.as_str());
    }
    let ads = snapshot.top_ad_count() as f64;
    suppression += (0.1 * ads).min(0.3);
    let clicks = (1.0 - suppression).max(0.2);
    builder
        .add(
            "clicks",
            20.0,
            Some(suppression),
            Some(clicks),
            "opportunity.clicks",
        )
        .with("features", takers)
        .with("ads", snapshot.top_ad_count());
    let proximity = match input.position {
        Some(1..=3) => 0.4,
        Some(4..=20) => 1.0,
        Some(_) => 0.8,
        None => 0.6,
    };
    builder
        .add(
            "position",
            15.0,
            input.position.map(f64::from),
            Some(proximity),
            "opportunity.position",
        )
        .with("position", input.position);
    builder.finish(OPPORTUNITY_MODEL)
}

#[cfg(test)]
mod tests {
    use octopuson_serp::{Engine, SerpQuery, SnapshotBuilder};

    use super::*;

    #[test]
    fn striking_distance_with_demand_is_an_opportunity() {
        let snapshot = SnapshotBuilder::new(
            SerpQuery::new(Engine::Google, "x"),
            "t",
            "2026-10-18T00:00:00Z",
        )
        .finish();
        let good = opportunity(
            &OpportunityInput {
                strength: Some(70.0),
                difficulty: 35.0,
                volume: Some(20_000),
                position: Some(8),
                owns_snippet: false,
            },
            &snapshot,
        );
        let poor = opportunity(
            &OpportunityInput {
                strength: Some(20.0),
                difficulty: 80.0,
                volume: Some(30),
                position: None,
                owns_snippet: false,
            },
            &snapshot,
        );
        assert!(good.score > 80.0, "{good:?}");
        assert!(poor.score < 50.0, "{poor:?}");
    }
}
