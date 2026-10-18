//! DuckDB session storage, queries, exports and redirect rule generation for Index Checker scans.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use duckdb::{Connection, params};
use octopuson_core::table::{Cell, Column, ColumnKind, TableSink};
use octopuson_report::ExportFormat;

use crate::FinderError;
use crate::model::{
    CreditUsage, EngineSummary, FinderPageRequest, FinderTablePage, FindingCategory, ScanConfig,
    ScanMeta, ScanOverview, VerifiedUrl,
};

const QUALIFIER: &str = "com";
const ORGANIZATION: &str = "octopusSEO";
const APPLICATION: &str = "octopusSEO 404 Finder";
const META_FILE: &str = "scan.json";
const DB_FILE: &str = "scan.duckdb";

const CREATE_SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS urls (
    url VARCHAR PRIMARY KEY,
    engines VARCHAR NOT NULL,
    serp_title VARCHAR NOT NULL,
    serp_snippet VARCHAR NOT NULL,
    found_by_query VARCHAR NOT NULL,
    initial_status INTEGER NOT NULL,
    final_status INTEGER NOT NULL,
    redirect_hops INTEGER NOT NULL,
    final_url VARCHAR NOT NULL,
    response_time_ms INTEGER NOT NULL,
    content_type VARCHAR NOT NULL,
    live_title VARCHAR NOT NULL,
    h1 VARCHAR NOT NULL,
    word_count INTEGER NOT NULL,
    canonical VARCHAR NOT NULL,
    noindex BOOLEAN NOT NULL,
    category VARCHAR NOT NULL,
    reason VARCHAR NOT NULL,
    suggested_target VARCHAR NOT NULL,
    error_message VARCHAR NOT NULL
);
";

/// Manages the directory of saved Index Checker scans.
#[derive(Debug, Clone)]
pub struct FinderStore {
    scans_dir: PathBuf,
}

impl FinderStore {
    /// Stores scans under `root/scans`.
    pub fn new(root: impl AsRef<Path>) -> Self {
        Self {
            scans_dir: root.as_ref().join("scans"),
        }
    }

    /// Resolves the platform default data directory (`com.octopusSEO.octopusSEO-404-Finder`).
    ///
    /// # Errors
    /// Returns [`FinderError::NoDataDir`] if the platform reports no home directory.
    pub fn platform_default() -> Result<Self, FinderError> {
        let dirs = directories::ProjectDirs::from(QUALIFIER, ORGANIZATION, APPLICATION)
            .ok_or(FinderError::NoDataDir)?;
        Ok(Self::new(dirs.data_dir()))
    }

    /// Directory containing all saved scan folders.
    pub fn path(&self) -> &Path {
        &self.scans_dir
    }

    /// Allocates a new atomic scan directory and writes initial `scan.json`.
    ///
    /// # Errors
    /// Returns an error if the directory or metadata file cannot be created.
    pub fn reserve_scan(
        &self,
        domain: &str,
        config: &ScanConfig,
    ) -> Result<(String, PathBuf, ScanMeta), FinderError> {
        fs::create_dir_all(&self.scans_dir).map_err(|source| FinderError::Io {
            path: self.scans_dir.clone(),
            source,
        })?;
        let now_ms = ScanMeta::now_ms();
        let slug: String = domain
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() {
                    c.to_ascii_lowercase()
                } else {
                    '-'
                }
            })
            .collect();
        let base_id = format!("{now_ms}-{}", slug.trim_matches('-'));
        let mut attempt = 1u32;
        let (id, dir) = loop {
            let candidate = if attempt == 1 {
                base_id.clone()
            } else {
                format!("{base_id}-{attempt}")
            };
            let dir = self.scans_dir.join(&candidate);
            match fs::create_dir(&dir) {
                Ok(()) => break (candidate, dir),
                Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {
                    attempt = attempt.saturating_add(1);
                }
                Err(source) => return Err(FinderError::Io { path: dir, source }),
            }
        };

        let meta = ScanMeta {
            id: id.clone(),
            domain: domain.to_owned(),
            created_at_ms: now_ms,
            finished_at_ms: now_ms,
            status: "running".to_owned(),
            config: config.clone(),
            queries_executed: 0,
            discovered_urls: 0,
            verified_urls: 0,
            count_404: 0,
            count_4xx: 0,
            count_5xx: 0,
            count_soft_404: 0,
            count_thin_200: 0,
            count_network_error: 0,
            count_healthy: 0,
            credits: CreditUsage::default(),
        };
        write_meta(&dir, &meta)?;
        Ok((id, dir, meta))
    }

    /// Writes verified URLs to `scan.duckdb` and updates `scan.json`.
    ///
    /// # Errors
    /// Returns an error if DuckDB or `scan.json` cannot be written.
    pub fn finish_scan(
        &self,
        dir: &Path,
        mut meta: ScanMeta,
        status: &str,
        queries_executed: u32,
        discovered_urls: u32,
        rows: &[VerifiedUrl],
    ) -> Result<ScanMeta, FinderError> {
        let db_path = dir.join(DB_FILE);
        let conn = Connection::open(&db_path).map_err(|e| FinderError::Database(e.to_string()))?;
        conn.execute_batch(CREATE_SCHEMA)
            .map_err(|e| FinderError::Database(e.to_string()))?;
        conn.execute_batch("DELETE FROM urls;")
            .map_err(|e| FinderError::Database(e.to_string()))?;

        let mut appender = conn
            .appender("urls")
            .map_err(|e| FinderError::Database(e.to_string()))?;

        let mut count_404 = 0u32;
        let mut count_4xx = 0u32;
        let mut count_5xx = 0u32;
        let mut count_soft_404 = 0u32;
        let mut count_thin_200 = 0u32;
        let mut count_network_error = 0u32;
        let mut count_healthy = 0u32;

        for row in rows {
            match row.category {
                FindingCategory::Error404 => count_404 = count_404.saturating_add(1),
                FindingCategory::Error4xx => count_4xx = count_4xx.saturating_add(1),
                FindingCategory::Error5xx => count_5xx = count_5xx.saturating_add(1),
                FindingCategory::Soft404 => count_soft_404 = count_soft_404.saturating_add(1),
                FindingCategory::Thin200 => count_thin_200 = count_thin_200.saturating_add(1),
                FindingCategory::NetworkError => {
                    count_network_error = count_network_error.saturating_add(1);
                }
                FindingCategory::Healthy200 => count_healthy = count_healthy.saturating_add(1),
            }
            appender
                .append_row(params![
                    row.url,
                    row.engines,
                    row.serp_title,
                    row.serp_snippet,
                    row.found_by_query,
                    i32::from(row.initial_status),
                    i32::from(row.final_status),
                    i32::from(row.redirect_hops),
                    row.final_url,
                    i64::from(row.response_time_ms),
                    row.content_type,
                    row.live_title,
                    row.h1,
                    i64::from(row.word_count),
                    row.canonical,
                    row.noindex,
                    row.category.as_str(),
                    row.reason,
                    row.suggested_target,
                    row.error_message,
                ])
                .map_err(|e| FinderError::Database(e.to_string()))?;
        }
        appender
            .flush()
            .map_err(|e| FinderError::Database(e.to_string()))?;

        meta.finished_at_ms = ScanMeta::now_ms();
        status.clone_into(&mut meta.status);
        meta.queries_executed = queries_executed;
        meta.discovered_urls = discovered_urls;
        meta.verified_urls = u32::try_from(rows.len()).unwrap_or(u32::MAX);
        meta.count_404 = count_404;
        meta.count_4xx = count_4xx;
        meta.count_5xx = count_5xx;
        meta.count_soft_404 = count_soft_404;
        meta.count_thin_200 = count_thin_200;
        meta.count_network_error = count_network_error;
        meta.count_healthy = count_healthy;

        write_meta(dir, &meta)?;
        Ok(meta)
    }

    /// Lists all saved scans sorted newest first.
    ///
    /// # Errors
    /// Returns an error if the directory cannot be read.
    pub fn list_scans(&self) -> Result<Vec<ScanMeta>, FinderError> {
        if !self.scans_dir.exists() {
            return Ok(Vec::new());
        }
        let entries = fs::read_dir(&self.scans_dir).map_err(|source| FinderError::Io {
            path: self.scans_dir.clone(),
            source,
        })?;
        let mut list = Vec::new();
        for entry in entries.flatten() {
            let meta_path = entry.path().join(META_FILE);
            if let Ok(bytes) = fs::read(&meta_path)
                && let Ok(meta) = serde_json::from_slice::<ScanMeta>(&bytes)
            {
                list.push(meta);
            }
        }
        list.sort_by(|a, b| {
            b.created_at_ms
                .cmp(&a.created_at_ms)
                .then_with(|| b.id.cmp(&a.id))
        });
        Ok(list)
    }

    /// Resolves `"latest"` or a specific scan ID to its `ScanMeta` and directory.
    ///
    /// # Errors
    /// Returns [`FinderError::NotFound`] if no matching scan exists.
    pub fn resolve_scan(&self, id_or_latest: &str) -> Result<(PathBuf, ScanMeta), FinderError> {
        if id_or_latest == "latest" {
            let mut scans = self.list_scans()?;
            if scans.is_empty() {
                return Err(FinderError::NotFound("latest".to_owned()));
            }
            let meta = scans.remove(0);
            let dir = self.scans_dir.join(&meta.id);
            return Ok((dir, meta));
        }
        if id_or_latest.contains('/') || id_or_latest.contains('\\') || id_or_latest.contains("..")
        {
            return Err(FinderError::NotFound(id_or_latest.to_owned()));
        }
        let dir = self.scans_dir.join(id_or_latest);
        let meta_path = dir.join(META_FILE);
        let bytes =
            fs::read(&meta_path).map_err(|_| FinderError::NotFound(id_or_latest.to_owned()))?;
        let meta: ScanMeta =
            serde_json::from_slice(&bytes).map_err(|e| FinderError::Database(e.to_string()))?;
        Ok((dir, meta))
    }

    /// Deletes a saved scan directory.
    ///
    /// # Errors
    /// Returns an error if the scan does not exist or cannot be removed.
    pub fn delete_scan(&self, id: &str) -> Result<(), FinderError> {
        let (dir, _) = self.resolve_scan(id)?;
        fs::remove_dir_all(&dir).map_err(|source| FinderError::Io { path: dir, source })
    }

    /// Computes the overview for a saved scan (`ScanOverview`).
    ///
    /// # Errors
    /// Returns an error if the scan or database cannot be read.
    pub fn overview(&self, id: &str) -> Result<ScanOverview, FinderError> {
        let (_, meta, all_rows) = self.load_all_rows(id)?;
        if all_rows.is_empty() {
            return Ok(ScanOverview {
                health_score: meta.health_score(),
                meta,
                engines: Vec::new(),
                clusters: Vec::new(),
                top_issues: Vec::new(),
            });
        }

        // Compute per-engine summary
        let mut engine_totals: BTreeMap<String, (u32, u32)> = BTreeMap::new();
        for row in &all_rows {
            let is_issue = row.category.is_issue();
            for eng in row.engines.split(',').filter(|s| !s.is_empty()) {
                let entry = engine_totals.entry(eng.to_owned()).or_insert((0, 0));
                entry.0 = entry.0.saturating_add(1);
                if is_issue {
                    entry.1 = entry.1.saturating_add(1);
                }
            }
        }
        let engines = engine_totals
            .into_iter()
            .map(|(engine, (total_urls, issue_urls))| EngineSummary {
                engine,
                total_urls,
                issue_urls,
            })
            .collect();

        let clusters = crate::cluster::build_path_clusters(&all_rows);

        // Top issues
        let page = self.query_page(
            id,
            &FinderPageRequest {
                category: "issues".to_owned(),
                limit: 15,
                ..FinderPageRequest::default()
            },
        )?;

        Ok(ScanOverview {
            health_score: meta.health_score(),
            meta,
            engines,
            clusters,
            top_issues: page.rows,
        })
    }

    /// Queries a filtered, paginated slice of verified URLs.
    ///
    /// # Errors
    /// Returns an error if the database cannot be queried.
    pub fn query_page(
        &self,
        id: &str,
        req: &FinderPageRequest,
    ) -> Result<FinderTablePage, FinderError> {
        let (dir, _) = self.resolve_scan(id)?;
        let db_path = dir.join(DB_FILE);
        if !db_path.exists() {
            return Ok(FinderTablePage {
                total: 0,
                rows: Vec::new(),
            });
        }
        let conn = Connection::open(&db_path).map_err(|e| FinderError::Database(e.to_string()))?;
        let where_sql = build_where_clause(req);

        let count_sql = format!("SELECT COUNT(*) FROM urls {where_sql}");
        let total: i64 = conn
            .query_row(&count_sql, [], |row| row.get(0))
            .unwrap_or(0);

        let limit = req.limit.clamp(1, 500);
        let offset = req.offset;
        let order_sql = build_order_by_clause(&req.sort_by);
        let query_sql = format!(
            "SELECT
                url, engines, serp_title, serp_snippet, found_by_query,
                initial_status, final_status, redirect_hops, final_url,
                response_time_ms, content_type, live_title, h1, word_count,
                canonical, noindex, category, reason, suggested_target, error_message
             FROM urls
             {where_sql}
             ORDER BY {order_sql}
             LIMIT {limit} OFFSET {offset}"
        );

        let rows = fetch_verified_rows(&conn, &query_sql)?;
        Ok(FinderTablePage {
            total: u32::try_from(total.max(0)).unwrap_or(u32::MAX),
            rows,
        })
    }

    /// Streams matching rows into a CSV, XLSX or JSON file via `octopuson-report`.
    ///
    /// # Errors
    /// Returns an error if the file cannot be written.
    pub fn export_table(
        &self,
        id: &str,
        req: &FinderPageRequest,
        path: &Path,
        format: Option<ExportFormat>,
    ) -> Result<u64, FinderError> {
        let format = format
            .or_else(|| ExportFormat::from_path(path))
            .ok_or_else(|| FinderError::UnknownExportFormat(path.to_owned()))?;
        let (dir, _) = self.resolve_scan(id)?;
        let db_path = dir.join(DB_FILE);
        let conn = Connection::open(&db_path).map_err(|e| FinderError::Database(e.to_string()))?;
        let where_sql = build_where_clause(req);
        let query_sql = format!(
            "SELECT
                url, engines, serp_title, serp_snippet, found_by_query,
                initial_status, final_status, redirect_hops, final_url,
                response_time_ms, content_type, live_title, h1, word_count,
                canonical, noindex, category, reason, suggested_target, error_message
             FROM urls
             {where_sql}
             ORDER BY category ASC, url ASC"
        );
        let rows = fetch_verified_rows(&conn, &query_sql)?;

        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let io_err = |source| FinderError::Io {
            path: path.to_owned(),
            source,
        };
        let temp = tempfile::NamedTempFile::new_in(parent).map_err(io_err)?;
        let mut sink: Box<dyn TableSink> = format.writer(temp.reopen().map_err(io_err)?);

        let columns = vec![
            Column::new("url", ColumnKind::Text),
            Column::new("engines", ColumnKind::Text),
            Column::new("category", ColumnKind::Text),
            Column::new("reason", ColumnKind::Text),
            Column::new("initial_status", ColumnKind::Integer),
            Column::new("final_status", ColumnKind::Integer),
            Column::new("redirect_hops", ColumnKind::Integer),
            Column::new("final_url", ColumnKind::Text),
            Column::new("word_count", ColumnKind::Integer),
            Column::new("live_title", ColumnKind::Text),
            Column::new("h1", ColumnKind::Text),
            Column::new("serp_title", ColumnKind::Text),
            Column::new("found_by_query", ColumnKind::Text),
            Column::new("suggested_target", ColumnKind::Text),
            Column::new("response_time_ms", ColumnKind::Integer),
        ];
        sink.begin(&columns).map_err(io_err)?;
        let mut count = 0u64;
        for row in &rows {
            sink.row(&[
                Cell::Text(row.url.clone()),
                Cell::Text(row.engines.clone()),
                Cell::Text(row.category.as_str().to_owned()),
                Cell::Text(row.reason.clone()),
                Cell::Integer(i64::from(row.initial_status)),
                Cell::Integer(i64::from(row.final_status)),
                Cell::Integer(i64::from(row.redirect_hops)),
                Cell::Text(row.final_url.clone()),
                Cell::Integer(i64::from(row.word_count)),
                Cell::Text(row.live_title.clone()),
                Cell::Text(row.h1.clone()),
                Cell::Text(row.serp_title.clone()),
                Cell::Text(row.found_by_query.clone()),
                Cell::Text(row.suggested_target.clone()),
                Cell::Integer(i64::from(row.response_time_ms)),
            ])
            .map_err(io_err)?;
            count = count.saturating_add(1);
        }
        sink.finish().map_err(io_err)?;
        drop(sink);
        temp.as_file().sync_all().map_err(io_err)?;
        temp.persist(path).map_err(|e| io_err(e.error))?;
        Ok(count)
    }

    /// Loads all verified rows for a scan (`id` or `"latest"`).
    ///
    /// # Errors
    /// Returns an error if the scan or database cannot be read.
    pub fn load_all_rows(
        &self,
        id: &str,
    ) -> Result<(PathBuf, ScanMeta, Vec<VerifiedUrl>), FinderError> {
        let (dir, meta) = self.resolve_scan(id)?;
        let db_path = dir.join(DB_FILE);
        if !db_path.exists() {
            return Ok((dir, meta, Vec::new()));
        }
        let conn = Connection::open(&db_path).map_err(|e| FinderError::Database(e.to_string()))?;
        let query_sql = "SELECT
                url, engines, serp_title, serp_snippet, found_by_query,
                initial_status, final_status, redirect_hops, final_url,
                response_time_ms, content_type, live_title, h1, word_count,
                canonical, noindex, category, reason, suggested_target, error_message
             FROM urls
             ORDER BY url ASC";
        let rows = fetch_verified_rows(&conn, query_sql)?;
        Ok((dir, meta, rows))
    }

    /// Generates ready-to-use 301 redirect rules, clean XML sitemaps, redirect chain CSVs, or `IndexNow` JSON.
    ///
    /// # Errors
    /// Returns an error if the database cannot be read.
    pub fn generate_redirect_rules(&self, id: &str, format: &str) -> Result<String, FinderError> {
        let (_, _, rows) = self.load_all_rows(id)?;
        Ok(crate::compare::format_redirect_rules(&rows, format))
    }

    /// Returns ranked 301 redirect candidates (live `200 OK` pages with slug similarity + fallback) for `url`.
    ///
    /// # Errors
    /// Returns an error if the scan database cannot be read.
    pub fn redirect_candidates(
        &self,
        id: &str,
        url: &str,
    ) -> Result<Vec<crate::matcher::RedirectCandidate>, FinderError> {
        let (_, _, rows) = self.load_all_rows(id)?;
        Ok(crate::matcher::rank_redirect_candidates(url, &rows, 5))
    }

    /// Updates the suggested 301 redirect target URL for a single URL, a newline-separated list of URLs,
    /// or a wildcard prefix (ending with `*`, e.g. `/kampanya/*`).
    ///
    /// # Errors
    /// Returns an error if the database cannot be updated.
    pub fn update_suggested_target(
        &self,
        id: &str,
        url: &str,
        new_target: &str,
    ) -> Result<(), FinderError> {
        let (dir, _) = self.resolve_scan(id)?;
        let db_path = dir.join(DB_FILE);
        if !db_path.exists() {
            return Err(FinderError::NotFound(id.to_owned()));
        }
        let conn = Connection::open(&db_path).map_err(|e| FinderError::Database(e.to_string()))?;
        let trimmed_target = new_target.trim();
        let trimmed_selector = url.trim();

        if let Some(prefix) = trimmed_selector.strip_suffix('*') {
            let clean_prefix = prefix.trim();
            if !clean_prefix.is_empty() {
                conn.execute(
                    "UPDATE urls SET suggested_target = ?1 WHERE contains(url, ?2) AND category != 'healthy_200'",
                    duckdb::params![trimmed_target, clean_prefix],
                )
                .map_err(|e| FinderError::Database(e.to_string()))?;
            }
            return Ok(());
        }

        for single_url in trimmed_selector
            .lines()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            conn.execute(
                "UPDATE urls SET suggested_target = ?1 WHERE url = ?2",
                duckdb::params![trimmed_target, single_url],
            )
            .map_err(|e| FinderError::Database(e.to_string()))?;
        }
        Ok(())
    }
}

fn write_meta(dir: &Path, meta: &ScanMeta) -> Result<(), FinderError> {
    let path = dir.join(META_FILE);
    let json = serde_json::to_vec_pretty(meta).map_err(|e| FinderError::Database(e.to_string()))?;
    fs::write(&path, json).map_err(|source| FinderError::Io { path, source })
}

fn build_order_by_clause(sort_by: &str) -> &'static str {
    match sort_by.trim() {
        "words_asc" => "word_count ASC, url ASC",
        "words_desc" => "word_count DESC, url ASC",
        "time_desc" => "response_time_ms DESC, url ASC",
        "status_desc" => "final_status DESC, url ASC",
        "hops_desc" => "redirect_hops DESC, url ASC",
        "engines_desc" => "length(engines) - length(replace(engines, ',', '')) DESC, url ASC",
        "url_asc" => "url ASC",
        _ => {
            "CASE category
                WHEN 'error_404' THEN 0
                WHEN 'soft_404' THEN 1
                WHEN 'thin_200' THEN 2
                WHEN 'error_4xx' THEN 3
                WHEN 'error_5xx' THEN 4
                WHEN 'network_error' THEN 5
                ELSE 6
            END ASC, url ASC"
        }
    }
}

fn build_where_clause(req: &FinderPageRequest) -> String {
    let mut clauses = Vec::new();
    match req.category.as_str() {
        "" | "all" => {}
        "issues" => clauses.push("category != 'healthy_200'".to_owned()),
        "thin_soft_404" => clauses.push("category IN ('thin_200', 'soft_404')".to_owned()),
        "redirected" => clauses.push("redirect_hops > 0".to_owned()),
        "noindex" => clauses.push("noindex = TRUE".to_owned()),
        "sitemap_issues" => {
            clauses.push("category != 'healthy_200' AND contains(engines, 'sitemap')".to_owned());
        }
        "orphan_indexed" => clauses.push("NOT contains(engines, 'sitemap')".to_owned()),
        other => {
            let safe = sql_escape(other);
            clauses.push(format!("category = '{safe}'"));
        }
    }
    if !req.engine.trim().is_empty() && req.engine != "all" {
        if req.engine.trim() == "multi" {
            clauses.push("contains(engines, ',')".to_owned());
        } else {
            let safe = sql_escape(req.engine.trim());
            clauses.push(format!("contains(engines, '{safe}')"));
        }
    }
    if !req.search.trim().is_empty() {
        let safe = sql_escape(&req.search.trim().to_lowercase());
        clauses.push(format!(
            "(contains(lower(url), '{safe}') OR contains(lower(serp_title), '{safe}') OR contains(lower(live_title), '{safe}'))"
        ));
    }
    if let Some(max_words) = req.max_words {
        clauses.push(format!("word_count <= {max_words}"));
    }
    if clauses.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", clauses.join(" AND "))
    }
}

fn sql_escape(value: &str) -> String {
    value.replace('\'', "''")
}

fn fetch_verified_rows(conn: &Connection, sql: &str) -> Result<Vec<VerifiedUrl>, FinderError> {
    let mut stmt = conn
        .prepare(sql)
        .map_err(|e| FinderError::Database(e.to_string()))?;
    let mut rows = stmt
        .query([])
        .map_err(|e| FinderError::Database(e.to_string()))?;
    let mut list = Vec::new();
    while let Ok(Some(row)) = rows.next() {
        let initial_i32: i32 = row.get(5).unwrap_or(0);
        let final_i32: i32 = row.get(6).unwrap_or(0);
        let hops_i32: i32 = row.get(7).unwrap_or(0);
        let rt_i64: i64 = row.get(9).unwrap_or(0);
        let wc_i64: i64 = row.get(13).unwrap_or(0);
        let cat_str: String = row.get(16).unwrap_or_default();

        list.push(VerifiedUrl {
            url: row.get(0).unwrap_or_default(),
            engines: row.get(1).unwrap_or_default(),
            serp_title: row.get(2).unwrap_or_default(),
            serp_snippet: row.get(3).unwrap_or_default(),
            found_by_query: row.get(4).unwrap_or_default(),
            initial_status: u16::try_from(initial_i32.max(0)).unwrap_or(0),
            final_status: u16::try_from(final_i32.max(0)).unwrap_or(0),
            redirect_hops: u8::try_from(hops_i32.max(0)).unwrap_or(0),
            final_url: row.get(8).unwrap_or_default(),
            response_time_ms: u32::try_from(rt_i64.max(0)).unwrap_or(0),
            content_type: row.get(10).unwrap_or_default(),
            live_title: row.get(11).unwrap_or_default(),
            h1: row.get(12).unwrap_or_default(),
            word_count: u32::try_from(wc_i64.max(0)).unwrap_or(0),
            canonical: row.get(14).unwrap_or_default(),
            noindex: row.get(15).unwrap_or(false),
            category: FindingCategory::parse(&cat_str),
            reason: row.get(17).unwrap_or_default(),
            suggested_target: row.get(18).unwrap_or_default(),
            error_message: row.get(19).unwrap_or_default(),
        });
    }
    Ok(list)
}
