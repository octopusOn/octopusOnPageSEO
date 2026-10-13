use crate::db::Migration;

/// Site database migrations.
///
/// Version 2 adds bot classification (`user_agents.bot`, `bot_category`), verdicts per
/// `(ip, bot)` pair (`bot_ips`) and the `verification` key of `r_day_url` (ADR 0005).
/// Version 3 adds `ip_geo`, country and autonomous system per client IP.
///
/// Raw hits live in Parquet files; this database holds dimensions (`urls`, `user_agents`),
/// insert-only partial rollups keyed by `import_id` (queries `SUM` them) and import history.
/// Uniqueness of `urls.raw_url` / `user_agents.raw` is maintained by the importer under the
/// site lock instead of ART indexes, which would cost memory on millions of URLs.
pub(super) const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        sql: "
        CREATE SEQUENCE import_id_seq START 1;
        CREATE TABLE imports (
            id BIGINT PRIMARY KEY,
            format_spec VARCHAR NOT NULL,
            started_at TIMESTAMP NOT NULL,
            finished_at TIMESTAMP NOT NULL,
            total_lines UBIGINT NOT NULL,
            parsed_lines UBIGINT NOT NULL,
            inserted_rows UBIGINT NOT NULL,
            duplicate_rows UBIGINT NOT NULL,
            first_ts TIMESTAMP,
            last_ts TIMESTAMP
        );
        CREATE TABLE imported_files (
            fingerprint VARCHAR PRIMARY KEY,
            import_id BIGINT NOT NULL,
            path VARCHAR NOT NULL,
            size_bytes UBIGINT NOT NULL
        );
        CREATE TABLE import_skips (
            import_id BIGINT NOT NULL,
            reason VARCHAR NOT NULL,
            lines UBIGINT NOT NULL
        );
        CREATE TABLE import_samples (
            import_id BIGINT NOT NULL,
            stream VARCHAR NOT NULL,
            line_number UBIGINT NOT NULL,
            reason VARCHAR NOT NULL,
            excerpt VARCHAR NOT NULL
        );

        CREATE SEQUENCE url_id_seq START 1;
        CREATE TABLE urls (
            url_id BIGINT NOT NULL,
            raw_url VARCHAR NOT NULL
        );
        CREATE SEQUENCE ua_id_seq START 1;
        CREATE TABLE user_agents (
            ua_id BIGINT NOT NULL,
            raw VARCHAR NOT NULL
        );

        CREATE TABLE r_day_url (
            day DATE NOT NULL,
            url_id BIGINT NOT NULL,
            ua_id BIGINT,
            status USMALLINT NOT NULL,
            hits UBIGINT NOT NULL,
            bytes UBIGINT,
            duration_sum_us BIGINT,
            duration_count UBIGINT NOT NULL,
            duration_max_us BIGINT,
            first_ts TIMESTAMP NOT NULL,
            last_ts TIMESTAMP NOT NULL,
            import_id BIGINT NOT NULL
        );
        CREATE TABLE r_hour (
            hour TIMESTAMP NOT NULL,
            status USMALLINT NOT NULL,
            hits UBIGINT NOT NULL,
            bytes UBIGINT,
            import_id BIGINT NOT NULL
        );
    ",
    },
    Migration {
        version: 2,
        sql: "
        ALTER TABLE user_agents ADD COLUMN bot VARCHAR;
        ALTER TABLE user_agents ADD COLUMN bot_category VARCHAR;
        ALTER TABLE r_day_url ADD COLUMN verification VARCHAR;
        CREATE TABLE bot_ips (
            ip VARCHAR NOT NULL,
            bot VARCHAR NOT NULL,
            verdict VARCHAR NOT NULL,
            method VARCHAR NOT NULL,
            checked_at TIMESTAMP NOT NULL
        );
        CREATE TABLE site_meta (
            key VARCHAR PRIMARY KEY,
            value VARCHAR NOT NULL
        );
    ",
    },
    Migration {
        version: 3,
        sql: "
        CREATE TABLE ip_geo (
            ip VARCHAR NOT NULL,
            country VARCHAR,
            asn INTEGER,
            as_org VARCHAR
        );
    ",
    },
];
