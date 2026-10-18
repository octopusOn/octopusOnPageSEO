//! IP geolocation data (DB-IP Lite country and ASN databases): download, status and the
//! enrichment of sites' client IPs.
//!
//! The data is downloaded only on request (`octopuson-log geo update`, desktop settings) into
//! `<data>/geo/`; lookups are local. DB-IP's licence (CC BY 4.0) requires showing
//! [`ATTRIBUTION`] wherever results appear.

use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use flate2::read::GzDecoder;
use octopuson_core::{Date, Timestamp};
pub use octopuson_geo::ATTRIBUTION;
use octopuson_geo::GeoDb;
use octopuson_net::HttpClient;
use octopuson_store::{GeoRefresh, IpGeo, IpGeoLookup, SiteStore, StoreError};
use serde::{Deserialize, Serialize};

use crate::USER_AGENT;
use crate::workspace::Workspace;

const COUNTRY_FILE: &str = "dbip-country-lite.csv.gz";
const ASN_FILE: &str = "dbip-asn-lite.csv.gz";
const RELEASE_FILE: &str = "release.json";
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(300);
/// The compressed files are about 10 MB each.
const MAX_DOWNLOAD: u64 = 128 * 1024 * 1024;

/// Errors of the geolocation use-cases.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum GeoDataError {
    /// A download failed.
    #[error("download of {url} failed: {message}")]
    Download {
        /// URL.
        url: String,
        /// Reason.
        message: String,
    },
    /// Downloaded or stored data is not a valid DB-IP file.
    #[error("invalid geolocation data in {file}: {reason}")]
    Invalid {
        /// File.
        file: String,
        /// Reason.
        reason: String,
    },
    /// A file could not be written.
    #[error("cannot write `{path}`")]
    Write {
        /// File.
        path: PathBuf,
        /// Underlying error.
        #[source]
        source: io::Error,
    },
}

/// Which data is installed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GeoRelease {
    /// Month of the country data, e.g. `2026-10`.
    pub country: String,
    /// Month of the ASN data.
    pub asn: String,
    /// Download time (RFC 3339).
    pub fetched_at: String,
}

impl GeoRelease {
    fn key(&self) -> String {
        format!("dbip-country-{}+asn-{}", self.country, self.asn)
    }
}

/// State of the geolocation data.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GeoStatus {
    /// Installed data, if any.
    pub release: Option<GeoRelease>,
    /// Credit to show with results.
    pub attribution: &'static str,
}

/// Result of [`update_geo_data`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GeoUpdateReport {
    /// Installed data.
    pub release: GeoRelease,
    /// Country ranges loaded.
    pub country_ranges: usize,
    /// ASN ranges loaded.
    pub asn_ranges: usize,
}

/// Installed geolocation data.
pub fn geo_status(workspace: &Workspace) -> GeoStatus {
    GeoStatus {
        release: read_release(&workspace.data_dir().geo_dir()),
        attribution: ATTRIBUTION,
    }
}

/// Downloads the current DB-IP Lite country and ASN databases (falling back to the previous
/// month early in a month) and installs them; sites are re-resolved when next opened.
///
/// # Errors
/// [`GeoDataError`]; the previously installed data stays in use on failure.
pub fn update_geo_data(workspace: &Workspace) -> Result<GeoUpdateReport, GeoDataError> {
    let client = HttpClient::new(USER_AGENT, DOWNLOAD_TIMEOUT);
    let (country_month, country_gz) = download_latest(&client, "country")?;
    let (asn_month, asn_gz) = download_latest(&client, "asn")?;
    let mut db = GeoDb::new();
    db.load_countries(&gunzip(&country_gz, COUNTRY_FILE)?)
        .map_err(|e| invalid(COUNTRY_FILE, &e))?;
    db.load_asns(&gunzip(&asn_gz, ASN_FILE)?)
        .map_err(|e| invalid(ASN_FILE, &e))?;
    let (country_ranges, asn_ranges) = db.len();

    let dir = workspace.data_dir().geo_dir();
    write_atomically(&dir.join(COUNTRY_FILE), &country_gz)?;
    write_atomically(&dir.join(ASN_FILE), &asn_gz)?;
    let release = GeoRelease {
        country: country_month,
        asn: asn_month,
        fetched_at: Timestamp::now().to_string(),
    };
    let json = serde_json::to_vec_pretty(&release).unwrap_or_default();
    write_atomically(&dir.join(RELEASE_FILE), &json)?;
    *CACHE.lock().unwrap_or_else(PoisonError::into_inner) = Some(Arc::new(LoadedGeo {
        release: release.key(),
        db,
    }));
    Ok(GeoUpdateReport {
        release,
        country_ranges,
        asn_ranges,
    })
}

/// Loaded data plus its release key; the store's lookup port.
#[derive(Debug)]
pub(crate) struct LoadedGeo {
    release: String,
    db: GeoDb,
}

impl IpGeoLookup for LoadedGeo {
    fn release(&self) -> &str {
        &self.release
    }

    fn lookup(&self, ips: &[String]) -> Vec<IpGeo> {
        ips.iter()
            .map(|ip| match ip.parse() {
                Ok(address) => {
                    let found = self.db.lookup(address);
                    IpGeo {
                        country: found.country.map(str::to_owned),
                        asn: found.asn,
                        as_org: found.as_org.map(str::to_owned),
                    }
                }
                Err(_) => IpGeo::default(),
            })
            .collect()
    }
}

/// Parsed data shared by operations in this process (parsing takes about a second).
static CACHE: Mutex<Option<Arc<LoadedGeo>>> = Mutex::new(None);

/// The installed data, parsed once per release; `None` when not installed or unreadable.
pub(crate) fn load_geo(workspace: &Workspace) -> Option<Arc<LoadedGeo>> {
    let dir = workspace.data_dir().geo_dir();
    let release = read_release(&dir)?;
    let key = release.key();
    let mut cache = CACHE.lock().unwrap_or_else(PoisonError::into_inner);
    if let Some(loaded) = cache.as_ref().filter(|loaded| loaded.release == key) {
        return Some(Arc::clone(loaded));
    }
    let load = || -> Result<GeoDb, GeoDataError> {
        let mut db = GeoDb::new();
        let read = |name: &str| {
            fs::read(dir.join(name)).map_err(|e| GeoDataError::Invalid {
                file: name.to_owned(),
                reason: e.to_string(),
            })
        };
        db.load_countries(&gunzip(&read(COUNTRY_FILE)?, COUNTRY_FILE)?)
            .map_err(|e| invalid(COUNTRY_FILE, &e))?;
        db.load_asns(&gunzip(&read(ASN_FILE)?, ASN_FILE)?)
            .map_err(|e| invalid(ASN_FILE, &e))?;
        Ok(db)
    };
    match load() {
        Ok(db) => {
            let loaded = Arc::new(LoadedGeo { release: key, db });
            *cache = Some(Arc::clone(&loaded));
            Some(loaded)
        }
        Err(error) => {
            tracing::warn!(%error, "ignoring unreadable IP geolocation data");
            None
        }
    }
}

/// The release key sites must carry to be up to date (`None` without data).
pub(crate) fn current_release_key(workspace: &Workspace) -> Option<String> {
    read_release(&workspace.data_dir().geo_dir()).map(|release| release.key())
}

/// Resolves the site's client IPs that have no country/ASN yet (all of them after a data
/// update). Does nothing without installed data.
pub(crate) fn refresh_site_geo(
    workspace: &Workspace,
    store: &mut SiteStore,
) -> Result<Option<GeoRefresh>, StoreError> {
    let Some(geo) = load_geo(workspace) else {
        return Ok(None);
    };
    let refresh = store.refresh_geo(geo.as_ref())?;
    if refresh.resolved > 0 {
        tracing::info!(
            resolved = refresh.resolved,
            replaced = refresh.replaced,
            "client IPs resolved to country and network"
        );
    }
    Ok(Some(refresh))
}

fn download_latest(client: &HttpClient, kind: &str) -> Result<(String, Vec<u8>), GeoDataError> {
    let today = Date::today();
    let (year, month, _) = today.ymd();
    // The last day of the previous month.
    let previous = today.add_days(-i32::from(today.ymd().2));
    let (prev_year, prev_month, _) = previous.ymd();
    let mut last_error = None;
    for (year, month) in [(year, month), (prev_year, prev_month)] {
        let release = format!("{year:04}-{month:02}");
        let url = format!("https://download.db-ip.com/free/dbip-{kind}-lite-{release}.csv.gz");
        match client.get_bytes_up_to(&url, MAX_DOWNLOAD) {
            Ok(bytes) => return Ok((release, bytes)),
            Err(error) => {
                last_error = Some(GeoDataError::Download {
                    url,
                    message: error.to_string(),
                });
            }
        }
    }
    Err(last_error.unwrap_or_else(|| GeoDataError::Download {
        url: format!("dbip-{kind}-lite"),
        message: "no release found".into(),
    }))
}

fn gunzip(bytes: &[u8], name: &str) -> Result<String, GeoDataError> {
    let mut text = String::new();
    GzDecoder::new(bytes)
        .read_to_string(&mut text)
        .map_err(|e| GeoDataError::Invalid {
            file: name.to_owned(),
            reason: e.to_string(),
        })?;
    Ok(text)
}

fn invalid(file: &str, error: &octopuson_geo::GeoError) -> GeoDataError {
    GeoDataError::Invalid {
        file: file.to_owned(),
        reason: error.to_string(),
    }
}

fn read_release(dir: &Path) -> Option<GeoRelease> {
    let text = fs::read_to_string(dir.join(RELEASE_FILE)).ok()?;
    let release: GeoRelease = serde_json::from_str(&text).ok()?;
    (dir.join(COUNTRY_FILE).exists() && dir.join(ASN_FILE).exists()).then_some(release)
}

fn write_atomically(path: &Path, contents: &[u8]) -> Result<(), GeoDataError> {
    let write = || -> io::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let partial = path.with_extension("partial");
        fs::write(&partial, contents)?;
        fs::rename(&partial, path)
    };
    write().map_err(|source| GeoDataError::Write {
        path: path.to_owned(),
        source,
    })
}

#[cfg(test)]
pub(crate) mod test_data {
    //! Installs a tiny data set, as if downloaded.

    use std::io::Write as _;

    use flate2::Compression;
    use flate2::write::GzEncoder;

    use super::*;

    fn gz(text: &str) -> Vec<u8> {
        let mut encoder = GzEncoder::new(Vec::new(), Compression::fast());
        encoder.write_all(text.as_bytes()).unwrap();
        encoder.finish().unwrap()
    }

    pub(crate) fn install(workspace: &Workspace) {
        let dir = workspace.data_dir().geo_dir();
        write_atomically(
            &dir.join(COUNTRY_FILE),
            &gz("66.249.64.0,66.249.95.255,US\n10.0.0.0,10.0.0.255,TR\n"),
        )
        .unwrap();
        write_atomically(
            &dir.join(ASN_FILE),
            &gz("66.249.64.0,66.249.95.255,15169,Google LLC\n10.0.0.0,10.0.0.255,9121,\"Turk Telekom, Inc.\"\n"),
        )
        .unwrap();
        let release = GeoRelease {
            country: "2026-10".into(),
            asn: "2026-10".into(),
            fetched_at: "2026-10-13T00:00:00Z".into(),
        };
        write_atomically(
            &dir.join(RELEASE_FILE),
            &serde_json::to_vec(&release).unwrap(),
        )
        .unwrap();
    }
}

#[cfg(test)]
mod tests {
    use octopuson_core::table::Cell;

    use super::*;
    use crate::test_support::Fixture;

    #[test]
    fn status_reflects_installed_data() {
        let fixture = Fixture::new();
        assert_eq!(geo_status(&fixture.workspace).release, None);
        assert!(load_geo(&fixture.workspace).is_none());
        test_data::install(&fixture.workspace);
        let status = geo_status(&fixture.workspace);
        assert_eq!(status.release.unwrap().country, "2026-10");
        let geo = load_geo(&fixture.workspace).unwrap();
        let answers = geo.lookup(&["10.0.0.1".into(), "nonsense".into()]);
        assert_eq!(answers[0].as_org.as_deref(), Some("Turk Telekom, Inc."));
        assert_eq!(answers[1], IpGeo::default());
    }

    #[test]
    fn queries_group_by_country_and_network_after_import() {
        let fixture = Fixture::new();
        test_data::install(&fixture.workspace);
        fixture.import("access.log", crate::test_support::LOG);
        let (outcome, table) = fixture.run("group:country,asn,as_org metric:hits sort:hits:desc");
        assert_eq!(outcome.source, "raw hits");
        assert_eq!(
            table.rows,
            [
                vec![
                    Cell::Text("TR".into()),
                    Cell::Integer(9121),
                    Cell::Text("Turk Telekom, Inc.".into()),
                    Cell::Integer(4)
                ],
                vec![
                    Cell::Text("US".into()),
                    Cell::Integer(15_169),
                    Cell::Text("Google LLC".into()),
                    Cell::Integer(2)
                ],
            ]
        );
        let (_, filtered) = fixture.run("country:US group:ip");
        assert_eq!(filtered.rows.len(), 1);
    }

    #[test]
    fn data_installed_later_is_applied_on_the_next_query() {
        let fixture = Fixture::imported();
        let (_, before) = fixture.run("group:country");
        assert_eq!(before.rows[0][0], Cell::Null);
        test_data::install(&fixture.workspace);
        let (_, after) = fixture.run("group:country sort:country:asc");
        assert_eq!(after.rows.len(), 2);
        assert_eq!(after.rows[0][0], Cell::Text("TR".into()));
    }
}
