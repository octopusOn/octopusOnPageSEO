//! Tables and tab views of a crawl database.

use duckdb::{Connection, params};

use crate::error::CrawlStoreError;

/// Forward-only schema changes; the index + 1 is the version.
const MIGRATIONS: &[&str] = &[
    r"
CREATE TABLE crawl_info (key VARCHAR PRIMARY KEY, value VARCHAR NOT NULL);

CREATE TABLE urls (
    id UINTEGER PRIMARY KEY,
    url VARCHAR NOT NULL,
    depth UINTEGER,
    internal BOOLEAN NOT NULL,
    crawled_at_ms BIGINT NOT NULL,
    status USMALLINT,
    error VARCHAR,
    blocked_by_robots BOOLEAN NOT NULL,
    robots_rule VARCHAR,
    content_kind VARCHAR NOT NULL,
    content_type VARCHAR,
    size_bytes UBIGINT NOT NULL,
    truncated BOOLEAN NOT NULL,
    response_ms UINTEGER NOT NULL,
    http_version VARCHAR,
    redirect_url VARCHAR,
    x_robots_tag VARCHAR,
    header_canonical VARCHAR,
    last_modified VARCHAR,
    content_encoding VARCHAR,
    cache_control VARCHAR,
    hsts BOOLEAN NOT NULL,
    noindex BOOLEAN NOT NULL,
    nofollow BOOLEAN NOT NULL,
    nosnippet BOOLEAN NOT NULL,
    noarchive BOOLEAN NOT NULL,
    noimageindex BOOLEAN NOT NULL,
    max_snippet INTEGER,
    unavailable_after VARCHAR,
    indexability VARCHAR NOT NULL,
    title VARCHAR,
    title_2 VARCHAR,
    title_count USMALLINT,
    meta_description VARCHAR,
    meta_description_2 VARCHAR,
    meta_description_count USMALLINT,
    meta_keywords VARCHAR,
    meta_robots VARCHAR,
    h1 VARCHAR,
    h1_2 VARCHAR,
    h1_count USMALLINT,
    h2 VARCHAR,
    h2_2 VARCHAR,
    h2_count USMALLINT,
    canonical VARCHAR,
    canonical_count USMALLINT,
    base_href VARCHAR,
    lang VARCHAR,
    rel_next VARCHAR,
    rel_prev VARCHAR,
    meta_refresh_seconds UINTEGER,
    meta_refresh_url VARCHAR,
    viewport VARCHAR,
    charset VARCHAR,
    amp_html VARCHAR,
    og_title VARCHAR,
    og_description VARCHAR,
    og_image VARCHAR,
    og_url VARCHAR,
    og_type VARCHAR,
    twitter_card VARCHAR,
    word_count UINTEGER,
    text_hash UBIGINT,
    simhash UBIGINT,
    insecure_forms UINTEGER,
    mixed_content UINTEGER
);

CREATE TABLE links (
    source UINTEGER NOT NULL,
    target UINTEGER NOT NULL,
    target_url VARCHAR NOT NULL,
    kind VARCHAR NOT NULL,
    text VARCHAR NOT NULL,
    nofollow BOOLEAN NOT NULL,
    ugc BOOLEAN NOT NULL,
    sponsored BOOLEAN NOT NULL,
    internal BOOLEAN NOT NULL,
    position UINTEGER NOT NULL
);

CREATE TABLE images (
    page UINTEGER NOT NULL,
    position UINTEGER NOT NULL,
    src VARCHAR NOT NULL,
    alt VARCHAR,
    has_dimensions BOOLEAN NOT NULL,
    lazy BOOLEAN NOT NULL
);

CREATE TABLE headings (
    page UINTEGER NOT NULL,
    position UINTEGER NOT NULL,
    level UTINYINT NOT NULL,
    text VARCHAR NOT NULL
);

CREATE TABLE hreflang (
    page UINTEGER NOT NULL,
    lang VARCHAR NOT NULL,
    target VARCHAR NOT NULL
);

CREATE TABLE structured_data (
    page UINTEGER NOT NULL,
    position UINTEGER NOT NULL,
    format VARCHAR NOT NULL,
    types VARCHAR NOT NULL,
    valid BOOLEAN NOT NULL
);

CREATE TABLE mixed_content (page UINTEGER NOT NULL, resource VARCHAR NOT NULL);

CREATE TABLE response_headers (
    page UINTEGER NOT NULL,
    name VARCHAR NOT NULL,
    value VARCHAR NOT NULL
);

CREATE TABLE html (page UINTEGER PRIMARY KEY, html VARCHAR NOT NULL);

CREATE TABLE robots_files (
    url VARCHAR NOT NULL,
    status VARCHAR NOT NULL,
    http_status USMALLINT,
    text VARCHAR NOT NULL,
    sitemaps VARCHAR NOT NULL
);

CREATE TABLE sitemaps (
    url VARCHAR NOT NULL,
    status USMALLINT,
    kind VARCHAR,
    error VARCHAR,
    entries UINTEGER NOT NULL,
    children UINTEGER NOT NULL
);

CREATE TABLE sitemap_urls (
    sitemap VARCHAR NOT NULL,
    url VARCHAR NOT NULL,
    lastmod VARCHAR
);

CREATE VIEW link_counts AS
SELECT u.id,
       coalesce(i.inlinks, 0) AS inlinks,
       coalesce(i.unique_inlinks, 0) AS unique_inlinks,
       coalesce(o.outlinks, 0) AS outlinks,
       coalesce(o.unique_outlinks, 0) AS unique_outlinks,
       coalesce(o.external_outlinks, 0) AS external_outlinks
FROM urls u
LEFT JOIN (
    SELECT target, count(*) AS inlinks, count(DISTINCT source) AS unique_inlinks
    FROM links WHERE kind = 'hyperlink' GROUP BY target
) i ON i.target = u.id
LEFT JOIN (
    SELECT source,
           count(*) FILTER (WHERE internal) AS outlinks,
           count(DISTINCT target) FILTER (WHERE internal) AS unique_outlinks,
           count(*) FILTER (WHERE NOT internal) AS external_outlinks
    FROM links WHERE kind = 'hyperlink' GROUP BY source
) o ON o.source = u.id;

CREATE VIEW tab_internal AS
SELECT u.url AS address,
       u.content_kind AS content,
       u.status AS status_code,
       u.error,
       u.indexability,
       u.title,
       length(u.title) AS title_length,
       u.title_count,
       u.meta_description,
       length(u.meta_description) AS meta_description_length,
       u.meta_description_count,
       u.h1,
       length(u.h1) AS h1_length,
       u.h1_count,
       u.h2,
       u.h2_count,
       u.canonical,
       u.header_canonical,
       u.meta_robots,
       u.x_robots_tag,
       u.redirect_url,
       u.depth AS crawl_depth,
       c.inlinks,
       c.unique_inlinks,
       c.outlinks,
       c.unique_outlinks,
       c.external_outlinks,
       u.word_count,
       u.size_bytes,
       u.response_ms AS response_time_ms,
       u.lang,
       u.rel_next,
       u.rel_prev,
       u.last_modified,
       u.blocked_by_robots,
       u.robots_rule,
       epoch_ms(u.crawled_at_ms) AS crawled_at
FROM urls u JOIN link_counts c USING (id)
WHERE u.internal
ORDER BY u.id;

CREATE VIEW tab_external AS
SELECT u.url AS address,
       u.content_kind AS content,
       u.status AS status_code,
       u.error,
       u.redirect_url,
       u.response_ms AS response_time_ms,
       c.inlinks,
       c.unique_inlinks,
       epoch_ms(u.crawled_at_ms) AS crawled_at
FROM urls u JOIN link_counts c USING (id)
WHERE NOT u.internal
ORDER BY u.id;

CREATE VIEW tab_response_codes AS
SELECT url AS address,
       internal,
       content_kind AS content,
       status AS status_code,
       error,
       blocked_by_robots,
       indexability,
       redirect_url,
       response_ms AS response_time_ms
FROM urls
ORDER BY id;

CREATE VIEW tab_links AS
SELECT s.url AS source,
       l.target_url AS destination,
       l.kind,
       l.text,
       l.nofollow,
       l.ugc,
       l.sponsored,
       l.internal,
       t.status AS destination_status,
       l.position
FROM links l
JOIN urls s ON s.id = l.source
LEFT JOIN urls t ON t.id = l.target
ORDER BY l.source, l.position;

CREATE VIEW tab_images AS
SELECT i.src AS image,
       p.url AS page,
       i.alt,
       i.alt IS NULL AS missing_alt,
       i.has_dimensions,
       i.lazy,
       t.status AS status_code,
       t.size_bytes
FROM images i
JOIN urls p ON p.id = i.page
LEFT JOIN urls t ON t.url = i.src
ORDER BY i.page, i.position;

CREATE VIEW tab_headings AS
SELECT p.url AS page, h.level, h.text, length(h.text) AS length, h.position
FROM headings h JOIN urls p ON p.id = h.page
ORDER BY h.page, h.position;

CREATE VIEW tab_hreflang AS
SELECT p.url AS page, h.lang, h.target, t.status AS target_status
FROM hreflang h
JOIN urls p ON p.id = h.page
LEFT JOIN urls t ON t.url = h.target
ORDER BY h.page;

CREATE VIEW tab_structured_data AS
SELECT p.url AS page, s.format, s.types, s.valid
FROM structured_data s JOIN urls p ON p.id = s.page
ORDER BY s.page, s.position;

CREATE VIEW tab_sitemaps AS
SELECT url, status AS status_code, kind, error, entries, children FROM sitemaps;

CREATE VIEW tab_sitemap_urls AS
SELECT s.url,
       s.sitemap,
       s.lastmod,
       u.status AS status_code,
       u.indexability,
       u.depth AS crawl_depth
FROM sitemap_urls s LEFT JOIN urls u ON u.url = s.url;

CREATE VIEW tab_robots AS
SELECT url, status, http_status, sitemaps, text FROM robots_files;
",
    r"
CREATE TABLE audit_findings (issue_id VARCHAR NOT NULL, url VARCHAR NOT NULL, PRIMARY KEY(issue_id,url));
CREATE VIEW tab_issues AS SELECT issue_id, url AS address FROM audit_findings ORDER BY issue_id,url;
CREATE TABLE site_files (url VARCHAR PRIMARY KEY, status USMALLINT, content_type VARCHAR, text VARCHAR, error VARCHAR);
",
    r"
CREATE TABLE rendering (
    page UINTEGER PRIMARY KEY,
    rendered BOOLEAN NOT NULL,
    rendered_url VARCHAR,
    error VARCHAR,
    elapsed_ms UINTEGER NOT NULL,
    truncated BOOLEAN NOT NULL,
    raw_title VARCHAR,
    raw_word_count UINTEGER NOT NULL,
    raw_link_count UINTEGER NOT NULL,
    rendered_link_count UINTEGER,
    raw_html VARCHAR
);
CREATE VIEW tab_rendering AS
SELECT u.url AS address,u.status AS status_code,r.rendered,r.rendered_url,
       r.error AS render_error,r.elapsed_ms AS render_time_ms,r.truncated AS render_truncated,
       r.raw_title,u.title AS rendered_title,r.raw_word_count,u.word_count AS rendered_word_count,
       r.raw_link_count,r.rendered_link_count
FROM rendering r JOIN urls u ON u.id=r.page ORDER BY u.id;
ALTER VIEW tab_internal RENAME TO tab_internal_base;
CREATE VIEW tab_internal AS
SELECT b.*,r.rendered,r.rendered_url,r.render_error,r.render_time_ms,r.render_truncated
FROM tab_internal_base b LEFT JOIN tab_rendering r ON r.address=b.address;
",
    // Recompute saved findings with full redirect-chain error propagation.
    "SELECT 1;",
    r"
ALTER TABLE rendering ADD COLUMN raw_meta_description VARCHAR;
ALTER TABLE rendering ADD COLUMN raw_canonical VARCHAR;
ALTER TABLE rendering ADD COLUMN raw_noindex BOOLEAN;
ALTER TABLE rendering ADD COLUMN raw_nofollow BOOLEAN;
CREATE OR REPLACE VIEW tab_rendering AS
SELECT u.url AS address,u.status AS status_code,r.rendered,r.rendered_url,
       r.error AS render_error,r.elapsed_ms AS render_time_ms,r.truncated AS render_truncated,
       r.raw_title,CASE WHEN r.rendered THEN u.title END AS rendered_title,
       r.raw_word_count,CASE WHEN r.rendered THEN u.word_count END AS rendered_word_count,
       r.raw_link_count,r.rendered_link_count,r.raw_meta_description,
       CASE WHEN r.rendered THEN u.meta_description END AS rendered_meta_description,
       r.raw_canonical,CASE WHEN r.rendered THEN coalesce(u.canonical,u.header_canonical) END AS rendered_canonical,
       r.raw_noindex,CASE WHEN r.rendered THEN u.noindex END AS rendered_noindex,
       r.raw_nofollow,CASE WHEN r.rendered THEN u.nofollow END AS rendered_nofollow
FROM rendering r JOIN urls u ON u.id=r.page ORDER BY u.id;
",
    r"
CREATE TABLE content_matches (page UINTEGER PRIMARY KEY, match_page UINTEGER NOT NULL, distance UTINYINT NOT NULL);
CREATE VIEW tab_content_matches AS
SELECT u.url AS address,u.status AS status_code,u.indexability,u.title,u.word_count,
       t.url AS match_address,t.title AS match_title,t.word_count AS match_word_count,m.distance AS hamming_distance
FROM content_matches m JOIN urls u ON u.id=m.page JOIN urls t ON t.id=m.match_page ORDER BY u.id;
",
    // Correct derived HTTP categories and refresh saved 5xx findings together.
    "UPDATE urls SET indexability='other-http' WHERE NOT blocked_by_robots AND status IS NOT NULL AND (status<200 OR status>=600);",
    r"
ALTER TABLE sitemap_urls ADD COLUMN IF NOT EXISTS crawl_url VARCHAR;
CREATE OR REPLACE VIEW tab_sitemap_urls AS
SELECT s.url,s.crawl_url,s.sitemap,s.lastmod,
       u.status AS status_code,u.indexability,u.depth AS crawl_depth
FROM sitemap_urls s LEFT JOIN urls u ON u.url=coalesce(s.crawl_url,s.url);
",
    r"
ALTER TABLE sitemaps ADD COLUMN IF NOT EXISTS redirect_url VARCHAR;
ALTER TABLE sitemaps ADD COLUMN IF NOT EXISTS redirect_location VARCHAR;
CREATE TABLE IF NOT EXISTS sitemap_redirects (
    url VARCHAR PRIMARY KEY,
    outcome VARCHAR NOT NULL,
    final_url VARCHAR,
    final_status USMALLINT,
    hops UINTEGER,
    error VARCHAR
);
CREATE OR REPLACE VIEW tab_sitemaps AS
SELECT s.url,s.status AS status_code,s.kind,coalesce(s.error,r.error) AS error,s.entries,s.children,
       s.redirect_url,s.redirect_location,r.outcome AS redirect_outcome,r.final_url,r.final_status,r.hops AS redirect_hops
FROM sitemaps s LEFT JOIN sitemap_redirects r ON r.url=s.url;
",
    r"
CREATE TABLE IF NOT EXISTS link_names (
    source UINTEGER NOT NULL,
    position UINTEGER NOT NULL,
    target_url VARCHAR NOT NULL,
    name VARCHAR NOT NULL,
    visible_text VARCHAR NOT NULL,
    name_source VARCHAR NOT NULL,
    has_svg BOOLEAN NOT NULL,
    has_image BOOLEAN NOT NULL,
    PRIMARY KEY(source,position)
);
",
    r"
CREATE TABLE IF NOT EXISTS schema_checks (
    page UINTEGER PRIMARY KEY,
    report VARCHAR,
    error VARCHAR,
    syntax_errors UINTEGER NOT NULL,
    required_errors UINTEGER NOT NULL,
    warnings UINTEGER NOT NULL,
    checked_items UINTEGER NOT NULL,
    unchecked_items UINTEGER NOT NULL,
    items UINTEGER NOT NULL,
    limited BOOLEAN NOT NULL
);
",
];

/// Versions 8/9 add optional sitemap identities/redirects; prepared v7/v8 stay readable.
pub(crate) const MIN_READ_VERSION: i64 = 7;

pub(crate) fn archive_compatible(version: i64) -> bool {
    (MIN_READ_VERSION..=latest_version()).contains(&version)
}

/// Creates or upgrades the schema.
pub(crate) fn latest_version() -> i64 {
    i64::try_from(MIGRATIONS.len()).unwrap_or(i64::MAX)
}

/// Creates or upgrades the schema.
pub(crate) fn migrate(connection: &mut Connection) -> Result<(), CrawlStoreError> {
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
        transaction.execute(
            "INSERT OR REPLACE INTO crawl_info VALUES ('audit_required','1')",
            [],
        )?;
        transaction.execute("INSERT INTO schema_migrations VALUES (?)", params![version])?;
        transaction.commit()?;
    }
    Ok(())
}
