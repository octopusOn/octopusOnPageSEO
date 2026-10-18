//! Conservative sitemap eligibility from saved observations, without site requests.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use url::Url;

use crate::{CrawlReader, CrawlStoreError};

/// Protocol limit per uncompressed XML sitemap, including the envelope.
pub const SITEMAP_MAX_BYTES: u64 = 52_428_800;
/// Protocol URL limit per sitemap (also the maximum number of sitemap index entries).
pub const SITEMAP_MAX_URLS: u64 = 50_000;
/// UTF-8 XML declaration and sitemap namespace.
pub const SITEMAP_HEADER: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">\n";
/// Sitemap closing tag.
pub const SITEMAP_FOOTER: &str = "</urlset>\n";

/// An exact HTTP(S) origin, without a trailing slash. Empty selects the first eligible origin.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct SitemapSelection {
    /// Scheme, host and optional non-default port.
    pub origin: String,
}

/// First reason a stored URL is ineligible; counts are disjoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SitemapReason {
    /// External records are never exported.
    External,
    /// robots.txt blocks this record.
    Blocked,
    /// Not an observed successful HTTP 200 response.
    Response,
    /// Not HTML.
    NonHtml,
    /// Effective noindex before or after JavaScript.
    Noindex,
    /// Failed rendering or a missing required JavaScript observation.
    Rendering,
    /// Original body or rendered DOM was truncated.
    Truncated,
    /// Multiple HTML canonicals, invalid canonical or canonical to another URL.
    Canonical,
    /// Other recorded non-indexability.
    NonIndexable,
    /// Invalid/unsupported URL, credentials, fragment, normalization mismatch or URL length.
    InvalidUrl,
    /// Duplicate stored URL records cannot determine a single observation.
    Duplicate,
    /// Otherwise eligible, but belongs to another origin.
    OtherOrigin,
}

/// Number of eligible URLs belonging to one origin.
#[derive(Debug, Clone, Serialize)]
pub struct SitemapOrigin {
    /// Canonical HTTP(S) origin.
    pub origin: String,
    /// Eligible records before the origin selection.
    pub count: u64,
}

/// One exclusion counter.
#[derive(Debug, Clone, Serialize)]
pub struct SitemapExclusion {
    /// Stable reason code.
    pub reason: SitemapReason,
    /// Number of stored URL records.
    pub count: u64,
}

/// Exact plan for a selected origin. Sample is capped independently of export size.
#[derive(Debug, Serialize)]
pub struct SitemapPlan {
    /// Selected origin, or empty when there are no eligible URLs.
    pub origin: String,
    /// Available origins with eligible counts; includes an explicitly selected empty origin.
    pub origins: Vec<SitemapOrigin>,
    /// Stored records examined, including external records.
    pub scanned: u64,
    /// Eligible URLs in this origin.
    pub included: u64,
    /// Disjoint reasons covering every excluded stored record.
    pub excluded: Vec<SitemapExclusion>,
    /// First 100 eligible URLs, in exact address order.
    pub sample: Vec<String>,
    /// Required XML sitemap documents (index is additional when exporting a package).
    pub parts: u64,
    /// Total uncompressed bytes of the sitemap documents, excluding the index.
    pub xml_bytes: u64,
    /// Stable warning codes supplied by the workspace metadata.
    pub warnings: Vec<String>,
}

const RECORDS: &str = "SELECT u.url,CASE WHEN NOT u.internal THEN 'external' WHEN u.blocked_by_robots THEN 'blocked' WHEN u.error IS NOT NULL OR u.status IS DISTINCT FROM 200 THEN 'response' WHEN u.content_kind<>'html' THEN 'non-html' WHEN u.noindex OR coalesce(r.raw_noindex,false) THEN 'noindex' WHEN r.error IS NOT NULL OR (? AND r.rendered IS DISTINCT FROM true) THEN 'rendering' WHEN u.truncated OR coalesce(r.truncated,false) THEN 'truncated' WHEN u.canonical_count>1 THEN 'canonical' WHEN u.indexability<>'indexable' THEN 'non-indexable' END AS reason,u.canonical,u.header_canonical,count(*) OVER(PARTITION BY u.url) AS duplicates FROM urls u LEFT JOIN rendering r ON r.page=u.id ORDER BY u.url,u.id";

impl CrawlReader {
    fn sitemap_records(
        &self,
        require_js: bool,
        mut visit: impl FnMut(
            &str,
            Option<SitemapReason>,
            Option<String>,
        ) -> Result<(), CrawlStoreError>,
    ) -> Result<(), CrawlStoreError> {
        let mut statement = self.connection.prepare(RECORDS)?;
        let mut rows = statement.query([require_js])?;
        while let Some(row) = rows.next()? {
            let address: String = row.get(0)?;
            let stored: Option<String> = row.get(1)?;
            let canonical: Option<String> = row.get(2)?;
            let header: Option<String> = row.get(3)?;
            let duplicates: u64 = row.get(4)?;
            let mut reason = match stored.as_deref() {
                Some("external") => Some(SitemapReason::External),
                Some("blocked") => Some(SitemapReason::Blocked),
                Some("response") => Some(SitemapReason::Response),
                Some("non-html") => Some(SitemapReason::NonHtml),
                Some("noindex") => Some(SitemapReason::Noindex),
                Some("rendering") => Some(SitemapReason::Rendering),
                Some("truncated") => Some(SitemapReason::Truncated),
                Some("canonical") => Some(SitemapReason::Canonical),
                Some("non-indexable") => Some(SitemapReason::NonIndexable),
                _ => None,
            };
            // A canonical mismatch is more actionable than the generic indexability label.
            if reason.is_none() || reason == Some(SitemapReason::NonIndexable) {
                if duplicates > 1 {
                    reason = Some(SitemapReason::Duplicate);
                } else if let Some(url) = valid_url(&address) {
                    if [canonical.as_deref(), header.as_deref()]
                        .into_iter()
                        .flatten()
                        .any(|value| {
                            value.is_empty()
                                || url
                                    .join(value)
                                    .map_or(true, |target| target.as_str() != address)
                        })
                    {
                        reason = Some(SitemapReason::Canonical);
                    }
                    visit(&address, reason, Some(url.origin().ascii_serialization()))?;
                    continue;
                } else {
                    reason = Some(SitemapReason::InvalidUrl);
                }
            }
            visit(&address, reason, None)?;
        }
        Ok(())
    }

    /// Plans all matches with bounded URL samples, accurate escaped byte sizes and disjoint exclusions.
    ///
    /// # Errors
    /// Invalid origin or database errors.
    pub fn sitemap_plan(
        &self,
        selection: &SitemapSelection,
        require_js: bool,
    ) -> Result<SitemapPlan, CrawlStoreError> {
        if !selection.origin.is_empty() {
            validate_origin(&selection.origin)?;
        }
        let mut origins = BTreeMap::<String, u64>::new();
        self.sitemap_records(require_js, |_, reason, origin| {
            if reason.is_none()
                && let Some(origin) = origin
            {
                *origins.entry(origin).or_default() += 1;
            }
            Ok(())
        })?;
        let origin = if selection.origin.is_empty() {
            origins
                .iter()
                .max_by(|(a, ac), (b, bc)| ac.cmp(bc).then_with(|| b.cmp(a)))
                .map(|(origin, _)| origin.clone())
                .unwrap_or_default()
        } else {
            selection.origin.clone()
        };
        if !origin.is_empty() {
            origins.entry(origin.clone()).or_default();
        }
        let mut scanned = 0;
        let mut included = 0;
        let mut sample = Vec::new();
        let mut excluded = BTreeMap::<SitemapReason, u64>::new();
        let mut layout = SitemapLayout::default();
        self.sitemap_records(require_js, |address, reason, address_origin| {
            scanned += 1;
            let reason = reason.or_else(|| {
                (address_origin.as_deref() != Some(origin.as_str()))
                    .then_some(SitemapReason::OtherOrigin)
            });
            if let Some(reason) = reason {
                *excluded.entry(reason).or_default() += 1;
            } else {
                included += 1;
                if sample.len() < 100 {
                    sample.push(address.to_owned());
                }
                layout.add(sitemap_url_entry(address).len() as u64);
            }
            Ok(())
        })?;
        Ok(SitemapPlan {
            origin,
            origins: origins
                .into_iter()
                .map(|(origin, count)| SitemapOrigin { origin, count })
                .collect(),
            scanned,
            included,
            excluded: excluded
                .into_iter()
                .map(|(reason, count)| SitemapExclusion { reason, count })
                .collect(),
            sample,
            parts: layout.parts,
            xml_bytes: layout.total_bytes,
            warnings: Vec::new(),
        })
    }

    /// Visits every eligible URL in the selected origin, in the same order and scope as the plan.
    ///
    /// # Errors
    /// Invalid origin, database or visitor/output errors.
    pub fn sitemap_urls(
        &self,
        selection: &SitemapSelection,
        require_js: bool,
        mut visit: impl FnMut(&str) -> Result<(), CrawlStoreError>,
    ) -> Result<u64, CrawlStoreError> {
        validate_origin(&selection.origin)?;
        let mut count = 0;
        self.sitemap_records(require_js, |address, reason, origin| {
            if reason.is_none() && origin.as_deref() == Some(selection.origin.as_str()) {
                visit(address)?;
                count += 1;
            }
            Ok(())
        })?;
        Ok(count)
    }
}

/// Streaming layout shared by the planner and package writer, for validated URL entries.
#[derive(Default)]
pub struct SitemapLayout {
    parts: u64,
    part_urls: u64,
    part_bytes: u64,
    total_bytes: u64,
}
impl SitemapLayout {
    /// Accounts for an entry and its XML envelope; returns true when a new part begins.
    pub fn add(&mut self, bytes: u64) -> bool {
        let envelope = (SITEMAP_HEADER.len() + SITEMAP_FOOTER.len()) as u64;
        let starts_part = self.parts == 0
            || self.part_urls == SITEMAP_MAX_URLS
            || self.part_bytes + bytes > SITEMAP_MAX_BYTES;
        if starts_part {
            self.parts += 1;
            self.part_urls = 0;
            self.part_bytes = envelope;
            self.total_bytes += envelope;
        }
        self.part_urls += 1;
        self.part_bytes += bytes;
        self.total_bytes += bytes;
        starts_part
    }
}

fn valid_url(raw: &str) -> Option<Url> {
    let url = Url::parse(raw).ok()?;
    (matches!(url.scheme(), "http" | "https")
        && url.has_host()
        && url.username().is_empty()
        && url.password().is_none()
        && url.fragment().is_none()
        && raw.len() < 2048
        && raw == url.as_str()
        && valid_percent_escapes(raw))
    .then_some(url)
}
fn valid_percent_escapes(raw: &str) -> bool {
    let bytes = raw.as_bytes();
    bytes.iter().enumerate().all(|(index, byte)| {
        *byte != b'%'
            || bytes.get(index + 1).is_some_and(u8::is_ascii_hexdigit)
                && bytes.get(index + 2).is_some_and(u8::is_ascii_hexdigit)
    })
}
fn validate_origin(origin: &str) -> Result<(), CrawlStoreError> {
    let url = valid_url(&format!("{origin}/")).ok_or_else(|| {
        CrawlStoreError::InvalidSelection(
            "choose a valid HTTP(S) sitemap origin without a trailing slash".into(),
        )
    })?;
    if url.origin().ascii_serialization() != origin {
        return Err(CrawlStoreError::InvalidSelection(
            "sitemap origin must not contain a path, query or fragment".into(),
        ));
    }
    Ok(())
}

/// Escapes XML text and attributes; callers validate their URLs first.
pub fn sitemap_escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
/// One deterministic sitemap entry. No inferred modification date or priority is emitted.
pub fn sitemap_url_entry(address: &str) -> String {
    format!("  <url><loc>{}</loc></url>\n", sitemap_escape(address))
}

#[cfg(test)]
mod tests;
