//! What the scores are computed from.

use octopuson_text::PageText;
use serde::{Deserialize, Serialize};

/// What is known about one crawled page.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[allow(clippy::struct_excessive_bools)]
pub struct PageSignals {
    /// Organic position (0 for a page that does not rank, such as the user's own).
    pub position: u16,
    /// URL after redirects.
    pub url: String,
    /// Registrable domain.
    pub domain: String,
    /// HTTP status.
    pub status: Option<u16>,
    /// Whether the page may be indexed.
    pub indexable: bool,
    /// Title.
    pub title: Option<String>,
    /// Meta description.
    pub meta_description: Option<String>,
    /// Text of the page.
    pub text: PageText,
    /// Structured data types (`Article`, `Product`, `FAQPage`, ...).
    pub schema_types: Vec<String>,
    /// Images in the page.
    pub images: u32,
    /// Images with alt text.
    pub images_with_alt: u32,
    /// Whether the page embeds a video.
    pub has_video: bool,
    /// Days since the page was last modified (from structured data or meta tags).
    pub days_since_modified: Option<u32>,
    /// Audit issue ids found on the page.
    pub findings: Vec<String>,
    /// Whether the URL is HTTPS.
    pub https: bool,
    /// Whether the response sent HSTS.
    pub hsts: bool,
    /// Time to the response headers, milliseconds.
    pub response_ms: u32,
}

impl PageSignals {
    /// Whether the page answered 200 and may be indexed.
    pub fn is_live(&self) -> bool {
        self.status == Some(200) && self.indexable
    }

    /// The URL's path and query, for keyword-in-URL checks.
    pub fn slug(&self) -> String {
        let without_scheme = self
            .url
            .split_once("://")
            .map_or(self.url.as_str(), |(_, rest)| rest);
        let path = without_scheme.split_once('/').map_or("", |(_, path)| path);
        path.replace(['-', '_', '/', '.', '+'], " ")
    }
}

/// What is known about a domain.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SiteSignals {
    /// Registrable domain.
    pub domain: String,
    /// Open PageRank, 0–10.
    pub open_page_rank: Option<f64>,
    /// Referring domains.
    pub referring_domains: Option<u64>,
    /// Tranco rank.
    pub tranco_rank: Option<u32>,
    /// Years since registration.
    pub age_years: Option<f64>,
    /// Whether its ranking page uses HTTPS.
    pub https: bool,
    /// Whether its ranking page sends HSTS.
    pub hsts: bool,
    /// Technical score of its ranking page.
    pub technical: Option<f64>,
}
