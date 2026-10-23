//! The database schema, upgraded in order by [`migrate`].

use duckdb::{Connection, params};

use crate::PlatformStoreError;

/// Schema versions; append only.
const MIGRATIONS: &[&str] = &[
    // 1: projects, cached provider answers and spending.
    "
    CREATE SEQUENCE project_ids START 1;
    CREATE TABLE projects (
        id BIGINT PRIMARY KEY DEFAULT nextval('project_ids'),
        name VARCHAR NOT NULL,
        domain VARCHAR NOT NULL,
        country VARCHAR NOT NULL,
        language VARCHAR NOT NULL,
        competitors VARCHAR NOT NULL DEFAULT '[]',
        brands VARCHAR NOT NULL DEFAULT '[]',
        settings VARCHAR NOT NULL DEFAULT '{}',
        created_at VARCHAR NOT NULL,
        updated_at VARCHAR NOT NULL
    );
    CREATE TABLE cache (
        key VARCHAR PRIMARY KEY,
        endpoint VARCHAR NOT NULL,
        class VARCHAR NOT NULL,
        project_id BIGINT,
        body VARCHAR NOT NULL,
        fetched_at VARCHAR NOT NULL,
        expires_at BIGINT NOT NULL,
        cost_usd DOUBLE NOT NULL,
        via VARCHAR NOT NULL
    );
    CREATE TABLE spend (
        spent_at VARCHAR NOT NULL,
        project_id BIGINT,
        module VARCHAR NOT NULL,
        endpoint VARCHAR NOT NULL,
        provider VARCHAR NOT NULL,
        via VARCHAR NOT NULL,
        requests INTEGER NOT NULL,
        cost_usd DOUBLE NOT NULL
    );
    ",
    // 2: keyword lists with the numbers of each keyword when it was added.
    "
    CREATE SEQUENCE keyword_list_ids START 1;
    CREATE TABLE keyword_lists (
        id BIGINT PRIMARY KEY DEFAULT nextval('keyword_list_ids'),
        project_id BIGINT NOT NULL,
        name VARCHAR NOT NULL,
        created_at VARCHAR NOT NULL
    );
    CREATE TABLE list_keywords (
        list_id BIGINT NOT NULL,
        keyword VARCHAR NOT NULL,
        data VARCHAR NOT NULL,
        added_at VARCHAR NOT NULL,
        PRIMARY KEY (list_id, keyword)
    );
    ",
    // 3: rank tracking: settings, places, keywords, runs and checks.
    "
    CREATE TABLE rank_settings (
        project_id BIGINT PRIMARY KEY,
        schedule VARCHAR NOT NULL,
        depth INTEGER NOT NULL,
        ai_overview BOOLEAN NOT NULL,
        last_run_at VARCHAR
    );
    CREATE SEQUENCE rank_tracker_ids START 1;
    CREATE TABLE rank_trackers (
        id BIGINT PRIMARY KEY DEFAULT nextval('rank_tracker_ids'),
        project_id BIGINT NOT NULL,
        engine VARCHAR NOT NULL,
        country VARCHAR NOT NULL,
        language VARCHAR NOT NULL,
        device VARCHAR NOT NULL,
        location VARCHAR,
        created_at VARCHAR NOT NULL
    );
    CREATE SEQUENCE tracked_keyword_ids START 1;
    CREATE TABLE tracked_keywords (
        id BIGINT PRIMARY KEY DEFAULT nextval('tracked_keyword_ids'),
        project_id BIGINT NOT NULL,
        keyword VARCHAR NOT NULL,
        tags VARCHAR NOT NULL DEFAULT '[]',
        volume UBIGINT,
        added_at VARCHAR NOT NULL
    );
    CREATE SEQUENCE rank_run_ids START 1;
    CREATE TABLE rank_runs (
        id BIGINT PRIMARY KEY DEFAULT nextval('rank_run_ids'),
        project_id BIGINT NOT NULL,
        scheduled BOOLEAN NOT NULL,
        status VARCHAR NOT NULL,
        total INTEGER NOT NULL,
        checked INTEGER NOT NULL DEFAULT 0,
        failed INTEGER NOT NULL DEFAULT 0,
        cost_usd DOUBLE NOT NULL DEFAULT 0,
        error VARCHAR,
        started_at VARCHAR NOT NULL,
        finished_at VARCHAR
    );
    CREATE TABLE rank_checks (
        run_id BIGINT NOT NULL,
        project_id BIGINT NOT NULL,
        keyword_id BIGINT NOT NULL,
        tracker_id BIGINT NOT NULL,
        checked_at VARCHAR NOT NULL,
        position INTEGER,
        url VARCHAR,
        pages VARCHAR NOT NULL DEFAULT '[]',
        features VARCHAR NOT NULL DEFAULT '[]',
        ai_overview BOOLEAN NOT NULL DEFAULT false,
        ai_cited BOOLEAN NOT NULL DEFAULT false,
        snippet BOOLEAN NOT NULL DEFAULT false,
        results VARCHAR NOT NULL DEFAULT '[]',
        error VARCHAR
    );
    ",
    // 4: site audits: settings and every audit with its scores.
    "
    CREATE TABLE audit_settings (
        project_id BIGINT PRIMARY KEY,
        schedule VARCHAR NOT NULL,
        max_pages INTEGER NOT NULL,
        last_run_at VARCHAR
    );
    CREATE SEQUENCE audit_ids START 1;
    CREATE TABLE audits (
        id BIGINT PRIMARY KEY DEFAULT nextval('audit_ids'),
        project_id BIGINT NOT NULL,
        crawl_id VARCHAR,
        scheduled BOOLEAN NOT NULL,
        status VARCHAR NOT NULL,
        start_url VARCHAR NOT NULL,
        pages INTEGER NOT NULL DEFAULT 0,
        health DOUBLE,
        geo DOUBLE,
        issues INTEGER NOT NULL DEFAULT 0,
        metered_pages INTEGER NOT NULL DEFAULT 0,
        charged_micros BIGINT NOT NULL DEFAULT 0,
        error VARCHAR,
        started_at VARCHAR NOT NULL,
        finished_at VARCHAR
    );
    ",
    // 5: AI visibility: settings, questions, runs and answers.
    "
    CREATE TABLE ai_settings (
        project_id BIGINT PRIMARY KEY,
        schedule VARCHAR NOT NULL,
        engines VARCHAR NOT NULL,
        models VARCHAR NOT NULL DEFAULT '{}',
        rivals VARCHAR NOT NULL DEFAULT '[]',
        last_run_at VARCHAR
    );
    CREATE SEQUENCE ai_prompt_ids START 1;
    CREATE TABLE ai_prompts (
        id BIGINT PRIMARY KEY DEFAULT nextval('ai_prompt_ids'),
        project_id BIGINT NOT NULL,
        prompt VARCHAR NOT NULL,
        tags VARCHAR NOT NULL DEFAULT '[]',
        added_at VARCHAR NOT NULL
    );
    CREATE SEQUENCE ai_run_ids START 1;
    CREATE TABLE ai_runs (
        id BIGINT PRIMARY KEY DEFAULT nextval('ai_run_ids'),
        project_id BIGINT NOT NULL,
        scheduled BOOLEAN NOT NULL,
        status VARCHAR NOT NULL,
        total INTEGER NOT NULL,
        checked INTEGER NOT NULL DEFAULT 0,
        failed INTEGER NOT NULL DEFAULT 0,
        cost_usd DOUBLE NOT NULL DEFAULT 0,
        error VARCHAR,
        started_at VARCHAR NOT NULL,
        finished_at VARCHAR
    );
    CREATE TABLE ai_answers (
        run_id BIGINT NOT NULL,
        project_id BIGINT NOT NULL,
        prompt_id BIGINT NOT NULL,
        engine VARCHAR NOT NULL,
        via VARCHAR NOT NULL,
        model VARCHAR,
        checked_at VARCHAR NOT NULL,
        text VARCHAR NOT NULL,
        mentioned BOOLEAN NOT NULL,
        position INTEGER,
        cited BOOLEAN NOT NULL,
        sentiment DOUBLE,
        brands VARCHAR NOT NULL DEFAULT '[]',
        citations VARCHAR NOT NULL DEFAULT '[]',
        error VARCHAR
    );
    ",
    // 6: Search Console and Google Analytics: the properties a project reads, its syncs and
    // the numbers of the latest one.
    "
    CREATE TABLE gsc_settings (
        project_id BIGINT PRIMARY KEY,
        property VARCHAR,
        ga4_property VARCHAR,
        ga4_name VARCHAR,
        days INTEGER NOT NULL,
        schedule VARCHAR NOT NULL,
        last_sync_at VARCHAR
    );
    CREATE SEQUENCE gsc_sync_ids START 1;
    CREATE TABLE gsc_syncs (
        id BIGINT PRIMARY KEY DEFAULT nextval('gsc_sync_ids'),
        project_id BIGINT NOT NULL,
        scheduled BOOLEAN NOT NULL,
        status VARCHAR NOT NULL,
        period_start VARCHAR,
        period_end VARCHAR,
        rows BIGINT NOT NULL DEFAULT 0,
        currency VARCHAR,
        limited BOOLEAN NOT NULL DEFAULT false,
        error VARCHAR,
        started_at VARCHAR NOT NULL,
        finished_at VARCHAR
    );
    CREATE TABLE gsc_rows (
        project_id BIGINT NOT NULL,
        kind VARCHAR NOT NULL,
        period VARCHAR NOT NULL,
        key VARCHAR NOT NULL,
        clicks DOUBLE NOT NULL,
        impressions DOUBLE NOT NULL,
        position DOUBLE NOT NULL
    );
    CREATE TABLE gsc_query_pages (
        project_id BIGINT NOT NULL,
        query VARCHAR NOT NULL,
        page VARCHAR NOT NULL,
        clicks DOUBLE NOT NULL,
        impressions DOUBLE NOT NULL,
        position DOUBLE NOT NULL
    );
    CREATE TABLE ga4_rows (
        project_id BIGINT NOT NULL,
        kind VARCHAR NOT NULL,
        period VARCHAR NOT NULL,
        key VARCHAR NOT NULL,
        sessions DOUBLE NOT NULL,
        engaged_sessions DOUBLE NOT NULL,
        key_events DOUBLE NOT NULL,
        revenue DOUBLE NOT NULL
    );
    ",
    // 7: Reports: the agency's branding, report definitions, share links and deliveries.
    "
    CREATE TABLE report_branding (
        id INTEGER PRIMARY KEY,
        company VARCHAR,
        logo VARCHAR,
        accent VARCHAR,
        website VARCHAR,
        footer VARCHAR,
        hide_octopuson BOOLEAN NOT NULL DEFAULT false
    );
    CREATE SEQUENCE report_ids START 1;
    CREATE TABLE reports (
        id BIGINT PRIMARY KEY DEFAULT nextval('report_ids'),
        project_id BIGINT NOT NULL,
        name VARCHAR NOT NULL,
        language VARCHAR NOT NULL,
        days INTEGER NOT NULL,
        widgets VARCHAR NOT NULL,
        schedule VARCHAR NOT NULL,
        recipients VARCHAR NOT NULL DEFAULT '[]',
        share_token VARCHAR,
        share_views BIGINT NOT NULL DEFAULT 0,
        last_sent_at VARCHAR,
        created_at VARCHAR NOT NULL,
        updated_at VARCHAR NOT NULL
    );
    CREATE SEQUENCE report_delivery_ids START 1;
    CREATE TABLE report_deliveries (
        id BIGINT PRIMARY KEY DEFAULT nextval('report_delivery_ids'),
        report_id BIGINT NOT NULL,
        project_id BIGINT NOT NULL,
        scheduled BOOLEAN NOT NULL,
        status VARCHAR NOT NULL,
        recipients INTEGER NOT NULL,
        error VARCHAR,
        sent_at VARCHAR NOT NULL
    );
    ",
    // 8: syncing with the account's web workspace (`sync`): every record that travels gets a
    // `uid` that is the same on every copy (random, or drawn from what makes it unique), the
    // state each record was last seen in, the sync's own values, and changes waiting for a
    // record they refer to.
    "
    ALTER TABLE projects ADD COLUMN uid VARCHAR;
    ALTER TABLE keyword_lists ADD COLUMN uid VARCHAR;
    ALTER TABLE list_keywords ADD COLUMN uid VARCHAR;
    ALTER TABLE rank_trackers ADD COLUMN uid VARCHAR;
    ALTER TABLE tracked_keywords ADD COLUMN uid VARCHAR;
    ALTER TABLE rank_runs ADD COLUMN uid VARCHAR;
    ALTER TABLE audits ADD COLUMN uid VARCHAR;
    ALTER TABLE ai_prompts ADD COLUMN uid VARCHAR;
    ALTER TABLE ai_runs ADD COLUMN uid VARCHAR;
    ALTER TABLE gsc_syncs ADD COLUMN uid VARCHAR;
    ALTER TABLE reports ADD COLUMN uid VARCHAR;
    ALTER TABLE report_deliveries ADD COLUMN uid VARCHAR;
    CREATE TABLE sync_rows (
        kind VARCHAR NOT NULL,
        key VARCHAR NOT NULL,
        parent VARCHAR,
        hash VARCHAR,
        seq BIGINT NOT NULL,
        changed_at BIGINT NOT NULL,
        origin VARCHAR NOT NULL,
        remote_seq BIGINT NOT NULL DEFAULT 0,
        deleted BOOLEAN NOT NULL DEFAULT false,
        PRIMARY KEY (kind, key)
    );
    CREATE TABLE sync_state (
        name VARCHAR PRIMARY KEY,
        value VARCHAR NOT NULL
    );
    CREATE TABLE sync_pending (
        kind VARCHAR NOT NULL,
        key VARCHAR NOT NULL,
        origin VARCHAR NOT NULL,
        change VARCHAR NOT NULL,
        waiting VARCHAR NOT NULL,
        PRIMARY KEY (kind, key)
    );
    ",
    // 9: snapshots: a project's whole state kept at a moment, to compare two moments, and how
    // often they are taken.
    "
    CREATE TABLE snapshot_settings (
        project_id BIGINT PRIMARY KEY,
        schedule VARCHAR NOT NULL,
        competitors VARCHAR NOT NULL DEFAULT '[]',
        last_run_at VARCHAR
    );
    CREATE SEQUENCE snapshot_ids START 1;
    CREATE TABLE snapshots (
        id BIGINT PRIMARY KEY DEFAULT nextval('snapshot_ids'),
        project_id BIGINT NOT NULL,
        label VARCHAR,
        kind VARCHAR NOT NULL,
        status VARCHAR NOT NULL,
        taken_at VARCHAR NOT NULL,
        finished_at VARCHAR,
        cost_usd DOUBLE NOT NULL DEFAULT 0,
        error VARCHAR,
        data VARCHAR,
        uid VARCHAR
    );
    ",
    // 10: a report's design (its look); none reads as the default.
    "
    ALTER TABLE reports ADD COLUMN design VARCHAR;
    ",
    // 11: a report's own colours over its design's (JSON); none keeps the design's.
    "
    ALTER TABLE reports ADD COLUMN colors VARCHAR;
    ",
    // 12: kept answers that stay on this copy (filtered or later pages of a list).
    "
    ALTER TABLE cache ADD COLUMN travels BOOLEAN DEFAULT true;
    ",
    // 13: metric history: a target's headline numbers each day they were fetched.
    "
    CREATE SEQUENCE metric_point_ids START 1;
    CREATE TABLE metric_points (
        id BIGINT PRIMARY KEY DEFAULT nextval('metric_point_ids'),
        project_id BIGINT NOT NULL,
        target VARCHAR NOT NULL,
        kind VARCHAR NOT NULL,
        market VARCHAR NOT NULL DEFAULT '',
        day VARCHAR NOT NULL,
        metrics VARCHAR NOT NULL,
        taken_at VARCHAR NOT NULL,
        uid VARCHAR
    );
    ",
    // 14: ad copies a domain was seen showing, from paid-keywords answers.
    "
    CREATE SEQUENCE ad_copy_ids START 1;
    CREATE TABLE ad_copies (
        id BIGINT PRIMARY KEY DEFAULT nextval('ad_copy_ids'),
        project_id BIGINT NOT NULL,
        domain VARCHAR NOT NULL,
        title VARCHAR NOT NULL,
        description VARCHAR,
        breadcrumb VARCHAR,
        url VARCHAR,
        keywords VARCHAR NOT NULL DEFAULT '[]',
        first_seen VARCHAR NOT NULL,
        last_seen VARCHAR NOT NULL,
        uid VARCHAR
    );
    ",
    // 15: link work: decisions on linking sites (keep, watch, disavow) and link prospects with
    // their outreach state; audits may check links to other sites.
    "
    CREATE SEQUENCE link_decision_ids START 1;
    CREATE TABLE link_decisions (
        id BIGINT PRIMARY KEY DEFAULT nextval('link_decision_ids'),
        project_id BIGINT NOT NULL,
        domain VARCHAR NOT NULL,
        decision VARCHAR NOT NULL,
        note VARCHAR,
        decided_at VARCHAR NOT NULL,
        uid VARCHAR
    );
    CREATE SEQUENCE link_prospect_ids START 1;
    CREATE TABLE link_prospects (
        id BIGINT PRIMARY KEY DEFAULT nextval('link_prospect_ids'),
        project_id BIGINT NOT NULL,
        domain VARCHAR NOT NULL,
        url VARCHAR,
        source VARCHAR NOT NULL,
        status VARCHAR NOT NULL,
        note VARCHAR,
        added_at VARCHAR NOT NULL,
        updated_at VARCHAR NOT NULL,
        uid VARCHAR
    );
    ALTER TABLE audit_settings ADD COLUMN check_outbound BOOLEAN DEFAULT false;
    ",
    // 16: rank checks waiting in the standard queue: paid, their answers read back by id. Only
    // the copy that posted them can read them, so they stay here.
    "
    CREATE SEQUENCE rank_task_ids START 1;
    CREATE TABLE rank_tasks (
        id BIGINT PRIMARY KEY DEFAULT nextval('rank_task_ids'),
        run_id BIGINT NOT NULL,
        project_id BIGINT NOT NULL,
        keyword_id BIGINT NOT NULL,
        tracker_id BIGINT NOT NULL,
        engine VARCHAR NOT NULL,
        task_id VARCHAR NOT NULL,
        via VARCHAR NOT NULL,
        posted_at VARCHAR NOT NULL,
        tries INTEGER NOT NULL DEFAULT 0,
        next_try_at BIGINT NOT NULL
    );
    ",
    // 17: local SEO: the business a project tracks on Google Maps and how its grid is checked,
    // every grid run with what each point showed, its reviews, its rating day by day, and the
    // tasks waiting in the provider's queue (kept on the copy that posted them).
    "
    CREATE TABLE local_settings (
        project_id BIGINT PRIMARY KEY,
        business VARCHAR,
        keywords VARCHAR NOT NULL DEFAULT '[]',
        grid INTEGER NOT NULL DEFAULT 5,
        radius_km DOUBLE NOT NULL DEFAULT 2,
        schedule VARCHAR NOT NULL DEFAULT 'manual',
        last_run_at VARCHAR
    );
    CREATE SEQUENCE local_run_ids START 1;
    CREATE TABLE local_runs (
        id BIGINT PRIMARY KEY DEFAULT nextval('local_run_ids'),
        project_id BIGINT NOT NULL,
        scheduled BOOLEAN NOT NULL,
        status VARCHAR NOT NULL,
        total INTEGER NOT NULL DEFAULT 0,
        checked INTEGER NOT NULL DEFAULT 0,
        failed INTEGER NOT NULL DEFAULT 0,
        cost_usd DOUBLE NOT NULL DEFAULT 0,
        error VARCHAR,
        started_at VARCHAR NOT NULL,
        finished_at VARCHAR,
        grid INTEGER NOT NULL,
        radius_km DOUBLE NOT NULL,
        lat DOUBLE NOT NULL,
        lng DOUBLE NOT NULL,
        uid VARCHAR
    );
    CREATE TABLE local_points (
        run_id BIGINT NOT NULL,
        project_id BIGINT NOT NULL,
        keyword VARCHAR NOT NULL,
        grid_row INTEGER NOT NULL,
        grid_col INTEGER NOT NULL,
        lat DOUBLE NOT NULL,
        lng DOUBLE NOT NULL,
        position INTEGER,
        top VARCHAR NOT NULL DEFAULT '[]',
        error VARCHAR,
        checked_at VARCHAR NOT NULL
    );
    CREATE SEQUENCE local_task_ids START 1;
    CREATE TABLE local_tasks (
        id BIGINT PRIMARY KEY DEFAULT nextval('local_task_ids'),
        project_id BIGINT NOT NULL,
        kind VARCHAR NOT NULL,
        run_id BIGINT,
        keyword VARCHAR,
        grid_row INTEGER,
        grid_col INTEGER,
        lat DOUBLE,
        lng DOUBLE,
        task_id VARCHAR NOT NULL,
        via VARCHAR NOT NULL,
        posted_at VARCHAR NOT NULL,
        tries INTEGER NOT NULL DEFAULT 0,
        next_try_at BIGINT NOT NULL
    );
    CREATE SEQUENCE local_review_ids START 1;
    CREATE TABLE local_reviews (
        id BIGINT PRIMARY KEY DEFAULT nextval('local_review_ids'),
        project_id BIGINT NOT NULL,
        review_id VARCHAR NOT NULL,
        rating INTEGER,
        text VARCHAR,
        author VARCHAR,
        author_url VARCHAR,
        url VARCHAR,
        published_at VARCHAR,
        owner_answer VARCHAR,
        answered_at VARCHAR,
        photos INTEGER NOT NULL DEFAULT 0,
        local_guide BOOLEAN NOT NULL DEFAULT false,
        first_seen VARCHAR NOT NULL,
        uid VARCHAR
    );
    CREATE SEQUENCE local_profile_ids START 1;
    CREATE TABLE local_profiles (
        id BIGINT PRIMARY KEY DEFAULT nextval('local_profile_ids'),
        project_id BIGINT NOT NULL,
        day VARCHAR NOT NULL,
        rating DOUBLE,
        votes BIGINT,
        profile VARCHAR,
        taken_at VARCHAR NOT NULL,
        uid VARCHAR
    );
    ",
    // 18: content briefs: what to write for a keyword, with the result page and ranking pages
    // they were made from, and the pages and drafts scored against them.
    "
    CREATE SEQUENCE content_brief_ids START 1;
    CREATE TABLE content_briefs (
        id BIGINT PRIMARY KEY DEFAULT nextval('content_brief_ids'),
        project_id BIGINT NOT NULL,
        keyword VARCHAR NOT NULL,
        country VARCHAR NOT NULL,
        language VARCHAR NOT NULL,
        mobile BOOLEAN NOT NULL DEFAULT false,
        status VARCHAR NOT NULL,
        brief VARCHAR,
        source VARCHAR,
        ai_text VARCHAR,
        ai_model VARCHAR,
        cost_usd DOUBLE NOT NULL DEFAULT 0,
        error VARCHAR,
        created_at VARCHAR NOT NULL,
        updated_at VARCHAR NOT NULL,
        uid VARCHAR
    );
    CREATE SEQUENCE content_score_ids START 1;
    CREATE TABLE content_scores (
        id BIGINT PRIMARY KEY DEFAULT nextval('content_score_ids'),
        brief_id BIGINT NOT NULL,
        project_id BIGINT NOT NULL,
        kind VARCHAR NOT NULL,
        url VARCHAR,
        title VARCHAR,
        description VARCHAR,
        body VARCHAR,
        score DOUBLE NOT NULL,
        result VARCHAR NOT NULL,
        suggestions VARCHAR,
        scored_at VARCHAR NOT NULL,
        uid VARCHAR
    );
    ",
    // 19: Core Web Vitals checked day by day (a site's real-user data with its weekly history,
    // pages' PageSpeed runs), and what a Search Console period is compared with.
    "
    CREATE SEQUENCE vital_check_ids START 1;
    CREATE TABLE vital_checks (
        id BIGINT PRIMARY KEY DEFAULT nextval('vital_check_ids'),
        project_id BIGINT NOT NULL,
        target VARCHAR NOT NULL,
        scope VARCHAR NOT NULL,
        device VARCHAR NOT NULL,
        day VARCHAR NOT NULL,
        field VARCHAR,
        lab VARCHAR,
        history VARCHAR,
        checked_at VARCHAR NOT NULL,
        uid VARCHAR
    );
    ALTER TABLE gsc_settings ADD COLUMN compare VARCHAR DEFAULT 'previous';
    ",
    // 20: what the user decided about an opportunity (done, ignored, snoozed until a day); an
    // opportunity without a row is open.
    "
    CREATE TABLE opportunity_states (
        project_id BIGINT NOT NULL,
        key VARCHAR NOT NULL,
        state VARCHAR NOT NULL,
        until VARCHAR,
        changed_at VARCHAR NOT NULL,
        uid VARCHAR
    );
    ",
    // 21: alerts: a project's alerting (rules, e-mails) and the alerts found, each kept once by
    // its key; when this copy last looked, mailed or notified is kept here only.
    "
    CREATE TABLE alert_settings (
        project_id BIGINT PRIMARY KEY,
        enabled BOOLEAN NOT NULL DEFAULT true,
        rules VARCHAR NOT NULL DEFAULT '{}',
        email VARCHAR NOT NULL DEFAULT 'off',
        recipients VARCHAR NOT NULL DEFAULT '[]',
        lang VARCHAR NOT NULL DEFAULT '',
        last_digest_at VARCHAR,
        links_last_run VARCHAR,
        evaluated_at VARCHAR
    );
    CREATE SEQUENCE alert_ids START 1;
    CREATE TABLE alerts (
        id BIGINT PRIMARY KEY DEFAULT nextval('alert_ids'),
        project_id BIGINT NOT NULL,
        key VARCHAR NOT NULL,
        kind VARCHAR NOT NULL,
        severity VARCHAR NOT NULL,
        page VARCHAR NOT NULL,
        params VARCHAR NOT NULL DEFAULT '{}',
        detail VARCHAR NOT NULL DEFAULT '{}',
        created_at VARCHAR NOT NULL,
        read_at VARCHAR,
        mailed_at VARCHAR,
        notified_at VARCHAR,
        uid VARCHAR
    );
    ",
];

/// Applies the migrations the database does not have yet.
pub(crate) fn migrate(connection: &mut Connection) -> Result<(), PlatformStoreError> {
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
#[cfg(test)]
pub(crate) fn latest() -> i64 {
    i64::try_from(MIGRATIONS.len()).unwrap_or(i64::MAX)
}
