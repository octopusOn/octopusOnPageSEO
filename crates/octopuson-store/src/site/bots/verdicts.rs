//! Maintaining the `bot_ips` verdict table.

use duckdb::{Connection, params};
use octopuson_core::Timestamp;

use super::ports::{BotIpPair, BotIpVerdict, BotIpVerifier};
use crate::error::StoreError;

/// Which stored verdicts to check again.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerifyScope {
    /// Only pairs whose verdict could change (DNS skipped or failed, data missing).
    Pending,
    /// Every pair.
    All,
}

/// Methods whose verdicts are provisional.
const RETRYABLE_METHODS: &str = "('dns-skipped', 'dns-failed', 'data-missing', 'not-checked')";

/// Verifies `(ip, bot)` pairs of the staged hits that have no verdict yet.
pub(crate) fn verify_staged_pairs(
    connection: &Connection,
    verifier: &dyn BotIpVerifier,
) -> Result<u64, StoreError> {
    let pairs = query_pairs(
        connection,
        "SELECT DISTINCT s.ip, s.bot FROM staging_hits s
         WHERE s.bot IS NOT NULL
           AND NOT EXISTS (SELECT 1 FROM bot_ips b WHERE b.ip = s.ip AND b.bot = s.bot)",
    )?;
    let verdicts = verifier.verify(&pairs);
    insert_verdicts(connection, &pairs, &verdicts)?;
    Ok(pairs.len() as u64)
}

/// Pairs of the user agents in the temp table `rebuild_user_agents` that occur in stored
/// hits (`hits_relation`) but have no verdict.
pub(crate) fn pairs_missing_from_hits(
    connection: &Connection,
    hits_relation: &str,
) -> Result<Vec<BotIpPair>, StoreError> {
    query_pairs(
        connection,
        &format!(
            "SELECT DISTINCT h.ip, a.bot
             FROM ({hits_relation}) h
             JOIN rebuild_user_agents r ON r.ua_id = h.ua_id
             JOIN user_agents a ON a.ua_id = h.ua_id
             WHERE a.bot IS NOT NULL
               AND NOT EXISTS (SELECT 1 FROM bot_ips b WHERE b.ip = h.ip AND b.bot = a.bot)"
        ),
    )
}

/// Stores verdicts of pairs that have none yet.
pub(crate) fn insert_verdicts(
    connection: &Connection,
    pairs: &[BotIpPair],
    verdicts: &[BotIpVerdict],
) -> Result<(), StoreError> {
    if pairs.is_empty() {
        return Ok(());
    }
    let now = Timestamp::now().as_unix_micros();
    let mut appender = connection.appender("bot_ips")?;
    for (pair, verdict) in pairs.iter().zip(verdicts) {
        appender.append_row(params![
            pair.ip,
            pair.bot,
            verdict.verdict,
            verdict.method,
            duckdb::types::Value::Timestamp(duckdb::types::TimeUnit::Microsecond, now),
        ])?;
    }
    appender.flush()?;
    Ok(())
}

/// Checks stored pairs again; returns the bots whose verdicts changed and how many pairs
/// were checked and changed.
pub(crate) fn reverify(
    connection: &Connection,
    verifier: &dyn BotIpVerifier,
    scope: VerifyScope,
) -> Result<(Vec<String>, u64, u64), StoreError> {
    let filter = match scope {
        VerifyScope::Pending => format!("WHERE method IN {RETRYABLE_METHODS}"),
        VerifyScope::All => String::new(),
    };
    let mut statement = connection.prepare(&format!(
        "SELECT ip, bot, verdict, method FROM bot_ips {filter}"
    ))?;
    let stored = statement
        .query_map([], |row| {
            Ok((
                BotIpPair {
                    ip: row.get(0)?,
                    bot: row.get(1)?,
                },
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    let pairs: Vec<BotIpPair> = stored.iter().map(|(pair, _, _)| pair.clone()).collect();
    let verdicts = verifier.verify(&pairs);

    connection.execute_batch(
        "CREATE OR REPLACE TEMP TABLE changed_verdicts (
             ip VARCHAR, bot VARCHAR, verdict VARCHAR, method VARCHAR);",
    )?;
    let mut changed_bots = Vec::new();
    let mut changed = 0_u64;
    {
        let mut appender =
            connection.appender_to_catalog_and_db("changed_verdicts", "temp", "main")?;
        for ((pair, verdict, method), new) in stored.iter().zip(&verdicts) {
            if new.verdict != verdict || new.method != method {
                appender.append_row(params![pair.ip, pair.bot, new.verdict, new.method])?;
                changed += 1;
                if new.verdict != verdict && !changed_bots.contains(&pair.bot) {
                    changed_bots.push(pair.bot.clone());
                }
            }
        }
        appender.flush()?;
    }
    connection.execute(
        "UPDATE bot_ips SET verdict = c.verdict, method = c.method, checked_at = make_timestamp(?)
         FROM changed_verdicts c
         WHERE bot_ips.ip = c.ip AND bot_ips.bot = c.bot",
        params![Timestamp::now().as_unix_micros()],
    )?;
    connection.execute_batch("DROP TABLE changed_verdicts;")?;
    Ok((changed_bots, pairs.len() as u64, changed))
}

fn query_pairs(connection: &Connection, sql: &str) -> Result<Vec<BotIpPair>, StoreError> {
    let mut statement = connection.prepare(sql)?;
    let pairs = statement
        .query_map([], |row| {
            Ok(BotIpPair {
                ip: row.get(0)?,
                bot: row.get(1)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(pairs)
}
