//! Bounded summaries and complete table scopes for the existing exact-duplicate rules.

use duckdb::{OptionalExt, params};
use serde::{Deserialize, Serialize};

use crate::{CrawlReader, CrawlStoreError};

/// Exact stored field used by a duplicate audit.
#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum DuplicateKind {
    /// First page title.
    Title,
    /// First meta description.
    Description,
    /// First H1.
    H1,
    /// First H2.
    H2,
    /// Hash of normalized visible text, including navigation/footer text.
    Content,
}

impl DuplicateKind {
    const ALL: [Self; 5] = [
        Self::Title,
        Self::Description,
        Self::H1,
        Self::H2,
        Self::Content,
    ];

    fn column(self) -> &'static str {
        match self {
            Self::Title => "title",
            Self::Description => "meta_description",
            Self::H1 => "h1",
            Self::H2 => "h2",
            Self::Content => "text_hash",
        }
    }

    fn present(self, alias: &str) -> String {
        if matches!(self, Self::Content) {
            format!("{alias}.word_count>0 AND {alias}.text_hash IS NOT NULL")
        } else {
            format!("trim({alias}.{})<>''", self.column())
        }
    }
}

/// Restrict an internal result table to this URL's exact duplicate group.
#[derive(Debug, Clone, Deserialize)]
pub struct DuplicateScope {
    /// Group field; arbitrary column names are not accepted.
    pub kind: DuplicateKind,
    /// A qualifying URL already recorded in this crawl.
    pub url: String,
}

impl DuplicateScope {
    pub(crate) fn predicate(&self, quoted_url: &str) -> String {
        let column = self.kind.column();
        format!(
            "address IN (SELECT p.url FROM urls p WHERE {} AND {} AND p.{column} IN (SELECT s.{column} FROM urls s WHERE s.url={quoted_url} AND {} AND {}))",
            eligible("p"),
            self.kind.present("p"),
            eligible("s"),
            self.kind.present("s")
        )
    }
}

/// One exact group with at least two eligible URLs, including the inspected URL.
#[derive(Debug, Clone, Serialize)]
pub struct DuplicateGroup {
    /// Field used to compare the complete stored values.
    pub kind: DuplicateKind,
    /// At most 512 characters of the field value; absent for text hashes.
    pub value: Option<String>,
    /// The preview is shorter than the value used for equality.
    pub value_truncated: bool,
    /// Eligible URLs in this group, including the selected URL.
    pub count: u64,
}

fn eligible(alias: &str) -> String {
    format!(
        "{alias}.internal AND {alias}.status=200 AND {alias}.content_kind='html' AND {alias}.indexability='indexable'"
    )
}

impl CrawlReader {
    /// Returns at most five duplicate group summaries from stored data only.
    ///
    /// Eligibility and equality follow the existing title/description/heading/content
    /// audit rules. This does not download or retain a complete group in memory.
    ///
    /// # Errors
    /// Database errors or an empty URL.
    pub fn duplicate_groups(&self, url: &str) -> Result<Vec<DuplicateGroup>, CrawlStoreError> {
        if url.is_empty() {
            return Err(CrawlStoreError::InvalidSelection(
                "empty duplicate group URL".into(),
            ));
        }
        let mut groups = Vec::new();
        for kind in DuplicateKind::ALL {
            let column = kind.column();
            let (preview, truncated) = if matches!(kind, DuplicateKind::Content) {
                ("NULL".into(), "false".into())
            } else {
                (
                    format!("left(s.{column},512)"),
                    format!("length(s.{column})>512"),
                )
            };
            let sql = format!(
                "SELECT {preview},{truncated},count(*) FROM urls s JOIN urls p ON p.{column}=s.{column} WHERE s.url=? AND {} AND {} AND {} AND {} GROUP BY s.id,s.{column} ORDER BY s.id LIMIT 1",
                eligible("s"),
                eligible("p"),
                kind.present("s"),
                kind.present("p")
            );
            let group = self
                .connection
                .query_row(&sql, params![url], |row| {
                    Ok(DuplicateGroup {
                        kind,
                        value: row.get(0)?,
                        value_truncated: row.get(1)?,
                        count: row.get(2)?,
                    })
                })
                .optional()?;
            if let Some(group) = group.filter(|group| group.count > 1) {
                groups.push(group);
            }
        }
        Ok(groups)
    }
}
