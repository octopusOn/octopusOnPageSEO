//! Metric history: a domain's or a link profile's headline numbers each day they were fetched,
//! so trends grow on their own beside the providers' own histories. One point per target, kind,
//! market and day; fetching again that day replaces the day's numbers.

use duckdb::{Row, params};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{PlatformStore, PlatformStoreError};

/// What a point measures.
pub const METRIC_KINDS: &[&str] = &["domain", "backlinks"];

/// A day's numbers of a target.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MetricPoint {
    /// The domain (or page) measured.
    pub target: String,
    /// `domain` (organic and paid traffic, keywords) or `backlinks` (authority and links).
    pub kind: String,
    /// `country-language` of a domain's numbers; empty for links, which have no market.
    #[serde(default)]
    pub market: String,
    /// `YYYY-MM-DD`.
    pub day: String,
    /// The numbers, as the app reads them.
    pub metrics: Value,
    /// When they were fetched, RFC 3339.
    pub taken_at: String,
}

fn point(row: &Row<'_>) -> duckdb::Result<MetricPoint> {
    let metrics: String = row.get(4)?;
    Ok(MetricPoint {
        target: row.get(0)?,
        kind: row.get(1)?,
        market: row.get(2)?,
        day: row.get(3)?,
        metrics: serde_json::from_str(&metrics).unwrap_or(Value::Null),
        taken_at: row.get(5)?,
    })
}

impl PlatformStore {
    /// Keeps a day's numbers of a target, replacing what that day held.
    ///
    /// # Errors
    /// [`PlatformStoreError::Invalid`] for an unknown kind or a day that is not `YYYY-MM-DD`;
    /// database errors.
    pub fn note_metrics(
        &self,
        project_id: i64,
        point: &MetricPoint,
    ) -> Result<(), PlatformStoreError> {
        if !METRIC_KINDS.contains(&point.kind.as_str()) {
            return Err(PlatformStoreError::Invalid(format!(
                "kind `{}`",
                point.kind
            )));
        }
        let day = point.day.as_bytes();
        let valid_day = day.len() == 10
            && day[4] == b'-'
            && day[7] == b'-'
            && day
                .iter()
                .enumerate()
                .all(|(index, byte)| index == 4 || index == 7 || byte.is_ascii_digit());
        if !valid_day {
            return Err(PlatformStoreError::Invalid(format!("day `{}`", point.day)));
        }
        let metrics = point.metrics.to_string();
        let changed = self.connection.execute(
            "UPDATE metric_points SET metrics = ?, taken_at = ?
             WHERE project_id = ? AND target = ? AND kind = ? AND market = ? AND day = ?",
            params![
                metrics,
                point.taken_at,
                project_id,
                point.target,
                point.kind,
                point.market,
                point.day
            ],
        )?;
        if changed == 0 {
            self.connection.execute(
                "INSERT INTO metric_points (project_id, target, kind, market, day, metrics, taken_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?)",
                params![project_id, point.target, point.kind, point.market, point.day, metrics, point.taken_at],
            )?;
        }
        Ok(())
    }

    /// A target's points of one kind and market, oldest first.
    ///
    /// # Errors
    /// Database errors.
    pub fn metric_points(
        &self,
        project_id: i64,
        target: &str,
        kind: &str,
        market: &str,
    ) -> Result<Vec<MetricPoint>, PlatformStoreError> {
        let mut statement = self.connection.prepare(
            "SELECT target, kind, market, day, metrics, taken_at FROM metric_points
             WHERE project_id = ? AND target = ? AND kind = ? AND market = ?
             ORDER BY day",
        )?;
        let rows = statement.query_map(params![project_id, target, kind, market], point)?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Removes a project's points (with the project).
    pub(crate) fn delete_metric_data(&self, project_id: i64) -> Result<(), PlatformStoreError> {
        self.connection.execute(
            "DELETE FROM metric_points WHERE project_id = ?",
            params![project_id],
        )?;
        Ok(())
    }
}
