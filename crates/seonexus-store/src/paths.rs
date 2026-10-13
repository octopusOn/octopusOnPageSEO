//! On-disk layout of the data directory.
//!
//! ```text
//! <root>/catalog.duckdb                       clients and sites
//! <root>/bot-ranges.json                      downloaded crawler IP lists
//! <root>/sites/<site-id>/site.duckdb          dimensions, rollups, import history
//! <root>/sites/<site-id>/.lock                single-writer lock
//! <root>/sites/<site-id>/hits/month=YYYY-MM/  immutable Parquet hit files
//! <root>/sites/<site-id>/hits/.staging-<id>/  files of an import not yet published
//! ```

use std::path::{Path, PathBuf};

use crate::catalog::SiteId;
use crate::error::StoreError;

const QUALIFIER: &str = "com";
const ORGANIZATION: &str = "octopusSEO";
const APPLICATION: &str = "octopusSEO Log Analyser";
/// Qualifier, organization and application of the default location before the product was
/// renamed to octopusSEO.
const LEGACY: (&str, &str, &str) = ("io", "SEONexus", "SEONexus Log Analyser");
const STAGING_PREFIX: &str = ".staging-";

/// Root of all persisted application data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataDir {
    root: PathBuf,
}

impl DataDir {
    /// Uses an explicit directory.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// The platform default, e.g. `~/Library/Application Support/com.octopusSEO.octopusSEO-Log-Analyser`.
    /// Installations that predate the rename keep using their existing directory.
    ///
    /// # Errors
    /// [`StoreError::NoDataDir`] when the platform reports no home directory.
    pub fn platform_default() -> Result<Self, StoreError> {
        let current = directories::ProjectDirs::from(QUALIFIER, ORGANIZATION, APPLICATION)
            .ok_or(StoreError::NoDataDir)?;
        let (qualifier, organization, application) = LEGACY;
        let legacy = directories::ProjectDirs::from(qualifier, organization, application);
        Ok(Self::new(preferred_root(
            current.data_dir(),
            legacy.as_ref().map(directories::ProjectDirs::data_dir),
        )))
    }

    /// Root directory.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Downloaded crawler IP lists (overrides the bundled snapshot per source).
    pub fn bot_ranges_file(&self) -> PathBuf {
        self.root.join("bot-ranges.json")
    }

    /// Downloaded IP geolocation data (country and ASN).
    pub fn geo_dir(&self) -> PathBuf {
        self.root.join("geo")
    }

    /// User-defined bots (shared by the CLI and the desktop app).
    pub fn custom_bots_file(&self) -> PathBuf {
        self.root.join("custom-bots.json")
    }

    pub(crate) fn catalog_db(&self) -> PathBuf {
        self.root.join("catalog.duckdb")
    }

    /// Where a file of a site archive goes when restoring site `id`: only the database
    /// (`site.duckdb`) and published hit files (`hits/month=YYYY-MM/<name>.parquet`) are
    /// accepted, so archive entries cannot escape the site directory.
    pub fn restore_path(&self, id: SiteId, relative: &str) -> Option<PathBuf> {
        let site = self.site(id);
        if relative == "site.duckdb" {
            return Some(site.database());
        }
        let rest = relative.strip_prefix("hits/month=")?;
        let (month, file) = rest.split_once('/')?;
        let valid_month = month.len() == 7
            && month.as_bytes()[4] == b'-'
            && month
                .bytes()
                .enumerate()
                .all(|(i, b)| i == 4 || b.is_ascii_digit());
        let valid_file = file.ends_with(".parquet")
            && file
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.'))
            && !file.starts_with('.');
        (valid_month && valid_file).then(|| site.month(month).join(file))
    }

    pub(crate) fn site(&self, id: SiteId) -> SitePaths {
        SitePaths {
            dir: self.root.join("sites").join(id.to_string()),
        }
    }
}

/// Paths inside one site directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SitePaths {
    dir: PathBuf,
}

impl SitePaths {
    pub(crate) fn dir(&self) -> &Path {
        &self.dir
    }

    pub(crate) fn database(&self) -> PathBuf {
        self.dir.join("site.duckdb")
    }

    pub(crate) fn lock_file(&self) -> PathBuf {
        self.dir.join(".lock")
    }

    pub(crate) fn hits(&self) -> PathBuf {
        self.dir.join("hits")
    }

    pub(crate) fn month(&self, month: &str) -> PathBuf {
        self.hits().join(format!("month={month}"))
    }

    pub(crate) fn staging(&self, import_id: i64) -> PathBuf {
        self.hits().join(format!("{STAGING_PREFIX}{import_id}"))
    }

    /// Parses the import id from a staging directory name.
    pub(crate) fn staging_import_id(name: &str) -> Option<i64> {
        name.strip_prefix(STAGING_PREFIX)?.parse().ok()
    }
}

/// `current`, unless only the `legacy` directory exists: its data stays where it is rather
/// than being moved behind the user's back.
fn preferred_root(current: &Path, legacy: Option<&Path>) -> PathBuf {
    match legacy {
        Some(legacy) if !current.exists() && legacy.exists() => legacy.to_path_buf(),
        _ => current.to_path_buf(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn site_layout() {
        let data = DataDir::new("/data");
        let site = data.site(SiteId::new(7));
        assert_eq!(site.database(), Path::new("/data/sites/7/site.duckdb"));
        assert_eq!(
            site.month("2026-10"),
            Path::new("/data/sites/7/hits/month=2026-10")
        );
        assert_eq!(site.staging(3), Path::new("/data/sites/7/hits/.staging-3"));
    }

    #[test]
    fn staging_names_round_trip() {
        assert_eq!(SitePaths::staging_import_id(".staging-42"), Some(42));
        assert_eq!(SitePaths::staging_import_id("month=2026-10"), None);
        assert_eq!(SitePaths::staging_import_id(".staging-x"), None);
    }

    #[test]
    fn existing_data_survives_the_rename() {
        let temp = tempfile::tempdir().unwrap();
        let current = temp.path().join("octopusSEO");
        let legacy = temp.path().join("SEONexus");

        // Fresh installation: the new location.
        assert_eq!(preferred_root(&current, Some(&legacy)), current);
        // Data only in the old location: keep using it.
        std::fs::create_dir(&legacy).unwrap();
        assert_eq!(preferred_root(&current, Some(&legacy)), legacy);
        // Once the new location exists it wins.
        std::fs::create_dir(&current).unwrap();
        assert_eq!(preferred_root(&current, Some(&legacy)), current);
        assert_eq!(preferred_root(&current, None), current);
    }
}
