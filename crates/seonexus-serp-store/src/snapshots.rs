//! Snapshots, their results, features and raw responses.

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use duckdb::{OptionalExt, params};
use seonexus_serp::provider::{Attempt, AttemptOutcome, RawPage, SerpFetch};
use seonexus_serp::{
    Device, Engine, Feature, FeatureItem, FeatureKind, Location, Organic, SerpQuery, SerpSnapshot,
    Sitelink, keyword_key,
};
use serde::{Deserialize, Serialize};

use crate::{SerpStore, SerpStoreError};

/// zstd level of raw responses: fast, and HTML compresses about tenfold anyway.
const RAW_LEVEL: i32 = 6;

/// Which snapshots to list.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotFilter {
    /// Only this keyword (compared by [`keyword_key`] in the query's language).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keyword: Option<String>,
    /// Only this engine.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub engine: Option<Engine>,
    /// Only snapshots of this run.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run_id: Option<i64>,
    /// Most rows returned (0 for 100).
    #[serde(default)]
    pub limit: usize,
}

/// A stored snapshot without its results.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SnapshotSummary {
    /// Snapshot id.
    pub id: i64,
    /// Run it belongs to.
    pub run_id: Option<i64>,
    /// Keyword as searched.
    pub keyword: String,
    /// Engine.
    pub engine: Engine,
    /// Country code.
    pub country: String,
    /// Language code.
    pub language: String,
    /// Device.
    pub device: Device,
    /// Depth asked for.
    pub depth: u16,
    /// Provider id.
    pub provider: String,
    /// Fetch time, RFC 3339.
    pub fetched_at: String,
    /// Organic results stored.
    pub organic_count: u32,
    /// Result count the engine reported.
    pub total_results: Option<u64>,
    /// US dollars.
    pub cost_usd: f64,
}

impl SerpStore {
    /// Stores a fetched snapshot, its raw responses and the attempts of the provider chain
    /// that produced it. Returns the snapshot id.
    ///
    /// # Errors
    /// I/O and database errors.
    pub fn save_fetch(
        &mut self,
        run_id: Option<i64>,
        keyword_id: Option<i64>,
        fetch: &SerpFetch,
        attempts: &[Attempt],
    ) -> Result<i64, SerpStoreError> {
        let snapshot = &fetch.snapshot;
        let query = &snapshot.query;
        let location = query.location.as_ref().map(json).transpose()?;
        let warnings = (!snapshot.warnings.is_empty())
            .then(|| json(&snapshot.warnings))
            .transpose()?;
        let transaction = self.connection.transaction()?;
        let id: i64 = transaction.query_row(
            "INSERT INTO snapshots (run_id, keyword_id, keyword, keyword_key, engine, country,
                 language, location, device, depth, provider, provider_rev, parser_rev, fetched_at,
                 total_results, organic_count, requests, cost_usd, cost_reported, warnings)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) RETURNING id",
            params![
                run_id,
                keyword_id,
                query.keyword,
                keyword_key(&query.keyword, &query.language),
                query.engine.as_str(),
                query.country,
                query.language,
                location,
                query.device.as_str(),
                query.depth,
                snapshot.provider,
                snapshot.provider_rev,
                snapshot.parser_rev,
                snapshot.fetched_at,
                snapshot.total_results,
                u32::try_from(snapshot.organic.len()).unwrap_or(u32::MAX),
                fetch.requests,
                fetch.cost_usd(),
                fetch.reported_cost_usd.is_some(),
                warnings,
            ],
            |row| row.get(0),
        )?;
        append_contents(&transaction, id, snapshot)?;
        write_pages(&transaction, &self.root, id, fetch)?;
        insert_attempts(&transaction, run_id, id, fetch, attempts)?;
        transaction.commit()?;
        Ok(id)
    }

    /// Records the attempts of a chain that returned no snapshot.
    ///
    /// # Errors
    /// Database errors.
    pub fn record_attempts(
        &self,
        run_id: Option<i64>,
        attempts: &[Attempt],
        at: &str,
    ) -> Result<(), SerpStoreError> {
        for attempt in attempts {
            self.connection.execute(
                "INSERT INTO provider_attempts (run_id, provider, attempted_at, outcome, message)
                 VALUES (?, ?, ?, ?, ?)",
                params![
                    run_id,
                    attempt.provider,
                    at,
                    outcome_name(&attempt.outcome),
                    attempt.message
                ],
            )?;
        }
        Ok(())
    }

    /// A stored snapshot.
    ///
    /// # Errors
    /// [`SerpStoreError::NotFound`] for an unknown id; database errors.
    pub fn snapshot(&self, id: i64) -> Result<SerpSnapshot, SerpStoreError> {
        let head = self
            .connection
            .query_row(
                "SELECT keyword, engine, country, language, location, device, depth, provider,
                        provider_rev, parser_rev, fetched_at, total_results, warnings
                 FROM snapshots WHERE id = ?",
                params![id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, Option<String>>(4)?,
                        row.get::<_, String>(5)?,
                        row.get::<_, u16>(6)?,
                        row.get::<_, String>(7)?,
                        row.get::<_, Option<String>>(8)?,
                        row.get::<_, Option<String>>(9)?,
                        row.get::<_, String>(10)?,
                        row.get::<_, Option<u64>>(11)?,
                        row.get::<_, Option<String>>(12)?,
                    ))
                },
            )
            .optional()?
            .ok_or_else(|| SerpStoreError::NotFound(format!("snapshot {id}")))?;
        let (
            keyword,
            engine,
            country,
            language,
            location,
            device,
            depth,
            provider,
            provider_rev,
            parser_rev,
            fetched_at,
            total_results,
            warnings,
        ) = head;
        let query = SerpQuery {
            engine: parse_enum(&engine)?,
            keyword,
            country,
            language,
            location: location.as_deref().map(from_json::<Location>).transpose()?,
            device: parse_enum(&device)?,
            depth,
        };
        Ok(SerpSnapshot {
            query,
            provider,
            provider_rev,
            parser_rev,
            fetched_at,
            total_results,
            organic: self.results(id)?,
            features: self.features(id)?,
            warnings: warnings
                .as_deref()
                .map(from_json::<Vec<String>>)
                .transpose()?
                .unwrap_or_default(),
        })
    }

    fn results(&self, id: i64) -> Result<Vec<Organic>, SerpStoreError> {
        let mut statement = self.connection.prepare(
            "SELECT position, absolute_position, page, url, host, domain, title, snippet,
                    displayed_url, date, sitelinks, coalesce(site_only, false)
             FROM results WHERE snapshot_id = ? ORDER BY position",
        )?;
        let rows = statement.query_map(params![id], |row| {
            Ok((
                Organic {
                    position: row.get(0)?,
                    absolute_position: row.get(1)?,
                    page: row.get(2)?,
                    url: row.get(3)?,
                    host: row.get(4)?,
                    domain: row.get(5)?,
                    title: row.get(6)?,
                    snippet: row.get(7)?,
                    displayed_url: row.get(8)?,
                    date: row.get(9)?,
                    sitelinks: Vec::new(),
                    site_only: row.get(11)?,
                },
                row.get::<_, Option<String>>(10)?,
            ))
        })?;
        let mut results = Vec::new();
        for row in rows {
            let (mut result, sitelinks) = row?;
            if let Some(sitelinks) = sitelinks {
                result.sitelinks = from_json::<Vec<Sitelink>>(&sitelinks)?;
            }
            results.push(result);
        }
        Ok(results)
    }

    fn features(&self, id: i64) -> Result<Vec<Feature>, SerpStoreError> {
        let mut statement = self.connection.prepare(
            "SELECT kind, page, absolute_position, items FROM features
             WHERE snapshot_id = ? ORDER BY ordinal",
        )?;
        let rows = statement.query_map(params![id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, u16>(1)?,
                row.get::<_, Option<u16>>(2)?,
                row.get::<_, Option<String>>(3)?,
            ))
        })?;
        let mut features = Vec::new();
        for row in rows {
            let (kind, page, absolute_position, items) = row?;
            features.push(Feature {
                kind: kind
                    .parse::<FeatureKind>()
                    .map_err(|error| SerpStoreError::Corrupt(error.to_string()))?,
                absolute_position,
                page,
                items: items
                    .as_deref()
                    .map(from_json::<Vec<FeatureItem>>)
                    .transpose()?
                    .unwrap_or_default(),
            });
        }
        Ok(features)
    }

    /// Stored snapshots, newest first.
    ///
    /// # Errors
    /// Database errors.
    pub fn snapshots(
        &self,
        filter: &SnapshotFilter,
    ) -> Result<Vec<SnapshotSummary>, SerpStoreError> {
        let limit = if filter.limit == 0 { 100 } else { filter.limit };
        let mut statement = self.connection.prepare(
            "SELECT id, run_id, keyword, engine, country, language, device, depth, provider,
                    fetched_at, organic_count, total_results, cost_usd, keyword_key
             FROM snapshots
             WHERE (?::VARCHAR IS NULL OR engine = ?)
               AND (?::BIGINT IS NULL OR run_id = ?)
             ORDER BY fetched_at DESC, id DESC",
        )?;
        let engine = filter.engine.map(Engine::as_str);
        let rows = statement.query_map(
            params![engine, engine, filter.run_id, filter.run_id],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, Option<i64>>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, u16>(7)?,
                    row.get::<_, String>(8)?,
                    row.get::<_, String>(9)?,
                    row.get::<_, u32>(10)?,
                    row.get::<_, Option<u64>>(11)?,
                    row.get::<_, f64>(12)?,
                    row.get::<_, String>(13)?,
                ))
            },
        )?;
        let mut summaries = Vec::new();
        for row in rows {
            let row = row?;
            if let Some(keyword) = &filter.keyword
                && keyword_key(keyword, &row.5) != row.13
            {
                continue;
            }
            summaries.push(SnapshotSummary {
                id: row.0,
                run_id: row.1,
                keyword: row.2,
                engine: parse_enum(&row.3)?,
                country: row.4,
                language: row.5,
                device: parse_enum(&row.6)?,
                depth: row.7,
                provider: row.8,
                fetched_at: row.9,
                organic_count: row.10,
                total_results: row.11,
                cost_usd: row.12,
            });
            if summaries.len() >= limit {
                break;
            }
        }
        Ok(summaries)
    }

    /// Raw responses of a snapshot, page by page.
    ///
    /// # Errors
    /// I/O and database errors.
    pub fn raw_pages(&self, id: i64) -> Result<Vec<RawPage>, SerpStoreError> {
        let mut statement = self.connection.prepare(
            "SELECT page, url, status, content_type, path FROM raw_pages
             WHERE snapshot_id = ? ORDER BY page",
        )?;
        let rows = statement.query_map(params![id], |row| {
            Ok((
                row.get::<_, u16>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, u16>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, String>(4)?,
            ))
        })?;
        let mut pages = Vec::new();
        for row in rows {
            let (page, url, status, content_type, path) = row?;
            pages.push(RawPage {
                page,
                url,
                status,
                content_type,
                body: read_raw(&self.root.join(path))?,
            });
        }
        Ok(pages)
    }

    /// Replaces the parsed results and features of a snapshot (read again from its raw
    /// responses with newer selectors). Raw responses and costs stay as they were.
    ///
    /// # Errors
    /// [`SerpStoreError::NotFound`] for an unknown id; database errors.
    pub fn replace_contents(
        &mut self,
        id: i64,
        snapshot: &SerpSnapshot,
    ) -> Result<(), SerpStoreError> {
        let warnings = (!snapshot.warnings.is_empty())
            .then(|| json(&snapshot.warnings))
            .transpose()?;
        let transaction = self.connection.transaction()?;
        let updated = transaction.execute(
            "UPDATE snapshots SET parser_rev = ?, total_results = ?, organic_count = ?, warnings = ?
             WHERE id = ?",
            params![
                snapshot.parser_rev,
                snapshot.total_results,
                u32::try_from(snapshot.organic.len()).unwrap_or(u32::MAX),
                warnings,
                id,
            ],
        )?;
        if updated == 0 {
            return Err(SerpStoreError::NotFound(format!("snapshot {id}")));
        }
        for table in ["results", "features"] {
            transaction.execute(
                &format!("DELETE FROM {table} WHERE snapshot_id = ?"),
                params![id],
            )?;
        }
        append_contents(&transaction, id, snapshot)?;
        transaction.commit()?;
        Ok(())
    }

    /// Deletes a snapshot with its results, features and raw responses.
    ///
    /// # Errors
    /// I/O and database errors.
    pub fn delete_snapshot(&mut self, id: i64) -> Result<(), SerpStoreError> {
        let paths: Vec<String> = {
            let mut statement = self
                .connection
                .prepare("SELECT path FROM raw_pages WHERE snapshot_id = ?")?;
            statement
                .query_map(params![id], |row| row.get(0))?
                .collect::<Result<_, _>>()?
        };
        let transaction = self.connection.transaction()?;
        for table in ["results", "features", "raw_pages"] {
            transaction.execute(
                &format!("DELETE FROM {table} WHERE snapshot_id = ?"),
                params![id],
            )?;
        }
        transaction.execute(
            "UPDATE provider_attempts SET snapshot_id = NULL WHERE snapshot_id = ?",
            params![id],
        )?;
        let deleted = transaction.execute("DELETE FROM snapshots WHERE id = ?", params![id])?;
        transaction.commit()?;
        if deleted == 0 {
            return Err(SerpStoreError::NotFound(format!("snapshot {id}")));
        }
        for path in paths {
            let path = self.root.join(path);
            match fs::remove_file(&path) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(source) => return Err(SerpStoreError::Io { path, source }),
            }
        }
        Ok(())
    }
}

/// Organic results and features of a new snapshot.
fn append_contents(
    connection: &duckdb::Connection,
    id: i64,
    snapshot: &SerpSnapshot,
) -> Result<(), SerpStoreError> {
    let mut results = connection.appender("results")?;
    for result in &snapshot.organic {
        let sitelinks = (!result.sitelinks.is_empty())
            .then(|| json(&result.sitelinks))
            .transpose()?;
        results.append_row(params![
            id,
            result.position,
            result.absolute_position,
            result.page,
            result.url,
            result.host,
            result.domain,
            result.title,
            result.snippet,
            result.displayed_url,
            result.date,
            sitelinks,
            result.site_only,
        ])?;
    }
    let mut features = connection.appender("features")?;
    for (ordinal, feature) in snapshot.features.iter().enumerate() {
        let items = (!feature.items.is_empty())
            .then(|| json(&feature.items))
            .transpose()?;
        features.append_row(params![
            id,
            u16::try_from(ordinal).unwrap_or(u16::MAX),
            feature.kind.as_str(),
            feature.page,
            feature.absolute_position,
            items,
        ])?;
    }
    Ok(())
}

/// Raw responses of a new snapshot: compressed files and their rows.
fn write_pages(
    connection: &duckdb::Connection,
    root: &Path,
    id: i64,
    fetch: &SerpFetch,
) -> Result<(), SerpStoreError> {
    let month = fetch.snapshot.fetched_at.get(..7).unwrap_or("unknown");
    for page in &fetch.pages {
        let relative = PathBuf::from("raw")
            .join(month)
            .join(format!("{id}-{}.zst", page.page));
        write_raw(&root.join(&relative), &page.body)?;
        connection.execute(
            "INSERT INTO raw_pages VALUES (?, ?, ?, ?, ?, ?, ?)",
            params![
                id,
                page.page,
                page.url,
                page.status,
                page.content_type,
                relative.to_string_lossy(),
                i64::try_from(page.body.len()).unwrap_or(i64::MAX),
            ],
        )?;
    }
    Ok(())
}

/// The chain's attempts; the successful one carries the requests and cost.
fn insert_attempts(
    connection: &duckdb::Connection,
    run_id: Option<i64>,
    id: i64,
    fetch: &SerpFetch,
    attempts: &[Attempt],
) -> Result<(), SerpStoreError> {
    for attempt in attempts {
        let succeeded = attempt.outcome == AttemptOutcome::Success
            && attempt.provider == fetch.snapshot.provider;
        connection.execute(
            "INSERT INTO provider_attempts
                 (run_id, snapshot_id, provider, attempted_at, outcome, message, requests,
                  cost_usd, cost_reported)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                run_id,
                succeeded.then_some(id),
                attempt.provider,
                fetch.snapshot.fetched_at,
                outcome_name(&attempt.outcome),
                attempt.message,
                if succeeded { fetch.requests } else { 0 },
                if succeeded { fetch.cost_usd() } else { 0.0 },
                succeeded && fetch.reported_cost_usd.is_some(),
            ],
        )?;
    }
    Ok(())
}

fn outcome_name(outcome: &AttemptOutcome) -> &str {
    match outcome {
        AttemptOutcome::Success => "success",
        AttemptOutcome::Skipped => "skipped",
        AttemptOutcome::Failed(code) => code,
    }
}

fn json<T: Serialize + ?Sized>(value: &T) -> Result<String, SerpStoreError> {
    serde_json::to_string(value).map_err(|error| SerpStoreError::Corrupt(error.to_string()))
}

fn from_json<T: for<'de> Deserialize<'de>>(text: &str) -> Result<T, SerpStoreError> {
    serde_json::from_str(text).map_err(|error| SerpStoreError::Corrupt(error.to_string()))
}

fn parse_enum<T: std::str::FromStr>(text: &str) -> Result<T, SerpStoreError>
where
    T::Err: std::fmt::Display,
{
    text.parse()
        .map_err(|error: T::Err| SerpStoreError::Corrupt(error.to_string()))
}

fn write_raw(path: &Path, body: &[u8]) -> Result<(), SerpStoreError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(SerpStoreError::io(parent))?;
    }
    let compressed = zstd::encode_all(body, RAW_LEVEL).map_err(SerpStoreError::io(path))?;
    fs::write(path, compressed).map_err(SerpStoreError::io(path))
}

fn read_raw(path: &Path) -> Result<Vec<u8>, SerpStoreError> {
    let file = fs::File::open(path).map_err(SerpStoreError::io(path))?;
    let mut decoder = zstd::Decoder::new(file).map_err(SerpStoreError::io(path))?;
    let mut body = Vec::new();
    decoder
        .read_to_end(&mut body)
        .map_err(SerpStoreError::io(path))?;
    Ok(body)
}
