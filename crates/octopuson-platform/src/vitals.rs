//! Core Web Vitals checked day by day: a site's real-user data with its weekly history, and
//! pages' PageSpeed runs with their own field data. One row per address, device and day; a
//! check run again the same day replaces it.

use duckdb::{Row, params};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{PlatformStore, PlatformStoreError};

/// One address checked on one day.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VitalCheck {
    /// The site (`https://www.example.com`) or the page.
    pub target: String,
    /// `origin` or `page`.
    pub scope: String,
    /// `phone` or `desktop`.
    pub device: String,
    /// `YYYY-MM-DD`.
    pub day: String,
    /// What real users experienced (the Chrome UX Report), when Chrome had enough visits.
    pub field: Option<Value>,
    /// The PageSpeed run (pages).
    pub lab: Option<Value>,
    /// The 75th percentiles week by week (sites).
    pub history: Option<Value>,
    /// RFC 3339.
    pub checked_at: String,
}

const COLUMNS: &str = "target, scope, device, day, field, lab, history, checked_at";

fn json_of(text: Option<String>) -> Result<Option<Value>, PlatformStoreError> {
    text.map(|text| serde_json::from_str(&text))
        .transpose()
        .map_err(|error| PlatformStoreError::Corrupt(format!("vitals: {error}")))
}

type Columns = (
    String,
    String,
    String,
    String,
    Option<String>,
    Option<String>,
    Option<String>,
    String,
);

fn columns(row: &Row<'_>) -> duckdb::Result<Columns> {
    Ok((
        row.get(0)?,
        row.get(1)?,
        row.get(2)?,
        row.get(3)?,
        row.get(4)?,
        row.get(5)?,
        row.get(6)?,
        row.get(7)?,
    ))
}

fn check(columns: Columns) -> Result<VitalCheck, PlatformStoreError> {
    let (target, scope, device, day, field, lab, history, checked_at) = columns;
    Ok(VitalCheck {
        target,
        scope,
        device,
        day,
        field: json_of(field)?,
        lab: json_of(lab)?,
        history: json_of(history)?,
        checked_at,
    })
}

impl PlatformStore {
    /// Keeps a check; one already kept for the address, device and day is replaced.
    ///
    /// # Errors
    /// [`PlatformStoreError::Invalid`] for an unknown scope or device; database errors.
    pub fn save_vital_check(
        &self,
        project_id: i64,
        check: &VitalCheck,
    ) -> Result<(), PlatformStoreError> {
        if !matches!(check.scope.as_str(), "origin" | "page")
            || !matches!(check.device.as_str(), "phone" | "desktop")
        {
            return Err(PlatformStoreError::Invalid(
                "a vitals check is of an origin or a page, on a phone or a desktop".to_owned(),
            ));
        }
        let text = |value: &Option<Value>| value.as_ref().map(Value::to_string);
        let changed = self.connection.execute(
            "UPDATE vital_checks SET scope = ?, field = ?, lab = ?, history = ?, checked_at = ?
             WHERE project_id = ? AND target = ? AND device = ? AND day = ?",
            params![
                check.scope,
                text(&check.field),
                text(&check.lab),
                text(&check.history),
                check.checked_at,
                project_id,
                check.target,
                check.device,
                check.day
            ],
        )?;
        if changed == 0 {
            self.connection.execute(
                "INSERT INTO vital_checks (project_id, target, scope, device, day, field, lab,
                 history, checked_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
                params![
                    project_id,
                    check.target,
                    check.scope,
                    check.device,
                    check.day,
                    text(&check.field),
                    text(&check.lab),
                    text(&check.history),
                    check.checked_at
                ],
            )?;
        }
        Ok(())
    }

    /// A project's checks, oldest first: every one, or those since a day.
    ///
    /// # Errors
    /// Database errors, or stored checks that are not JSON.
    pub fn vital_checks(
        &self,
        project_id: i64,
        since: Option<&str>,
    ) -> Result<Vec<VitalCheck>, PlatformStoreError> {
        let mut statement = self.connection.prepare(&format!(
            "SELECT {COLUMNS} FROM vital_checks
             WHERE project_id = ? AND (?::VARCHAR IS NULL OR day >= ?)
             ORDER BY day, scope DESC, target, device"
        ))?;
        let rows = statement.query_map(params![project_id, since, since], columns)?;
        rows.map(|row| check(row?)).collect()
    }

    /// The latest check of each address and device of a project.
    ///
    /// # Errors
    /// Database errors, or stored checks that are not JSON.
    pub fn latest_vitals(&self, project_id: i64) -> Result<Vec<VitalCheck>, PlatformStoreError> {
        let mut statement = self.connection.prepare(&format!(
            "SELECT {COLUMNS} FROM vital_checks WHERE project_id = ?
             QUALIFY row_number() OVER (PARTITION BY target, device ORDER BY day DESC, checked_at DESC) = 1
             ORDER BY scope DESC, target, device"
        ))?;
        let rows = statement.query_map(params![project_id], columns)?;
        rows.map(|row| check(row?)).collect()
    }

    /// Removes a project's checks.
    pub(crate) fn delete_vital_data(&self, project_id: i64) -> Result<(), PlatformStoreError> {
        self.connection.execute(
            "DELETE FROM vital_checks WHERE project_id = ?",
            params![project_id],
        )?;
        Ok(())
    }
}
