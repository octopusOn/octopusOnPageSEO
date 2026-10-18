//! The database schema, upgraded in order by [`migrate`].

use duckdb::{Connection, params};

use crate::KwStoreError;

/// Schema versions; append only.
const MIGRATIONS: &[&str] = &[
    // 1: projects, keywords with their sources, tags and numbers, result pages, clusterings,
    // manual placements, the site's pages, runs and spending.
    "
    CREATE SEQUENCE project_ids START 1;
    CREATE TABLE projects (
        id BIGINT PRIMARY KEY DEFAULT nextval('project_ids'),
        name VARCHAR NOT NULL,
        country VARCHAR NOT NULL,
        language VARCHAR NOT NULL,
        engines VARCHAR NOT NULL,
        site VARCHAR,
        competitors VARCHAR NOT NULL DEFAULT '[]',
        settings VARCHAR NOT NULL DEFAULT '{}',
        created_at VARCHAR NOT NULL
    );
    CREATE SEQUENCE keyword_ids START 1;
    CREATE TABLE keywords (
        id BIGINT PRIMARY KEY DEFAULT nextval('keyword_ids'),
        project_id BIGINT NOT NULL,
        keyword VARCHAR NOT NULL,
        key VARCHAR NOT NULL,
        stems VARCHAR NOT NULL,
        status VARCHAR NOT NULL DEFAULT 'active',
        added_at VARCHAR NOT NULL,
        UNIQUE (project_id, key)
    );
    CREATE TABLE keyword_tags (
        keyword_id BIGINT NOT NULL,
        tag VARCHAR NOT NULL,
        PRIMARY KEY (keyword_id, tag)
    );
    CREATE TABLE keyword_sources (
        keyword_id BIGINT NOT NULL,
        source VARCHAR NOT NULL,
        seed VARCHAR,
        found_at VARCHAR NOT NULL,
        PRIMARY KEY (keyword_id, source)
    );
    CREATE TABLE metrics (
        keyword_id BIGINT NOT NULL,
        source VARCHAR NOT NULL,
        engine VARCHAR NOT NULL,
        country VARCHAR NOT NULL,
        volume UBIGINT,
        volume_low UBIGINT,
        volume_high UBIGINT,
        cpc_usd DOUBLE,
        competition DOUBLE,
        difficulty DOUBLE,
        monthly VARCHAR,
        fetched_at VARCHAR NOT NULL,
        PRIMARY KEY (keyword_id, source, engine, country)
    );
    CREATE TABLE intents (
        keyword_id BIGINT PRIMARY KEY,
        primary_intent VARCHAR NOT NULL,
        secondary_intent VARCHAR,
        source VARCHAR NOT NULL
    );
    CREATE TABLE serps (
        keyword_id BIGINT NOT NULL,
        engine VARCHAR NOT NULL,
        device VARCHAR NOT NULL,
        provider VARCHAR NOT NULL,
        fetched_at VARCHAR NOT NULL,
        urls VARCHAR NOT NULL,
        site_only BOOLEAN NOT NULL DEFAULT false,
        features VARCHAR NOT NULL DEFAULT '[]',
        questions VARCHAR NOT NULL DEFAULT '[]',
        related VARCHAR NOT NULL DEFAULT '[]',
        raw_path VARCHAR,
        PRIMARY KEY (keyword_id, engine, device)
    );
    CREATE SEQUENCE clustering_ids START 1;
    CREATE TABLE clusterings (
        id BIGINT PRIMARY KEY DEFAULT nextval('clustering_ids'),
        project_id BIGINT NOT NULL,
        method VARCHAR NOT NULL,
        params VARCHAR NOT NULL,
        created_at VARCHAR NOT NULL,
        keywords UINTEGER NOT NULL,
        clusters UINTEGER NOT NULL
    );
    CREATE TABLE clusters (
        clustering_id BIGINT NOT NULL,
        number UINTEGER NOT NULL,
        pillar_id BIGINT NOT NULL,
        label VARCHAR NOT NULL,
        size UINTEGER NOT NULL,
        volume UBIGINT NOT NULL,
        intent VARCHAR,
        mixed_intent BOOLEAN NOT NULL,
        difficulty DOUBLE,
        cpc_usd DOUBLE,
        verified BOOLEAN NOT NULL,
        target_url VARCHAR,
        competing VARCHAR NOT NULL DEFAULT '[]',
        gap BOOLEAN NOT NULL DEFAULT false,
        competitors VARCHAR NOT NULL DEFAULT '[]',
        PRIMARY KEY (clustering_id, number)
    );
    CREATE TABLE members (
        clustering_id BIGINT NOT NULL,
        number UINTEGER NOT NULL,
        keyword_id BIGINT NOT NULL,
        score DOUBLE NOT NULL,
        shared USMALLINT,
        PRIMARY KEY (clustering_id, keyword_id)
    );
    CREATE TABLE pins (
        project_id BIGINT NOT NULL,
        keyword_id BIGINT NOT NULL,
        pillar_id BIGINT NOT NULL,
        PRIMARY KEY (project_id, keyword_id)
    );
    CREATE TABLE site_pages (
        project_id BIGINT NOT NULL,
        keyword_id BIGINT NOT NULL,
        url VARCHAR NOT NULL,
        source VARCHAR NOT NULL,
        clicks DOUBLE,
        impressions DOUBLE,
        position DOUBLE,
        period VARCHAR NOT NULL,
        PRIMARY KEY (project_id, keyword_id, url, source)
    );
    CREATE SEQUENCE run_ids START 1;
    CREATE TABLE runs (
        id BIGINT PRIMARY KEY DEFAULT nextval('run_ids'),
        project_id BIGINT,
        kind VARCHAR NOT NULL,
        status VARCHAR NOT NULL,
        planned UINTEGER NOT NULL DEFAULT 0,
        done UINTEGER NOT NULL DEFAULT 0,
        estimated_cost_usd DOUBLE NOT NULL DEFAULT 0,
        cost_usd DOUBLE NOT NULL DEFAULT 0,
        started_at VARCHAR NOT NULL,
        finished_at VARCHAR,
        error VARCHAR
    );
    CREATE TABLE provider_attempts (
        run_id BIGINT,
        provider VARCHAR NOT NULL,
        attempted_at VARCHAR NOT NULL,
        outcome VARCHAR NOT NULL,
        requests UINTEGER NOT NULL,
        items UINTEGER NOT NULL,
        cost_usd DOUBLE NOT NULL,
        error VARCHAR
    );
    -- Staging tables for large writes: filled with an appender and emptied in the same
    -- transaction.
    CREATE TABLE incoming_keywords (
        ordinal UBIGINT NOT NULL,
        keyword VARCHAR NOT NULL,
        key VARCHAR NOT NULL,
        stems VARCHAR NOT NULL,
        source VARCHAR NOT NULL,
        seed VARCHAR
    );
    CREATE TABLE incoming_ids (id BIGINT NOT NULL);
    ",
    // 2: the engine of result-page attempts, for the browser's daily limit per engine.
    "ALTER TABLE provider_attempts ADD COLUMN engine VARCHAR;",
    // 3: how an attempt was paid: `own` key or OctopusOn SEO `credit`.
    "ALTER TABLE provider_attempts ADD COLUMN via VARCHAR;",
];

/// Creates or upgrades the schema.
pub(crate) fn migrate(connection: &mut Connection) -> Result<(), KwStoreError> {
    connection.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_migrations (version BIGINT PRIMARY KEY);",
    )?;
    let current: i64 = connection.query_row(
        "SELECT coalesce(max(version), 0) FROM schema_migrations",
        [],
        |row| row.get(0),
    )?;
    for (index, sql) in MIGRATIONS.iter().enumerate() {
        let version = i64::try_from(index).unwrap_or(i64::MAX) + 1;
        if version <= current {
            continue;
        }
        let transaction = connection.transaction()?;
        transaction.execute_batch(sql)?;
        transaction.execute("INSERT INTO schema_migrations VALUES (?)", params![version])?;
        transaction.commit()?;
    }
    Ok(())
}

/// Latest schema version.
pub(crate) fn latest() -> i64 {
    i64::try_from(MIGRATIONS.len()).unwrap_or(i64::MAX)
}
