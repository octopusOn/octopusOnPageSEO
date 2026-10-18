//! Rebuilding `r_day_url` rows of selected user agents from the Parquet hits.
//!
//! Needed when the bot or verdict behind stored hits changes; rows of other user agents are
//! never touched.

use duckdb::Connection;

use crate::error::StoreError;

/// Re-aggregates the rollup rows of the user agents listed in the temp table
/// `rebuild_user_agents (ua_id BIGINT)`. `hits_relation` provides the Parquet hits with a
/// `filename` column, or `None` when no hits are published.
pub(crate) fn rebuild_user_agent_rollups(
    connection: &Connection,
    hits_relation: Option<&str>,
) -> Result<(), StoreError> {
    connection.execute_batch(
        "DELETE FROM r_day_url WHERE ua_id IN (SELECT ua_id FROM rebuild_user_agents);",
    )?;
    let Some(hits) = hits_relation else {
        return Ok(());
    };
    // `import_id` lives only in file names (`import_<id>_f<k>_<i>.parquet`).
    connection.execute_batch(&format!(
        "INSERT INTO r_day_url (day, url_id, ua_id, status, hits, bytes, duration_sum_us,
                               duration_count, duration_max_us, first_ts, last_ts, import_id,
                               verification)
         SELECT CAST(h.ts AS DATE), h.url_id, h.ua_id, h.status, count(*), sum(h.bytes),
                sum(h.duration_us), count(h.duration_us), max(h.duration_us), min(h.ts),
                max(h.ts),
                CAST(regexp_extract(h.filename, 'import_([0-9]+)_', 1) AS BIGINT),
                b.verdict
         FROM ({hits}) h
         JOIN rebuild_user_agents r ON r.ua_id = h.ua_id
         JOIN user_agents a ON a.ua_id = h.ua_id
         LEFT JOIN bot_ips b ON b.ip = h.ip AND b.bot = a.bot
         GROUP BY CAST(h.ts AS DATE), h.url_id, h.ua_id, h.status,
                  regexp_extract(h.filename, 'import_([0-9]+)_', 1), b.verdict;"
    ))?;
    Ok(())
}
