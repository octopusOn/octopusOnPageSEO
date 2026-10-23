//! Every paid request with what it cost, for the usage page and monthly budgets.

use duckdb::params;
use serde::{Deserialize, Serialize};

use crate::{PlatformStore, PlatformStoreError};

/// One paid request, or a batch of them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpendRecord {
    /// RFC 3339.
    pub at: String,
    /// The project it was for.
    pub project_id: Option<i64>,
    /// Module that asked (`overview`).
    pub module: String,
    /// Provider endpoint.
    pub endpoint: String,
    /// Provider id (`dataforseo`).
    pub provider: String,
    /// `own` or `credit`.
    pub via: String,
    /// Requests sent.
    pub requests: u32,
    /// US dollars.
    pub cost_usd: f64,
}

/// Spending of one provider, paid one way, in one month.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UsageRow {
    /// `YYYY-MM`.
    pub month: String,
    /// Provider id.
    pub provider: String,
    /// `own` or `credit`.
    pub via: String,
    /// Requests sent.
    pub requests: u64,
    /// US dollars.
    pub cost_usd: f64,
}

impl PlatformStore {
    /// Records spending.
    ///
    /// # Errors
    /// Database errors.
    pub fn record_spend(&self, record: &SpendRecord) -> Result<(), PlatformStoreError> {
        self.connection.execute(
            "INSERT INTO spend (spent_at, project_id, module, endpoint, provider, via, requests, cost_usd)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                record.at,
                record.project_id,
                record.module,
                record.endpoint,
                record.provider,
                record.via,
                record.requests,
                record.cost_usd
            ],
        )?;
        Ok(())
    }

    /// Spending by month (newest first), provider and how it was paid.
    ///
    /// # Errors
    /// Database errors.
    pub fn usage(&self) -> Result<Vec<UsageRow>, PlatformStoreError> {
        let mut statement = self.connection.prepare(
            "SELECT substr(spent_at, 1, 7) AS month, provider, via, sum(requests)::UBIGINT, sum(cost_usd)
             FROM spend GROUP BY month, provider, via ORDER BY month DESC, provider, via",
        )?;
        let rows = statement.query_map([], |row| {
            Ok(UsageRow {
                month: row.get(0)?,
                provider: row.get(1)?,
                via: row.get(2)?,
                requests: row.get(3)?,
                cost_usd: row.get(4)?,
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// US dollars a project spent in `month` (`YYYY-MM`) by module, most first.
    ///
    /// # Errors
    /// Database errors.
    pub fn project_spend(
        &self,
        project_id: i64,
        month: &str,
    ) -> Result<Vec<(String, f64)>, PlatformStoreError> {
        let mut statement = self.connection.prepare(
            "SELECT module, sum(cost_usd) FROM spend
             WHERE project_id = ? AND substr(spent_at, 1, 7) = ?
             GROUP BY module ORDER BY 2 DESC, module",
        )?;
        let rows = statement.query_map(params![project_id, month], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// US dollars spent on `provider` in `month` (`YYYY-MM`), both ways of paying.
    ///
    /// # Errors
    /// Database errors.
    pub fn spent_in(&self, provider: &str, month: &str) -> Result<f64, PlatformStoreError> {
        Ok(self.connection.query_row(
            "SELECT coalesce(sum(cost_usd), 0) FROM spend WHERE provider = ? AND substr(spent_at, 1, 7) = ?",
            params![provider, month],
            |row| row.get(0),
        )?)
    }
}
