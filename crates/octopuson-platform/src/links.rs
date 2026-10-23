//! Link work of a project: what was decided about each linking site after the toxic-link audit
//! (keep, watch, disavow; the disavow file is made from these), and link prospects with their
//! outreach state. Both are the user's own records and travel to every copy.

use duckdb::{Row, params};
use serde::{Deserialize, Serialize};

use crate::projects::normalize_domain;
use crate::{PlatformStore, PlatformStoreError};

/// What can be decided about a linking site.
pub const LINK_DECISIONS: &[&str] = &["keep", "watch", "disavow"];

/// Where a prospect came from.
pub const PROSPECT_SOURCES: &[&str] = &["gap", "lost", "broken", "competitor_page", "manual"];

/// Outreach states of a prospect.
pub const PROSPECT_STATES: &[&str] = &["prospect", "contacted", "won", "lost"];

/// Longest note kept.
const MAX_NOTE: usize = 2_000;

/// What was decided about a linking site.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LinkDecision {
    /// The linking site.
    pub domain: String,
    /// `keep`, `watch` or `disavow`.
    pub decision: String,
    /// Why.
    #[serde(default)]
    pub note: Option<String>,
    /// RFC 3339.
    pub decided_at: String,
}

/// A site that could link to the project.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LinkProspect {
    /// The site.
    pub domain: String,
    /// The page to get a link on (or the page that linked before).
    #[serde(default)]
    pub url: Option<String>,
    /// `gap`, `lost`, `broken`, `competitor_page` or `manual`.
    pub source: String,
    /// `prospect`, `contacted`, `won` or `lost`.
    pub status: String,
    /// Contact, offer, answers.
    #[serde(default)]
    pub note: Option<String>,
    /// RFC 3339.
    #[serde(default)]
    pub added_at: String,
    /// RFC 3339.
    #[serde(default)]
    pub updated_at: String,
}

fn site(domain: &str) -> Result<String, PlatformStoreError> {
    let domain = normalize_domain(domain);
    if domain.is_empty() || !domain.contains('.') {
        return Err(PlatformStoreError::Invalid(format!(
            "domain: `{domain}` is not a site"
        )));
    }
    Ok(domain)
}

fn note(note: Option<&str>) -> Option<String> {
    note.map(str::trim)
        .filter(|note| !note.is_empty())
        .map(|note| note.chars().take(MAX_NOTE).collect())
}

fn prospect_of(row: &Row<'_>) -> duckdb::Result<LinkProspect> {
    Ok(LinkProspect {
        domain: row.get(0)?,
        url: row.get(1)?,
        source: row.get(2)?,
        status: row.get(3)?,
        note: row.get(4)?,
        added_at: row.get(5)?,
        updated_at: row.get(6)?,
    })
}

impl PlatformStore {
    /// Decides about a linking site; `None` forgets what was decided.
    ///
    /// # Errors
    /// [`PlatformStoreError::Invalid`] for an unknown decision or a domain that is not a site,
    /// [`PlatformStoreError::NotFound`] for an unknown project, or database errors.
    pub fn decide_link(
        &self,
        project_id: i64,
        domain: &str,
        decision: Option<&str>,
        note_text: Option<&str>,
        now: &str,
    ) -> Result<(), PlatformStoreError> {
        self.project(project_id)?;
        let domain = site(domain)?;
        let Some(decision) = decision else {
            self.connection.execute(
                "DELETE FROM link_decisions WHERE project_id = ? AND domain = ?",
                params![project_id, domain],
            )?;
            return Ok(());
        };
        if !LINK_DECISIONS.contains(&decision) {
            return Err(PlatformStoreError::Invalid(
                "decision: keep, watch or disavow".to_owned(),
            ));
        }
        let note = note(note_text);
        let changed = self.connection.execute(
            "UPDATE link_decisions SET decision = ?, note = ?, decided_at = ? WHERE project_id = ? AND domain = ?",
            params![decision, note, now, project_id, domain],
        )?;
        if changed == 0 {
            self.connection.execute(
                "INSERT INTO link_decisions (project_id, domain, decision, note, decided_at) VALUES (?, ?, ?, ?, ?)",
                params![project_id, domain, decision, note, now],
            )?;
        }
        Ok(())
    }

    /// What was decided about each linking site of a project, by domain.
    ///
    /// # Errors
    /// Database errors.
    pub fn link_decisions(&self, project_id: i64) -> Result<Vec<LinkDecision>, PlatformStoreError> {
        let mut statement = self.connection.prepare(
            "SELECT domain, decision, note, decided_at FROM link_decisions WHERE project_id = ? ORDER BY domain",
        )?;
        let rows = statement.query_map(params![project_id], |row| {
            Ok(LinkDecision {
                domain: row.get(0)?,
                decision: row.get(1)?,
                note: row.get(2)?,
                decided_at: row.get(3)?,
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Adds a prospect or updates the one of the same site; the first source and the time it
    /// was added stay.
    ///
    /// # Errors
    /// [`PlatformStoreError::Invalid`] for an unknown source or state or a domain that is not a
    /// site, [`PlatformStoreError::NotFound`] for an unknown project, or database errors.
    pub fn save_prospect(
        &self,
        project_id: i64,
        prospect: &LinkProspect,
        now: &str,
    ) -> Result<LinkProspect, PlatformStoreError> {
        self.project(project_id)?;
        let domain = site(&prospect.domain)?;
        if !PROSPECT_SOURCES.contains(&prospect.source.as_str()) {
            return Err(PlatformStoreError::Invalid(format!(
                "source: `{}`",
                prospect.source
            )));
        }
        if !PROSPECT_STATES.contains(&prospect.status.as_str()) {
            return Err(PlatformStoreError::Invalid(
                "status: prospect, contacted, won or lost".to_owned(),
            ));
        }
        let note = note(prospect.note.as_deref());
        let url = prospect
            .url
            .as_deref()
            .map(str::trim)
            .filter(|url| !url.is_empty())
            .map(str::to_owned);
        let changed = self.connection.execute(
            "UPDATE link_prospects SET url = coalesce(?, url), status = ?, note = ?, updated_at = ?
             WHERE project_id = ? AND domain = ?",
            params![url, prospect.status, note, now, project_id, domain],
        )?;
        if changed == 0 {
            self.connection.execute(
                "INSERT INTO link_prospects (project_id, domain, url, source, status, note, added_at, updated_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
                params![project_id, domain, url, prospect.source, prospect.status, note, now, now],
            )?;
        }
        self.connection
            .query_row(
                "SELECT domain, url, source, status, note, added_at, updated_at FROM link_prospects
                 WHERE project_id = ? AND domain = ?",
                params![project_id, domain],
                prospect_of,
            )
            .map_err(PlatformStoreError::from)
    }

    /// A project's prospects, latest change first.
    ///
    /// # Errors
    /// Database errors.
    pub fn prospects(&self, project_id: i64) -> Result<Vec<LinkProspect>, PlatformStoreError> {
        let mut statement = self.connection.prepare(
            "SELECT domain, url, source, status, note, added_at, updated_at FROM link_prospects
             WHERE project_id = ? ORDER BY updated_at DESC, domain",
        )?;
        let rows = statement.query_map(params![project_id], prospect_of)?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Removes a prospect.
    ///
    /// # Errors
    /// Database errors.
    pub fn remove_prospect(&self, project_id: i64, domain: &str) -> Result<(), PlatformStoreError> {
        self.connection.execute(
            "DELETE FROM link_prospects WHERE project_id = ? AND domain = ?",
            params![project_id, normalize_domain(domain)],
        )?;
        Ok(())
    }

    /// Removes a project's link work (with the project).
    pub(crate) fn delete_link_data(&self, project_id: i64) -> Result<(), PlatformStoreError> {
        self.connection.execute(
            "DELETE FROM link_decisions WHERE project_id = ?",
            params![project_id],
        )?;
        self.connection.execute(
            "DELETE FROM link_prospects WHERE project_id = ?",
            params![project_id],
        )?;
        Ok(())
    }
}
