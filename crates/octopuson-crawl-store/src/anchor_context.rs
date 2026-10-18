//! Link labels recovered from saved observations, never from a live network request.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex, OnceLock};

use duckdb::{Connection, params};
use octopuson_html::LinkName;

use crate::CrawlStoreError;

const COLUMNS: &str = "source UINTEGER,position UINTEGER,target_url VARCHAR,name VARCHAR,visible_text VARCHAR,name_source VARCHAR,has_svg BOOLEAN,has_image BOOLEAN";

/// Effective name and original stored text remain separately inspectable.
pub(crate) const LINKS: &str = "(SELECT l.* EXCLUDE(text),coalesce(n.name,l.text) AS text,l.text AS recorded_text,n.visible_text,n.name_source,n.has_svg,n.has_image,n.observation FROM links l LEFT JOIN current_link_names n ON n.source=l.source AND n.position=l.position AND n.target_url=l.target_url)";

/// The documented wa.me phone route normally redirects into `WhatsApp`. This never
/// exempts broken responses, internal redirects or other routes/hosts.
pub(crate) fn contact_url(url: &str, internal: &str) -> String {
    format!(
        "NOT {internal} AND regexp_matches({url},'^https://wa[.]me/[0-9]{{6,15}}/?([?][^#]*)?$')"
    )
}

pub(crate) fn ctes() -> String {
    let target = crate::score_exclusions::system_url("l.target_url");
    let generic = crate::score_exclusions::generic_anchor("l.text");
    let contact = contact_url("l.target_url", "l.internal");
    format!("effective_links AS (SELECT l.*,({generic}) AS generic_name FROM {LINKS} l), anchor_resolved AS (
    SELECT DISTINCT u.url,'anchor-empty' AS issue_id FROM urls u
    WHERE EXISTS(SELECT 1 FROM current_link_names n WHERE n.source=u.id AND n.name<>'')
    AND NOT EXISTS(SELECT 1 FROM effective_links l WHERE l.source=u.id AND l.kind='hyperlink' AND NOT ({target}) AND trim(l.text)='')
    UNION SELECT DISTINCT u.url,'anchor-generic' FROM urls u
    JOIN current_link_names n ON n.source=u.id WHERE n.name<>''
    AND NOT EXISTS(SELECT 1 FROM effective_links l WHERE l.source=u.id AND l.kind='hyperlink' AND NOT ({target}) AND l.generic_name)
), anchor_unverified AS (
    SELECT DISTINCT u.url,'anchor-empty' AS issue_id FROM urls u
    WHERE EXISTS(SELECT 1 FROM effective_links l WHERE l.source=u.id AND l.kind='hyperlink' AND NOT ({target}) AND trim(l.text)='' AND l.name_source IS NULL)
    AND NOT EXISTS(SELECT 1 FROM effective_links l WHERE l.source=u.id AND l.kind='hyperlink' AND NOT ({target}) AND trim(l.text)='' AND l.name_source IS NOT NULL)
    UNION SELECT DISTINCT u.url,'anchor-generic' FROM urls u
    WHERE EXISTS(SELECT 1 FROM effective_links l WHERE l.source=u.id AND l.kind='hyperlink' AND NOT ({target}) AND l.generic_name AND l.name_source IS NULL)
    AND NOT EXISTS(SELECT 1 FROM effective_links l WHERE l.source=u.id AND l.kind='hyperlink' AND NOT ({target}) AND l.generic_name AND l.name_source IS NOT NULL)
), contact_resolved AS (
    SELECT DISTINCT u.url,'links-to-redirects' AS issue_id FROM urls u
    WHERE EXISTS(SELECT 1 FROM links l JOIN urls t ON t.id=l.target WHERE l.source=u.id AND l.kind='hyperlink' AND t.status IN (301,302,303,307,308) AND t.error IS NULL AND ({contact}))
    AND NOT EXISTS(SELECT 1 FROM links l JOIN urls t ON t.id=l.target WHERE l.source=u.id AND l.kind='hyperlink' AND NOT ({target}) AND t.status IN (301,302,303,307,308) AND NOT(t.error IS NULL AND ({contact})))
)")
}

#[derive(Clone)]
struct Recovered {
    source: u32,
    position: u32,
    target: String,
    name: LinkName,
}

type Cache = VecDeque<(String, Arc<Vec<Recovered>>)>;
static CACHE: OnceLock<Mutex<Cache>> = OnceLock::new();

fn exists(connection: &Connection, table: &str) -> Result<bool, CrawlStoreError> {
    Ok(connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM information_schema.tables WHERE table_catalog=current_database() AND table_schema=current_schema() AND table_name=?)",
        params![table],
        |r| r.get(0),
    )?)
}

/// Only ephemeral tables/views are created, including on read-only archive connections.
pub(crate) fn prepare(connection: &Connection) -> Result<(), CrawlStoreError> {
    let observed = if exists(connection, "link_names")? {
        "SELECT * FROM link_names".to_owned()
    } else {
        "SELECT * FROM (SELECT NULL::UINTEGER AS source,NULL::UINTEGER AS position,NULL::VARCHAR AS target_url,NULL::VARCHAR AS name,NULL::VARCHAR AS visible_text,NULL::VARCHAR AS name_source,NULL::BOOLEAN AS has_svg,NULL::BOOLEAN AS has_image) WHERE false".to_owned()
    };
    connection.execute_batch(&format!("CREATE OR REPLACE TEMP VIEW observed_link_names AS {observed}; CREATE OR REPLACE TEMP TABLE recovered_link_names({COLUMNS}); CREATE OR REPLACE TEMP VIEW current_link_names AS SELECT *, 'recorded' AS observation FROM observed_link_names UNION ALL SELECT *, 'saved-html' AS observation FROM recovered_link_names r WHERE NOT EXISTS(SELECT 1 FROM observed_link_names n WHERE n.source=r.source AND n.position=r.position AND n.target_url=r.target_url)"))?;
    let key = cache_key(connection);
    let cached = key.as_ref().and_then(|key| {
        CACHE
            .get_or_init(Mutex::default)
            .lock()
            .ok()?
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, rows)| Arc::clone(rows))
    });
    let recovered = if let Some(rows) = cached {
        rows
    } else {
        let rows = Arc::new(recover(connection)?);
        if rows.len() <= 100_000
            && let Some(key) = key
            && let Ok(mut cache) = CACHE.get_or_init(Mutex::default).lock()
        {
            cache.push_front((key, Arc::clone(&rows)));
            cache.truncate(3);
        }
        rows
    };
    if !recovered.is_empty() {
        let mut appender =
            connection.appender_to_catalog_and_db("recovered_link_names", "temp", "main")?;
        for row in recovered.iter() {
            appender.append_row(params![
                row.source,
                row.position,
                row.target,
                row.name.name,
                row.name.visible_text,
                row.name.source,
                row.name.has_svg,
                row.name.has_image
            ])?;
        }
        appender.flush()?;
    }
    Ok(())
}

fn cache_key(connection: &Connection) -> Option<String> {
    let path: Option<String> = connection
        .query_row(
            "SELECT path FROM duckdb_databases() WHERE database_name=current_database()",
            [],
            |r| r.get(0),
        )
        .ok()?;
    let path = path?;
    if std::path::Path::new(&format!("{path}.wal")).exists() {
        return None;
    }
    let meta = std::fs::metadata(&path).ok()?;
    let modified = meta
        .modified()
        .ok()?
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_nanos();
    Some(format!("{path}:{}:{modified}", meta.len()))
}

fn recover(connection: &Connection) -> Result<Vec<Recovered>, CrawlStoreError> {
    if !exists(connection, "html")? || !exists(connection, "rendering")? {
        return Ok(Vec::new());
    }
    let config: Option<String> = connection
        .query_row("SELECT value FROM crawl_info WHERE key='config'", [], |r| {
            r.get(0)
        })
        .ok();
    let plan = config
        .and_then(|s| serde_json::from_str::<octopuson_crawl::CrawlConfig>(&s).ok())
        .and_then(|c| octopuson_crawl::CrawlPlan::new(c).ok());
    let generic = crate::score_exclusions::generic_anchor("l.text");
    let mut pages=connection.prepare(&format!("SELECT u.id,coalesce(CASE WHEN r.rendered THEN r.rendered_url END,u.url),h.html FROM urls u JOIN html h ON h.page=u.id LEFT JOIN rendering r ON r.page=u.id WHERE u.content_kind='html' AND NOT u.truncated AND NOT coalesce(r.truncated,false) AND EXISTS(SELECT 1 FROM links l WHERE l.source=u.id AND l.kind='hyperlink' AND (trim(l.text)='' OR ({generic})) AND NOT EXISTS(SELECT 1 FROM observed_link_names n WHERE n.source=l.source AND n.position=l.position AND n.target_url=l.target_url)) ORDER BY u.id"))?;
    let mut pages = pages.query([])?;
    let mut out = Vec::new();
    while let Some(row) = pages.next()? {
        let source = row.get(0)?;
        let address: String = row.get(1)?;
        let Ok(base) = url::Url::parse(&address) else {
            continue;
        };
        let html: String = row.get(2)?;
        let facts = octopuson_html::parse_page(&html, &base, &[]);
        let parsed = facts.hyperlinks().collect::<Vec<_>>();
        let mut stored=connection.prepare("SELECT position,target_url,text FROM links WHERE source=? AND kind='hyperlink' ORDER BY position")?;
        let stored = stored
            .query_map(params![source], |r| {
                Ok((
                    r.get::<_, u32>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        // Match the complete ordered hyperlink sequence, including repeated destinations.
        // A changed parser, base or normalization must not attach one link's label to another.
        if parsed.len() != stored.len()
            || !parsed.iter().zip(&stored).all(|(p, (_, target, _))| {
                plan.as_ref()
                    .and_then(|plan| plan.normalize_http_url(&p.url))
                    .map_or_else(|| p.url == *target, |url| url.as_str() == target)
            })
        {
            continue;
        }
        for (parsed, (position, target, text)) in parsed.into_iter().zip(stored) {
            if (text.trim().is_empty() || octopuson_audit::is_generic_anchor(&text))
                && let Some(name) = parsed.name.clone()
            {
                out.push(Recovered {
                    source,
                    position,
                    target,
                    name,
                });
            }
        }
    }
    Ok(out)
}
