//! Keeping bot classification and verdicts of stored hits up to date.

use duckdb::{Connection, params};

use super::bots::{
    self, BotIpVerifier, UserAgentClassifier, VerifyScope, insert_verdicts,
    pairs_missing_from_hits, rebuild_user_agent_rollups, reverify,
};
use super::{SiteStore, relation};
use crate::error::StoreError;
use crate::sql::string_literal;

/// Result of [`SiteStore::reclassify_user_agents`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ReclassifyOutcome {
    /// Whether the stored classification was outdated.
    pub performed: bool,
    /// User agents whose bot changed.
    pub changed_user_agents: u64,
    /// `(ip, bot)` pairs verified for the first time.
    pub new_pairs: u64,
}

/// Result of [`SiteStore::verify_bots`].
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BotVerifyOutcome {
    /// Pairs checked.
    pub checked: u64,
    /// Pairs whose verdict or method changed.
    pub changed: u64,
    /// Bots whose rollup rows were rebuilt.
    pub rebuilt_bots: Vec<String>,
}

const REBUILD_TABLE_SQL: &str =
    "CREATE OR REPLACE TEMP TABLE rebuild_user_agents (ua_id BIGINT NOT NULL);";

impl SiteStore {
    /// Classifies stored user agents again when `classifier.version()` differs from the
    /// version they were classified with, verifies `(ip, bot)` pairs of newly identified bots
    /// and rebuilds the rollup rows of every user agent whose bot changed.
    ///
    /// # Errors
    /// Database or filesystem errors; nothing changes on failure.
    pub fn reclassify_user_agents(
        &mut self,
        classifier: &dyn UserAgentClassifier,
        verifier: &dyn BotIpVerifier,
    ) -> Result<ReclassifyOutcome, StoreError> {
        if bots::stored_catalog_version(&self.connection)? == classifier.version() {
            return Ok(ReclassifyOutcome::default());
        }
        let hits = relation::hits_with_filename(&self.paths)?;
        in_transaction(&self.connection, |connection| {
            let changed = update_classification(connection, classifier)?;
            let mut new_pairs = 0;
            if changed > 0 {
                if let Some(hits) = &hits {
                    let pairs = pairs_missing_from_hits(connection, hits)?;
                    let verdicts = verifier.verify(&pairs);
                    insert_verdicts(connection, &pairs, &verdicts)?;
                    new_pairs = pairs.len() as u64;
                }
                rebuild_user_agent_rollups(connection, hits.as_deref())?;
            }
            connection.execute_batch("DROP TABLE rebuild_user_agents;")?;
            bots::set_catalog_version(connection, classifier.version())?;
            Ok(ReclassifyOutcome {
                performed: true,
                changed_user_agents: changed,
                new_pairs,
            })
        })
    }

    /// Checks stored `(ip, bot)` verdicts again (e.g. with DNS or newer IP lists) and rebuilds
    /// the rollup rows of bots whose verdicts changed.
    ///
    /// # Errors
    /// Database or filesystem errors; nothing changes on failure.
    pub fn verify_bots(
        &mut self,
        verifier: &dyn BotIpVerifier,
        scope: VerifyScope,
    ) -> Result<BotVerifyOutcome, StoreError> {
        let hits = relation::hits_with_filename(&self.paths)?;
        in_transaction(&self.connection, |connection| {
            let (rebuilt_bots, checked, changed) = reverify(connection, verifier, scope)?;
            if !rebuilt_bots.is_empty() {
                let list: Vec<String> = rebuilt_bots.iter().map(|b| string_literal(b)).collect();
                connection.execute_batch(&format!(
                    "{REBUILD_TABLE_SQL}
                     INSERT INTO rebuild_user_agents
                     SELECT ua_id FROM user_agents WHERE bot IN ({});",
                    list.join(", ")
                ))?;
                rebuild_user_agent_rollups(connection, hits.as_deref())?;
                connection.execute_batch("DROP TABLE rebuild_user_agents;")?;
            }
            Ok(BotVerifyOutcome {
                checked,
                changed,
                rebuilt_bots,
            })
        })
    }

    /// Counts stored `(ip, bot)` verdicts by verdict name.
    ///
    /// # Errors
    /// Database errors.
    pub fn verdict_counts(&self) -> Result<Vec<(String, u64)>, StoreError> {
        let mut statement = self
            .connection
            .prepare("SELECT verdict, count(*) FROM bot_ips GROUP BY 1 ORDER BY 1")?;
        let rows = statement.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;
        Ok(rows.collect::<Result<_, _>>()?)
    }
}

/// Re-classifies every user agent; changed ones are listed in `rebuild_user_agents`.
fn update_classification(
    connection: &Connection,
    classifier: &dyn UserAgentClassifier,
) -> Result<u64, StoreError> {
    let mut statement = connection.prepare("SELECT ua_id, raw, bot FROM user_agents")?;
    let stored = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    let raws: Vec<String> = stored.iter().map(|(_, raw, _)| raw.clone()).collect();
    let tags = classifier.classify(&raws);

    connection.execute_batch(&format!(
        "{REBUILD_TABLE_SQL}
         CREATE OR REPLACE TEMP TABLE ua_updates (
             ua_id BIGINT NOT NULL, bot VARCHAR, bot_category VARCHAR);"
    ))?;
    let mut changed = 0_u64;
    {
        let mut updates = connection.appender_to_catalog_and_db("ua_updates", "temp", "main")?;
        let mut rebuild =
            connection.appender_to_catalog_and_db("rebuild_user_agents", "temp", "main")?;
        for ((ua_id, _, old), tag) in stored.iter().zip(&tags) {
            let new = tag.map(|t| t.id);
            updates.append_row(params![ua_id, new, tag.map(|t| t.category)])?;
            if old.as_deref() != new {
                rebuild.append_row(params![ua_id])?;
                changed += 1;
            }
        }
        updates.flush()?;
        rebuild.flush()?;
    }
    connection.execute_batch(
        "UPDATE user_agents SET bot = u.bot, bot_category = u.bot_category
         FROM ua_updates u WHERE user_agents.ua_id = u.ua_id;
         DROP TABLE ua_updates;",
    )?;
    Ok(changed)
}

/// Runs `work` in a transaction that is rolled back on error.
pub(super) fn in_transaction<T>(
    connection: &Connection,
    work: impl FnOnce(&Connection) -> Result<T, StoreError>,
) -> Result<T, StoreError> {
    connection.execute_batch("BEGIN TRANSACTION;")?;
    match work(connection) {
        Ok(value) => {
            connection.execute_batch("COMMIT;")?;
            Ok(value)
        }
        Err(error) => {
            if let Err(rollback) = connection.execute_batch("ROLLBACK;") {
                tracing::error!(%rollback, "rollback failed");
            }
            Err(error)
        }
    }
}
