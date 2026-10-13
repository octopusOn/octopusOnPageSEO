//! One flush: de-duplicate staged rows, resolve dimension ids, write Parquet, add rollups.

use duckdb::{Connection, params};

use super::dedupe::remove_duplicates;
use crate::error::StoreError;
use crate::paths::SitePaths;
use crate::site::BotIpVerifier;
use crate::site::bots::verify_staged_pairs;
use crate::sql::{path_literal, string_literal};

/// Rows written and dropped by one flush.
#[derive(Debug, Clone, Copy, Default)]
pub(super) struct FlushResult {
    pub(super) inserted: u64,
    pub(super) duplicates: u64,
}

pub(super) fn flush(
    connection: &Connection,
    paths: &SitePaths,
    import_id: i64,
    flush_index: u32,
    verifier: &dyn BotIpVerifier,
) -> Result<FlushResult, StoreError> {
    let duplicates = timed("dedupe", || remove_duplicates(connection, paths, import_id))?;
    let inserted: u64 =
        connection.query_row("SELECT count(*) FROM staging_hits", [], |row| row.get(0))?;
    if inserted > 0 {
        timed("dimensions", || register_dimensions(connection))?;
        timed("bot verdicts", || verify_staged_pairs(connection, verifier))?;
        timed("parquet", || {
            write_parquet(connection, paths, import_id, flush_index)
        })?;
        timed("rollups", || insert_rollups(connection, import_id))?;
    }
    connection.execute_batch("DELETE FROM staging_hits;")?;
    Ok(FlushResult {
        inserted,
        duplicates,
    })
}

fn timed<T>(step: &str, run: impl FnOnce() -> Result<T, StoreError>) -> Result<T, StoreError> {
    let started = std::time::Instant::now();
    let result = run();
    tracing::debug!(
        step,
        elapsed_ms = started.elapsed().as_millis(),
        "flush step"
    );
    result
}

fn register_dimensions(connection: &Connection) -> Result<(), StoreError> {
    connection.execute_batch(
        "INSERT INTO urls (url_id, raw_url)
         SELECT nextval('url_id_seq'), n.target
         FROM (SELECT DISTINCT target FROM staging_hits) n
         WHERE NOT EXISTS (SELECT 1 FROM urls u WHERE u.raw_url = n.target);

         INSERT INTO user_agents (ua_id, raw, bot, bot_category)
         SELECT nextval('ua_id_seq'), n.user_agent, n.bot, n.bot_category
         FROM (
             SELECT user_agent, any_value(bot) AS bot, any_value(bot_category) AS bot_category
             FROM staging_hits
             WHERE user_agent IS NOT NULL
             GROUP BY user_agent
         ) n
         WHERE NOT EXISTS (SELECT 1 FROM user_agents a WHERE a.raw = n.user_agent);",
    )?;
    Ok(())
}

/// Replaces URL and user-agent strings with ids; the column order defines the Parquet schema.
const RESOLVED_ROWS_SQL: &str = "
    SELECT s.ts, s.ip, s.method, u.url_id, s.protocol, s.status, s.bytes, s.referer,
           a.ua_id, s.duration_us, s.host, s.router, s.upstream, s.request_seq, s.line_hash,
           strftime(s.ts, '%Y-%m') AS month
    FROM staging_hits s
    JOIN urls u ON u.raw_url = s.target
    LEFT JOIN user_agents a ON a.raw = s.user_agent
";

/// Rows keep log order, which is already (nearly) chronological, so Parquet row-group
/// timestamp statistics stay tight without paying for a sort.
fn write_parquet(
    connection: &Connection,
    paths: &SitePaths,
    import_id: i64,
    flush_index: u32,
) -> Result<(), StoreError> {
    let target = paths
        .staging(import_id)
        .join(format!("flush-{flush_index}"));
    let pattern = string_literal(&format!("import_{import_id}_f{flush_index}_{{i}}"));
    connection.execute_batch(&format!(
        "COPY ({RESOLVED_ROWS_SQL})
         TO {target} (FORMAT parquet, COMPRESSION zstd, PARTITION_BY (month),
                      FILENAME_PATTERN {pattern});",
        target = path_literal(&target)?,
    ))?;
    Ok(())
}

/// Aggregates on raw strings first; a raw value maps to exactly one id, so joining the
/// (much smaller) aggregate afterwards yields the same groups. Bot hits are split by the
/// verdict of their `(ip, bot)` pair.
fn insert_rollups(connection: &Connection, import_id: i64) -> Result<(), StoreError> {
    connection.execute(
        "INSERT INTO r_day_url (day, url_id, ua_id, status, hits, bytes, duration_sum_us,
                                duration_count, duration_max_us, first_ts, last_ts, import_id,
                                verification)
         SELECT g.day, u.url_id, a.ua_id, g.status, g.hits, g.bytes, g.duration_sum,
                g.duration_count, g.duration_max, g.first_ts, g.last_ts, ?, g.verification
         FROM (
             SELECT CAST(s.ts AS DATE) AS day, s.target, s.user_agent, s.status,
                    b.verdict AS verification,
                    count(*) AS hits, sum(s.bytes) AS bytes,
                    sum(s.duration_us) AS duration_sum, count(s.duration_us) AS duration_count,
                    max(s.duration_us) AS duration_max, min(s.ts) AS first_ts,
                    max(s.ts) AS last_ts
             FROM staging_hits s
             LEFT JOIN bot_ips b ON b.ip = s.ip AND b.bot = s.bot
             GROUP BY 1, 2, 3, 4, 5
         ) g
         JOIN urls u ON u.raw_url = g.target
         LEFT JOIN user_agents a ON a.raw = g.user_agent",
        params![import_id],
    )?;
    connection.execute(
        "INSERT INTO r_hour
         SELECT date_trunc('hour', ts), status, count(*), sum(bytes), ?
         FROM staging_hits
         GROUP BY 1, 2",
        params![import_id],
    )?;
    Ok(())
}
