//! Crawler IP lists: the bundled snapshot, downloaded updates and their status.

use std::fs;
use std::io;
use std::path::Path;
use std::time::Duration;

use octopuson_botverify::CustomBot;
use octopuson_botverify::custom::url_source_id;
use octopuson_botverify::ranges::{RangeSnapshot, SOURCES, parse_prefix_list};
use octopuson_core::Timestamp;
use octopuson_net::HttpClient;
use serde::Serialize;

use super::BotError;
use super::custom::custom_bots_or_none;
use crate::USER_AGENT;
use crate::workspace::Workspace;

/// Lists older than this trigger a warning.
const STALE_AFTER_DAYS: i64 = 30;
const SECONDS_PER_DAY: i64 = 86_400;
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(30);

/// The bundled snapshot overridden by downloaded lists where those are newer.
pub(crate) fn load_snapshot(workspace: &Workspace) -> RangeSnapshot {
    let bundled = RangeSnapshot::bundled();
    let snapshot = match read_local(&workspace.data_dir().bot_ranges_file()) {
        Some(local) => bundled.merged_with(local),
        None => bundled,
    };
    if let Some(age) = snapshot.oldest_fetch().map(age_days)
        && age > STALE_AFTER_DAYS
    {
        tracing::warn!(age_days = age, "crawler IP lists are outdated; update them");
    }
    snapshot
}

fn read_local(path: &Path) -> Option<RangeSnapshot> {
    match fs::read_to_string(path) {
        Ok(text) => match RangeSnapshot::from_json(&text) {
            Ok(snapshot) => Some(snapshot),
            Err(error) => {
                tracing::warn!(%error, path = %path.display(), "ignoring unreadable IP lists");
                None
            }
        },
        Err(error) if error.kind() == io::ErrorKind::NotFound => None,
        Err(error) => {
            tracing::warn!(%error, path = %path.display(), "ignoring unreadable IP lists");
            None
        }
    }
}

fn age_days(fetched_at: i64) -> i64 {
    (Timestamp::now().as_unix_micros() / 1_000_000 - fetched_at) / SECONDS_PER_DAY
}

/// State of one IP list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RangeSourceStatus {
    /// Source id (`custom-url:<bot>` for custom bots).
    pub id: String,
    /// Operator (the bot's name for custom bots).
    pub operator: String,
    /// Download URL.
    pub url: String,
    /// Prefixes in use.
    pub prefixes: usize,
    /// When the list in use was downloaded (RFC 3339).
    pub fetched_at: Option<String>,
    /// Age of the list in days.
    pub age_days: Option<i64>,
    /// `downloaded`, `bundled` or `missing`.
    pub origin: &'static str,
}

/// Status of every known IP list, custom bots' lists last.
pub fn bot_range_status(workspace: &Workspace) -> Vec<RangeSourceStatus> {
    let bundled = RangeSnapshot::bundled();
    let local = read_local(&workspace.data_dir().bot_ranges_file()).unwrap_or_default();
    let merged = bundled.clone().merged_with(local.clone());
    let status = |id: String, operator: &str, url: &str| {
        let ranges = merged.sources.get(&id);
        let origin = match (local.sources.get(&id), ranges) {
            (_, None) => "missing",
            (Some(l), Some(m)) if l.fetched_at == m.fetched_at => "downloaded",
            _ => "bundled",
        };
        RangeSourceStatus {
            prefixes: ranges.map_or(0, |r| r.prefixes.len()),
            fetched_at: ranges.map(|r| unix_to_rfc3339(r.fetched_at)),
            age_days: ranges.map(|r| age_days(r.fetched_at)),
            origin,
            id,
            operator: operator.to_owned(),
            url: url.to_owned(),
        }
    };
    let builtin = SOURCES
        .iter()
        .map(|source| status(source.id.to_owned(), source.operator, source.url));
    let custom = custom_bots_or_none(workspace)
        .into_iter()
        .filter_map(|bot| {
            let url = bot.ip_list_url.clone()?;
            Some(status(url_source_id(&bot.id), &bot.name, &url))
        });
    builtin.chain(custom).collect()
}

/// A list that could not be downloaded.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SourceFailure {
    /// Source id.
    pub id: String,
    /// What went wrong.
    pub error: String,
}

/// Result of downloading every list.
#[derive(Debug, Clone, Default)]
pub struct RangeDownload {
    /// Lists downloaded successfully.
    pub snapshot: RangeSnapshot,
    /// Lists that failed.
    pub failures: Vec<SourceFailure>,
}

/// Downloads every known IP list concurrently.
pub fn download_ranges() -> RangeDownload {
    let client = HttpClient::new(USER_AGENT, DOWNLOAD_TIMEOUT);
    let now = Timestamp::now().as_unix_micros() / 1_000_000;
    let results: Vec<_> = std::thread::scope(|scope| {
        let handles: Vec<_> = SOURCES
            .iter()
            .map(|source| {
                let client = &client;
                scope.spawn(move || {
                    let prefixes = client
                        .get_text(source.url)
                        .map_err(|e| e.to_string())
                        .and_then(|text| parse_prefix_list(&text).map_err(|e| e.to_string()));
                    (source, prefixes)
                })
            })
            .collect();
        handles
            .into_iter()
            .filter_map(|handle| handle.join().ok())
            .collect()
    });
    let mut download = RangeDownload::default();
    for (source, result) in results {
        match result {
            Ok(prefixes) => download.snapshot.insert(source, now, &prefixes),
            Err(error) => download.failures.push(SourceFailure {
                id: source.id.to_owned(),
                error,
            }),
        }
    }
    download
}

/// Downloads the IP lists of custom bots that publish one.
pub fn download_custom_ranges(bots: &[CustomBot]) -> RangeDownload {
    let client = HttpClient::new(USER_AGENT, DOWNLOAD_TIMEOUT);
    let now = Timestamp::now().as_unix_micros() / 1_000_000;
    let mut download = RangeDownload::default();
    for bot in bots {
        let Some(url) = &bot.ip_list_url else {
            continue;
        };
        let result = client
            .get_text(url)
            .map_err(|e| e.to_string())
            .and_then(|text| parse_prefix_list(&text).map_err(|e| e.to_string()));
        let id = url_source_id(&bot.id);
        match result {
            Ok(prefixes) => download
                .snapshot
                .insert_custom(id, url.clone(), now, &prefixes),
            Err(error) => download.failures.push(SourceFailure { id, error }),
        }
    }
    download
}

/// Result of [`update_bot_ranges`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RangeUpdateReport {
    /// Written file.
    pub path: String,
    /// Sources updated.
    pub updated: Vec<String>,
    /// Sources that failed (their previous lists stay in use).
    pub failed: Vec<SourceFailure>,
}

/// Downloads current crawler IP lists into the data directory.
///
/// # Errors
/// [`BotError::Write`] when the file cannot be written. Failed downloads are reported, not
/// errors; their previous lists remain in use.
pub fn update_bot_ranges(workspace: &Workspace) -> Result<RangeUpdateReport, BotError> {
    let path = workspace.data_dir().bot_ranges_file();
    let mut download = download_ranges();
    let custom = download_custom_ranges(&custom_bots_or_none(workspace));
    download.snapshot = download.snapshot.merged_with(custom.snapshot);
    download.failures.extend(custom.failures);
    let updated: Vec<String> = download.snapshot.sources.keys().cloned().collect();
    let local = read_local(&path).unwrap_or_default();
    let merged = local.merged_with(download.snapshot);
    write_atomically(&path, &merged.to_json()).map_err(|source| BotError::Write {
        path: path.clone(),
        source,
    })?;
    Ok(RangeUpdateReport {
        path: path.display().to_string(),
        updated,
        failed: download.failures,
    })
}

pub(super) fn write_atomically(path: &Path, contents: &str) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let partial = path.with_extension("json.partial");
    fs::write(&partial, contents)?;
    fs::rename(&partial, path)
}

fn unix_to_rfc3339(seconds: i64) -> String {
    Timestamp::from_unix_micros(seconds.saturating_mul(1_000_000)).to_string()
}
