//! Site audits: how often a project's site is crawled, how many pages at most, and every audit
//! with its scores. The crawl itself lives in its own crawl database (`crawls/<id>/`).

use duckdb::{Row, params};
use serde::{Deserialize, Serialize};

use crate::{PlatformStore, PlatformStoreError};

/// Schedules of an audit.
const SCHEDULES: &[&str] = &["weekly", "monthly", "manual"];

/// How a project's site is audited.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditSettings {
    /// `weekly`, `monthly` or `manual`.
    pub schedule: String,
    /// URLs fetched at most (pages, images, redirects).
    pub max_pages: u32,
    /// When the last audit started, RFC 3339.
    #[serde(default)]
    pub last_run_at: Option<String>,
}

impl Default for AuditSettings {
    /// Audited only when asked until the user picks a schedule.
    fn default() -> Self {
        Self {
            schedule: "manual".to_owned(),
            max_pages: 500,
            last_run_at: None,
        }
    }
}

/// One audit.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AuditRow {
    /// Audit id.
    pub id: i64,
    /// Its project.
    pub project_id: i64,
    /// The crawl's folder, once it exists.
    pub crawl_id: Option<String>,
    /// Started by the schedule rather than by hand.
    pub scheduled: bool,
    /// `running`, `done`, `stopped`, `failed` or `interrupted`.
    pub status: String,
    /// Where the crawl started.
    pub start_url: String,
    /// URLs fetched.
    pub pages: u32,
    /// Technical health, 0–100.
    pub health: Option<f64>,
    /// GEO readiness, 0–100.
    pub geo: Option<f64>,
    /// Findings of every check.
    pub issues: u32,
    /// URLs metered on the web.
    pub metered_pages: u32,
    /// Micro-dollars charged for them.
    pub charged_micros: i64,
    /// Why it stopped or failed.
    pub error: Option<String>,
    /// RFC 3339.
    pub started_at: String,
    /// RFC 3339.
    pub finished_at: Option<String>,
}

/// How an audit ended, for [`PlatformStore::finish_audit`].
#[derive(Debug, Clone, PartialEq)]
pub struct AuditEnd<'a> {
    /// `done`, `stopped` or `failed`.
    pub status: &'a str,
    /// URLs fetched.
    pub pages: u32,
    /// Technical health.
    pub health: Option<f64>,
    /// GEO readiness.
    pub geo: Option<f64>,
    /// Findings.
    pub issues: u32,
    /// URLs metered.
    pub metered_pages: u32,
    /// Micro-dollars charged.
    pub charged_micros: i64,
    /// Why it stopped or failed.
    pub error: Option<&'a str>,
}

const COLUMNS: &str = "id, project_id, crawl_id, scheduled, status, start_url, pages, health, geo, issues, metered_pages, charged_micros, error, started_at, finished_at";

fn audit(row: &Row<'_>) -> duckdb::Result<AuditRow> {
    Ok(AuditRow {
        id: row.get(0)?,
        project_id: row.get(1)?,
        crawl_id: row.get(2)?,
        scheduled: row.get(3)?,
        status: row.get(4)?,
        start_url: row.get(5)?,
        pages: row.get(6)?,
        health: row.get(7)?,
        geo: row.get(8)?,
        issues: row.get(9)?,
        metered_pages: row.get(10)?,
        charged_micros: row.get(11)?,
        error: row.get(12)?,
        started_at: row.get(13)?,
        finished_at: row.get(14)?,
    })
}

impl PlatformStore {
    /// How a project's site is audited; the defaults until saved.
    ///
    /// # Errors
    /// Database errors.
    pub fn audit_settings(&self, project_id: i64) -> Result<AuditSettings, PlatformStoreError> {
        let mut statement = self.connection.prepare(
            "SELECT schedule, max_pages, last_run_at FROM audit_settings WHERE project_id = ?",
        )?;
        let mut rows = statement.query_map(params![project_id], |row| {
            Ok(AuditSettings {
                schedule: row.get(0)?,
                max_pages: row.get(1)?,
                last_run_at: row.get(2)?,
            })
        })?;
        Ok(rows.next().transpose()?.unwrap_or_default())
    }

    /// Saves how a project's site is audited; the last run time stays.
    ///
    /// # Errors
    /// [`PlatformStoreError::NotFound`] for an unknown project, [`PlatformStoreError::Invalid`]
    /// for an unknown schedule or no pages, or database errors.
    pub fn save_audit_settings(
        &self,
        project_id: i64,
        settings: &AuditSettings,
    ) -> Result<AuditSettings, PlatformStoreError> {
        self.project(project_id)?;
        if !SCHEDULES.contains(&settings.schedule.as_str()) {
            return Err(PlatformStoreError::Invalid(
                "schedule: weekly, monthly or manual".to_owned(),
            ));
        }
        if settings.max_pages == 0 {
            return Err(PlatformStoreError::Invalid(
                "max_pages: at least 1".to_owned(),
            ));
        }
        self.connection.execute(
            "INSERT INTO audit_settings (project_id, schedule, max_pages) VALUES (?, ?, ?)
             ON CONFLICT (project_id) DO UPDATE SET schedule = excluded.schedule,
             max_pages = excluded.max_pages",
            params![project_id, settings.schedule, settings.max_pages],
        )?;
        self.audit_settings(project_id)
    }

    /// Every project's audit settings.
    ///
    /// # Errors
    /// Database errors.
    pub fn audit_plans(&self) -> Result<Vec<(i64, AuditSettings)>, PlatformStoreError> {
        let mut statement = self.connection.prepare(
            "SELECT s.project_id, s.schedule, s.max_pages, s.last_run_at
             FROM audit_settings s JOIN projects p ON p.id = s.project_id ORDER BY s.project_id",
        )?;
        let rows = statement.query_map([], |row| {
            Ok((
                row.get(0)?,
                AuditSettings {
                    schedule: row.get(1)?,
                    max_pages: row.get(2)?,
                    last_run_at: row.get(3)?,
                },
            ))
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Starts an audit at `now` and moves the schedule on; an audit of the project left running
    /// by a program that stopped becomes `interrupted`.
    ///
    /// # Errors
    /// Database errors.
    pub fn begin_audit(
        &self,
        project_id: i64,
        scheduled: bool,
        start_url: &str,
        now: &str,
    ) -> Result<i64, PlatformStoreError> {
        let defaults = AuditSettings::default();
        self.connection.execute(
            "INSERT INTO audit_settings (project_id, schedule, max_pages, last_run_at)
             VALUES (?, ?, ?, ?)
             ON CONFLICT (project_id) DO UPDATE SET last_run_at = excluded.last_run_at",
            params![project_id, defaults.schedule, defaults.max_pages, now],
        )?;
        self.connection.execute(
            "UPDATE audits SET status = 'interrupted', finished_at = ?
             WHERE project_id = ? AND status = 'running'",
            params![now, project_id],
        )?;
        Ok(self.connection.query_row(
            "INSERT INTO audits (project_id, scheduled, status, start_url, started_at)
             VALUES (?, ?, 'running', ?, ?) RETURNING id",
            params![project_id, scheduled, start_url, now],
            |row| row.get(0),
        )?)
    }

    /// Notes the crawl folder of an audit.
    ///
    /// # Errors
    /// Database errors.
    pub fn set_audit_crawl(&self, audit_id: i64, crawl_id: &str) -> Result<(), PlatformStoreError> {
        self.connection.execute(
            "UPDATE audits SET crawl_id = ? WHERE id = ?",
            params![crawl_id, audit_id],
        )?;
        Ok(())
    }

    /// Records how an audit ended at `now`.
    ///
    /// # Errors
    /// Database errors.
    pub fn finish_audit(
        &self,
        audit_id: i64,
        end: &AuditEnd<'_>,
        now: &str,
    ) -> Result<(), PlatformStoreError> {
        self.connection.execute(
            "UPDATE audits SET status = ?, pages = ?, health = ?, geo = ?, issues = ?,
             metered_pages = ?, charged_micros = ?, error = ?, finished_at = ? WHERE id = ?",
            params![
                end.status,
                end.pages,
                end.health,
                end.geo,
                end.issues,
                end.metered_pages,
                end.charged_micros,
                end.error,
                now,
                audit_id
            ],
        )?;
        Ok(())
    }

    /// A project's audits, newest first.
    ///
    /// # Errors
    /// Database errors.
    pub fn audits(&self, project_id: i64, limit: u32) -> Result<Vec<AuditRow>, PlatformStoreError> {
        let mut statement = self.connection.prepare(&format!(
            "SELECT {COLUMNS} FROM audits WHERE project_id = ? ORDER BY id DESC LIMIT ?"
        ))?;
        let rows = statement.query_map(params![project_id, limit], audit)?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// One audit of a project.
    ///
    /// # Errors
    /// [`PlatformStoreError::NotFound`] or database errors.
    pub fn audit(&self, project_id: i64, audit_id: i64) -> Result<AuditRow, PlatformStoreError> {
        let mut statement = self.connection.prepare(&format!(
            "SELECT {COLUMNS} FROM audits WHERE project_id = ? AND id = ?"
        ))?;
        let mut rows = statement.query_map(params![project_id, audit_id], audit)?;
        rows.next()
            .transpose()?
            .ok_or_else(|| PlatformStoreError::NotFound(format!("audit {audit_id}")))
    }

    /// Forgets an audit; returns its crawl folder for the caller to delete.
    ///
    /// # Errors
    /// [`PlatformStoreError::NotFound`] or database errors.
    pub fn delete_audit(
        &self,
        project_id: i64,
        audit_id: i64,
    ) -> Result<Option<String>, PlatformStoreError> {
        let crawl = self.audit(project_id, audit_id)?.crawl_id;
        self.connection.execute(
            "DELETE FROM audits WHERE project_id = ? AND id = ?",
            params![project_id, audit_id],
        )?;
        Ok(crawl)
    }

    /// The crawl folders of a project's audits (to delete with the project).
    ///
    /// # Errors
    /// Database errors.
    pub fn audit_crawls(&self, project_id: i64) -> Result<Vec<String>, PlatformStoreError> {
        let mut statement = self
            .connection
            .prepare("SELECT crawl_id FROM audits WHERE project_id = ? AND crawl_id IS NOT NULL")?;
        let rows = statement.query_map(params![project_id], |row| row.get(0))?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Removes a project's audits and audit settings.
    pub(crate) fn delete_audit_data(&self, project_id: i64) -> Result<(), PlatformStoreError> {
        self.connection.execute(
            "DELETE FROM audits WHERE project_id = ?",
            params![project_id],
        )?;
        self.connection.execute(
            "DELETE FROM audit_settings WHERE project_id = ?",
            params![project_id],
        )?;
        Ok(())
    }
}
