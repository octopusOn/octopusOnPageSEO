//! Storage of the OctopusOn SEO Keyword Research.
//!
//! One DuckDB database (`kw.duckdb`) holds projects, their keywords with the sources that found
//! them, search numbers, intents and result pages, clusterings with their clusters and members,
//! manual placements that survive a new clustering, the site's own pages, and every run and
//! provider attempt with what it cost. A lock file keeps a second program from writing at the
//! same time.
//!
//! Large writes (an import of 50,000 keywords) go through appenders into staging tables, and
//! reads come in bounded pages.

mod clusterings;
mod keywords;
mod lock;
mod projects;
mod schema;
mod serps;
mod sites;

use std::fs;
use std::path::{Path, PathBuf};

use duckdb::{Connection, params};
use serde::{Deserialize, Serialize};

pub use clusterings::{
    ClusterPage, ClusterQuery, ClusterRecord, ClusterSort, ClusteringRow, ClusteringSpec,
    Competitor, MemberRecord, MemberRow, Pin,
};
pub use keywords::{
    Added, ClusterInput, IntentRecord, KeywordPage, KeywordQuery, KeywordRow, KeywordSort,
    KeywordStatus, MetricRecord, MonthlyVolume, NewKeyword,
};
pub use projects::{ProjectRow, ProjectSpec};
pub use serps::SerpRecord;
pub use sites::SitePageRecord;

use crate::lock::StoreLock;

/// Database file name.
pub const DATABASE: &str = "kw.duckdb";

/// Most rows one page query returns; exports read in pages of this size.
pub const MAX_PAGE: u64 = 10_000;

/// Clusterings kept per project; older ones are removed when a new one is saved.
pub const KEPT_CLUSTERINGS: u32 = 10;

/// Errors of the store.
#[derive(Debug, thiserror::Error)]
pub enum KwStoreError {
    /// Another program is using the data directory.
    #[error("the SEO Keyword Research data in `{}` is in use by another program", .0.display())]
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

impl KwStoreError {
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
    /// Keywords read from a file.
    Import,
    /// New keywords asked from sources.
    Discover,
    /// Search numbers, difficulty or intent asked for known keywords.
    Enrich,
    /// Result pages fetched for clustering.
    Serp,
    /// A clustering.
    Cluster,
    /// The site's pages read for mapping.
    Map,
}

impl RunKind {
    /// Stored name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Import => "import",
            Self::Discover => "discover",
            Self::Enrich => "enrich",
            Self::Serp => "serp",
            Self::Cluster => "cluster",
            Self::Map => "map",
        }
    }

    fn parse(text: &str) -> Result<Self, KwStoreError> {
        Ok(match text {
            "import" => Self::Import,
            "discover" => Self::Discover,
            "enrich" => Self::Enrich,
            "serp" => Self::Serp,
            "cluster" => Self::Cluster,
            "map" => Self::Map,
            other => return Err(KwStoreError::Corrupt(format!("run kind `{other}`"))),
        })
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
    /// Stored name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Done => "done",
            Self::Partial => "partial",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }

    fn parse(text: &str) -> Result<Self, KwStoreError> {
        Ok(match text {
            "running" => Self::Running,
            "done" => Self::Done,
            "partial" => Self::Partial,
            "failed" => Self::Failed,
            "cancelled" => Self::Cancelled,
            other => return Err(KwStoreError::Corrupt(format!("run status `{other}`"))),
        })
    }
}

/// One run, as listed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RunRow {
    /// Run id.
    pub id: i64,
    /// Project, when the run belongs to one.
    pub project_id: Option<i64>,
    /// What it did.
    pub kind: RunKind,
    /// How it ended, or `running`.
    pub status: RunStatus,
    /// Planned steps (keywords, requests).
    pub planned: u32,
    /// Finished steps.
    pub done: u32,
    /// Cost estimated before the run, US dollars.
    pub estimated_cost_usd: f64,
    /// Cost of its attempts, US dollars.
    pub cost_usd: f64,
    /// RFC 3339.
    pub started_at: String,
    /// RFC 3339, once it ended.
    pub finished_at: Option<String>,
    /// Why it failed.
    pub error: Option<String>,
}

/// How one request to a provider went.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AttemptOutcome {
    /// It answered.
    Success,
    /// It failed (network, server or response error).
    Failed,
    /// It refused: rate limit, blocked, check page.
    Refused,
    /// It was not asked (no key, over budget, switched off).
    Skipped,
}

impl AttemptOutcome {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::Failed => "failed",
            Self::Refused => "refused",
            Self::Skipped => "skipped",
        }
    }
}

/// Requests sent to a provider, for budgets and usage.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AttemptRecord {
    /// The run it belongs to.
    pub run_id: Option<i64>,
    /// Provider id (`wordstat`, `dataforseo`, `google-suggest` …).
    pub provider: String,
    /// RFC 3339.
    pub attempted_at: String,
    /// How it went.
    pub outcome: AttemptOutcome,
    /// Requests sent.
    pub requests: u32,
    /// Keywords or results it returned.
    pub items: u32,
    /// US dollars, reported where the provider reports it, else estimated.
    pub cost_usd: f64,
    /// Why it failed.
    pub error: Option<String>,
    /// Engine of a result-page request.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub engine: Option<String>,
    /// How it was paid: `own` key or OctopusOn SEO `credit`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub via: Option<String>,
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
    /// Keywords or results returned.
    pub items: u64,
    /// Attempts that failed or were refused.
    pub failures: u64,
    /// US dollars.
    pub cost_usd: f64,
    /// The part paid with OctopusOn SEO credits, US dollars.
    #[serde(default)]
    pub credit_usd: f64,
}

/// The SEO Keyword Research database of one data directory.
#[derive(Debug)]
pub struct KwStore {
    connection: Connection,
    root: PathBuf,
    _lock: StoreLock,
}

impl KwStore {
    /// Opens (creating and upgrading as needed) the store in `root`.
    ///
    /// # Errors
    /// [`KwStoreError::Busy`] while another program holds the store; I/O and database errors
    /// otherwise.
    pub fn open(root: &Path) -> Result<Self, KwStoreError> {
        fs::create_dir_all(root).map_err(KwStoreError::io(root))?;
        let lock = StoreLock::acquire(&root.join(".lock"))?;
        let mut connection = Connection::open(root.join(DATABASE))?;
        schema::migrate(&mut connection)?;
        // Staging rows of an interrupted write.
        connection.execute_batch("DELETE FROM incoming_keywords; DELETE FROM incoming_ids;")?;
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
    pub fn schema_version(&self) -> Result<i64, KwStoreError> {
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
    pub fn is_current(&self) -> Result<bool, KwStoreError> {
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
    ) -> Result<i64, KwStoreError> {
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

    /// Records the progress of a run.
    ///
    /// # Errors
    /// Database errors.
    pub fn progress_run(&self, run_id: i64, done: u32) -> Result<(), KwStoreError> {
        self.connection.execute(
            "UPDATE runs SET done = ? WHERE id = ?",
            params![done, run_id],
        )?;
        Ok(())
    }

    /// Ends a run; its cost is the sum of its attempts.
    ///
    /// # Errors
    /// Database errors.
    pub fn finish_run(
        &self,
        run_id: i64,
        status: RunStatus,
        done: u32,
        finished_at: &str,
        error: Option<&str>,
    ) -> Result<(), KwStoreError> {
        self.connection.execute(
            "UPDATE runs SET status = ?, done = ?, finished_at = ?, error = ?,
                 cost_usd = (SELECT coalesce(sum(cost_usd), 0) FROM provider_attempts WHERE run_id = ?)
             WHERE id = ?",
            params![status.as_str(), done, finished_at, error, run_id, run_id],
        )?;
        Ok(())
    }

    /// The newest runs, of one project or of all.
    ///
    /// # Errors
    /// Database errors, unknown stored names.
    pub fn runs(&self, project_id: Option<i64>, limit: u32) -> Result<Vec<RunRow>, KwStoreError> {
        let mut statement = self.connection.prepare(
            "SELECT id, project_id, kind, status, planned, done, estimated_cost_usd, cost_usd,
                    started_at, finished_at, error
             FROM runs WHERE ? IS NULL OR project_id = ? ORDER BY id DESC LIMIT ?",
        )?;
        let rows = statement.query_map(params![project_id, project_id, limit], |row| {
            Ok((
                RunRow {
                    id: row.get(0)?,
                    project_id: row.get(1)?,
                    kind: RunKind::Import,
                    status: RunStatus::Running,
                    planned: row.get(4)?,
                    done: row.get(5)?,
                    estimated_cost_usd: row.get(6)?,
                    cost_usd: row.get(7)?,
                    started_at: row.get(8)?,
                    finished_at: row.get(9)?,
                    error: row.get(10)?,
                },
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
            ))
        })?;
        let mut runs = Vec::new();
        for row in rows {
            let (mut run, kind, status) = row?;
            run.kind = RunKind::parse(&kind)?;
            run.status = RunStatus::parse(&status)?;
            runs.push(run);
        }
        Ok(runs)
    }

    /// Marks runs left `running` by a program that stopped as failed.
    ///
    /// # Errors
    /// Database errors.
    pub fn close_stale_runs(&self, finished_at: &str) -> Result<u64, KwStoreError> {
        let closed = self.connection.execute(
            "UPDATE runs SET status = 'failed', finished_at = ?, error = 'interrupted'
             WHERE status = 'running'",
            params![finished_at],
        )?;
        Ok(u64::try_from(closed).unwrap_or(u64::MAX))
    }

    /// Records requests sent to a provider.
    ///
    /// # Errors
    /// Database errors.
    pub fn record_attempt(&self, attempt: &AttemptRecord) -> Result<(), KwStoreError> {
        self.connection.execute(
            "INSERT INTO provider_attempts
                 (run_id, provider, attempted_at, outcome, requests, items, cost_usd, error, engine, via)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                attempt.run_id,
                attempt.provider,
                attempt.attempted_at,
                attempt.outcome.as_str(),
                attempt.requests,
                attempt.items,
                attempt.cost_usd,
                attempt.error,
                attempt.engine,
                attempt.via
            ],
        )?;
        Ok(())
    }

    /// Requests and spending per provider and month, newest month first.
    ///
    /// # Errors
    /// Database errors.
    pub fn usage(&self) -> Result<Vec<ProviderUsage>, KwStoreError> {
        let mut statement = self.connection.prepare(
            "SELECT provider, substr(attempted_at, 1, 7) AS month,
                    sum(requests)::UBIGINT, sum(items)::UBIGINT,
                    count(*) FILTER (WHERE outcome IN ('failed', 'refused'))::UBIGINT,
                    sum(cost_usd),
                    coalesce(sum(cost_usd) FILTER (WHERE via = 'credit'), 0)
             FROM provider_attempts GROUP BY provider, month ORDER BY month DESC, provider",
        )?;
        let rows = statement.query_map([], |row| {
            Ok(ProviderUsage {
                provider: row.get(0)?,
                month: row.get(1)?,
                requests: row.get(2)?,
                items: row.get(3)?,
                failures: row.get(4)?,
                cost_usd: row.get(5)?,
                credit_usd: row.get(6)?,
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Requests sent to a provider on a day (`YYYY-MM-DD`, UTC), for daily limits.
    ///
    /// # Errors
    /// Database errors.
    pub fn requests_on(&self, provider: &str, day: &str) -> Result<u64, KwStoreError> {
        Ok(self.connection.query_row(
            "SELECT coalesce(sum(requests), 0)::UBIGINT FROM provider_attempts
             WHERE provider = ? AND substr(attempted_at, 1, 10) = ?",
            params![provider, day],
            |row| row.get(0),
        )?)
    }

    /// Result-page requests sent to a provider for an engine on a day (`YYYY-MM-DD`, UTC),
    /// for the browser's daily limit.
    ///
    /// # Errors
    /// Database errors.
    pub fn engine_requests_on(
        &self,
        provider: &str,
        engine: &str,
        day: &str,
    ) -> Result<u64, KwStoreError> {
        Ok(self.connection.query_row(
            "SELECT coalesce(sum(requests), 0)::UBIGINT FROM provider_attempts
             WHERE provider = ? AND engine = ? AND substr(attempted_at, 1, 10) = ?",
            params![provider, engine, day],
            |row| row.get(0),
        )?)
    }

    /// Whether a provider refused a request on a day (`YYYY-MM-DD`, UTC).
    ///
    /// # Errors
    /// Database errors.
    pub fn refused_on(&self, provider: &str, day: &str) -> Result<bool, KwStoreError> {
        Ok(self.connection.query_row(
            "SELECT count(*) > 0 FROM provider_attempts
             WHERE provider = ? AND outcome = 'refused' AND substr(attempted_at, 1, 10) = ?",
            params![provider, day],
            |row| row.get(0),
        )?)
    }

    /// Requests sent to a provider since a moment (RFC 3339), for hourly quotas.
    ///
    /// # Errors
    /// Database errors.
    pub fn requests_since(&self, provider: &str, since: &str) -> Result<u64, KwStoreError> {
        Ok(self.connection.query_row(
            "SELECT coalesce(sum(requests), 0)::UBIGINT FROM provider_attempts
             WHERE provider = ? AND attempted_at >= ?",
            params![provider, since],
            |row| row.get(0),
        )?)
    }

    /// US dollars spent with a provider in a month (`YYYY-MM`).
    ///
    /// # Errors
    /// Database errors.
    pub fn spent(&self, provider: &str, month: &str) -> Result<f64, KwStoreError> {
        Ok(self.connection.query_row(
            "SELECT coalesce(sum(cost_usd), 0) FROM provider_attempts
             WHERE provider = ? AND substr(attempted_at, 1, 7) = ?",
            params![provider, month],
            |row| row.get(0),
        )?)
    }
}

/// Fills `incoming_ids` with `ids`; the caller empties it in the same transaction.
fn stage_ids(connection: &Connection, ids: &[i64]) -> Result<(), KwStoreError> {
    connection.execute("DELETE FROM incoming_ids", [])?;
    let mut appender = connection.appender("incoming_ids")?;
    for id in ids {
        appender.append_row(params![id])?;
    }
    appender.flush()?;
    Ok(())
}

fn json<T: Serialize + ?Sized>(value: &T) -> Result<String, KwStoreError> {
    serde_json::to_string(value).map_err(|error| KwStoreError::Corrupt(error.to_string()))
}

fn parse<T: for<'de> Deserialize<'de>>(text: &str) -> Result<T, KwStoreError> {
    serde_json::from_str(text).map_err(|error| KwStoreError::Corrupt(error.to_string()))
}

#[cfg(test)]
mod tests;
