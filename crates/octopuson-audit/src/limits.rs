//! Thresholds of the checks. The catalog texts quote them; keep both in step.

/// Titles longer than this many characters are truncated in results.
pub const TITLE_MAX_CHARS: u32 = 60;
/// Titles shorter than this many characters waste the headline.
pub const TITLE_MIN_CHARS: u32 = 30;
/// Titles wider than this (in [`TITLE_FONT_PX`] Arial) are truncated in results.
pub const TITLE_MAX_PX: u32 = 561;
/// Font size titles are measured in.
pub const TITLE_FONT_PX: f64 = 20.0;
/// Meta descriptions longer than this many characters are truncated.
pub const DESCRIPTION_MAX_CHARS: u32 = 155;
/// Meta descriptions shorter than this many characters waste the snippet.
pub const DESCRIPTION_MIN_CHARS: u32 = 70;
/// Meta descriptions wider than this (in [`DESCRIPTION_FONT_PX`] Arial) are truncated.
pub const DESCRIPTION_MAX_PX: u32 = 985;
/// Font size meta descriptions are measured in.
pub const DESCRIPTION_FONT_PX: f64 = 14.0;
/// H1 headings longer than this many characters.
pub const H1_MAX_CHARS: u32 = 70;
/// URLs longer than this many characters.
pub const URL_MAX_CHARS: u32 = 115;
/// Indexable pages with fewer words have low content.
pub const LOW_CONTENT_WORDS: u32 = 200;
/// Near-duplicate detection ignores pages with fewer words.
pub const NEAR_DUPLICATE_MIN_WORDS: u32 = 50;
/// SimHash fingerprints differing in at most this many bits are near duplicates.
pub const NEAR_DUPLICATE_BITS: u32 = 3;
/// Alt texts longer than this many characters.
pub const ALT_MAX_CHARS: u32 = 100;
/// Images larger than this many bytes.
pub const LARGE_IMAGE_BYTES: u64 = 100 * 1024;
/// HTML responses slower than this many milliseconds.
pub const SLOW_RESPONSE_MS: u32 = 1000;
/// HTML documents larger than this many bytes.
pub const LARGE_HTML_BYTES: u64 = 1024 * 1024;
/// Text responses at least this large should be compressed.
pub const COMPRESSION_MIN_BYTES: u64 = 1024;
/// Pages with more internal outlinks.
pub const MAX_INTERNAL_OUTLINKS: u32 = 300;
/// Pages at least this many clicks from the start are deep.
pub const DEEP_PAGE_DEPTH: u32 = 4;
/// Sitemaps may list at most this many URLs.
pub const SITEMAP_MAX_URLS: u32 = 50_000;
