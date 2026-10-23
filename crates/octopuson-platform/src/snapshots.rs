//! Snapshots: a project's whole state kept at a moment (traffic, keywords, backlinks, rankings,
//! the audit, AI visibility, Search Console), so two moments can be compared — where the site
//! stood when the work began and where it stands a month later (ADR 0080). What a snapshot holds
//! is the app's JSON; the store keeps it, with when and how it was taken and what it cost.

use duckdb::{Row, params};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::projects::{MAX_COMPETITORS, normalize_domain};
use crate::{PlatformStore, PlatformStoreError, list_json, parse_list};

/// How often snapshots are taken on their own.
pub const SNAPSHOT_SCHEDULES: &[&str] = &["manual", "weekly", "monthly"];

/// How a project's snapshots are taken.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotSettings {
    /// `manual`, `weekly` or `monthly`.
    pub schedule: String,
    /// Competitors whose numbers are kept too (some of the project's).
    #[serde(default)]
    pub competitors: Vec<String>,
    /// When the last snapshot started, RFC 3339.
    #[serde(default)]
    pub last_run_at: Option<String>,
}

impl Default for SnapshotSettings {
    /// Taken only when asked until the user picks a schedule: nothing is paid on its own.
    fn default() -> Self {
        Self {
            schedule: "manual".to_owned(),
            competitors: Vec::new(),
            last_run_at: None,
        }
    }
}

/// One snapshot.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SnapshotRow {
    /// Snapshot id.
    pub id: i64,
    /// Its project.
    pub project_id: i64,
    /// A name the user gave it.
    pub label: Option<String>,
    /// `manual` or `scheduled`.
    pub kind: String,
    /// `running`, `done`, `partial` (some parts missing), `failed` or `interrupted`.
    pub status: String,
    /// RFC 3339.
    pub taken_at: String,
    /// RFC 3339.
    pub finished_at: Option<String>,
    /// US dollars paid for it.
    pub cost_usd: f64,
    /// Why parts are missing, or why it failed.
    pub error: Option<String>,
    /// What it holds; left out of lists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

/// How a snapshot ended, for [`PlatformStore::finish_snapshot`].
#[derive(Debug, Clone, PartialEq)]
pub struct SnapshotEnd<'a> {
    /// `done`, `partial` or `failed`.
    pub status: &'a str,
    /// US dollars paid.
    pub cost_usd: f64,
    /// Why parts are missing or it failed.
    pub error: Option<&'a str>,
    /// What it holds.
    pub data: &'a Value,
}

const COLUMNS: &str = "id, project_id, label, kind, status, taken_at, finished_at, cost_usd, error";

fn snapshot(row: &Row<'_>) -> duckdb::Result<SnapshotRow> {
    Ok(SnapshotRow {
        id: row.get(0)?,
        project_id: row.get(1)?,
        label: row.get(2)?,
        kind: row.get(3)?,
        status: row.get(4)?,
        taken_at: row.get(5)?,
        finished_at: row.get(6)?,
        cost_usd: row.get(7)?,
        error: row.get(8)?,
        data: None,
    })
}

fn invalid(message: &str) -> PlatformStoreError {
    PlatformStoreError::Invalid(message.to_owned())
}

impl PlatformStore {
    /// How a project's snapshots are taken; the defaults until saved.
    ///
    /// # Errors
    /// Database errors, or stored settings that are not JSON.
    pub fn snapshot_settings(
        &self,
        project_id: i64,
    ) -> Result<SnapshotSettings, PlatformStoreError> {
        let mut statement = self.connection.prepare(
            "SELECT schedule, competitors, last_run_at FROM snapshot_settings WHERE project_id = ?",
        )?;
        let mut rows = statement.query_map(params![project_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
            ))
        })?;
        match rows.next().transpose()? {
            Some((schedule, competitors, last_run_at)) => Ok(SnapshotSettings {
                schedule,
                competitors: parse_list(&competitors)?,
                last_run_at,
            }),
            None => Ok(SnapshotSettings::default()),
        }
    }

    /// Saves how a project's snapshots are taken; the last run time stays.
    ///
    /// # Errors
    /// [`PlatformStoreError::NotFound`] for an unknown project, [`PlatformStoreError::Invalid`]
    /// for an unknown schedule or too many competitors, or database errors.
    pub fn save_snapshot_settings(
        &self,
        project_id: i64,
        settings: &SnapshotSettings,
    ) -> Result<SnapshotSettings, PlatformStoreError> {
        self.project(project_id)?;
        if !SNAPSHOT_SCHEDULES.contains(&settings.schedule.as_str()) {
            return Err(invalid("schedule: manual, weekly or monthly"));
        }
        let mut competitors: Vec<String> = Vec::new();
        for competitor in &settings.competitors {
            let domain = normalize_domain(competitor);
            if !domain.is_empty() && !competitors.contains(&domain) {
                competitors.push(domain);
            }
        }
        if competitors.len() > MAX_COMPETITORS {
            return Err(invalid("competitors: at most 10"));
        }
        self.connection.execute(
            "INSERT INTO snapshot_settings (project_id, schedule, competitors) VALUES (?, ?, ?)
             ON CONFLICT (project_id) DO UPDATE SET schedule = excluded.schedule,
             competitors = excluded.competitors",
            params![project_id, settings.schedule, list_json(&competitors)],
        )?;
        self.snapshot_settings(project_id)
    }

    /// Every project's snapshot settings with a schedule.
    ///
    /// # Errors
    /// Database errors.
    pub fn snapshot_plans(&self) -> Result<Vec<(i64, SnapshotSettings)>, PlatformStoreError> {
        let ids: Vec<i64> = {
            let mut statement = self.connection.prepare(
                "SELECT s.project_id FROM snapshot_settings s JOIN projects p ON p.id = s.project_id
                 WHERE s.schedule <> 'manual' ORDER BY s.project_id",
            )?;
            let rows = statement.query_map([], |row| row.get(0))?;
            rows.collect::<Result<_, _>>()?
        };
        ids.into_iter()
            .map(|id| Ok((id, self.snapshot_settings(id)?)))
            .collect()
    }

    /// Starts a snapshot at `now`: notes the run, and marks one left running (a closed
    /// program) as interrupted. Returns its id.
    ///
    /// # Errors
    /// [`PlatformStoreError::NotFound`] for an unknown project, or database errors.
    pub fn begin_snapshot(
        &self,
        project_id: i64,
        scheduled: bool,
        label: Option<&str>,
        now: &str,
    ) -> Result<i64, PlatformStoreError> {
        self.project(project_id)?;
        let defaults = SnapshotSettings::default();
        self.connection.execute(
            "INSERT INTO snapshot_settings (project_id, schedule, competitors, last_run_at) VALUES (?, ?, '[]', ?)
             ON CONFLICT (project_id) DO UPDATE SET last_run_at = excluded.last_run_at",
            params![project_id, defaults.schedule, now],
        )?;
        self.connection.execute(
            "UPDATE snapshots SET status = 'interrupted', finished_at = ?
             WHERE project_id = ? AND status = 'running'",
            params![now, project_id],
        )?;
        let label = label
            .map(str::trim)
            .filter(|label| !label.is_empty())
            .map(|label| label.chars().take(120).collect::<String>());
        Ok(self.connection.query_row(
            "INSERT INTO snapshots (project_id, label, kind, status, taken_at) VALUES (?, ?, ?, 'running', ?)
             RETURNING id",
            params![project_id, label, if scheduled { "scheduled" } else { "manual" }, now],
            |row| row.get(0),
        )?)
    }

    /// Records what a snapshot holds and how it ended, at `now`.
    ///
    /// # Errors
    /// Database errors.
    pub fn finish_snapshot(
        &self,
        id: i64,
        end: &SnapshotEnd<'_>,
        now: &str,
    ) -> Result<(), PlatformStoreError> {
        self.connection.execute(
            "UPDATE snapshots SET status = ?, cost_usd = ?, error = ?, data = ?, finished_at = ? WHERE id = ?",
            params![
                end.status,
                end.cost_usd,
                end.error,
                end.data.to_string(),
                now,
                id
            ],
        )?;
        Ok(())
    }

    /// A project's snapshots, newest first, without what they hold.
    ///
    /// # Errors
    /// Database errors.
    pub fn snapshots(&self, project_id: i64) -> Result<Vec<SnapshotRow>, PlatformStoreError> {
        let mut statement = self.connection.prepare(&format!(
            "SELECT {COLUMNS} FROM snapshots WHERE project_id = ? ORDER BY taken_at DESC, id DESC"
        ))?;
        let rows = statement.query_map(params![project_id], snapshot)?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// One snapshot with what it holds.
    ///
    /// # Errors
    /// [`PlatformStoreError::NotFound`], stored data that is not JSON, or database errors.
    pub fn snapshot(&self, project_id: i64, id: i64) -> Result<SnapshotRow, PlatformStoreError> {
        let mut statement = self.connection.prepare(&format!(
            "SELECT {COLUMNS}, data FROM snapshots WHERE project_id = ? AND id = ?"
        ))?;
        let mut rows = statement.query_map(params![project_id, id], |row| {
            Ok((snapshot(row)?, row.get::<_, Option<String>>(9)?))
        })?;
        let Some((mut row, data)) = rows.next().transpose()? else {
            return Err(PlatformStoreError::NotFound(format!("snapshot {id}")));
        };
        row.data = data
            .map(|text| serde_json::from_str(&text))
            .transpose()
            .map_err(|error| PlatformStoreError::Corrupt(error.to_string()))?;
        Ok(row)
    }

    /// Removes a snapshot.
    ///
    /// # Errors
    /// [`PlatformStoreError::NotFound`] or database errors.
    pub fn delete_snapshot(&self, project_id: i64, id: i64) -> Result<(), PlatformStoreError> {
        let removed = self.connection.execute(
            "DELETE FROM snapshots WHERE project_id = ? AND id = ?",
            params![project_id, id],
        )?;
        if removed == 0 {
            return Err(PlatformStoreError::NotFound(format!("snapshot {id}")));
        }
        Ok(())
    }

    /// Removes a project's snapshots and their settings.
    pub(crate) fn delete_snapshot_data(&self, project_id: i64) -> Result<(), PlatformStoreError> {
        for table in ["snapshots", "snapshot_settings"] {
            self.connection.execute(
                &format!("DELETE FROM {table} WHERE project_id = ?"),
                params![project_id],
            )?;
        }
        Ok(())
    }
}
