//! Content briefs and scores: what the pages ranking for a keyword have in common, read with
//! the SERP Checker's measures (shared terms, length and headings, questions, schema, titles),
//! and how a page — published, or a draft pasted in — measures up to them.
//!
//! A brief keeps the result page and the ranking pages it was made from (their main text cut
//! to [`KEPT_CHARS`]), so a page is scored later against the same pages without reading them
//! again.

use octopuson_crawl::{FetchRequest, Fetcher, follows_location};
use octopuson_serp::{SerpSnapshot, registrable_domain};
use octopuson_serp_score::context::SerpContext;
use octopuson_serp_score::intent::intent;
use octopuson_serp_score::page::content_score;
use octopuson_serp_score::recommend::{Range, TitlePattern, recommend};
use octopuson_serp_score::{Breakdown, IntentClass, PageSignals};
use octopuson_text::{DocTerms, PageText, gaps, readability};
use serde::{Deserialize, Serialize};
use std::fmt::Write as _;
use url::Url;

/// Bytes of a page read at most.
const PAGE_BYTES: u64 = 3_000_000;

/// Redirects followed to a page.
const REDIRECTS: usize = 5;

/// Characters of a ranking page's main text kept with a brief.
pub(crate) const KEPT_CHARS: usize = 60_000;

/// Shared terms listed in a brief.
const BRIEF_TERMS: usize = 40;

/// Headings suggested for the outline from the ranking pages' topics.
const OUTLINE_TOPICS: usize = 8;

/// Questions suggested for the outline's FAQ.
const OUTLINE_QUESTIONS: usize = 6;

/// A term most ranking pages use.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct BriefTerm {
    /// As written most often.
    pub(crate) term: String,
    /// Ranking pages using it.
    pub(crate) pages: u32,
    /// Share of the pages using it.
    pub(crate) share: f64,
    /// Average uses on a page using it.
    pub(crate) average: f64,
}

/// A ranking page in a brief.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct Competitor {
    pub(crate) position: u16,
    pub(crate) url: String,
    pub(crate) domain: String,
    pub(crate) title: Option<String>,
    /// Words of main content.
    pub(crate) words: usize,
    /// H2 headings.
    pub(crate) h2: usize,
    /// Its content score against the others, when it could be read.
    pub(crate) score: Option<f64>,
    /// HTTP status; `None` when it did not answer.
    pub(crate) status: Option<u16>,
}

/// A page of the project's own site worth linking to from the new content.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct InternalLink {
    pub(crate) url: String,
    pub(crate) title: String,
    /// Share of the keyword's and the brief's top terms in its title and H1, 0–1.
    pub(crate) relevance: f64,
}

/// What to write for a keyword.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct Brief {
    pub(crate) keyword: String,
    /// What searchers want (`informational`, `commercial` …).
    pub(crate) intent: String,
    /// Words of main content the ranking pages have.
    pub(crate) words: Option<Range>,
    /// Their H2 headings.
    pub(crate) h2: Option<Range>,
    /// How their titles are written.
    pub(crate) title: TitlePattern,
    /// Topics several of them have as headings.
    pub(crate) headings: Vec<String>,
    /// A suggested outline: H2 headings, then the questions to answer.
    pub(crate) outline: Vec<String>,
    /// "People also ask" questions.
    pub(crate) questions: Vec<String>,
    /// Questions and related searches worth a section or a page.
    pub(crate) ideas: Vec<String>,
    /// Structured data types most of them use.
    pub(crate) schema: Vec<String>,
    /// The page shows an answer box or AI overview: open with a short, direct answer.
    pub(crate) short_answer: bool,
    /// Terms most of them use.
    pub(crate) terms: Vec<BriefTerm>,
    pub(crate) competitors: Vec<Competitor>,
    pub(crate) internal_links: Vec<InternalLink>,
}

/// What a brief was made from, to score pages against it later.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct BriefSource {
    pub(crate) snapshot: SerpSnapshot,
    pub(crate) pages: Vec<PageSignals>,
}

fn header<'a>(headers: &'a [(String, String)], name: &str) -> Option<&'a str> {
    headers
        .iter()
        .find(|(key, _)| key == name)
        .map(|(_, value)| value.as_str())
}

/// Reads a page: follows redirects, decodes and parses it. A page that does not answer has no
/// status and no text.
pub(crate) fn read_page(fetcher: &dyn Fetcher, url: &str, position: u16) -> PageSignals {
    let mut current = url.to_owned();
    let mut status = None;
    let mut html = String::new();
    let mut response_ms = 0;
    for _ in 0..=REDIRECTS {
        let Ok(response) = fetcher.fetch(&FetchRequest {
            url: &current,
            body_limit: PAGE_BYTES,
        }) else {
            status = None;
            break;
        };
        status = Some(response.status);
        response_ms = u32::try_from(response.response_time.as_millis()).unwrap_or(u32::MAX);
        if follows_location(response.status)
            && let Some(next) = header(&response.headers, "location")
                .and_then(|location| Url::parse(&current).ok()?.join(location).ok())
        {
            current = next.to_string();
            continue;
        }
        html =
            octopuson_html::decode_html(&response.body, header(&response.headers, "content-type"));
        break;
    }
    let mut page = signals_of(&current, status, &html, position);
    page.response_ms = response_ms;
    page
}

/// What the measures need from a page's HTML.
pub(crate) fn signals_of(url: &str, status: Option<u16>, html: &str, position: u16) -> PageSignals {
    let parsed = Url::parse(url).ok();
    let facts = parsed
        .as_ref()
        .map(|address| octopuson_html::parse_page(html, address, &["googlebot"]))
        .unwrap_or_default();
    let text = octopuson_text::extract(html);
    let lower = html.to_ascii_lowercase();
    PageSignals {
        position,
        url: url.to_owned(),
        domain: parsed
            .as_ref()
            .and_then(Url::host_str)
            .map(registrable_domain)
            .unwrap_or_default(),
        status,
        indexable: !facts.directives.noindex,
        title: facts
            .title()
            .map(str::to_owned)
            .or_else(|| text.title.clone()),
        meta_description: facts.meta_descriptions.first().cloned(),
        schema_types: facts
            .structured_data
            .iter()
            .flat_map(|data| data.types.iter().cloned())
            .collect(),
        images: u32::try_from(facts.images.len()).unwrap_or(u32::MAX),
        images_with_alt: u32::try_from(
            facts
                .images
                .iter()
                .filter(|image| {
                    image
                        .alt
                        .as_deref()
                        .is_some_and(|alt| !alt.trim().is_empty())
                })
                .count(),
        )
        .unwrap_or(u32::MAX),
        has_video: lower.contains("<video")
            || lower.contains("youtube.com/embed")
            || lower.contains("player.vimeo.com")
            || html.contains("VideoObject"),
        https: parsed
            .as_ref()
            .is_some_and(|address| address.scheme() == "https"),
        text,
        ..PageSignals::default()
    }
}

/// A draft pasted in, read as Markdown: `#` lines are headings, `-`, `*` and numbered lines
/// list items, `|` lines tables.
pub(crate) fn signals_of_draft(title: &str, description: &str, body: &str) -> PageSignals {
    let mut headings = Vec::new();
    let mut paragraphs = Vec::new();
    let mut lines = Vec::new();
    let mut list_items = 0_u32;
    let mut tables = 0_u32;
    let mut in_table = false;
    let mut paragraph = String::new();
    let flush = |paragraph: &mut String, paragraphs: &mut Vec<String>| {
        if !paragraph.trim().is_empty() {
            paragraphs.push(paragraph.trim().to_owned());
        }
        paragraph.clear();
    };
    for raw in body.lines() {
        let line = raw.trim();
        if line.starts_with('|') {
            if !in_table {
                tables += 1;
                in_table = true;
            }
            let cells: String = line
                .split('|')
                .map(str::trim)
                .filter(|cell| !cell.is_empty() && !cell.chars().all(|c| matches!(c, '-' | ':')))
                .collect::<Vec<_>>()
                .join(" ");
            if !cells.is_empty() {
                lines.push(cells);
            }
            continue;
        }
        in_table = false;
        if line.is_empty() {
            flush(&mut paragraph, &mut paragraphs);
            continue;
        }
        let hashes = line.chars().take_while(|c| *c == '#').count();
        if (1..=6).contains(&hashes) && line[hashes..].starts_with(' ') {
            flush(&mut paragraph, &mut paragraphs);
            let text = line[hashes..].trim().to_owned();
            lines.push(text.clone());
            headings.push((u8::try_from(hashes).unwrap_or(6), text));
            continue;
        }
        let item = line
            .strip_prefix("- ")
            .or_else(|| line.strip_prefix("* "))
            .or_else(|| {
                let digits = line.chars().take_while(char::is_ascii_digit).count();
                (digits > 0)
                    .then(|| line[digits..].strip_prefix(". "))
                    .flatten()
            });
        let text = item.unwrap_or(line);
        if item.is_some() {
            list_items += 1;
        }
        lines.push(text.to_owned());
        if !paragraph.is_empty() {
            paragraph.push(' ');
        }
        paragraph.push_str(text);
    }
    flush(&mut paragraph, &mut paragraphs);
    let title = title.trim();
    let main = lines.join("\n");
    PageSignals {
        status: Some(200),
        indexable: true,
        title: (!title.is_empty()).then(|| title.to_owned()),
        meta_description: Some(description.trim().to_owned()).filter(|text| !text.is_empty()),
        text: PageText {
            title: (!title.is_empty()).then(|| title.to_owned()),
            text: main.clone(),
            main,
            paragraphs,
            headings,
            list_items,
            tables,
        },
        https: true,
        ..PageSignals::default()
    }
}

/// A page cut down to what scoring reads, so a brief stays small.
fn kept(mut page: PageSignals) -> PageSignals {
    if page.text.main.len() > KEPT_CHARS {
        let mut end = KEPT_CHARS;
        while !page.text.main.is_char_boundary(end) {
            end -= 1;
        }
        page.text.main.truncate(end);
    }
    page.text.text.clear();
    page.text.paragraphs.clear();
    page
}

/// The terms of a page as the measures count them: its title, headings and main text.
fn terms_of(context: &SerpContext<'_>, page: &PageSignals) -> DocTerms {
    let headings = page
        .text
        .headings
        .iter()
        .map(|(_, text)| text.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    context.analyzer.analyze(&format!(
        "{}\n{headings}\n{}",
        page.title.as_deref().unwrap_or_default(),
        page.text.main
    ))
}

fn intent_name(class: IntentClass) -> String {
    serde_json::to_value(class)
        .ok()
        .and_then(|value| value.as_str().map(str::to_owned))
        .unwrap_or_default()
}

/// Makes a brief from a result page and its ranking pages, as read.
pub(crate) fn build_brief(snapshot: SerpSnapshot, pages: Vec<PageSignals>) -> (Brief, BriefSource) {
    let pages: Vec<PageSignals> = pages.into_iter().map(kept).collect();
    let brief = {
        let context = SerpContext::new(&snapshot, &pages);
        let class = intent(&snapshot, &pages).primary;
        let recommendations = recommend(&context, None);
        let terms = context
            .consensus_without(None, BRIEF_TERMS)
            .into_iter()
            .map(|term| BriefTerm {
                term: term.surface,
                pages: term.pages,
                share: (term.share * 100.0).round() / 100.0,
                average: (term.average * 10.0).round() / 10.0,
            })
            .collect();
        let competitors = pages
            .iter()
            .enumerate()
            .map(|(index, page)| Competitor {
                position: page.position,
                url: page.url.clone(),
                domain: page.domain.clone(),
                title: page.title.clone(),
                words: page.text.main_words(),
                h2: page
                    .text
                    .headings
                    .iter()
                    .filter(|(level, _)| *level == 2)
                    .count(),
                score: page.is_live().then(|| {
                    content_score(&context, page, &context.terms[index], Some(index), class).score
                }),
                status: page.status,
            })
            .collect();
        let mut outline: Vec<String> = recommendations
            .headings
            .iter()
            .take(OUTLINE_TOPICS)
            .cloned()
            .collect();
        outline.extend(
            recommendations
                .questions
                .iter()
                .take(OUTLINE_QUESTIONS)
                .cloned(),
        );
        Brief {
            keyword: snapshot.query.keyword.clone(),
            intent: intent_name(class),
            words: recommendations.words,
            h2: recommendations.h2,
            title: recommendations.title,
            headings: recommendations.headings,
            outline,
            questions: recommendations.questions,
            ideas: recommendations.ideas,
            schema: recommendations.schema,
            short_answer: recommendations.short_answer,
            terms,
            competitors,
            internal_links: Vec::new(),
        }
    };
    (brief, BriefSource { snapshot, pages })
}

/// Pages of the site worth linking to: those whose title and H1 share most of the keyword's
/// and the brief's top terms, leaving out `except` (the page being written).
pub(crate) fn internal_links(
    brief: &Brief,
    source: &BriefSource,
    pages: &[(String, String, String)],
    except: Option<&str>,
    limit: usize,
) -> Vec<InternalLink> {
    let context = SerpContext::new(&source.snapshot, &source.pages);
    let mut wanted: Vec<String> = context
        .analyzer
        .analyze(&brief.keyword)
        .counts
        .keys()
        .filter(|term| term.n == 1)
        .map(|term| term.key.clone())
        .collect();
    let top: Vec<String> = context
        .consensus_without(None, 12)
        .into_iter()
        .filter(|term| term.term.n == 1)
        .map(|term| term.term.key)
        .collect();
    wanted.extend(top);
    wanted.sort();
    wanted.dedup();
    if wanted.is_empty() {
        return Vec::new();
    }
    #[allow(clippy::cast_precision_loss)] // a few terms
    let total = wanted.len() as f64;
    let mut links: Vec<InternalLink> = pages
        .iter()
        .filter(|(url, ..)| except.is_none_or(|except| url != except))
        .filter_map(|(url, title, h1)| {
            let terms = context.analyzer.analyze(&format!("{title}\n{h1}"));
            #[allow(clippy::cast_precision_loss)]
            let found = wanted.iter().filter(|term| terms.has(term)).count() as f64;
            let relevance = (found / total * 100.0).round() / 100.0;
            (found > 0.0).then(|| InternalLink {
                url: url.clone(),
                title: if title.is_empty() {
                    h1.clone()
                } else {
                    title.clone()
                },
                relevance,
            })
        })
        .collect();
    links.sort_by(|a, b| {
        b.relevance
            .total_cmp(&a.relevance)
            .then_with(|| a.url.len().cmp(&b.url.len()))
    });
    links.truncate(limit);
    links
}

/// A brief term and how often a page uses it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct TermUse {
    pub(crate) term: String,
    /// Uses on the page.
    pub(crate) count: u32,
    /// Average uses on the ranking pages using it.
    pub(crate) average: f64,
    /// Share of the ranking pages using it.
    pub(crate) share: f64,
}

/// How a page measures up to a brief.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct ContentScore {
    /// 0–100, with its parts.
    pub(crate) breakdown: Breakdown,
    pub(crate) words: usize,
    pub(crate) h2: usize,
    pub(crate) title: Option<String>,
    pub(crate) title_chars: usize,
    pub(crate) description_chars: usize,
    /// The keyword (by its word stems) is in the title.
    pub(crate) keyword_in_title: bool,
    /// Reading ease, where the language has a formula (Turkish, English).
    pub(crate) readability: Option<f64>,
    /// The brief's terms the page uses, most widely used first.
    pub(crate) terms: Vec<TermUse>,
    /// The brief's terms it lacks.
    pub(crate) missing: Vec<TermUse>,
    /// The "People also ask" questions it does not seem to answer.
    pub(crate) unanswered: Vec<String>,
    /// The page answered (published pages).
    pub(crate) status: Option<u16>,
}

/// Scores a page against the pages a brief was made from.
pub(crate) fn score_page(source: &BriefSource, page: &PageSignals) -> ContentScore {
    let context = SerpContext::new(&source.snapshot, &source.pages);
    let class = intent(&source.snapshot, &source.pages).primary;
    let terms = terms_of(&context, page);
    let breakdown = content_score(&context, page, &terms, None, class);
    let consensus = context.consensus_without(None, BRIEF_TERMS);
    let lacking = gaps(&terms, &consensus);
    let entry = |term: &octopuson_text::ConsensusTerm| TermUse {
        term: term.surface.clone(),
        count: terms.count(&term.term.key),
        average: (term.average * 10.0).round() / 10.0,
        share: (term.share * 100.0).round() / 100.0,
    };
    let missing: Vec<TermUse> = lacking.iter().map(|term| entry(term)).collect();
    let used: Vec<TermUse> = consensus
        .iter()
        .filter(|term| terms.has(&term.term.key))
        .map(entry)
        .collect();
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
    let unanswered = context
        .questions
        .iter()
        .filter(|question| context.analyzer.coverage(&body, question) < 0.6)
        .cloned()
        .collect();
    let title = page.title.clone();
    ContentScore {
        breakdown,
        words: page.text.main_words(),
        h2: page
            .text
            .headings
            .iter()
            .filter(|(level, _)| *level == 2)
            .count(),
        title_chars: title.as_deref().map_or(0, |title| title.chars().count()),
        description_chars: page
            .meta_description
            .as_deref()
            .map_or(0, |text| text.chars().count()),
        keyword_in_title: title
            .as_deref()
            .is_some_and(|title| context.analyzer.contains(title, context.keyword)),
        readability: readability(&page.text.main, context.analyzer.language())
            .map(|reading| (reading.score * 10.0).round() / 10.0),
        terms: used,
        missing,
        unanswered,
        status: page.status,
        title,
    }
}

/// Characters of a page's text an assistant gets to suggest improvements from.
const PAGE_MATERIAL_CHARS: usize = 20_000;

/// Headings of each ranking page an assistant gets with a brief.
const MATERIAL_HEADINGS: usize = 25;

/// The language a code names, in English, for instructions.
pub(crate) fn language_name(code: &str) -> String {
    let name = match code.split(['-', '_']).next().unwrap_or(code) {
        "tr" => "Turkish",
        "en" => "English",
        "de" => "German",
        "es" => "Spanish",
        "fr" => "French",
        "it" => "Italian",
        "nl" => "Dutch",
        "pt" => "Portuguese",
        "ru" => "Russian",
        "ar" => "Arabic",
        "az" => "Azerbaijani",
        "pl" => "Polish",
        _ => return format!("the language with the code `{code}`"),
    };
    name.to_owned()
}

fn cut(text: &str, chars: usize) -> &str {
    text.char_indices()
        .nth(chars)
        .map_or(text, |(end, _)| &text[..end])
}

fn range_text(range: Option<&Range>) -> String {
    range.map_or_else(
        || "unknown".to_owned(),
        |range| {
            format!(
                "{:.0}–{:.0} (median {:.0})",
                range.low, range.high, range.median
            )
        },
    )
}

/// What an assistant writes a brief from: the measures of the ranking pages and their headings.
pub(crate) fn brief_material(brief: &Brief, source: &BriefSource) -> String {
    let mut out = format!(
        "# Keyword: {}\n\nSearch intent: {}\nMain-content words of the ranking pages: {}\nH2 headings: {}\n",
        brief.keyword,
        brief.intent,
        range_text(brief.words.as_ref()),
        range_text(brief.h2.as_ref())
    );
    let _ = writeln!(
        out,
        "Titles: median {:.0} characters; {:.0}% start with the keyword, {:.0}% have a year, {:.0}% a number",
        brief.title.median_chars,
        brief.title.keyword_first * 100.0,
        brief.title.with_year * 100.0,
        brief.title.with_number * 100.0
    );
    if brief.short_answer {
        out.push_str("The result page shows a featured answer or an AI overview: open with a short, direct answer.\n");
    }
    if !brief.schema.is_empty() {
        let _ = writeln!(out, "Structured data most use: {}", brief.schema.join(", "));
    }
    out.push_str("\n## Terms the ranking pages share (pages using it / average uses)\n");
    for term in &brief.terms {
        let _ = writeln!(
            out,
            "- {} ({} / {:.1})",
            term.term, term.pages, term.average
        );
    }
    if !brief.headings.is_empty() {
        out.push_str("\n## Topics several pages have as headings\n");
        for heading in &brief.headings {
            let _ = writeln!(out, "- {heading}");
        }
    }
    if !brief.questions.is_empty() {
        out.push_str("\n## People also ask\n");
        for question in &brief.questions {
            let _ = writeln!(out, "- {question}");
        }
    }
    if !brief.ideas.is_empty() {
        out.push_str("\n## Related searches\n");
        for idea in &brief.ideas {
            let _ = writeln!(out, "- {idea}");
        }
    }
    out.push_str("\n## The ranking pages\n");
    for page in source.pages.iter().filter(|page| page.is_live()) {
        let _ = writeln!(
            out,
            "\n### {}. {} — {}\n{} words",
            page.position,
            page.title.as_deref().unwrap_or("(no title)"),
            page.url,
            page.text.main_words()
        );
        for (level, text) in page
            .text
            .headings
            .iter()
            .filter(|(level, _)| (2..=3).contains(level))
            .take(MATERIAL_HEADINGS)
        {
            let _ = writeln!(out, "{}- {}", if *level == 3 { "  " } else { "" }, text);
        }
    }
    if !brief.internal_links.is_empty() {
        out.push_str("\n## Pages of our site to link to\n");
        for link in &brief.internal_links {
            let _ = writeln!(out, "- {} — {}", link.title, link.url);
        }
    }
    out
}

/// How an assistant writes a brief.
pub(crate) fn brief_instructions(language: &str) -> String {
    format!(
        "You are an SEO content strategist preparing a brief for a writer. Write it in {}, in \
         Markdown, from the material only: the measures of the pages ranking for the keyword, \
         their headings and the questions searchers ask. Do not invent facts, figures or \
         sources. Sections:\n\
         1. Search intent and angle: what searchers want and how the page can serve them better \
         than the ranking pages.\n\
         2. Three title options (at most 60 characters) and one meta description (at most 155 \
         characters).\n\
         3. Outline: H2 and H3 headings in order, with one or two lines on what each section \
         covers.\n\
         4. Terms to use naturally, grouped by section.\n\
         5. Questions to answer, with where they fit.\n\
         6. Internal links: which of our pages to link to and from where (only those listed).\n\
         7. Length and format: target words, lists, tables, images, structured data.\n\
         8. What the ranking pages miss: topics or depth that would set the page apart.\n\
         Keep it practical and concise.",
        language_name(language)
    )
}

/// What an assistant suggests improvements from: the brief's measures, what the page scored
/// and the page itself.
pub(crate) fn improvement_material(
    brief: &Brief,
    score: &ContentScore,
    page: &PageSignals,
) -> String {
    let mut out = format!(
        "# Keyword: {}\n\nThe page scores {:.0} of 100 against the pages ranking for it.\n\
         Words: {} (ranking pages: {})\nH2 headings: {} (ranking pages: {})\n\
         Title: {} ({} characters; ranking pages' median {:.0})\n\
         Meta description: {} characters\nKeyword in the title: {}\n",
        brief.keyword,
        score.breakdown.score,
        score.words,
        range_text(brief.words.as_ref()),
        score.h2,
        range_text(brief.h2.as_ref()),
        score.title.as_deref().unwrap_or("(none)"),
        score.title_chars,
        brief.title.median_chars,
        score.description_chars,
        if score.keyword_in_title { "yes" } else { "no" }
    );
    if let Some(reading) = score.readability {
        let _ = writeln!(out, "Reading ease: {reading:.0} of 100");
    }
    out.push_str("\n## Parts of the score (0–1, lowest first)\n");
    let mut parts: Vec<(&str, f64, &str)> = score
        .breakdown
        .components
        .iter()
        .filter_map(|part| Some((part.id.as_str(), part.normalized?, part.reason.as_str())))
        .collect();
    parts.sort_by(|a, b| a.1.total_cmp(&b.1));
    for (id, normalized, reason) in parts {
        let _ = writeln!(out, "- {id}: {normalized:.2} — {reason}");
    }
    if !score.missing.is_empty() {
        out.push_str("\n## Terms the ranking pages use and the page lacks (share of pages)\n");
        for term in &score.missing {
            let _ = writeln!(out, "- {} ({:.0}%)", term.term, term.share * 100.0);
        }
    }
    if !score.unanswered.is_empty() {
        out.push_str("\n## Questions searchers ask that the page does not seem to answer\n");
        for question in &score.unanswered {
            let _ = writeln!(out, "- {question}");
        }
    }
    if !brief.headings.is_empty() {
        out.push_str("\n## Topics several ranking pages have as headings\n");
        for heading in &brief.headings {
            let _ = writeln!(out, "- {heading}");
        }
    }
    out.push_str("\n## The page\n");
    if let Some(description) = &page.meta_description {
        let _ = writeln!(out, "Meta description: {description}");
    }
    out.push_str("Headings:\n");
    for (level, text) in page.text.headings.iter().take(60) {
        let _ = writeln!(out, "{} {text}", "#".repeat(usize::from(*level)));
    }
    out.push_str("\nText:\n");
    out.push_str(cut(&page.text.main, PAGE_MATERIAL_CHARS));
    out
}

/// How an assistant suggests improvements.
pub(crate) fn improvement_instructions(language: &str) -> String {
    format!(
        "You are an SEO editor. From the material (how a page measures up to the pages ranking \
         for its keyword, and the page itself), suggest concrete improvements in {}, in \
         Markdown. Start with the three changes that matter most. For each suggestion say \
         what to change, where, and give example text the writer can adapt (a new heading, a \
         rewritten title or description, a paragraph that answers a question). Use the missing \
         terms only where they fit naturally; never suggest stuffing. Do not invent facts or \
         figures about the business; mark anything the writer must fill in. Be concise.",
        language_name(language)
    )
}

/// What an assistant drafts a reply to a review from.
pub(crate) fn reply_material(
    business: &str,
    category: Option<&str>,
    rating: Option<u8>,
    author: Option<&str>,
    text: Option<&str>,
) -> String {
    format!(
        "Business: {business}\nCategory: {}\nRating: {}\nReviewer: {}\nReview:\n{}\n",
        category.unwrap_or("unknown"),
        rating.map_or_else(|| "none".to_owned(), |stars| format!("{stars} of 5 stars")),
        author.unwrap_or("anonymous"),
        text.filter(|text| !text.trim().is_empty())
            .unwrap_or("(a rating without text)")
    )
}

/// How an assistant drafts a reply to a review.
pub(crate) fn reply_instructions(language: &str) -> String {
    format!(
        "You write replies from a business owner to Google reviews. Write one reply in the \
         review's own language (if the review has no text, in {}). Thank the reviewer, by first \
         name if given. Respond to what they said specifically. For a negative review, \
         apologise without admitting fault on details you cannot check, and invite them to get \
         in touch to put it right without inventing contact details. Never promise refunds or \
         gifts. Plain text only, at most 90 words, no placeholders, no signature line.",
        language_name(language)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_markdown_draft_reads_as_a_page() {
        let page = signals_of_draft(
            "Spor Ayakkabı Rehberi",
            "Nasıl seçilir?",
            "# Spor ayakkabı\nGiriş paragrafı.\n\n## Nasıl seçilir\n- Taban\n- Numara\n1. Dene\n\n| Model | Fiyat |\n|---|---|\n| A | 10 |\n",
        );
        assert_eq!(page.title.as_deref(), Some("Spor Ayakkabı Rehberi"));
        assert_eq!(page.meta_description.as_deref(), Some("Nasıl seçilir?"));
        assert_eq!(
            page.text.headings,
            [
                (1, "Spor ayakkabı".to_owned()),
                (2, "Nasıl seçilir".to_owned())
            ]
        );
        assert_eq!(page.text.list_items, 3);
        assert_eq!(page.text.tables, 1);
        assert!(page.text.main.contains("Model Fiyat"));
        assert!(page.is_live());
    }

    #[test]
    fn a_page_is_read_from_its_html() {
        let html = r#"<html lang="tr"><head><title>Spor Ayakkabı</title><meta name="description" content="En iyi modeller">
            <script type="application/ld+json">{"@context":"https://schema.org","@type":"Article"}</script></head>
            <body><main><h1>Spor ayakkabı</h1><p>Koşu için hafif modeller.</p><img src="a.jpg" alt="ayakkabı"><img src="b.jpg"></main></body></html>"#;
        let page = signals_of("https://www.example.com/spor", Some(200), html, 3);
        assert_eq!(page.domain, "example.com");
        assert_eq!(page.title.as_deref(), Some("Spor Ayakkabı"));
        assert_eq!(page.meta_description.as_deref(), Some("En iyi modeller"));
        assert_eq!(page.schema_types, ["Article"]);
        assert_eq!((page.images, page.images_with_alt), (2, 1));
        assert!(page.is_live());
        assert_eq!(page.position, 3);
    }
}
