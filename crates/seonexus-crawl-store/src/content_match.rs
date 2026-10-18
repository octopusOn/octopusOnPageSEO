//! A saved example of similar content; no site requests or pairwise text downloads.

use duckdb::{OptionalExt, params};
use serde::Serialize;

use crate::{CrawlReader, CrawlStoreError};

/// One observed page with similar text, rather than an exhaustive list or cluster.
#[derive(Debug, Clone, Serialize)]
pub struct ContentMatch {
    /// URL selected for inspection.
    pub url: String,
    /// A different text hash with a nearby fingerprint.
    pub match_url: String,
    /// Stored title of the example URL.
    pub match_title: Option<String>,
    /// Number of differing bits out of 64; this is not a text-similarity percentage.
    pub distance: u32,
    /// Visible word count for the selected page.
    pub words: u32,
    /// Visible word count for the example URL.
    pub match_words: u32,
}

impl CrawlReader {
    /// Reads this URL's saved near-duplicate example, if one qualified.
    ///
    /// # Errors
    /// Database errors or an empty URL.
    pub fn content_match(&self, url: &str) -> Result<Option<ContentMatch>, CrawlStoreError> {
        if url.is_empty() {
            return Err(CrawlStoreError::InvalidSelection(
                "empty content match URL".into(),
            ));
        }
        Ok(self.connection.query_row("SELECT address,match_address,match_title,hamming_distance,word_count,match_word_count FROM tab_content_matches WHERE address=? LIMIT 1", params![url], |row| Ok(ContentMatch {
            url: row.get(0)?, match_url: row.get(1)?, match_title: row.get(2)?, distance: row.get(3)?, words: row.get(4)?, match_words: row.get(5)?
        })).optional()?)
    }
}
