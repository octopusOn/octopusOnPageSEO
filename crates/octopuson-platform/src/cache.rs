//! Answers of paid providers kept for a while, so asking the same thing again is free.
//!
//! An entry is found by [`cache_key`]: the endpoint and the task with its fields in a fixed
//! order. How long an answer stays fresh depends on how fast its data changes
//! ([`CacheClass`]); a stale answer is still shown, marked with its age, until the user pays to
//! refresh it.

use duckdb::params;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{PlatformStore, PlatformStoreError};

/// How fast a kind of data changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CacheClass {
    /// Domain rankings and traffic: updated monthly by the provider.
    Domain,
    /// Backlink profiles.
    Backlinks,
    /// Live result pages.
    Serp,
    /// Keyword numbers: volumes, difficulty, intent.
    Keywords,
    /// AI answers and mentions.
    Ai,
}

impl CacheClass {
    /// Seconds an answer stays fresh.
    pub const fn ttl_seconds(self) -> i64 {
        const DAY: i64 = 86_400;
        match self {
            Self::Domain => 7 * DAY,
            Self::Backlinks => 3 * DAY,
            Self::Serp | Self::Ai => DAY,
            Self::Keywords => 30 * DAY,
        }
    }

    /// Stored name.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Domain => "domain",
            Self::Backlinks => "backlinks",
            Self::Serp => "serp",
            Self::Keywords => "keywords",
            Self::Ai => "ai",
        }
    }
}

/// A kept answer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CacheEntry {
    /// What the provider answered (its result list).
    pub body: Value,
    /// RFC 3339.
    pub fetched_at: String,
    /// Unix seconds after which it is stale.
    pub expires_at: i64,
    /// What it cost when it was fetched, US dollars.
    pub cost_usd: f64,
    /// `own` or `credit`.
    pub via: String,
}

impl CacheEntry {
    /// Whether it is still fresh at `now` (Unix seconds).
    pub const fn is_fresh(&self, now: i64) -> bool {
        now < self.expires_at
    }
}

/// The value with object fields sorted, at every depth.
fn canonical(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut entries: Vec<(&String, &Value)> = map.iter().collect();
            entries.sort_by_key(|(key, _)| *key);
            Value::Object(
                entries
                    .into_iter()
                    .map(|(key, value)| (key.clone(), canonical(value)))
                    .collect(),
            )
        }
        Value::Array(items) => Value::Array(items.iter().map(canonical).collect()),
        other => other.clone(),
    }
}

/// The key of an answer: a hash of the endpoint and the task, whatever order its fields were
/// written in.
pub fn cache_key(endpoint: &str, task: &Value) -> String {
    let mut hasher = blake3::Hasher::new();
    hasher.update(endpoint.as_bytes());
    hasher.update(b"\n");
    hasher.update(canonical(task).to_string().as_bytes());
    hasher.finalize().to_hex().to_string()
}

impl PlatformStore {
    /// The kept answer under `key`, fresh or stale.
    ///
    /// # Errors
    /// Database errors, or a stored body that is not JSON.
    pub fn cached(&self, key: &str) -> Result<Option<CacheEntry>, PlatformStoreError> {
        let mut statement = self.connection.prepare(
            "SELECT body, fetched_at, expires_at, cost_usd, via FROM cache WHERE key = ?",
        )?;
        let mut rows = statement.query_map(params![key], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, f64>(3)?,
                row.get::<_, String>(4)?,
            ))
        })?;
        let Some(row) = rows.next() else {
            return Ok(None);
        };
        let (body, fetched_at, expires_at, cost_usd, via) = row?;
        Ok(Some(CacheEntry {
            body: serde_json::from_str(&body)
                .map_err(|error| PlatformStoreError::Corrupt(error.to_string()))?,
            fetched_at,
            expires_at,
            cost_usd,
            via,
        }))
    }

    /// Keeps an answer under `key`, replacing an older one.
    ///
    /// # Errors
    /// Database errors.
    pub fn keep(
        &self,
        key: &str,
        endpoint: &str,
        class: CacheClass,
        project_id: Option<i64>,
        entry: &CacheEntry,
    ) -> Result<(), PlatformStoreError> {
        self.connection.execute(
            "INSERT OR REPLACE INTO cache (key, endpoint, class, project_id, body, fetched_at, expires_at, cost_usd, via)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                key,
                endpoint,
                class.as_str(),
                project_id,
                entry.body.to_string(),
                entry.fetched_at,
                entry.expires_at,
                entry.cost_usd,
                entry.via
            ],
        )?;
        Ok(())
    }

    /// Removes answers stale for more than `grace` seconds at `now` (Unix seconds); returns how
    /// many went.
    ///
    /// # Errors
    /// Database errors.
    pub fn drop_stale(&self, now: i64, grace: i64) -> Result<usize, PlatformStoreError> {
        Ok(self.connection.execute(
            "DELETE FROM cache WHERE expires_at < ?",
            params![now.saturating_sub(grace)],
        )?)
    }
}
