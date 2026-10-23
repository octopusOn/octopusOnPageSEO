//! Ad copies: the ads a domain was seen showing, kept from each paid-keywords answer, so an ad
//! history grows from what the user fetched: when each text was first and last seen and for
//! which keywords.

use duckdb::{Row, params};
use serde::{Deserialize, Serialize};

use crate::projects::normalize_domain;
use crate::{PlatformStore, PlatformStoreError, list_json, parse_list};

/// Keywords kept with one ad.
pub const MAX_AD_KEYWORDS: usize = 50;

/// An ad seen in one answer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdSeen {
    /// The headline.
    pub title: String,
    /// The description.
    pub description: Option<String>,
    /// The address shown.
    pub breadcrumb: Option<String>,
    /// Where it leads.
    pub url: Option<String>,
    /// The keyword it showed for.
    pub keyword: String,
}

/// An ad text and when it was seen.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdCopy {
    /// The advertiser.
    pub domain: String,
    /// The headline.
    pub title: String,
    /// The description.
    pub description: Option<String>,
    /// The address shown.
    pub breadcrumb: Option<String>,
    /// Where it leads (the latest seen).
    pub url: Option<String>,
    /// Keywords it showed for, most recent first.
    pub keywords: Vec<String>,
    /// First seen, RFC 3339.
    pub first_seen: String,
    /// Last seen, RFC 3339.
    pub last_seen: String,
}

fn copy(row: &Row<'_>) -> duckdb::Result<(AdCopy, String)> {
    Ok((
        AdCopy {
            domain: row.get(0)?,
            title: row.get(1)?,
            description: row.get(2)?,
            breadcrumb: row.get(3)?,
            url: row.get(4)?,
            keywords: Vec::new(),
            first_seen: row.get(6)?,
            last_seen: row.get(7)?,
        },
        row.get(5)?,
    ))
}

impl PlatformStore {
    /// Keeps the ads a domain was seen showing at `seen_at`: a text seen before gets the new
    /// keywords and its last sighting, a new text is added.
    ///
    /// # Errors
    /// Database errors.
    pub fn note_ads(
        &self,
        project_id: i64,
        domain: &str,
        ads: &[AdSeen],
        seen_at: &str,
    ) -> Result<(), PlatformStoreError> {
        let domain = normalize_domain(domain);
        let mut texts: Vec<(&AdSeen, Vec<String>)> = Vec::new();
        for ad in ads.iter().filter(|ad| !ad.title.trim().is_empty()) {
            match texts
                .iter_mut()
                .find(|(seen, _)| seen.title == ad.title && seen.description == ad.description)
            {
                Some((_, keywords)) => keywords.push(ad.keyword.clone()),
                None => texts.push((ad, vec![ad.keyword.clone()])),
            }
        }
        for (ad, keywords) in texts {
            let description = ad.description.clone().unwrap_or_default();
            let known: Option<(String, String)> = self
                .connection
                .query_row(
                    "SELECT keywords, last_seen FROM ad_copies
                     WHERE project_id = ? AND domain = ? AND title = ? AND coalesce(description, '') = ?",
                    params![project_id, domain, ad.title, description],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .ok();
            if let Some((kept, last_seen)) = known {
                // The newer sighting's keywords come first.
                let newer = seen_at >= last_seen.as_str();
                let kept = parse_list(&kept)?;
                let (first, second) = if newer {
                    (keywords, kept)
                } else {
                    (kept, keywords)
                };
                let mut all = first;
                for keyword in second {
                    if !all.contains(&keyword) {
                        all.push(keyword);
                    }
                }
                all.truncate(MAX_AD_KEYWORDS);
                // The address shown is the latest sighting's.
                let (latest, url, breadcrumb) = if newer {
                    (seen_at.to_owned(), ad.url.clone(), ad.breadcrumb.clone())
                } else {
                    (last_seen, None, None)
                };
                self.connection.execute(
                        "UPDATE ad_copies SET keywords = ?, last_seen = ?, url = coalesce(?, url), breadcrumb = coalesce(?, breadcrumb)
                         WHERE project_id = ? AND domain = ? AND title = ? AND coalesce(description, '') = ?",
                        params![list_json(&all), latest, url, breadcrumb, project_id, domain, ad.title, description],
                    )?;
            } else {
                let mut keywords = keywords;
                keywords.truncate(MAX_AD_KEYWORDS);
                self.connection.execute(
                        "INSERT INTO ad_copies (project_id, domain, title, description, breadcrumb, url, keywords, first_seen, last_seen)
                         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
                        params![project_id, domain, ad.title, ad.description, ad.breadcrumb, ad.url, list_json(&keywords), seen_at, seen_at],
                    )?;
            }
        }
        Ok(())
    }

    /// The ads a domain was seen showing, most recently seen first.
    ///
    /// # Errors
    /// Database errors.
    pub fn ad_copies(
        &self,
        project_id: i64,
        domain: &str,
    ) -> Result<Vec<AdCopy>, PlatformStoreError> {
        let mut statement = self.connection.prepare(
            "SELECT domain, title, description, breadcrumb, url, keywords, first_seen, last_seen FROM ad_copies
             WHERE project_id = ? AND domain = ?
             ORDER BY last_seen DESC, first_seen DESC, title",
        )?;
        let rows = statement.query_map(params![project_id, normalize_domain(domain)], copy)?;
        let mut copies = Vec::new();
        for row in rows {
            let (mut copy, keywords) = row?;
            copy.keywords = parse_list(&keywords)?;
            copies.push(copy);
        }
        Ok(copies)
    }

    /// Removes a project's ad copies (with the project).
    pub(crate) fn delete_ad_data(&self, project_id: i64) -> Result<(), PlatformStoreError> {
        self.connection.execute(
            "DELETE FROM ad_copies WHERE project_id = ?",
            params![project_id],
        )?;
        Ok(())
    }
}
