//! Smart 301 redirect target matching engine.
//!
//! Matches broken (`404`, `4xx`, `5xx`, `soft_404`) indexed URLs against healthy `200 OK`
//! URLs discovered in the same scan using URL path/slug token overlap, directory section
//! affinity, and character bigrams, falling back to the parent category path when no close
//! live match exists.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use url::Url;

use crate::model::{FindingCategory, VerifiedUrl};
use crate::verify::suggest_redirect_target;

/// Minimum similarity score (`0..=100`) required to automatically override the parent-folder fallback.
const AUTO_MATCH_THRESHOLD: u8 = 42;

/// A ranked 301 redirect target recommendation for a broken URL.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RedirectCandidate {
    /// Recommended destination URL.
    pub target_url: String,
    /// Title of the live page (or fallback label).
    pub title: String,
    /// Similarity confidence score (`0..=100`).
    pub similarity: u8,
    /// Shared slug tokens between the broken URL and the candidate URL.
    pub matched_tokens: Vec<String>,
    /// Recommendation source (`smart_slug` or `category_fallback`).
    pub kind: String,
}

#[derive(Debug)]
struct IndexedTarget {
    url: String,
    title: String,
    section: String,
    tokens: BTreeSet<String>,
    title_tokens: BTreeSet<String>,
    slug: String,
}

/// Automatically upgrades `suggested_target` on broken URLs when a high-confidence live `200 OK`
/// match exists in `rows`.
pub fn enrich_smart_redirect_targets(rows: &mut [VerifiedUrl]) {
    let healthy_pool: Vec<IndexedTarget> = rows
        .iter()
        .filter(|r| r.category == FindingCategory::Healthy200)
        .filter_map(build_indexed_target)
        .collect();

    if healthy_pool.is_empty() {
        return;
    }

    for row in rows.iter_mut() {
        if !row.category.is_issue() {
            continue;
        }
        if let Some(best) =
            find_best_candidate(&row.url, &row.serp_title, &row.serp_snippet, &healthy_pool)
            && best.similarity >= AUTO_MATCH_THRESHOLD
        {
            row.suggested_target = best.target_url;
        }
    }
}

/// Returns up to `limit` ranked 301 redirect candidates for `source_url` from the scan's healthy URLs,
/// plus the parent-directory fallback.
pub fn rank_redirect_candidates(
    source_url: &str,
    rows: &[VerifiedUrl],
    limit: usize,
) -> Vec<RedirectCandidate> {
    let source_row = rows.iter().find(|r| r.url == source_url);
    let source_title = source_row.map_or("", |r| r.serp_title.as_str());
    let source_snippet = source_row.map_or("", |r| r.serp_snippet.as_str());

    let healthy_pool: Vec<IndexedTarget> = rows
        .iter()
        .filter(|r| r.category == FindingCategory::Healthy200 && r.url != source_url)
        .filter_map(build_indexed_target)
        .collect();

    let mut candidates: Vec<RedirectCandidate> = healthy_pool
        .iter()
        .filter_map(|target| score_target(source_url, source_title, source_snippet, target))
        .filter(|c| c.similarity >= 20)
        .collect();

    candidates.sort_by(|a, b| {
        b.similarity
            .cmp(&a.similarity)
            .then_with(|| a.target_url.cmp(&b.target_url))
    });
    let mut seen_targets = BTreeSet::new();
    candidates.retain(|c| seen_targets.insert(c.target_url.clone()));
    candidates.truncate(limit.max(1));

    let fallback = suggest_redirect_target(source_url);
    if !fallback.is_empty() && !candidates.iter().any(|c| c.target_url == fallback) {
        candidates.push(RedirectCandidate {
            target_url: fallback,
            title: "Üst Kategori / Dizin".to_owned(),
            similarity: 25,
            matched_tokens: Vec::new(),
            kind: "category_fallback".to_owned(),
        });
    }

    candidates
}

fn find_best_candidate(
    source_url: &str,
    source_title: &str,
    source_snippet: &str,
    pool: &[IndexedTarget],
) -> Option<RedirectCandidate> {
    pool.iter()
        .filter(|t| t.url != source_url)
        .filter_map(|target| score_target(source_url, source_title, source_snippet, target))
        .max_by(|a, b| a.similarity.cmp(&b.similarity))
}

fn build_indexed_target(row: &VerifiedUrl) -> Option<IndexedTarget> {
    let dest_url = if row.final_url.trim().is_empty() {
        row.url.as_str()
    } else {
        row.final_url.as_str()
    };
    let parsed = Url::parse(dest_url).ok()?;
    let path = parsed.path();
    let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    let section = segments.first().copied().unwrap_or("").to_ascii_lowercase();
    let slug = fold_turkish_ascii(segments.last().copied().unwrap_or(""));
    let mut tokens = tokenize_path(path);
    if row.url != dest_url
        && let Ok(orig_parsed) = Url::parse(&row.url)
    {
        tokens.extend(tokenize_path(orig_parsed.path()));
    }
    let title = if row.live_title.trim().is_empty() {
        row.serp_title.trim().to_owned()
    } else {
        row.live_title.trim().to_owned()
    };
    let mut title_tokens = tokenize_title(&title);
    if !row.h1.trim().is_empty() {
        title_tokens.extend(tokenize_title(&row.h1));
    }
    title_tokens.extend(tokens.iter().cloned());
    Some(IndexedTarget {
        url: dest_url.to_owned(),
        title,
        section,
        tokens,
        title_tokens,
        slug,
    })
}

fn score_target(
    source_url: &str,
    source_title: &str,
    source_snippet: &str,
    target: &IndexedTarget,
) -> Option<RedirectCandidate> {
    let parsed = Url::parse(source_url).ok()?;
    let path = parsed.path();
    let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    let source_section = segments.first().copied().unwrap_or("").to_ascii_lowercase();
    let source_slug = fold_turkish_ascii(segments.last().copied().unwrap_or(""));
    let source_tokens = tokenize_path(path);
    let mut combined_source = tokenize_title(source_title);
    if !source_snippet.is_empty()
        && !source_snippet.starts_with("XML Sitemap")
        && !source_snippet.starts_with("Archived HTTP")
        && !source_snippet.starts_with("Common Crawl")
    {
        combined_source.extend(tokenize_title(source_snippet));
    }
    combined_source.extend(source_tokens.iter().cloned());

    if combined_source.is_empty() && source_section.is_empty() {
        return None;
    }

    let path_matched: Vec<String> = source_tokens
        .intersection(&target.tokens)
        .cloned()
        .collect();
    let title_matched: Vec<String> = combined_source
        .intersection(&target.title_tokens)
        .cloned()
        .collect();

    let path_score = dice_score(source_tokens.len(), target.tokens.len(), path_matched.len());
    let title_score = dice_score(
        combined_source.len(),
        target.title_tokens.len(),
        title_matched.len(),
    );
    let token_score = path_score.max(title_score);
    let matched_tokens = if title_score > path_score {
        title_matched
    } else {
        path_matched
    };

    // Same top-level directory bonus (e.g. both under `/urunler/` or `/blog/`) (0..=20 points)
    let section_bonus = if !source_section.is_empty() && source_section == target.section {
        20u32
    } else {
        0u32
    };

    // Character bigram overlap on the final slug segment (0..=12 points)
    let slug_bonus = bigram_similarity(&source_slug, &target.slug, 12);

    let total = (token_score + section_bonus + slug_bonus).min(99);
    let similarity = u8::try_from(total).unwrap_or(0);
    if similarity == 0 {
        return None;
    }

    Some(RedirectCandidate {
        target_url: target.url.clone(),
        title: target.title.clone(),
        similarity,
        matched_tokens,
        kind: "smart_slug".to_owned(),
    })
}

fn dice_score(a_len: usize, b_len: usize, overlap_len: usize) -> u32 {
    if a_len == 0 || b_len == 0 || overlap_len == 0 {
        return 0;
    }
    let overlap = u32::try_from(overlap_len).unwrap_or(0);
    let denom = u32::try_from(a_len + b_len).unwrap_or(1);
    (overlap.saturating_mul(2).saturating_mul(68)) / denom.max(1)
}

fn tokenize_title(title: &str) -> BTreeSet<String> {
    let folded = fold_turkish_ascii(title);
    if folded.contains("404") || folded.contains("bulunamad") || folded.contains("not found") {
        return BTreeSet::new();
    }
    folded
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|tok| {
            tok.len() >= 3
                && !tok.chars().all(|c| c.is_ascii_digit())
                && !matches!(
                    *tok,
                    "eski" | "old" | "yeni" | "new" | "page" | "sayfa" | "index" | "amp" | "icin"
                )
        })
        .map(str::to_owned)
        .collect()
}

fn fold_turkish_ascii(input: &str) -> String {
    input
        .chars()
        .map(|c| match c {
            'ç' | 'Ç' => 'c',
            'ğ' | 'Ğ' => 'g',
            'ı' | 'I' | 'İ' => 'i',
            'ö' | 'Ö' => 'o',
            'ş' | 'Ş' => 's',
            'ü' | 'Ü' => 'u',
            other => other.to_ascii_lowercase(),
        })
        .collect()
}

fn tokenize_path(path: &str) -> BTreeSet<String> {
    let folded = fold_turkish_ascii(path);
    folded
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|tok| {
            tok.len() >= 2
                && !tok.chars().all(|c| c.is_ascii_digit())
                && !matches!(
                    *tok,
                    "eski"
                        | "old"
                        | "yeni"
                        | "new"
                        | "page"
                        | "sayfa"
                        | "index"
                        | "amp"
                        | "html"
                        | "htm"
                        | "php"
                        | "aspx"
                        | "asp"
                )
        })
        .map(str::to_owned)
        .collect()
}

fn bigram_similarity(a: &str, b: &str, max_points: u32) -> u32 {
    if a.len() < 2 || b.len() < 2 {
        return 0;
    }
    let a_bytes: Vec<char> = a.chars().collect();
    let b_bytes: Vec<char> = b.chars().collect();
    let a_set: BTreeSet<(char, char)> = a_bytes.windows(2).map(|w| (w[0], w[1])).collect();
    let b_set: BTreeSet<(char, char)> = b_bytes.windows(2).map(|w| (w[0], w[1])).collect();
    if a_set.is_empty() || b_set.is_empty() {
        return 0;
    }
    let overlap = u32::try_from(a_set.intersection(&b_set).count()).unwrap_or(0);
    let denom = u32::try_from(a_set.len() + b_set.len()).unwrap_or(1);
    (overlap.saturating_mul(2).saturating_mul(max_points)) / denom.max(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_row(url: &str, category: FindingCategory, title: &str) -> VerifiedUrl {
        VerifiedUrl {
            url: url.to_owned(),
            engines: "google,yandex".to_owned(),
            serp_title: title.to_owned(),
            serp_snippet: String::new(),
            found_by_query: "site:ornek.com.tr".to_owned(),
            initial_status: if category == FindingCategory::Healthy200 {
                200
            } else {
                404
            },
            final_status: if category == FindingCategory::Healthy200 {
                200
            } else {
                404
            },
            redirect_hops: 0,
            final_url: url.to_owned(),
            response_time_ms: 40,
            content_type: "text/html".to_owned(),
            live_title: title.to_owned(),
            h1: title.to_owned(),
            word_count: 250,
            canonical: url.to_owned(),
            noindex: false,
            category,
            reason: category.as_str().to_owned(),
            suggested_target: suggest_redirect_target(url),
            error_message: String::new(),
        }
    }

    #[test]
    fn matches_broken_slug_to_closest_healthy_200_url() {
        let mut redirected_healthy = sample_row(
            "https://magaza.ornek.com.tr/kampanya/kis-mont-2023",
            FindingCategory::Healthy200,
            "Kışlık Mont Koleksiyonu",
        );
        redirected_healthy.redirect_hops = 1;
        redirected_healthy.final_url =
            "https://magaza.ornek.com.tr/urunler/kislik-mont-2026".to_owned();

        let mut rows = vec![
            sample_row(
                "https://magaza.ornek.com.tr/urunler/yazlik-keten-gomlek-2024",
                FindingCategory::Error404,
                "404 Bulunamadı",
            ),
            sample_row(
                "https://magaza.ornek.com.tr/p/sku-9812",
                FindingCategory::Error404,
                "Yazlık Keten Gömlek 2026",
            ),
            sample_row(
                "https://magaza.ornek.com.tr/kampanya/kis-mont-2024",
                FindingCategory::Error404,
                "404 Bulunamadı",
            ),
            sample_row(
                "https://magaza.ornek.com.tr/urunler/yazlik-keten-gomlek-2026",
                FindingCategory::Healthy200,
                "Yazlık Keten Gömlek 2026 Koleksiyonu",
            ),
            redirected_healthy,
            sample_row(
                "https://magaza.ornek.com.tr/iletisim",
                FindingCategory::Healthy200,
                "İletişim",
            ),
        ];

        enrich_smart_redirect_targets(&mut rows);
        assert_eq!(
            rows[0].suggested_target,
            "https://magaza.ornek.com.tr/urunler/yazlik-keten-gomlek-2026"
        );
        assert_eq!(
            rows[1].suggested_target,
            "https://magaza.ornek.com.tr/urunler/yazlik-keten-gomlek-2026"
        );
        assert_eq!(
            rows[2].suggested_target,
            "https://magaza.ornek.com.tr/urunler/kislik-mont-2026"
        );

        let candidates = rank_redirect_candidates(&rows[0].url, &rows, 5);
        assert!(candidates.len() >= 2);
        assert_eq!(
            candidates[0].target_url,
            "https://magaza.ornek.com.tr/urunler/yazlik-keten-gomlek-2026"
        );
        assert!(candidates[0].similarity >= 70);
    }
}
