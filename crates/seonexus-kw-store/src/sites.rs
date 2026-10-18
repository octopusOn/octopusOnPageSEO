//! The site's own pages per keyword, from Search Console or result pages.

use duckdb::params;
use serde::{Deserialize, Serialize};

use crate::{KwStore, KwStoreError};

/// A page of the site seen for a keyword.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SitePageRecord {
    /// The keyword.
    pub keyword_id: i64,
    /// Page address.
    pub url: String,
    /// `gsc` or `serp`.
    pub source: String,
    /// Clicks in the period.
    pub clicks: Option<f64>,
    /// Impressions in the period.
    pub impressions: Option<f64>,
    /// Average position.
    pub position: Option<f64>,
    /// `YYYY-MM-DD..YYYY-MM-DD`, or the fetch day for result pages.
    pub period: String,
}

impl KwStore {
    /// Replaces a project's site pages from one source.
    ///
    /// # Errors
    /// Database errors.
    pub fn put_site_pages(
        &mut self,
        project_id: i64,
        source: &str,
        pages: &[SitePageRecord],
    ) -> Result<(), KwStoreError> {
        let transaction = self.connection.transaction()?;
        transaction.execute(
            "DELETE FROM site_pages WHERE project_id = ? AND source = ?",
            params![project_id, source],
        )?;
        {
            let mut seen = std::collections::HashSet::new();
            let mut appender = transaction.appender("site_pages")?;
            for page in pages {
                if !seen.insert((page.keyword_id, page.url.as_str())) {
                    continue;
                }
                appender.append_row(params![
                    project_id,
                    page.keyword_id,
                    page.url,
                    source,
                    page.clicks,
                    page.impressions,
                    page.position,
                    page.period
                ])?;
            }
            appender.flush()?;
        }
        transaction.commit()?;
        Ok(())
    }

    /// Every site page of a project.
    ///
    /// # Errors
    /// Database errors.
    pub fn site_pages(&self, project_id: i64) -> Result<Vec<SitePageRecord>, KwStoreError> {
        let mut statement = self.connection.prepare(
            "SELECT keyword_id, url, source, clicks, impressions, position, period
             FROM site_pages WHERE project_id = ? ORDER BY keyword_id, impressions DESC NULLS LAST, url",
        )?;
        let rows = statement.query_map(params![project_id], |row| {
            Ok(SitePageRecord {
                keyword_id: row.get(0)?,
                url: row.get(1)?,
                source: row.get(2)?,
                clicks: row.get(3)?,
                impressions: row.get(4)?,
                position: row.get(5)?,
                period: row.get(6)?,
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }
}
