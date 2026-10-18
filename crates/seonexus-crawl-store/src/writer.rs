//! Appending a running crawl's records in batches.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use duckdb::types::Value;
use duckdb::{Connection, appender_params_from_iter, params};
use seonexus_crawl::{
    CrawlPlan, CrawlProgress, CrawlSink, CrawlSummary, LinkRecord, RobotsRecord, SitemapRecord,
    UrlRecord,
};
use seonexus_html::{Hreflang, PageFacts};

use crate::error::CrawlStoreError;
use crate::library::{CrawlMeta, CrawlStatus, write_meta};
use crate::reader::{self, CrawlReader};
use crate::schema;

/// Records buffered before they are appended.
const BATCH: usize = 250;
/// Buffered records are appended at least this often while URLs arrive.
const FLUSH_INTERVAL: Duration = Duration::from_secs(2);
/// `crawl.json` is rewritten at most this often while the crawl runs.
const META_INTERVAL: Duration = Duration::from_secs(1);

/// The [`CrawlSink`] of a running crawl.
pub struct CrawlWriter {
    connection: Connection,
    meta_path: PathBuf,
    meta: CrawlMeta,
    plan: CrawlPlan,
    urls: Vec<UrlRecord>,
    links: Vec<LinkRecord>,
    last_flush: Instant,
    last_meta: Instant,
}

impl CrawlWriter {
    pub(crate) fn create(
        database: &Path,
        meta_path: PathBuf,
        meta: CrawlMeta,
        plan: CrawlPlan,
    ) -> Result<Self, CrawlStoreError> {
        let mut connection = reader::connect(database, false)?;
        schema::migrate(&mut connection)?;
        let config =
            serde_json::to_string(&meta.config).map_err(|source| CrawlStoreError::Meta {
                path: meta_path.clone(),
                source,
            })?;
        connection.execute(
            "INSERT OR REPLACE INTO crawl_info VALUES ('config', ?)",
            params![config],
        )?;
        if plan.config().schema_validation {
            connection.execute(
                "INSERT OR REPLACE INTO crawl_info VALUES ('schema_validation','1')",
                [],
            )?;
        }
        // New crawls retain header annotations even without optional source storage. Old
        // archives lack this marker, so an absent PDF return link remains unverified there.
        connection.execute(
            "INSERT OR REPLACE INTO crawl_info VALUES ('http_hreflang_observed','1')",
            [],
        )?;
        write_meta(&meta_path, &meta)?;
        Ok(Self {
            connection,
            meta_path,
            meta,
            plan,
            urls: Vec::new(),
            links: Vec::new(),
            last_flush: Instant::now(),
            last_meta: Instant::now(),
        })
    }

    /// The crawl's metadata as last written.
    pub fn meta(&self) -> &CrawlMeta {
        &self.meta
    }

    /// A reader sharing this writer's database, for live views of the running crawl. It sees
    /// records once they are flushed.
    ///
    /// # Errors
    /// Database errors.
    pub fn reader(&self) -> Result<CrawlReader, CrawlStoreError> {
        Ok(CrawlReader::from_connection(self.connection.try_clone()?))
    }

    /// Appends buffered records now.
    ///
    /// # Errors
    /// Database errors.
    pub fn flush(&mut self) -> Result<(), CrawlStoreError> {
        self.last_flush = Instant::now();
        if self.urls.is_empty() && self.links.is_empty() {
            return Ok(());
        }
        append_urls(&self.connection, &self.urls)?;
        append_links(&self.connection, &self.links)?;
        self.urls.clear();
        self.links.clear();
        Ok(())
    }

    /// Writes the remaining records and marks the crawl completed or stopped.
    ///
    /// # Errors
    /// Database or file system errors.
    pub fn finish(mut self, summary: &CrawlSummary) -> Result<CrawlMeta, CrawlStoreError> {
        self.flush()?;
        self.meta.progress = summary.progress;
        self.meta.limit_reached = summary.limit_reached;
        self.meta.status = if summary.stopped {
            CrawlStatus::Stopped
        } else {
            CrawlStatus::Completed
        };
        self.close()
    }

    /// Writes what can still be written and marks the crawl failed with `message`.
    ///
    /// # Errors
    /// File system errors writing the metadata.
    pub fn fail(mut self, message: &str) -> Result<CrawlMeta, CrawlStoreError> {
        // The crawl already failed; keep as much of it as possible.
        let _ = self.flush();
        self.meta.status = CrawlStatus::Failed;
        self.meta.error = Some(message.to_owned());
        self.close()
    }

    fn close(mut self) -> Result<CrawlMeta, CrawlStoreError> {
        if let Err(error) = crate::audit::run(&self.connection) {
            self.meta.status = CrawlStatus::Failed;
            self.meta.error = Some(error.to_string());
            let _ = write_meta(&self.meta_path, &self.meta);
            return Err(error);
        }
        self.meta.finished_at_ms = Some(now_ms());
        let progress = serde_json::to_string(&self.meta.progress).unwrap_or_default();
        self.connection.execute(
            "INSERT OR REPLACE INTO crawl_info VALUES ('progress', ?)",
            params![progress],
        )?;
        self.connection.execute_batch("CHECKPOINT")?;
        write_meta(&self.meta_path, &self.meta)?;
        Ok(self.meta)
    }
}

impl CrawlSink for CrawlWriter {
    type Error = CrawlStoreError;

    fn site_file(
        &mut self,
        record: &seonexus_crawl::SiteFileRecord,
    ) -> Result<(), CrawlStoreError> {
        self.connection.execute(
            "INSERT OR REPLACE INTO site_files VALUES (?,?,?,?,?)",
            params![
                record.url,
                record.status,
                record.content_type,
                record.text,
                record.error
            ],
        )?;
        Ok(())
    }

    fn url(&mut self, record: &UrlRecord, links: &[LinkRecord]) -> Result<(), CrawlStoreError> {
        self.urls.push(record.clone());
        self.links.extend_from_slice(links);
        if self.urls.len() >= BATCH || self.last_flush.elapsed() >= FLUSH_INTERVAL {
            self.flush()?;
        }
        Ok(())
    }

    fn robots(&mut self, record: &RobotsRecord) -> Result<(), CrawlStoreError> {
        self.connection.execute(
            "INSERT INTO robots_files VALUES (?, ?, ?, ?, ?)",
            params![
                record.url,
                record.status.as_str(),
                record.http_status,
                record.text,
                record.sitemaps.join("\n")
            ],
        )?;
        Ok(())
    }

    fn sitemap(&mut self, record: &SitemapRecord) -> Result<(), CrawlStoreError> {
        self.connection.execute(
            "INSERT INTO sitemaps (url,status,kind,error,entries,children,redirect_url,redirect_location) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                record.url,
                record.status,
                record.kind.map(seonexus_crawl::SitemapKind::as_str),
                record.error,
                count32(record.entries.len()),
                count32(record.children.len()),
                record.redirect_url,
                record.redirect_location
            ],
        )?;
        if !record.entries.is_empty() {
            let mut appender = self.connection.appender("sitemap_urls")?;
            for entry in &record.entries {
                let crawl_url = self.plan.normalize_http_url(&entry.url).map(String::from);
                appender.append_row(params![record.url, entry.url, entry.lastmod, crawl_url])?;
            }
            appender.flush()?;
        }
        Ok(())
    }

    fn progress(&mut self, progress: &CrawlProgress) {
        self.meta.progress = *progress;
        if self.last_meta.elapsed() >= META_INTERVAL {
            self.last_meta = Instant::now();
            // Progress is advisory; a failed write is retried on the next report.
            let _ = write_meta(&self.meta_path, &self.meta);
        }
    }
}

fn append_urls(connection: &Connection, urls: &[UrlRecord]) -> Result<(), CrawlStoreError> {
    let mut rows = connection.appender("urls")?;
    for record in urls {
        rows.append_row(appender_params_from_iter(url_row(record)))?;
    }
    rows.flush()?;
    drop(rows);

    append_rendering(connection, urls)?;
    append_hreflang(connection, urls)?;
    crate::schema_checks::append(connection, urls)?;

    let pages = || {
        urls.iter()
            .filter_map(|r| r.page.as_ref().map(|page| (r.id, page)))
    };
    if pages().any(|(_, page)| !page.images.is_empty()) {
        let mut images = connection.appender("images")?;
        for (id, page) in pages() {
            for (position, image) in page.images.iter().enumerate() {
                images.append_row(params![
                    id,
                    count32(position),
                    image.url,
                    image.alt,
                    image.has_dimensions,
                    image.lazy
                ])?;
            }
        }
        images.flush()?;
    }
    if pages().any(|(_, page)| !page.headings.is_empty()) {
        let mut headings = connection.appender("headings")?;
        for (id, page) in pages() {
            for (position, heading) in page.headings.iter().enumerate() {
                headings.append_row(params![id, count32(position), heading.level, heading.text])?;
            }
        }
        headings.flush()?;
    }
    if pages().any(|(_, page)| !page.structured_data.is_empty()) {
        let mut blocks = connection.appender("structured_data")?;
        for (id, page) in pages() {
            for (position, block) in page.structured_data.iter().enumerate() {
                blocks.append_row(params![
                    id,
                    count32(position),
                    block.format.as_str(),
                    block.types.join(", "),
                    block.valid
                ])?;
            }
        }
        blocks.flush()?;
    }
    if pages().any(|(_, page)| !page.mixed_content.is_empty()) {
        let mut mixed = connection.appender("mixed_content")?;
        for (id, page) in pages() {
            for resource in &page.mixed_content {
                mixed.append_row(params![id, resource])?;
            }
        }
        mixed.flush()?;
    }
    if urls.iter().any(|r| !r.headers.is_empty()) {
        let mut headers = connection.appender("response_headers")?;
        for record in urls {
            for (name, value) in &record.headers {
                headers.append_row(params![record.id, name, value])?;
            }
        }
        headers.flush()?;
    }
    if urls.iter().any(|r| r.html.is_some()) {
        let mut html = connection.appender("html")?;
        for record in urls {
            if let Some(text) = &record.html {
                html.append_row(params![record.id, text])?;
            }
        }
        html.flush()?;
    }
    Ok(())
}

fn annotations(record: &UrlRecord) -> impl Iterator<Item = &Hreflang> {
    record
        .page
        .iter()
        .flat_map(|page| &page.hreflang)
        .chain(&record.header_hreflang)
}

fn append_hreflang(connection: &Connection, urls: &[UrlRecord]) -> Result<(), CrawlStoreError> {
    if !urls
        .iter()
        .any(|record| annotations(record).next().is_some())
    {
        return Ok(());
    }
    let mut hreflang = connection.appender("hreflang")?;
    for record in urls {
        let mut seen = HashSet::new();
        for alternate in annotations(record) {
            if seen.insert((alternate.lang.to_ascii_lowercase(), alternate.url.as_str())) {
                hreflang.append_row(params![record.id, alternate.lang, alternate.url])?;
            }
        }
    }
    hreflang.flush()?;
    Ok(())
}

fn append_links(connection: &Connection, links: &[LinkRecord]) -> Result<(), CrawlStoreError> {
    if links.is_empty() {
        return Ok(());
    }
    let mut rows = connection.appender("links")?;
    for link in links {
        rows.append_row(params![
            link.source,
            link.target,
            link.target_url,
            link.kind.as_str(),
            link.text,
            link.nofollow,
            link.ugc,
            link.sponsored,
            link.internal,
            link.position
        ])?;
    }
    rows.flush()?;
    drop(rows);
    let mut names = connection.appender("link_names")?;
    for link in links {
        if let Some(name) = &link.name {
            names.append_row(params![
                link.source,
                link.position,
                link.target_url,
                name.name,
                name.visible_text,
                name.source,
                name.has_svg,
                name.has_image
            ])?;
        }
    }
    names.flush()?;
    Ok(())
}

/// One `urls` row, in column order.
fn url_row(record: &UrlRecord) -> Vec<Value> {
    let directives = &record.directives;
    let mut row = vec![
        Value::UInt(record.id),
        Value::Text(record.url.clone()),
        record.depth.map_or(Value::Null, Value::UInt),
        Value::Boolean(record.internal),
        Value::BigInt(record.crawled_at_ms),
        record.status.map_or(Value::Null, Value::USmallInt),
        text(record.error.as_deref()),
        Value::Boolean(record.blocked_by_robots),
        text(record.robots_rule.as_deref()),
        Value::Text(record.content_kind.as_str().to_owned()),
        text(record.content_type.as_deref()),
        Value::UBigInt(record.size_bytes),
        Value::Boolean(record.truncated),
        Value::UInt(record.response_ms),
        text(record.http_version.as_deref()),
        text(record.redirect_url.as_deref()),
        text(record.x_robots_tag.as_deref()),
        text(record.header_canonical.as_deref()),
        text(record.last_modified.as_deref()),
        text(record.content_encoding.as_deref()),
        text(record.cache_control.as_deref()),
        Value::Boolean(record.hsts),
        Value::Boolean(directives.noindex),
        Value::Boolean(directives.nofollow),
        Value::Boolean(directives.nosnippet),
        Value::Boolean(directives.noarchive),
        Value::Boolean(directives.noimageindex),
        directives.max_snippet.map_or(Value::Null, Value::Int),
        text(directives.unavailable_after.as_deref()),
        Value::Text(record.indexability.as_str().to_owned()),
    ];
    match &record.page {
        Some(page) => row.extend(page_values(page)),
        None => row.extend(std::iter::repeat_n(Value::Null, PAGE_COLUMNS)),
    }
    row
}

/// Columns of `urls` that only HTML pages fill.
const PAGE_COLUMNS: usize = 36;

fn page_values(page: &PageFacts) -> [Value; PAGE_COLUMNS] {
    let nth = |values: &[String], index: usize| text(values.get(index).map(String::as_str));
    let h1: Vec<&str> = page.headings_of(1).collect();
    let h2: Vec<&str> = page.headings_of(2).collect();
    let nth_str = |values: &[&str], index: usize| text(values.get(index).copied());
    let social = &page.social;
    let refresh = page.meta_refresh.as_ref();
    [
        nth(&page.titles, 0),
        nth(&page.titles, 1),
        count16(page.titles.len()),
        nth(&page.meta_descriptions, 0),
        nth(&page.meta_descriptions, 1),
        count16(page.meta_descriptions.len()),
        text(page.meta_keywords.as_deref()),
        text(
            Some(page.meta_robots.join("; "))
                .as_deref()
                .filter(|s| !s.is_empty()),
        ),
        nth_str(&h1, 0),
        nth_str(&h1, 1),
        count16(h1.len()),
        nth_str(&h2, 0),
        nth_str(&h2, 1),
        count16(h2.len()),
        nth(&page.canonicals, 0),
        count16(page.canonicals.len()),
        text(page.base_href.as_deref()),
        text(page.lang.as_deref()),
        text(page.rel_next.as_deref()),
        text(page.rel_prev.as_deref()),
        refresh.map_or(Value::Null, |r| Value::UInt(r.delay_seconds)),
        text(refresh.and_then(|r| r.url.as_deref())),
        text(page.viewport.as_deref()),
        text(page.charset.as_deref()),
        text(page.amp_html.as_deref()),
        text(social.og_title.as_deref()),
        text(social.og_description.as_deref()),
        text(social.og_image.as_deref()),
        text(social.og_url.as_deref()),
        text(social.og_type.as_deref()),
        text(social.twitter_card.as_deref()),
        Value::UInt(page.word_count),
        Value::UBigInt(page.text_hash),
        Value::UBigInt(page.simhash),
        Value::UInt(page.insecure_forms),
        Value::UInt(count32(page.mixed_content.len())),
    ]
}

fn text(value: Option<&str>) -> Value {
    value.map_or(Value::Null, |value| Value::Text(value.to_owned()))
}

fn count16(count: usize) -> Value {
    Value::USmallInt(u16::try_from(count).unwrap_or(u16::MAX))
}

fn count32(count: usize) -> u32 {
    u32::try_from(count).unwrap_or(u32::MAX)
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| {
            i64::try_from(elapsed.as_millis()).unwrap_or(i64::MAX)
        })
}

fn append_rendering(connection: &Connection, urls: &[UrlRecord]) -> Result<(), CrawlStoreError> {
    if urls.iter().any(|record| record.rendering.is_some()) {
        let mut rendering = connection.appender("rendering")?;
        for record in urls {
            if let Some(r) = &record.rendering {
                rendering.append_row(params![
                    record.id,
                    r.rendered,
                    r.url,
                    r.error,
                    r.elapsed_ms,
                    r.truncated,
                    r.raw_title,
                    r.raw_word_count,
                    r.raw_link_count,
                    r.rendered_link_count,
                    r.raw_html,
                    r.raw_meta_description,
                    r.raw_canonical,
                    r.raw_noindex,
                    r.raw_nofollow
                ])?;
            }
        }
        rendering.flush()?;
    }

    Ok(())
}
