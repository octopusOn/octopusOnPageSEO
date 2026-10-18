//! What the top pages of a result page have in common.

use std::collections::BTreeMap;

use octopuson_serp::{FeatureKind, SerpSnapshot};
use octopuson_text::{Analyzer, ConsensusTerm, DocTerms, Language, consensus};

use crate::breakdown::median;
use crate::signals::PageSignals;

/// Share of pages a term must appear in to count as expected.
pub const CONSENSUS_SHARE: f64 = 0.4;

/// Share of pages a schema type must appear in to count as expected.
pub const SCHEMA_SHARE: f64 = 0.3;

/// The keyword and what the competing pages look like.
pub struct SerpContext<'a> {
    /// The keyword.
    pub keyword: &'a str,
    /// Text analyzer of the query's language.
    pub analyzer: Analyzer,
    /// Live pages, as crawled.
    pub pages: &'a [PageSignals],
    /// Term counts of each live page's main content, by index into `pages`.
    pub terms: Vec<DocTerms>,
    /// Median main-content words of live pages.
    pub median_words: f64,
    /// Median H2 count of live pages.
    pub median_h2: f64,
    /// Share of live pages using each schema type.
    pub schema_shares: BTreeMap<String, f64>,
    /// "People also ask" questions.
    pub questions: Vec<String>,
    /// Whether the page shows videos.
    pub has_videos: bool,
    /// The snapshot.
    pub snapshot: &'a SerpSnapshot,
}

impl<'a> SerpContext<'a> {
    /// Analyses the pages of a snapshot.
    #[allow(clippy::cast_precision_loss)] // page counts
    pub fn new(snapshot: &'a SerpSnapshot, pages: &'a [PageSignals]) -> Self {
        let language = Language::from_code(&snapshot.query.language);
        let analyzer = Analyzer::new(language);
        let terms: Vec<DocTerms> = pages
            .iter()
            .map(|page| {
                let headings = page
                    .text
                    .headings
                    .iter()
                    .map(|(_, text)| text.as_str())
                    .collect::<Vec<_>>()
                    .join("\n");
                analyzer.analyze(&format!(
                    "{}\n{headings}\n{}",
                    page.title.as_deref().unwrap_or_default(),
                    page.text.main
                ))
            })
            .collect();
        let live: Vec<&PageSignals> = pages.iter().filter(|page| page.is_live()).collect();
        #[allow(clippy::cast_precision_loss)]
        let mut words: Vec<f64> = live
            .iter()
            .map(|page| page.text.main_words() as f64)
            .collect();
        let mut h2: Vec<f64> = live
            .iter()
            .map(|page| {
                page.text
                    .headings
                    .iter()
                    .filter(|(level, _)| *level == 2)
                    .count() as f64
            })
            .collect();
        let mut schema_counts: BTreeMap<String, usize> = BTreeMap::new();
        for page in &live {
            let mut types: Vec<&String> = page.schema_types.iter().collect();
            types.sort_unstable();
            types.dedup();
            for schema in types {
                *schema_counts.entry(schema.clone()).or_default() += 1;
            }
        }
        let live_count = live.len().max(1) as f64;
        let questions = snapshot
            .features_of(FeatureKind::PeopleAlsoAsk)
            .flat_map(|feature| feature.items.iter().filter_map(|item| item.text.clone()))
            .collect();
        Self {
            keyword: snapshot.query.keyword.as_str(),
            analyzer,
            pages,
            median_words: median(&mut words),
            median_h2: median(&mut h2),
            schema_shares: schema_counts
                .into_iter()
                .map(|(schema, count)| (schema, count as f64 / live_count))
                .collect(),
            questions,
            has_videos: snapshot.has_feature(FeatureKind::Videos),
            terms,
            snapshot,
        }
    }

    /// Consensus terms of the live pages other than `except` (an index into `pages`).
    pub fn consensus_without(&self, except: Option<usize>, limit: usize) -> Vec<ConsensusTerm> {
        let documents: Vec<&DocTerms> = self
            .pages
            .iter()
            .enumerate()
            .filter(|(index, page)| page.is_live() && Some(*index) != except)
            .map(|(index, _)| &self.terms[index])
            .collect();
        consensus(&documents, CONSENSUS_SHARE, limit)
    }

    /// Schema types at least [`SCHEMA_SHARE`] of the live pages use.
    pub fn expected_schema(&self) -> Vec<&str> {
        self.schema_shares
            .iter()
            .filter(|(_, share)| **share >= SCHEMA_SHARE)
            .map(|(schema, _)| schema.as_str())
            .collect()
    }
}
