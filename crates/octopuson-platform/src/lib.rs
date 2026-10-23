//! Domain model and storage of the OctopusOn GEO-SEO Platform.
//!
//! One DuckDB database (`platform.duckdb`) per workspace holds the projects every module works
//! on, the answers of paid providers kept for a while so the same question is not paid twice
//! ([`cache`]), every paid request with what it cost ([`spend`]), keyword lists ([`lists`]),
//! rank tracking ([`rank`]), site audits ([`audits`]) and AI visibility ([`ai`]). On the
//! desktop the workspace is the program's data folder; on the web every account has its own
//! folder, which desktops signed in to the account keep in step with ([`sync`]). A lock file
//! keeps a second program from writing at the same time.

pub mod ads;
pub mod ai;
pub mod alerts;
pub mod audits;
pub mod cache;
pub mod content;
pub mod gsc;
pub mod links;
pub mod lists;
pub mod local;
mod lock;
pub mod metrics;
pub mod opportunities;
pub mod projects;
pub mod rank;
pub mod reports;
mod schema;
pub mod snapshots;
pub mod spend;
pub mod sync;
pub mod vitals;

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use duckdb::Connection;

pub use ads::{AdCopy, AdSeen, MAX_AD_KEYWORDS};
pub use ai::{
    AI_ENGINES, AiAnswerRow, AiPrompt, AiRunRow, AiSettings, AnswerBrand, AnswerSource,
    DEFAULT_AI_ENGINES, MAX_PROMPTS, Rival,
};
pub use alerts::{
    ALERT_EMAILS, ALERT_LANGUAGES, AlertRow, AlertRules, AlertSettings, MAX_ALERT_RECIPIENTS,
    NewAlert,
};
pub use audits::{AuditEnd, AuditRow, AuditSettings};
pub use cache::{CacheClass, CacheEntry, cache_key};
pub use content::{ContentBriefRow, ContentScoreRow, MAX_DRAFT_CHARS, PageCandidate, ScoreToSave};
pub use gsc::{
    GSC_COMPARE, GSC_DAYS, GSC_SCHEDULES, GSC_YEAR_DAYS, GscData, GscSettings, GscSyncEnd,
    GscSyncRow, Period, QueryPage, SearchKind, SearchRow, TrafficKind, TrafficRow,
};
pub use links::{LINK_DECISIONS, LinkDecision, LinkProspect, PROSPECT_SOURCES, PROSPECT_STATES};
pub use lists::{KeywordListRow, ListKeyword, MAX_LIST_KEYWORDS};
pub use local::{
    LocalPointRow, LocalProfileRow, LocalReviewRow, LocalRunRow, LocalSettings, LocalTaskRow,
    MAX_LOCAL_KEYWORDS,
};
pub use metrics::{METRIC_KINDS, MetricPoint};
pub use opportunities::{OPPORTUNITY_STATES, OpportunityState};
pub use projects::{MAX_BRANDS, MAX_COMPETITORS, ProjectRow, ProjectSpec, normalize_domain};
pub use rank::{
    CheckResult, MAX_TRACKED, MAX_TRACKERS, RankCheckRow, RankPlan, RankRunRow, RankSettings,
    RankTaskRow, RunEnd, TrackedKeyword, TrackerRow, TrackerSpec,
};
pub use reports::{
    Branding, DEFAULT_DESIGN, DeliveryRow, MAX_RECIPIENTS, REPORT_DAYS, REPORT_DESIGNS,
    REPORT_LANGUAGES, REPORT_SCHEDULES, REPORT_WIDGETS, ReportColors, ReportRow, ReportSpec,
};
pub use snapshots::{SNAPSHOT_SCHEDULES, SnapshotEnd, SnapshotRow, SnapshotSettings};
pub use spend::{SpendRecord, UsageRow};
pub use sync::kinds::{first_sync_kinds, sync_kind_versions};
pub use sync::{AuditCrawl, SyncAck, SyncApplied, SyncBatch, SyncChange, SyncSide};
pub use vitals::VitalCheck;

use crate::lock::StoreLock;

/// Database file name.
pub const DATABASE: &str = "platform.duckdb";

/// Errors of the store.
#[derive(Debug, thiserror::Error)]
pub enum PlatformStoreError {
    /// Another program is using the data directory.
    #[error("the GEO-SEO Platform data in `{}` is in use by another program", .0.display())]
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
    /// Stored data could not be read.
    #[error("stored data is invalid: {0}")]
    Corrupt(String),
    /// No such record.
    #[error("{0} not found")]
    NotFound(String),
    /// A value the user gave is not acceptable.
    #[error("{0}")]
    Invalid(String),
}

impl PlatformStoreError {
    fn io(path: &Path) -> impl FnOnce(std::io::Error) -> Self + '_ {
        move |source| Self::Io {
            path: path.to_owned(),
            source,
        }
    }
}

/// The platform's database in one data directory.
#[derive(Debug)]
pub struct PlatformStore {
    connection: Connection,
    root: PathBuf,
    _lock: StoreLock,
}

impl PlatformStore {
    /// Opens (and creates or upgrades) the database in `root`.
    ///
    /// # Errors
    /// [`PlatformStoreError::Busy`] while another program uses it, or I/O and database errors.
    pub fn open(root: &Path) -> Result<Self, PlatformStoreError> {
        fs::create_dir_all(root).map_err(PlatformStoreError::io(root))?;
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
    pub fn schema_version(&self) -> Result<i64, PlatformStoreError> {
        Ok(self.connection.query_row(
            "SELECT coalesce(max(version), 0) FROM schema_migrations",
            [],
            |row| row.get(0),
        )?)
    }
}

/// Seconds since the Unix epoch.
pub fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| {
            i64::try_from(elapsed.as_secs()).unwrap_or(i64::MAX)
        })
}

/// A list stored as a JSON array.
fn list_json(values: &[String]) -> String {
    serde_json::to_string(values).unwrap_or_else(|_| "[]".to_owned())
}

/// A stored JSON array.
fn parse_list(text: &str) -> Result<Vec<String>, PlatformStoreError> {
    serde_json::from_str(text).map_err(|error| PlatformStoreError::Corrupt(error.to_string()))
}

#[cfg(test)]
mod tests;
