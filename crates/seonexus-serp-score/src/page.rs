//! Scores of one page: how well its content answers the keyword, and its technical health.

use seonexus_audit::limits::{
    DESCRIPTION_FONT_PX, DESCRIPTION_MAX_CHARS, DESCRIPTION_MAX_PX, DESCRIPTION_MIN_CHARS,
    TITLE_FONT_PX, TITLE_MAX_CHARS, TITLE_MAX_PX, TITLE_MIN_CHARS,
};
use seonexus_audit::{IssueKind, Priority, catalog, pixel_width};
use seonexus_text::DocTerms;

use crate::breakdown::{Breakdown, Builder, round};
use crate::context::SerpContext;
use crate::intent::IntentClass;
use crate::signals::PageSignals;

/// Version of the content model.
pub const CONTENT_MODEL: &str = "content-1";
/// Version of the technical model.
pub const TECHNICAL_MODEL: &str = "tech-1";

/// Checks about relations between pages of one site, which mean nothing for pages of
/// different sites crawled side by side; they do not count against a page.
const CROSS_PAGE_CHECKS: &[&str] = &[
    "title-duplicate",
    "description-duplicate",
    "h1-duplicate",
    "h2-duplicate",
    "content-exact-duplicate",
    "content-near-duplicate",
    "orphan-page",
    "single-inlink",
    "deep-page",
    "links-to-redirects",
    "links-to-broken",
    "links-to-broken-external",
    "internal-blocked-robots",
    "internal-no-response",
    "internal-redirect",
    "internal-redirect-temporary",
    "internal-client-error",
    "internal-server-error",
    "external-no-response",
    "external-client-error",
    "external-server-error",
    "hreflang-missing-return",
    "hreflang-to-non-200",
    "hreflang-to-non-indexable",
    "pagination-to-non-200",
    "pagination-canonicalised",
];

/// Categories of site-wide files (sitemaps, robots.txt, `llms.txt`).
const SITE_CATEGORIES: &[&str] = &["sitemaps", "robots", "ai"];

/// Schema types that suit an intent.
fn intent_schema(intent: IntentClass) -> &'static [&'static str] {
    match intent {
        IntentClass::Informational => &[
            "Article",
            "BlogPosting",
            "NewsArticle",
            "HowTo",
            "FAQPage",
            "QAPage",
        ],
        IntentClass::Commercial => &[
            "Review",
            "AggregateRating",
            "ItemList",
            "Product",
            "FAQPage",
        ],
        IntentClass::Transactional => &["Product", "Offer", "AggregateOffer", "BreadcrumbList"],
        IntentClass::Local => &[
            "LocalBusiness",
            "Organization",
            "Restaurant",
            "Store",
            "PostalAddress",
        ],
        IntentClass::Navigational => &["Organization", "WebSite", "SearchAction"],
    }
}

/// How well a text carries the keyword: 1 for the exact phrase (by stems), else 0.6 × the
/// share of its words present.
fn placement(context: &SerpContext<'_>, text: &str) -> f64 {
    if text.trim().is_empty() {
        return 0.0;
    }
    if context.analyzer.contains(text, context.keyword) {
        1.0
    } else {
        context.analyzer.coverage(text, context.keyword) * 0.6
    }
}

/// How well a page targets the keyword in its title (for keyword difficulty).
pub fn targeting(context: &SerpContext<'_>, page: &PageSignals) -> f64 {
    placement(context, page.title.as_deref().unwrap_or_default())
}

/// The content score of a page.
///
/// `index` is the page's place in the context's pages (it is then left out of the consensus
/// it is compared with), `None` for a page that is not one of them.
#[allow(clippy::too_many_lines)] // one block per component reads best
#[allow(clippy::cast_precision_loss)] // word and element counts
pub fn content_score(
    context: &SerpContext<'_>,
    page: &PageSignals,
    terms: &DocTerms,
    index: Option<usize>,
    intent: IntentClass,
) -> Breakdown {
    let mut builder = Builder::new();

    // Keyword placement.
    let title = placement(context, page.title.as_deref().unwrap_or_default());
    let h1 = page
        .text
        .headings
        .iter()
        .filter(|(level, _)| *level == 1)
        .map(|(_, text)| placement(context, text))
        .fold(0.0, f64::max);
    let opening = placement(context, &page.text.opening(100));
    let slug = placement(context, &page.slug());
    let meta = placement(
        context,
        page.meta_description.as_deref().unwrap_or_default(),
    );
    let placed = (35.0 * title + 25.0 * h1 + 20.0 * opening + 10.0 * slug + 10.0 * meta) / 100.0;
    builder
        .add(
            "placement",
            20.0,
            Some(placed),
            Some(placed),
            "content.placement",
        )
        .with("title", round(title))
        .with("h1", round(h1))
        .with("opening", round(opening))
        .with("slug", round(slug))
        .with("meta_description", round(meta));

    // Topical coverage: the terms most competing pages use.
    let expected = context.consensus_without(index, 40);
    if expected.is_empty() {
        builder.add("coverage", 25.0, None, None, "content.coverage.unknown");
    } else {
        let total: f64 = expected.iter().map(|term| term.share).sum();
        let present: f64 = expected
            .iter()
            .filter(|term| terms.has(&term.term.key))
            .map(|term| term.share)
            .sum();
        let missing: Vec<&str> = expected
            .iter()
            .filter(|term| !terms.has(&term.term.key))
            .take(8)
            .map(|term| term.surface.as_str())
            .collect();
        let covered = present / total;
        builder
            .add(
                "coverage",
                25.0,
                Some(covered),
                Some(covered),
                "content.coverage",
            )
            .with("expected", expected.len())
            .with("missing", missing);
    }

    // Depth against the median of the competing pages.
    let words = page.text.main_words() as f64;
    if context.median_words > 0.0 {
        let ratio = words / context.median_words;
        let normalized = if ratio < 0.3 {
            0.0
        } else if ratio < 1.0 {
            (ratio - 0.3) / 0.7
        } else if ratio <= 3.0 {
            1.0
        } else {
            (1.0 - (ratio - 3.0) * 0.1).max(0.7)
        };
        builder
            .add(
                "depth",
                15.0,
                Some(words),
                Some(normalized),
                "content.depth",
            )
            .with("words", words)
            .with("median", round(context.median_words));
    } else {
        builder.add("depth", 15.0, Some(words), None, "content.depth.unknown");
    }

    // Structure.
    let h1_count = page
        .text
        .headings
        .iter()
        .filter(|(level, _)| *level == 1)
        .count();
    let h2_count = page
        .text
        .headings
        .iter()
        .filter(|(level, _)| *level == 2)
        .count() as f64;
    let h1_part = match h1_count {
        1 => 0.4,
        0 => 0.0,
        _ => 0.2,
    };
    let h2_part = if context.median_h2 > 0.0 {
        (h2_count / context.median_h2).min(1.0) * 0.4
    } else if h2_count > 0.0 {
        0.4
    } else {
        0.2
    };
    let lists = if page.text.list_items > 0 || page.text.tables > 0 {
        0.2
    } else {
        0.0
    };
    builder
        .add(
            "structure",
            10.0,
            None,
            Some(h1_part + h2_part + lists),
            "content.structure",
        )
        .with("h1", h1_count)
        .with("h2", h2_count)
        .with("median_h2", round(context.median_h2))
        .with("lists", page.text.list_items)
        .with("tables", page.text.tables);

    // "People also ask" questions the page answers.
    if !context.questions.is_empty() {
        let body = format!(
            "{}\n{}",
            page.text
                .headings
                .iter()
                .map(|(_, text)| text.as_str())
                .collect::<Vec<_>>()
                .join("\n"),
            page.text.main
        );
        let answered = context
            .questions
            .iter()
            .filter(|question| context.analyzer.coverage(&body, question) >= 0.6)
            .count();
        let share = answered as f64 / context.questions.len() as f64;
        builder
            .add(
                "questions",
                5.0,
                Some(share),
                Some(share),
                "content.questions",
            )
            .with("answered", answered)
            .with("questions", context.questions.len());
    }

    // Images, alt text and video.
    let has_images = f64::from(u8::from(page.images > 0));
    let alt = if page.images > 0 {
        f64::from(page.images_with_alt) / f64::from(page.images)
    } else {
        0.0
    };
    let media = if context.has_videos {
        0.2 * has_images + 0.6 * alt + 0.2 * f64::from(u8::from(page.has_video))
    } else {
        (0.2 * has_images + 0.6 * alt) / 0.8
    };
    builder
        .add("media", 5.0, None, Some(media), "content.media")
        .with("images", page.images)
        .with("with_alt", page.images_with_alt)
        .with("video", page.has_video);

    // Structured data.
    let expected_schema = context.expected_schema();
    let has = |schema: &str| {
        page.schema_types
            .iter()
            .any(|own| own.eq_ignore_ascii_case(schema))
    };
    let suits_intent = intent_schema(intent).iter().any(|schema| has(schema));
    let schema = if expected_schema.is_empty() {
        0.5 * f64::from(u8::from(!page.schema_types.is_empty()))
            + 0.5 * f64::from(u8::from(suits_intent))
    } else {
        let present = expected_schema.iter().filter(|schema| has(schema)).count() as f64;
        0.7 * present / expected_schema.len() as f64 + 0.3 * f64::from(u8::from(suits_intent))
    };
    builder
        .add("schema", 10.0, None, Some(schema), "content.schema")
        .with("types", page.schema_types.clone())
        .with(
            "expected",
            expected_schema
                .iter()
                .map(|schema| (*schema).to_owned())
                .collect::<Vec<_>>(),
        );

    // Title and description that fit the result snippet.
    let title_text = page.title.as_deref().unwrap_or_default();
    let title_chars = u32::try_from(title_text.chars().count()).unwrap_or(u32::MAX);
    let title_fits = (TITLE_MIN_CHARS..=TITLE_MAX_CHARS).contains(&title_chars)
        && pixel_width(title_text, TITLE_FONT_PX) <= TITLE_MAX_PX;
    let description = page.meta_description.as_deref().unwrap_or_default();
    let description_chars = u32::try_from(description.chars().count()).unwrap_or(u32::MAX);
    let description_part = if description.is_empty() {
        0.0
    } else if (DESCRIPTION_MIN_CHARS..=DESCRIPTION_MAX_CHARS).contains(&description_chars)
        && pixel_width(description, DESCRIPTION_FONT_PX) <= DESCRIPTION_MAX_PX
    {
        0.5
    } else {
        0.25
    };
    let title_part = if title_fits {
        0.5
    } else if title_text.is_empty() {
        0.0
    } else {
        0.25
    };
    builder
        .add(
            "snippet",
            5.0,
            None,
            Some(title_part + description_part),
            "content.snippet",
        )
        .with("title_chars", title_chars)
        .with("description_chars", description_chars);

    // Freshness.
    match page.days_since_modified {
        Some(days) => {
            let fresh = match days {
                0..=90 => 1.0,
                91..=365 => 0.7,
                366..=730 => 0.4,
                _ => 0.2,
            };
            builder
                .add(
                    "freshness",
                    5.0,
                    Some(f64::from(days)),
                    Some(fresh),
                    "content.freshness",
                )
                .with("days", days);
        }
        None => {
            builder.add("freshness", 5.0, None, None, "content.freshness.unknown");
        }
    }

    builder.finish(CONTENT_MODEL)
}

/// Penalty points of one finding.
fn penalty(priority: Priority, kind: IssueKind) -> f64 {
    let points = match priority {
        Priority::High => 12.0,
        Priority::Medium => 6.0,
        Priority::Low => 2.0,
    };
    points
        * match kind {
            IssueKind::Issue => 1.0,
            IssueKind::Warning => 0.5,
            IssueKind::Opportunity => 0.25,
        }
}

/// The technical score of a page: audit findings, HTTPS and response time.
pub fn technical_score(page: &PageSignals) -> Breakdown {
    let mut builder = Builder::new();
    if !page.is_live() {
        builder
            .add("health", 70.0, None, Some(0.0), "technical.not_live")
            .with(
                "status",
                page.status
                    .map_or(serde_json::Value::Null, serde_json::Value::from),
            )
            .with("indexable", page.indexable);
        builder.add("security", 10.0, None, Some(0.0), "technical.security");
        builder.add("speed", 20.0, None, Some(0.0), "technical.speed");
        return builder.finish(TECHNICAL_MODEL);
    }
    let catalog = catalog();
    let mut counted: Vec<(String, f64)> = page
        .findings
        .iter()
        .filter(|id| !CROSS_PAGE_CHECKS.contains(&id.as_str()))
        .filter_map(|id| {
            let issue = catalog.issue(id)?;
            (!SITE_CATEGORIES.contains(&issue.category.as_str()))
                .then(|| (id.clone(), penalty(issue.priority, issue.kind)))
        })
        .collect();
    counted.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    counted.dedup_by(|a, b| a.0 == b.0);
    let points: f64 = counted
        .iter()
        .map(|(_, points)| points)
        .sum::<f64>()
        .min(100.0);
    builder
        .add(
            "health",
            70.0,
            Some(points),
            Some(1.0 - points / 100.0),
            "technical.health",
        )
        .with(
            "findings",
            counted.iter().map(|(id, _)| id.clone()).collect::<Vec<_>>(),
        )
        .with("penalty", round(points));
    let security = if page.https { 0.7 } else { 0.0 } + if page.hsts { 0.3 } else { 0.0 };
    builder
        .add("security", 10.0, None, Some(security), "technical.security")
        .with("https", page.https)
        .with("hsts", page.hsts);
    let speed = match page.response_ms {
        0..=200 => 1.0,
        201..=600 => 0.7,
        601..=1000 => 0.4,
        _ => 0.1,
    };
    builder
        .add(
            "speed",
            20.0,
            Some(f64::from(page.response_ms)),
            Some(speed),
            "technical.speed",
        )
        .with("ttfb_ms", page.response_ms);
    builder.finish(TECHNICAL_MODEL)
}
