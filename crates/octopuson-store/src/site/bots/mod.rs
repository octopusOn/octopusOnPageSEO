//! Bot verdicts per `(ip, bot)` pair and the rollup rows derived from them.

mod ports;
mod rollup;
mod verdicts;

pub use ports::{
    BotIpPair, BotIpVerdict, BotIpVerifier, BotTag, NoBotVerification, UserAgentClassifier,
};
pub(super) use rollup::rebuild_user_agent_rollups;
pub use verdicts::VerifyScope;
pub(super) use verdicts::{
    insert_verdicts, pairs_missing_from_hits, reverify, verify_staged_pairs,
};

use duckdb::{Connection, OptionalExt, params};

use crate::error::StoreError;

const CATALOG_VERSION_KEY: &str = "bot_catalog_version";

/// Classification rules version the stored user agents were classified with (0 = never).
pub(super) fn stored_catalog_version(connection: &Connection) -> Result<u32, StoreError> {
    let value: Option<String> = connection
        .query_row(
            "SELECT value FROM site_meta WHERE key = ?",
            params![CATALOG_VERSION_KEY],
            |row| row.get(0),
        )
        .optional()?;
    Ok(value.and_then(|v| v.parse().ok()).unwrap_or(0))
}

pub(super) fn set_catalog_version(connection: &Connection, version: u32) -> Result<(), StoreError> {
    connection.execute(
        "INSERT OR REPLACE INTO site_meta VALUES (?, ?)",
        params![CATALOG_VERSION_KEY, version.to_string()],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests;
