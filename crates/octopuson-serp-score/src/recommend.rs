//! What a page needs to compete, read off the pages that rank.

use octopuson_audit::limits::TITLE_FONT_PX;
use octopuson_audit::pixel_width;
use octopuson_serp::FeatureKind;
use octopuson_text::{DocTerms, consensus};
use serde::{Deserialize, Serialize};

use crate::breakdown::{percentile, round};
use crate::context::SerpContext;

/// A range from the competing pages.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Range {
    /// 25th percentile.
    pub low: f64,
    /// Median.
    pub median: f64,
    /// 75th percentile.
    pub high: f64,
}

/// How the ranking titles are written.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct TitlePattern {
    /// Median length in characters.
    pub median_chars: f64,
    /// Median width in pixels.
    pub median_px: f64,
    /// Share of titles that start with the keyword.
    pub keyword_first: f64,
    /// Share of titles with a year.
    pub with_year: f64,
    /// Share of titles with another number.
    pub with_number: f64,
}

/// What to write.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Recommendations {
    /// Words of main content.
    pub words: Option<Range>,
    /// H2 headings.
    pub h2: Option<Range>,
    /// Topics several ranking pages have as headings.
    pub headings: Vec<String>,
    /// Structured data types most ranking pages use.
    pub schema: Vec<String>,
    /// How the ranking titles are written.
    pub title: TitlePattern,
    /// Questions searchers ask ("People also ask"), for an FAQ section.
    pub questions: Vec<String>,
    /// The page shows an answer box or AI overview: open with a short, direct answer.
    pub short_answer: bool,
    /// Terms the ranking pages use that the analysed page lacks (or the top terms when no
    /// page is analysed).
    pub terms: Vec<String>,
    /// Keyword ideas from questions and related searches.
    pub ideas: Vec<String>,
}

/// Recommendations for a page; `gaps` are its missing terms when there is a page to compare.
#[allow(clippy::cast_precision_loss)] // counts of pages
pub fn recommend(context: &SerpContext<'_>, gaps: Option<Vec<String>>) -> Recommendations {
    let live: Vec<usize> = (0..context.pages.len())
        .filter(|index| context.pages[*index].is_live())
        .collect();
    let range = |mut values: Vec<f64>| {
        (!values.is_empty()).then(|| Range {
            low: round(percentile(&mut values, 0.25)),
            median: round(percentile(&mut values, 0.5)),
            high: round(percentile(&mut values, 0.75)),
        })
    };
    let words = range(
        live.iter()
            .map(|index| context.pages[*index].text.main_words() as f64)
            .collect(),
    );
    let h2 = range(
        live.iter()
            .map(|index| {
                context.pages[*index]
                    .text
                    .headings
                    .iter()
                    .filter(|(level, _)| *level == 2)
                    .count() as f64
            })
            .collect(),
    );

    let headings = heading_topics(context, &live);
    let title = title_pattern(context, &live);

    let snapshot = context.snapshot;
    let first_page =
        |kind: FeatureKind| snapshot.features_of(kind).any(|feature| feature.page == 1);
    let mut ideas: Vec<String> = context.questions.clone();
    ideas.extend(
        snapshot
            .features_of(FeatureKind::RelatedSearches)
            .flat_map(|feature| feature.items.iter().filter_map(|item| item.text.clone())),
    );
    ideas.dedup();

    Recommendations {
        words,
        h2,
        headings,
        schema: context
            .expected_schema()
            .into_iter()
            .map(str::to_owned)
            .collect(),
        title,
        questions: context.questions.clone(),
        short_answer: first_page(FeatureKind::FeaturedSnippet)
            || first_page(FeatureKind::AiOverview),
        terms: gaps.unwrap_or_else(|| {
            context
                .consensus_without(None, 25)
                .into_iter()
                .map(|term| term.surface)
                .collect()
        }),
        ideas,
    }
}

/// Topics several ranking pages have as headings: consensus over their H2/H3 texts.
fn heading_topics(context: &SerpContext<'_>, live: &[usize]) -> Vec<String> {
    let heading_terms: Vec<DocTerms> = live
        .iter()
        .map(|index| {
            let text = context.pages[*index]
                .text
                .headings
                .iter()
                .filter(|(level, _)| (2..=3).contains(level))
                .map(|(_, text)| text.as_str())
                .collect::<Vec<_>>()
                .join(". ");
            context.analyzer.analyze(&text)
        })
        .collect();
    let heading_refs: Vec<&DocTerms> = heading_terms.iter().collect();
    let keyword_key = context.analyzer.key(context.keyword);
    consensus(&heading_refs, 0.3, 12)
        .into_iter()
        .filter(|term| term.term.key != keyword_key)
        .map(|term| term.surface)
        .collect()
}

/// How the ranking titles are written.
#[allow(clippy::cast_precision_loss)] // counts of pages
fn title_pattern(context: &SerpContext<'_>, live: &[usize]) -> TitlePattern {
    let titles: Vec<&str> = live
        .iter()
        .filter_map(|index| context.pages[*index].title.as_deref())
        .collect();
    let count = titles.len().max(1) as f64;
    let mut chars: Vec<f64> = titles
        .iter()
        .map(|title| title.chars().count() as f64)
        .collect();
    let mut pixels: Vec<f64> = titles
        .iter()
        .map(|title| f64::from(pixel_width(title, TITLE_FONT_PX)))
        .collect();
    let starts = titles
        .iter()
        .filter(|title| {
            let opening: String = title
                .split_whitespace()
                .take(context.keyword.split_whitespace().count() + 1)
                .collect::<Vec<_>>()
                .join(" ");
            context.analyzer.contains(&opening, context.keyword)
        })
        .count() as f64;

    let years = titles.iter().filter(|title| has_year(title)).count() as f64;
    let numbers = titles
        .iter()
        .filter(|title| !has_year(title) && title.chars().any(|ch| ch.is_ascii_digit()))
        .count() as f64;

    TitlePattern {
        median_chars: round(percentile(&mut chars, 0.5)),
        median_px: round(percentile(&mut pixels, 0.5)),
        keyword_first: round(starts / count),
        with_year: round(years / count),
        with_number: round(numbers / count),
    }
}

/// Whether a title mentions a year (2000–2099).
fn has_year(title: &str) -> bool {
    title
        .split(|ch: char| !ch.is_ascii_digit())
        .any(|digits| digits.len() == 4 && digits.starts_with("20"))
}
