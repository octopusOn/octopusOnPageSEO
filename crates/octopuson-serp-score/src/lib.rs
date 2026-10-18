//! Explainable, versioned scores of the SERP Checker.
//!
//! Every score is a [`Breakdown`]: the number, the model version and each component with its
//! measured value, weight, contribution and an i18n reason key, so the user sees why a
//! keyword is hard and what a page lacks. A missing signal (no Open PageRank key, a page
//! that failed to load) is left out and the remaining weights renormalised; the breakdown's
//! confidence says how much of the model was backed by data. Scores are pure functions of
//! stored data, so a new model version can be applied to old analyses without new requests.
//!
//! - [`page::content_score`] (`content-1`): keyword placement, topical coverage against the
//!   terms most ranking pages share, depth and structure against their medians, "People also
//!   ask" coverage, media, structured data, snippet fit, freshness.
//! - [`page::technical_score`] (`tech-1`): Spider audit findings by priority, HTTPS, speed.
//! - [`site::site_score`] (`site-1`): Open PageRank, referring domains, Tranco, age.
//! - [`difficulty::difficulty`] (`kd-1`): click-weighted strength of the top ten, adjusted for
//!   brands, forums and ads.
//! - [`opportunity::opportunity`] (`opp-1`), [`intent::intent`] (`intent-1`) and
//!   [`recommend::recommend`].

pub mod breakdown;
pub mod context;
pub mod difficulty;
pub mod intent;
pub mod opportunity;
pub mod page;
pub mod recommend;
pub mod signals;
pub mod site;

use std::collections::BTreeMap;

use octopuson_serp::{FeatureKind, SerpSnapshot};
use serde::{Deserialize, Serialize};

pub use breakdown::{Breakdown, Component};
pub use difficulty::{Band, ResultStrength};
pub use intent::{Intent, IntentClass, keyword_intent};
pub use recommend::Recommendations;
pub use signals::{PageSignals, SiteSignals};

use crate::context::SerpContext;
use crate::opportunity::OpportunityInput;

/// Model versions an analysis was scored with.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Models {
    /// Content model.
    pub content: String,
    /// Technical model.
    pub technical: String,
    /// Site model.
    pub site: String,
    /// Difficulty model.
    pub difficulty: String,
    /// Intent model.
    pub intent: String,
}

impl Default for Models {
    fn default() -> Self {
        Self {
            content: page::CONTENT_MODEL.to_owned(),
            technical: page::TECHNICAL_MODEL.to_owned(),
            site: site::SITE_MODEL.to_owned(),
            difficulty: difficulty::DIFFICULTY_MODEL.to_owned(),
            intent: intent::INTENT_MODEL.to_owned(),
        }
    }
}

/// Scores of one ranking page.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PageScore {
    /// Organic position (0 for the user's own page when it does not rank).
    pub position: u16,
    /// URL.
    pub url: String,
    /// Registrable domain.
    pub domain: String,
    /// Content.
    pub content: Option<Breakdown>,
    /// Technical health.
    pub technical: Option<Breakdown>,
    /// Site strength.
    pub site: Option<Breakdown>,
    /// The blended strength used for keyword difficulty (0–100).
    pub strength: Option<f64>,
    /// Words of main content.
    pub words: usize,
}

/// The user's site, for opportunity and gaps.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TargetInput {
    /// Registrable domain.
    pub domain: String,
    /// Its site signals.
    pub site: SiteSignals,
    /// Its page for the keyword (the ranking page, or one the user named), if crawled.
    pub page: Option<PageSignals>,
    /// Its current position.
    pub position: Option<u16>,
}

/// The user's site against the keyword.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TargetScore {
    /// Domain.
    pub domain: String,
    /// Current position.
    pub position: Option<u16>,
    /// Scores of its page.
    pub page: Option<PageScore>,
    /// Opportunity.
    pub opportunity: Breakdown,
    /// Terms the ranking pages use that its page lacks.
    pub gaps: Vec<String>,
}

/// Everything an analysis needs.
#[derive(Debug, Clone)]
pub struct AnalysisInput<'a> {
    /// The result page.
    pub snapshot: &'a SerpSnapshot,
    /// Crawled ranking pages, by position.
    pub pages: &'a [PageSignals],
    /// Site signals by registrable domain.
    pub sites: &'a BTreeMap<String, SiteSignals>,
    /// Whether Tranco ranks are available.
    pub tranco: bool,
    /// Monthly searches, if known.
    pub volume: Option<u64>,
    /// The user's site.
    pub target: Option<TargetInput>,
}

/// The analysis of a keyword.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KeywordAnalysis {
    /// Model versions.
    pub models: Models,
    /// Keyword difficulty.
    pub difficulty: Breakdown,
    /// Its band.
    pub band: Band,
    /// Search intent.
    pub intent: Intent,
    /// Scores of the ranking pages.
    pub pages: Vec<PageScore>,
    /// Recommendations.
    pub recommendations: Recommendations,
    /// The user's site, if given.
    pub target: Option<TargetScore>,
    /// Monthly searches used.
    pub volume: Option<u64>,
}

fn is_brand(site: Option<&SiteSignals>) -> bool {
    site.is_some_and(|site| {
        site.tranco_rank.is_some_and(|rank| rank <= 10_000)
            || site.open_page_rank.is_some_and(|score| score >= 6.5)
    })
}

/// Scores one page in a context.
fn score_page(
    context: &SerpContext<'_>,
    page: &PageSignals,
    index: Option<usize>,
    intent: IntentClass,
    sites: &BTreeMap<String, SiteSignals>,
    tranco: bool,
) -> (PageScore, ResultStrength) {
    let terms = match index {
        Some(index) => context.terms[index].clone(),
        None => context.analyzer.analyze(&format!(
            "{}\n{}\n{}",
            page.title.as_deref().unwrap_or_default(),
            page.text
                .headings
                .iter()
                .map(|(_, text)| text.as_str())
                .collect::<Vec<_>>()
                .join("\n"),
            page.text.main
        )),
    };
    let technical = page::technical_score(page);
    let content = page
        .is_live()
        .then(|| page::content_score(context, page, &terms, index, intent));
    let site = sites.get(&page.domain).map(|site| {
        let mut site = site.clone();
        site.https = page.https;
        site.hsts = page.hsts;
        site.technical = Some(technical.score);
        site::site_score(&site, tranco)
    });
    let strength = ResultStrength {
        position: page.position,
        domain: page.domain.clone(),
        site: site.as_ref().map(|site| site.score),
        content: content.as_ref().map(|content| content.score),
        targeting: page.is_live().then(|| page::targeting(context, page)),
        strength: None,
        brand: is_brand(sites.get(&page.domain)),
    }
    .blend();
    (
        PageScore {
            position: page.position,
            url: page.url.clone(),
            domain: page.domain.clone(),
            content,
            technical: Some(technical),
            site,
            strength: strength
                .strength
                .map(|strength| breakdown::round(strength * 100.0)),
            words: page.text.main_words(),
        },
        strength,
    )
}

/// Opportunity and gaps of the user's site.
fn score_target(
    context: &SerpContext<'_>,
    input: &AnalysisInput<'_>,
    target: &TargetInput,
    intent: IntentClass,
    difficulty: f64,
) -> TargetScore {
    let ranked = target.page.as_ref().and_then(|page| {
        input
            .pages
            .iter()
            .position(|candidate| candidate.url == page.url)
    });
    let scored = target.page.as_ref().map(|page| {
        let mut sites = input.sites.clone();
        sites
            .entry(target.domain.clone())
            .or_insert_with(|| target.site.clone());
        score_page(context, page, ranked, intent, &sites, input.tranco)
    });
    let gaps = target
        .page
        .as_ref()
        .map(|page| {
            let terms = context.analyzer.analyze(&format!(
                "{}\n{}",
                page.title.as_deref().unwrap_or_default(),
                page.text.main
            ));
            context
                .consensus_without(ranked, 40)
                .into_iter()
                .filter(|term| !terms.has(&term.term.key))
                .map(|term| term.surface)
                .take(25)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let strength = scored
        .as_ref()
        .and_then(|(_, strength)| strength.strength)
        .or_else(|| Some(site::site_score(&target.site, input.tranco).score / 100.0))
        .map(|strength| strength * 100.0);
    let owns_snippet = input
        .snapshot
        .features_of(FeatureKind::FeaturedSnippet)
        .flat_map(|feature| feature.items.iter())
        .any(|item| item.domain.as_deref() == Some(target.domain.as_str()));
    TargetScore {
        domain: target.domain.clone(),
        position: target.position,
        page: scored.map(|(page, _)| page),
        opportunity: opportunity::opportunity(
            &OpportunityInput {
                strength,
                difficulty,
                volume: input.volume,
                position: target.position,
                owns_snippet,
            },
            input.snapshot,
        ),
        gaps,
    }
}

/// Analyses a keyword: scores every ranking page, the difficulty, the intent and, for a
/// target site, the opportunity and its gaps.
pub fn analyze(input: &AnalysisInput<'_>) -> KeywordAnalysis {
    let context = SerpContext::new(input.snapshot, input.pages);
    let intent = intent::intent(input.snapshot, input.pages);
    let mut pages = Vec::new();
    let mut strengths = Vec::new();
    for (index, page) in input.pages.iter().enumerate() {
        let (score, strength) = score_page(
            &context,
            page,
            Some(index),
            intent.primary,
            input.sites,
            input.tranco,
        );
        pages.push(score);
        strengths.push(strength);
    }
    // Ranking results that could not be crawled still count with their site strength.
    for result in input.snapshot.top(10) {
        if strengths
            .iter()
            .any(|strength| strength.position == result.position)
        {
            continue;
        }
        let site = input
            .sites
            .get(&result.domain)
            .map(|site| site::site_score(site, input.tranco).score);
        strengths.push(
            ResultStrength {
                position: result.position,
                domain: result.domain.clone(),
                site,
                content: None,
                targeting: None,
                strength: None,
                brand: is_brand(input.sites.get(&result.domain)),
            }
            .blend(),
        );
    }
    let difficulty = difficulty::difficulty(&strengths, input.snapshot);

    let target = input
        .target
        .as_ref()
        .map(|target| score_target(&context, input, target, intent.primary, difficulty.score));
    let gaps = target
        .as_ref()
        .filter(|target| !target.gaps.is_empty())
        .map(|target| target.gaps.clone());
    KeywordAnalysis {
        models: Models::default(),
        band: Band::of(difficulty.score),
        difficulty,
        intent,
        pages,
        recommendations: recommend::recommend(&context, gaps),
        target,
        volume: input.volume,
    }
}

#[cfg(test)]
mod tests;
