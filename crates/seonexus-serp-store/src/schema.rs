//! The database schema, upgraded in order by [`migrate`].

use duckdb::{Connection, params};

use crate::SerpStoreError;

/// Schema versions; append only.
const MIGRATIONS: &[&str] = &[
    // 1: runs, snapshots and what they contain.
    "
    CREATE SEQUENCE run_ids START 1;
    CREATE TABLE runs (
        id BIGINT PRIMARY KEY DEFAULT nextval('run_ids'),
        kind VARCHAR NOT NULL,
        project_id BIGINT,
        status VARCHAR NOT NULL,
        started_at VARCHAR NOT NULL,
        finished_at VARCHAR,
        planned INTEGER NOT NULL DEFAULT 0,
        done INTEGER NOT NULL DEFAULT 0,
        estimated_cost_usd DOUBLE NOT NULL DEFAULT 0,
        cost_usd DOUBLE NOT NULL DEFAULT 0,
        error VARCHAR
    );
    CREATE SEQUENCE snapshot_ids START 1;
    CREATE TABLE snapshots (
        id BIGINT PRIMARY KEY DEFAULT nextval('snapshot_ids'),
        run_id BIGINT,
        keyword_id BIGINT,
        keyword VARCHAR NOT NULL,
        keyword_key VARCHAR NOT NULL,
        engine VARCHAR NOT NULL,
        country VARCHAR NOT NULL,
        language VARCHAR NOT NULL,
        location VARCHAR,
        device VARCHAR NOT NULL,
        depth SMALLINT NOT NULL,
        provider VARCHAR NOT NULL,
        provider_rev VARCHAR,
        parser_rev VARCHAR,
        fetched_at VARCHAR NOT NULL,
        total_results UBIGINT,
        organic_count INTEGER NOT NULL,
        requests INTEGER NOT NULL,
        cost_usd DOUBLE NOT NULL,
        cost_reported BOOLEAN NOT NULL,
        warnings VARCHAR
    );
    CREATE INDEX snapshots_keyword ON snapshots (keyword_key, engine, country, device);
    CREATE TABLE results (
        snapshot_id BIGINT NOT NULL,
        position SMALLINT NOT NULL,
        absolute_position SMALLINT,
        page SMALLINT NOT NULL,
        url VARCHAR NOT NULL,
        host VARCHAR NOT NULL,
        domain VARCHAR NOT NULL,
        title VARCHAR NOT NULL,
        snippet VARCHAR,
        displayed_url VARCHAR,
        date VARCHAR,
        sitelinks VARCHAR,
        PRIMARY KEY (snapshot_id, position)
    );
    CREATE TABLE features (
        snapshot_id BIGINT NOT NULL,
        ordinal SMALLINT NOT NULL,
        kind VARCHAR NOT NULL,
        page SMALLINT NOT NULL,
        absolute_position SMALLINT,
        items VARCHAR,
        PRIMARY KEY (snapshot_id, ordinal)
    );
    CREATE TABLE raw_pages (
        snapshot_id BIGINT NOT NULL,
        page SMALLINT NOT NULL,
        url VARCHAR NOT NULL,
        status SMALLINT NOT NULL,
        content_type VARCHAR,
        path VARCHAR NOT NULL,
        bytes BIGINT NOT NULL,
        PRIMARY KEY (snapshot_id, page)
    );
    CREATE TABLE provider_attempts (
        run_id BIGINT,
        snapshot_id BIGINT,
        provider VARCHAR NOT NULL,
        attempted_at VARCHAR NOT NULL,
        outcome VARCHAR NOT NULL,
        message VARCHAR,
        requests INTEGER NOT NULL DEFAULT 0,
        cost_usd DOUBLE NOT NULL DEFAULT 0,
        cost_reported BOOLEAN NOT NULL DEFAULT false
    );
    ",
    // 2: facts about domains, cached with an expiry, and imported datasets.
    "
    CREATE TABLE domain_facts (
        domain VARCHAR NOT NULL,
        signal VARCHAR NOT NULL,
        value VARCHAR,
        source VARCHAR,
        fetched_at VARCHAR NOT NULL,
        expires_at VARCHAR NOT NULL,
        error VARCHAR,
        PRIMARY KEY (domain, signal)
    );
    CREATE TABLE tranco (domain VARCHAR PRIMARY KEY, rank INTEGER NOT NULL);
    CREATE TABLE datasets (
        name VARCHAR PRIMARY KEY,
        version VARCHAR,
        imported_at VARCHAR NOT NULL,
        rows BIGINT NOT NULL
    );
    ",
    // 3: keyword analyses with their scoring inputs, so they can be scored again.
    "
    CREATE SEQUENCE analysis_ids START 1;
    CREATE TABLE analyses (
        id BIGINT PRIMARY KEY DEFAULT nextval('analysis_ids'),
        snapshot_id BIGINT NOT NULL,
        keyword VARCHAR NOT NULL,
        keyword_key VARCHAR NOT NULL,
        engine VARCHAR NOT NULL,
        country VARCHAR NOT NULL,
        language VARCHAR NOT NULL,
        device VARCHAR NOT NULL,
        created_at VARCHAR NOT NULL,
        target_domain VARCHAR,
        difficulty DOUBLE NOT NULL,
        band VARCHAR NOT NULL,
        intent VARCHAR NOT NULL,
        opportunity DOUBLE,
        volume UBIGINT,
        models VARCHAR NOT NULL,
        result VARCHAR NOT NULL,
        inputs VARCHAR NOT NULL,
        domains VARCHAR,
        crawl_ids VARCHAR
    );
    CREATE TABLE page_scores (
        analysis_id BIGINT NOT NULL,
        position SMALLINT NOT NULL,
        url VARCHAR NOT NULL,
        domain VARCHAR NOT NULL,
        content DOUBLE,
        technical DOUBLE,
        site DOUBLE,
        strength DOUBLE,
        words INTEGER NOT NULL,
        PRIMARY KEY (analysis_id, url)
    );
    ",
    // 4: rank tracking projects.
    "
    CREATE SEQUENCE project_ids START 1;
    CREATE TABLE projects (
        id BIGINT PRIMARY KEY DEFAULT nextval('project_ids'),
        name VARCHAR NOT NULL,
        target VARCHAR NOT NULL,
        schedule VARCHAR NOT NULL,
        providers VARCHAR,
        created_at VARCHAR NOT NULL,
        last_run_at VARCHAR
    );
    CREATE SEQUENCE tracker_ids START 1;
    CREATE TABLE trackers (
        id BIGINT PRIMARY KEY DEFAULT nextval('tracker_ids'),
        project_id BIGINT NOT NULL,
        engine VARCHAR NOT NULL,
        country VARCHAR NOT NULL,
        language VARCHAR NOT NULL,
        device VARCHAR NOT NULL,
        depth SMALLINT NOT NULL,
        location VARCHAR
    );
    CREATE SEQUENCE keyword_ids START 1;
    CREATE TABLE keywords (
        id BIGINT PRIMARY KEY DEFAULT nextval('keyword_ids'),
        project_id BIGINT NOT NULL,
        keyword VARCHAR NOT NULL,
        keyword_key VARCHAR NOT NULL,
        tags VARCHAR,
        created_at VARCHAR NOT NULL,
        UNIQUE (project_id, keyword_key)
    );
    CREATE TABLE competitors (
        project_id BIGINT NOT NULL,
        domain VARCHAR NOT NULL,
        PRIMARY KEY (project_id, domain)
    );
    CREATE TABLE rankings (
        run_id BIGINT NOT NULL,
        project_id BIGINT NOT NULL,
        keyword_id BIGINT NOT NULL,
        tracker_id BIGINT NOT NULL,
        snapshot_id BIGINT,
        checked_at VARCHAR NOT NULL,
        position SMALLINT,
        url VARCHAR,
        positions VARCHAR,
        error VARCHAR
    );
    CREATE INDEX rankings_keyword ON rankings (project_id, keyword_id, tracker_id, checked_at);
    CREATE TABLE keyword_volumes (
        keyword_key VARCHAR NOT NULL,
        engine VARCHAR NOT NULL,
        country VARCHAR NOT NULL,
        volume UBIGINT,
        source VARCHAR NOT NULL,
        fetched_at VARCHAR NOT NULL,
        PRIMARY KEY (keyword_key, engine, country)
    );
    ",
    // 5: results whose page address the engine hid (Google's encrypted links).
    "
    ALTER TABLE results ADD COLUMN site_only BOOLEAN DEFAULT false;
    ",
    // 6: how an attempt was paid: `own` key or OctopusOn SEO `credit`.
    "
    ALTER TABLE provider_attempts ADD COLUMN via VARCHAR;
    ",
];

/// Creates or upgrades the schema.
pub(crate) fn migrate(connection: &mut Connection) -> Result<(), SerpStoreError> {
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
