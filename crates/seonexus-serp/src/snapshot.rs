//! Assembling a snapshot from the pages of one query.

use std::collections::HashSet;

use crate::mapping::ParsedPage;
use crate::{SerpQuery, SerpSnapshot};

/// Share of matched results that must be usable before a page is reported as read badly.
pub const LOW_COVERAGE: f64 = 0.5;

/// Collects result pages into a [`SerpSnapshot`]: organic positions continue across pages,
/// a URL already seen on an earlier page is not counted again, and results beyond the query's
/// depth are dropped.
#[derive(Debug, Clone)]
pub struct SnapshotBuilder {
    snapshot: SerpSnapshot,
    seen: HashSet<String>,
}

impl SnapshotBuilder {
    /// Starts a snapshot of `query` from `provider`, first fetched at `fetched_at` (RFC 3339).
    pub fn new(
        query: SerpQuery,
        provider: impl Into<String>,
        fetched_at: impl Into<String>,
    ) -> Self {
        Self {
            snapshot: SerpSnapshot {
                query,
                provider: provider.into(),
                provider_rev: None,
                parser_rev: None,
                fetched_at: fetched_at.into(),
                total_results: None,
                organic: Vec::new(),
                features: Vec::new(),
                warnings: Vec::new(),
            },
            seen: HashSet::new(),
        }
    }

    /// Records the provider protocol or spec revision.
    #[must_use]
    pub fn provider_rev(mut self, revision: impl Into<String>) -> Self {
        self.snapshot.provider_rev = Some(revision.into());
        self
    }

    /// Records the HTML parser profile.
    #[must_use]
    pub fn parser_rev(mut self, revision: impl Into<String>) -> Self {
        self.snapshot.parser_rev = Some(revision.into());
        self
    }

    /// Adds a page. Returns how many new organic results it contributed.
    pub fn add_page(&mut self, page: ParsedPage) -> usize {
        if self.snapshot.total_results.is_none() {
            self.snapshot.total_results = page.total_results;
        }
        if page.coverage.matched > 0 && page.coverage.ratio() < LOW_COVERAGE {
            self.warn(format!(
                "page {}: only {} of {} matched results could be read; the selectors may be outdated",
                page.page, page.coverage.usable, page.coverage.matched
            ));
        }
        let hidden = page
            .organic
            .iter()
            .filter(|result| result.site_only)
            .count();
        if hidden > 0 {
            self.warn(format!(
                "page {}: the engine hid the page addresses of {hidden} results; only their sites are known",
                page.page
            ));
        }
        let site_filter = extract_site_filter(&self.snapshot.query.keyword);
        let depth = self.snapshot.query.depth;
        let mut added = 0;
        let mut skipped_off_site = 0usize;
        for mut result in page.organic {
            if let Some(ref target_host) = site_filter
                && !url_matches_site_filter(&result.url, target_host)
            {
                skipped_off_site += 1;
                continue;
            }
            let next = u16::try_from(self.snapshot.organic.len() + 1).unwrap_or(u16::MAX);
            if next > depth {
                break;
            }
            if !self.seen.insert(result.identity()) {
                continue;
            }
            result.position = next;
            self.snapshot.organic.push(result);
            added += 1;
        }
        if skipped_off_site > 0 {
            self.warn(format!(
                "sayfa {}: arama motoru site: operatörünü esneterek farklı alan adlarından {skipped_off_site} sonuç döndürdü (filtrelendi)",
                page.page
            ));
        }
        self.snapshot.features.extend(page.features);
        added
    }

    /// Adds a warning.
    pub fn warn(&mut self, warning: impl Into<String>) {
        self.snapshot.warnings.push(warning.into());
    }

    /// Organic results so far.
    pub fn organic_count(&self) -> usize {
        self.snapshot.organic.len()
    }

    /// Whether the query's depth is reached.
    pub fn is_full(&self) -> bool {
        self.snapshot.organic.len() >= usize::from(self.snapshot.query.depth)
    }

    /// The snapshot.
    pub fn finish(self) -> SerpSnapshot {
        self.snapshot
    }
}

fn extract_site_filter(keyword: &str) -> Option<String> {
    for token in keyword.split_whitespace() {
        let clean_token = token.trim_start_matches('+').trim_matches('"');
        let Some(rest) = clean_token
            .strip_prefix("site:")
            .or_else(|| clean_token.strip_prefix("SITE:"))
        else {
            continue;
        };
        let without_scheme = rest
            .strip_prefix("https://")
            .or_else(|| rest.strip_prefix("http://"))
            .unwrap_or(rest);
        let host = without_scheme
            .split(['/', '?', '#'])
            .next()
            .unwrap_or("")
            .trim()
            .trim_start_matches("*.")
            .to_ascii_lowercase();
        let bare = host.strip_prefix("www.").unwrap_or(&host);
        if !bare.is_empty() && bare.contains('.') {
            return Some(bare.to_owned());
        }
    }
    None
}

fn url_matches_site_filter(raw_url: &str, target_host: &str) -> bool {
    let Ok(parsed) = url::Url::parse(raw_url) else {
        return false;
    };
    let Some(host) = parsed.host_str() else {
        return false;
    };
    let lower = host.to_ascii_lowercase();
    let bare = lower.strip_prefix("www.").unwrap_or(&lower);
    bare == target_host || bare.ends_with(&format!(".{target_host}"))
}
