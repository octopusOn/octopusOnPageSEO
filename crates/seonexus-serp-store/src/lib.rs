//! Storage of the SERP Checker.
//!
//! One DuckDB database (`serp.duckdb`) holds runs, snapshots with their organic results and
//! feature blocks, and every provider attempt with what it cost. Raw responses are kept beside
//! it as zstd files under `raw/YYYY-MM/`, so a page can be read again after a selector fix
//! without a new request. A lock file keeps a second program from writing at the same time.

mod analyses;
mod domains;
mod lock;
mod projects;
mod schema;
mod snapshots;

use std::fs;
use std::path::{Path, PathBuf};

use duckdb::{Connection, params};
use serde::{Deserialize, Serialize};

pub use analyses::{AnalysisRecord, AnalysisSummary, PageScoreRow};
pub use domains::{Dataset, DomainFact};
pub use projects::{KeywordRow, ProjectRow, RankingRow, TrackerRow};
pub use snapshots::{SnapshotFilter, SnapshotSummary};

use crate::lock::StoreLock;

/// Database file name.
pub const DATABASE: &str = "serp.duckdb";

/// Errors of the store.
#[derive(Debug, thiserror::Error)]
pub enum SerpStoreError {
    /// Another program is using the data directory.
    #[error("the SERP Checker data in `{}` is in use by another program", .0.display())]
    Busy(PathBuf),
    /// A file could not be read or written.
    #[error("cannot access `{}`: {source}", path.display())]
    Io {
        /// The file.
        path: PathBuf,
        /// The error.
        source: std::io::Error,
    },
    /// The database failed.
    #[error("database error: {0}")]
    Database(#[from] duckdb::Error),
    /// Stored JSON could not be read.
    #[error("stored data is invalid: {0}")]
    Corrupt(String),
    /// No such record.
    #[error("{0} not found")]
    NotFound(String),
}

impl SerpStoreError {
    fn io(path: &Path) -> impl FnOnce(std::io::Error) -> Self + '_ {
        move |source| Self::Io {
            path: path.to_owned(),
            source,
        }
    }
}

/// What a run does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RunKind {
    /// One or more searches.
    Search,
    /// A keyword analysis (search, crawl, scores).
    Analysis,
    /// A rank check of a project.
    Rank,
}

impl RunKind {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Search => "search",
            Self::Analysis => "analysis",
            Self::Rank => "rank",
        }
    }
}

/// How a run ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RunStatus {
    /// Still going.
    Running,
    /// Every planned step finished.
    Done,
    /// Some steps failed.
    Partial,
    /// It stopped with an error.
    Failed,
    /// The user stopped it.
    Cancelled,
}

impl RunStatus {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Done => "done",
            Self::Partial => "partial",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }
}

/// Requests and spending of one provider in one month.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProviderUsage {
    /// Provider id.
    pub provider: String,
    /// `YYYY-MM`.
    pub month: String,
    /// Requests sent.
    pub requests: u64,
    /// Snapshots it returned.
    pub successes: u64,
    /// Attempts that failed.
    pub failures: u64,
    /// US dollars, reported where the provider reports it, else estimated.
    pub cost_usd: f64,
    /// The part paid with OctopusOn SEO credits, US dollars.
    #[serde(default)]
    pub credit_usd: f64,
}

/// The SERP Checker database of one data directory.
#[derive(Debug)]
pub struct SerpStore {
    connection: Connection,
    root: PathBuf,
    _lock: StoreLock,
}

impl SerpStore {
    /// Opens (creating and upgrading as needed) the store in `root`.
    ///
    /// # Errors
    /// [`SerpStoreError::Busy`] while another program holds the store; I/O and database
    /// errors otherwise.
    pub fn open(root: &Path) -> Result<Self, SerpStoreError> {
        fs::create_dir_all(root).map_err(SerpStoreError::io(root))?;
        let lock = StoreLock::acquire(&root.join(".lock"))?;
        let mut connection = Connection::open(root.join(DATABASE))?;
        schema::migrate(&mut connection)?;
        Ok(Self {
            connection,
            root: root.to_owned(),
            _lock: lock,
        })
    }

    /// The data directory.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Schema version of the database.
    ///
    /// # Errors
    /// Database errors.
    pub fn schema_version(&self) -> Result<i64, SerpStoreError> {
        Ok(self.connection.query_row(
            "SELECT coalesce(max(version), 0) FROM schema_migrations",
            [],
            |row| row.get(0),
        )?)
    }

    /// Whether the schema is the newest this build knows.
    ///
    /// # Errors
    /// Database errors.
    pub fn is_current(&self) -> Result<bool, SerpStoreError> {
        Ok(self.schema_version()? == schema::latest())
    }

    /// Starts a run.
    ///
    /// # Errors
    /// Database errors.
    pub fn begin_run(
        &self,
        kind: RunKind,
        project_id: Option<i64>,
        planned: u32,
        estimated_cost_usd: f64,
        started_at: &str,
    ) -> Result<i64, SerpStoreError> {
        Ok(self.connection.query_row(
            "INSERT INTO runs (kind, project_id, status, started_at, planned, estimated_cost_usd)
             VALUES (?, ?, 'running', ?, ?, ?) RETURNING id",
            params![
                kind.as_str(),
                project_id,
                started_at,
                planned,
                estimated_cost_usd
            ],
            |row| row.get(0),
        )?)
    }

    /// Ends a run.
    ///
    /// # Errors
    /// Database errors.
    pub fn finish_run(
        &self,
        run_id: i64,
        status: RunStatus,
        finished_at: &str,
        error: Option<&str>,
    ) -> Result<(), SerpStoreError> {
        self.connection.execute(
            "UPDATE runs SET status = ?, finished_at = ?, error = ?,
                 done = (SELECT count(*) FROM snapshots WHERE run_id = ?),
                 cost_usd = (SELECT coalesce(sum(cost_usd), 0) FROM provider_attempts WHERE run_id = ?)
             WHERE id = ?",
            params![status.as_str(), finished_at, error, run_id, run_id, run_id],
        )?;
        Ok(())
    }

    /// Requests and spending per provider and month, newest month first.
    ///
    /// # Errors
    /// Database errors.
    pub fn usage(&self) -> Result<Vec<ProviderUsage>, SerpStoreError> {
        let mut statement = self.connection.prepare(
            "SELECT provider, substr(attempted_at, 1, 7) AS month,
                    sum(requests)::UBIGINT,
                    count(*) FILTER (WHERE outcome = 'success')::UBIGINT,
                    count(*) FILTER (WHERE outcome NOT IN ('success', 'skipped'))::UBIGINT,
                    sum(cost_usd),
                    coalesce(sum(cost_usd) FILTER (WHERE via = 'credit'), 0)
             FROM provider_attempts GROUP BY provider, month ORDER BY month DESC, provider",
        )?;
        let rows = statement.query_map([], |row| {
            Ok(ProviderUsage {
                provider: row.get(0)?,
                month: row.get(1)?,
                requests: row.get(2)?,
                successes: row.get(3)?,
                failures: row.get(4)?,
                cost_usd: row.get(5)?,
                credit_usd: row.get(6)?,
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Marks how a snapshot's successful attempt was paid (`own` or `credit`).
    ///
    /// # Errors
    /// Database errors.
    pub fn set_attempt_via(&self, snapshot_id: i64, via: &str) -> Result<(), SerpStoreError> {
        self.connection.execute(
            "UPDATE provider_attempts SET via = ? WHERE snapshot_id = ? AND outcome = 'success'",
            params![via, snapshot_id],
        )?;
        Ok(())
    }

    /// Requests a provider sent for an engine on a day (`YYYY-MM-DD`, UTC).
    ///
    /// # Errors
    /// Database errors.
    pub fn requests_on(
        &self,
        provider: &str,
        engine: &str,
        day: &str,
    ) -> Result<u64, SerpStoreError> {
        Ok(self.connection.query_row(
            "SELECT coalesce(sum(requests), 0)::UBIGINT FROM snapshots
             WHERE provider = ? AND engine = ? AND substr(fetched_at, 1, 10) = ?",
            params![provider, engine, day],
            |row| row.get(0),
        )?)
    }

    /// US dollars spent with a provider in a month (`YYYY-MM`).
    ///
    /// # Errors
    /// Database errors.
    pub fn spent(&self, provider: &str, month: &str) -> Result<f64, SerpStoreError> {
        Ok(self.connection.query_row(
            "SELECT coalesce(sum(cost_usd), 0) FROM provider_attempts
             WHERE provider = ? AND substr(attempted_at, 1, 7) = ?",
            params![provider, month],
            |row| row.get(0),
        )?)
    }
}

#[cfg(test)]
mod tests;
