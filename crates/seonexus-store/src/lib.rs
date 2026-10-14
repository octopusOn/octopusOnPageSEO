//! Storage adapter of the octopusSEO Log Analyser.
//!
//! - [`Catalog`] stores clients, sites and saved queries (`catalog.duckdb`).
//! - [`SiteStore`] owns one site: a DuckDB database with dimensions, rollups and import
//!   history, plus immutable monthly Parquet files holding raw hits.
//! - [`ImportSession`] appends [`HitBatch`]es transactionally with de-duplication against
//!   earlier imports.
//! - [`SiteReader`] runs compiled queries read-only. See ADR 0003 for the storage design.

mod batch;
mod catalog;
mod db;
mod error;
mod fingerprint;
mod lock;
mod paths;
mod settings;
mod site;
mod sql;

pub use batch::{HitBatch, HitBatchBuilder};
pub use catalog::{Catalog, Client, ClientId, NewSite, SavedQuery, Site, SiteId};
pub use error::StoreError;
pub use fingerprint::FileFingerprint;
pub use paths::DataDir;
pub use settings::StoreSettings;
pub use site::{
    BotIpPair, BotIpVerdict, BotIpVerifier, BotTag, BotVerifyOutcome, GeoRefresh,
    ImportHistoryEntry, ImportOptions, ImportOutcome, ImportRecord, ImportSession, ImportedFile,
    IpGeo, IpGeoLookup, NoBotVerification, ReclassifyOutcome, SiteReader, SiteSnapshot, SiteStore,
    SiteTotals, SkippedSample, UserAgentClassifier, VerifyScope,
};
