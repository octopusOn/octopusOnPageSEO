//! What the user decided about the opportunities the program finds: done, ignored, or snoozed
//! until a day. Opportunities themselves are worked out from the other data each time they are
//! shown; only these decisions are kept (and synced).

use duckdb::params;
use serde::{Deserialize, Serialize};

use crate::{PlatformStore, PlatformStoreError};

/// Decisions kept; `open` removes the decision.
pub const OPPORTUNITY_STATES: &[&str] = &["open", "done", "ignored", "snoozed"];

/// Longest opportunity key.
const MAX_KEY: usize = 600;

/// A decision about one opportunity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpportunityState {
    /// The opportunity (`near_top|koşu ayakkabısı`).
    pub key: String,
    /// `done`, `ignored` or `snoozed`.
    pub state: String,
    /// Snoozed until, RFC 3339.
    pub until: Option<String>,
    /// When it was decided, RFC 3339.
    pub changed_at: String,
}

impl PlatformStore {
    /// A project's decisions.
    ///
    /// # Errors
    /// Database errors.
    pub fn opportunity_states(
        &self,
        project_id: i64,
    ) -> Result<Vec<OpportunityState>, PlatformStoreError> {
        let mut statement = self.connection.prepare(
            "SELECT key, state, until, changed_at FROM opportunity_states WHERE project_id = ?
             ORDER BY changed_at DESC, key",
        )?;
        let rows = statement.query_map(params![project_id], |row| {
            Ok(OpportunityState {
                key: row.get(0)?,
                state: row.get(1)?,
                until: row.get(2)?,
                changed_at: row.get(3)?,
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Decides about an opportunity; `open` takes the decision back.
    ///
    /// # Errors
    /// [`PlatformStoreError::Invalid`] for an unknown state, a long key or a snooze without a
    /// day; database errors.
    pub fn set_opportunity_state(
        &self,
        project_id: i64,
        key: &str,
        state: &str,
        until: Option<&str>,
        now: &str,
    ) -> Result<(), PlatformStoreError> {
        self.project(project_id)?;
        let key = key.trim();
        if key.is_empty() || key.chars().count() > MAX_KEY {
            return Err(PlatformStoreError::Invalid(
                "key: an opportunity's key".to_owned(),
            ));
        }
        if !OPPORTUNITY_STATES.contains(&state) {
            return Err(PlatformStoreError::Invalid(format!(
                "state: open, done, ignored or snoozed, not `{state}`"
            )));
        }
        if state == "snoozed" && until.is_none_or(|until| until.trim().is_empty()) {
            return Err(PlatformStoreError::Invalid(
                "until: a snoozed opportunity needs a day".to_owned(),
            ));
        }
        if state == "open" {
            self.connection.execute(
                "DELETE FROM opportunity_states WHERE project_id = ? AND key = ?",
                params![project_id, key],
            )?;
            return Ok(());
        }
        let until = until.filter(|_| state == "snoozed");
        let changed = self.connection.execute(
            "UPDATE opportunity_states SET state = ?, until = ?, changed_at = ?
             WHERE project_id = ? AND key = ?",
            params![state, until, now, project_id, key],
        )?;
        if changed == 0 {
            self.connection.execute(
                "INSERT INTO opportunity_states (project_id, key, state, until, changed_at)
                 VALUES (?, ?, ?, ?, ?)",
                params![project_id, key, state, until, now],
            )?;
        }
        Ok(())
    }

    /// Removes a project's decisions.
    pub(crate) fn delete_opportunity_states(
        &self,
        project_id: i64,
    ) -> Result<(), PlatformStoreError> {
        self.connection.execute(
            "DELETE FROM opportunity_states WHERE project_id = ?",
            params![project_id],
        )?;
        Ok(())
    }
}
