//! The directory of saved crawls.

use std::fs;
use std::path::{Path, PathBuf};

use seonexus_core::{Date, Timestamp};
use seonexus_crawl::{CrawlConfig, CrawlMode, CrawlPlan, CrawlProgress};
use serde::{Deserialize, Serialize};

use crate::error::CrawlStoreError;
use crate::reader::CrawlReader;
use crate::writer::CrawlWriter;

const QUALIFIER: &str = "com";
const ORGANIZATION: &str = "octopusSEO";
const APPLICATION: &str = "octopusSEO Spider";
const META_FILE: &str = "crawl.json";
const DATABASE_FILE: &str = "crawl.duckdb";

/// Name, settings and state of a saved crawl (`crawl.json`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CrawlMeta {
    /// Directory name, e.g. `20261016-142501-example-com`.
    pub id: String,
    /// Display name: the first start host.
    pub name: String,
    /// Start URLs (spider mode) or the list (list mode).
    pub start_urls: Vec<String>,
    /// Spider or list mode.
    pub mode: CrawlMode,
    /// Milliseconds since the Unix epoch.
    pub created_at_ms: i64,
    /// When the crawl ended.
    pub finished_at_ms: Option<i64>,
    /// Running, completed, stopped or failed.
    pub status: CrawlStatus,
    /// Latest counters.
    pub progress: CrawlProgress,
    /// The URL limit ended the crawl early.
    #[serde(default)]
    pub limit_reached: bool,
    /// Why the crawl failed.
    pub error: Option<String>,
    /// The settings used.
    pub config: CrawlConfig,
}

/// The state of a saved crawl.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CrawlStatus {
    /// Still writing (or the process ended without finishing it).
    Running,
    /// The frontier was exhausted or the URL limit reached.
    Completed,
    /// Stopped by the user.
    Stopped,
    /// Ended by an error.
    Failed,
}

/// The directory holding every crawl.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrawlLibrary {
    crawls: PathBuf,
}

impl CrawlLibrary {
    /// Crawls stored under `root/crawls`.
    pub fn new(root: impl AsRef<Path>) -> Self {
        Self {
            crawls: root.as_ref().join("crawls"),
        }
    }

    /// The platform data directory, e.g.
    /// `~/Library/Application Support/com.octopusSEO.octopusSEO-Spider`.
    ///
    /// # Errors
    /// [`CrawlStoreError::NoDataDir`] when the platform reports no home directory.
    pub fn platform_default() -> Result<Self, CrawlStoreError> {
        let dirs = directories::ProjectDirs::from(QUALIFIER, ORGANIZATION, APPLICATION)
            .ok_or(CrawlStoreError::NoDataDir)?;
        Ok(Self::new(dirs.data_dir()))
    }

    /// The `crawls` directory.
    pub fn path(&self) -> &Path {
        &self.crawls
    }

    /// Starts a new crawl of `plan`.
    ///
    /// # Errors
    /// The directory or database cannot be created.
    pub fn create(&self, plan: &CrawlPlan) -> Result<CrawlWriter, CrawlStoreError> {
        let config = plan.config().clone();
        let name = config
            .start_urls
            .iter()
            .find_map(|raw| host_of(raw))
            .unwrap_or_else(|| "crawl".to_owned());
        let now = Timestamp::now();
        let base = format!("{}-{}", stamp(now), slug(&name));
        fs::create_dir_all(&self.crawls).map_err(CrawlStoreError::io(&self.crawls))?;
        let mut attempt = 1;
        let (id, dir) = loop {
            let id = if attempt == 1 {
                base.clone()
            } else {
                format!("{base}-{attempt}")
            };
            let dir = self.crawls.join(&id);
            // Reserve atomically: simultaneous crawls of the same host must never share a DB.
            match fs::create_dir(&dir) {
                Ok(()) => break (id, dir),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => attempt += 1,
                Err(error) => return Err(CrawlStoreError::io(&dir)(error)),
            }
        };
        let meta = CrawlMeta {
            id,
            name,
            start_urls: config.start_urls.clone(),
            mode: config.mode,
            created_at_ms: now.as_unix_micros() / 1000,
            finished_at_ms: None,
            status: CrawlStatus::Running,
            progress: CrawlProgress::default(),
            limit_reached: false,
            error: None,
            config,
        };
        CrawlWriter::create(
            &dir.join(DATABASE_FILE),
            dir.join(META_FILE),
            meta,
            plan.clone(),
        )
    }

    /// Every crawl, newest first. Directories without readable metadata are skipped.
    ///
    /// # Errors
    /// The directory cannot be listed.
    pub fn list(&self) -> Result<Vec<CrawlMeta>, CrawlStoreError> {
        if !self.crawls.exists() {
            return Ok(Vec::new());
        }
        let entries = fs::read_dir(&self.crawls).map_err(CrawlStoreError::io(&self.crawls))?;
        let mut crawls: Vec<CrawlMeta> = entries
            .filter_map(Result::ok)
            .filter_map(|entry| read_meta(&entry.path().join(META_FILE)).ok())
            .collect();
        crawls.sort_by(|a, b| b.created_at_ms.cmp(&a.created_at_ms).then(b.id.cmp(&a.id)));
        Ok(crawls)
    }

    /// The metadata of crawl `id`.
    ///
    /// # Errors
    /// [`CrawlStoreError::NotFound`] for unknown or invalid identifiers.
    pub fn meta(&self, id: &str) -> Result<CrawlMeta, CrawlStoreError> {
        read_meta(&self.dir(id)?.join(META_FILE))
    }

    /// Opens crawl `id` read-only.
    ///
    /// # Errors
    /// Unknown crawl or database errors.
    pub fn open(&self, id: &str) -> Result<CrawlReader, CrawlStoreError> {
        CrawlReader::open(&self.dir(id)?.join(DATABASE_FILE))
    }

    /// Opens a finished archive without migrating or refreshing any saved analysis.
    ///
    /// # Errors
    /// Running, unknown or incompatible archive; database errors.
    pub fn open_archive(&self, id: &str) -> Result<CrawlReader, CrawlStoreError> {
        if self.meta(id)?.status == CrawlStatus::Running {
            return Err(CrawlStoreError::InvalidSelection(
                "finish the crawl before using its archive".into(),
            ));
        }
        let connection = crate::reader::connect(&self.dir(id)?.join(DATABASE_FILE), true)?;
        let version: i64 = connection.query_row(
            "SELECT coalesce(max(version),0) FROM schema_migrations",
            [],
            |r| r.get(0),
        )?;
        let pending: bool = connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM crawl_info WHERE key='audit_required' AND value='1')",
            [],
            |r| r.get(0),
        )?;
        if !crate::schema::archive_compatible(version) || pending {
            return Err(CrawlStoreError::InvalidSelection(
                "open the finished crawl in the explorer to update its analysis first".into(),
            ));
        }
        Ok(CrawlReader::from_connection(connection))
    }

    /// Deletes crawl `id` with all its data.
    ///
    /// # Errors
    /// Unknown crawl or file system errors.
    pub fn delete(&self, id: &str) -> Result<(), CrawlStoreError> {
        let dir = self.dir(id)?;
        fs::remove_dir_all(&dir).map_err(CrawlStoreError::io(&dir))
    }

    pub(crate) fn dir(&self, id: &str) -> Result<PathBuf, CrawlStoreError> {
        let valid = !id.is_empty()
            && !id.starts_with('.')
            && id
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'));
        let dir = self.crawls.join(id);
        if valid && dir.join(META_FILE).is_file() {
            Ok(dir)
        } else {
            Err(CrawlStoreError::NotFound(id.to_owned()))
        }
    }
}

pub(crate) fn read_meta(path: &Path) -> Result<CrawlMeta, CrawlStoreError> {
    let text = fs::read_to_string(path).map_err(CrawlStoreError::io(path))?;
    serde_json::from_str(&text).map_err(|source| CrawlStoreError::Meta {
        path: path.to_owned(),
        source,
    })
}

/// Writes `crawl.json` atomically (temporary file, then rename).
pub(crate) fn write_meta(path: &Path, meta: &CrawlMeta) -> Result<(), CrawlStoreError> {
    let text = serde_json::to_string_pretty(meta).map_err(|source| CrawlStoreError::Meta {
        path: path.to_owned(),
        source,
    })?;
    let temporary = path.with_extension("json.tmp");
    fs::write(&temporary, text).map_err(CrawlStoreError::io(&temporary))?;
    fs::rename(&temporary, path).map_err(CrawlStoreError::io(path))
}

fn host_of(raw: &str) -> Option<String> {
    let rest = raw
        .trim()
        .split_once("://")
        .map_or(raw.trim(), |(_, rest)| rest);
    let host = rest.split(['/', '?', '#']).next()?.rsplit('@').next()?;
    let host = host.split(':').next()?.to_ascii_lowercase();
    (!host.is_empty()).then(|| host.strip_prefix("www.").unwrap_or(&host).to_owned())
}

/// `example.com` → `example-com`.
fn slug(name: &str) -> String {
    let slug: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .take(60)
        .collect();
    slug.trim_matches('-').to_owned()
}

/// `20261016-142501` (UTC).
fn stamp(now: Timestamp) -> String {
    let (year, month, day) = Date::of(now).ymd();
    let seconds = (now.as_unix_micros() / 1_000_000).rem_euclid(86_400);
    let (hour, minute, second) = (seconds / 3600, seconds / 60 % 60, seconds % 60);
    format!("{year:04}{month:02}{day:02}-{hour:02}{minute:02}{second:02}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_and_ids() {
        assert_eq!(
            host_of("https://www.Example.com:8080/a?b").as_deref(),
            Some("example.com")
        );
        assert_eq!(host_of("example.com/blog").as_deref(), Some("example.com"));
        assert_eq!(slug("shop.example.com"), "shop-example-com");
        assert_eq!(
            stamp(Timestamp::from_unix_micros(1_760_624_701_000_000)),
            "20251016-142501"
        );
    }
}
