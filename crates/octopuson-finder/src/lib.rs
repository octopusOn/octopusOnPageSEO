//! OctopusOn Index Checker (`octopuson-finder`).
//!
//! Discovers indexed URLs of a domain across free search engines (Google, Yandex, Bing,
//! DuckDuckGo), Internet Archive Wayback CDX, and optional search APIs (Google Custom Search,
//! Yandex XML/Search API, Serper.dev), then verifies each URL over HTTP and HTML analysis to
//! identify indexed 404/410s, other 4xx/5xx errors, Soft-404s, and thin-content 200 OK pages.
//!
//! Serper and SerpApi run on the user's own key or, with a connected OctopusOn SEO account, on
//! OctopusOn SEO credits through the OctopusOn SEO gateway.

pub mod archive;
pub mod browser_serp;
mod cloud;
pub mod cluster;
pub mod compare;
pub mod matcher;
mod model;
pub mod serp;
mod store;
pub mod verify;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use octopuson_core::http::HttpTransport;

pub use cloud::{CloudStatus, CreditOffer, CreditSource, FinderSettings, app_name};
pub use cluster::PathCluster;
pub use matcher::RedirectCandidate;
pub use model::{
    ApiConfig, ComparedUrl, CreditUsage, DiffStatus, DiscoveredUrl, EngineSummary,
    FinderPageRequest, FinderTablePage, FindingCategory, IndexEngine, ScanComparison, ScanConfig,
    ScanControl, ScanMeta, ScanOverview, ScanPhase, ScanProgress, VerifiedUrl,
};
pub use octopuson_cloud::{Account, SignIn, SourceMode, TokenStore, Via};
pub use octopuson_report::ExportFormat;
pub use store::FinderStore;

/// Errors produced by the Index Checker workspace.
#[derive(Debug, thiserror::Error)]
pub enum FinderError {
    /// Target domain is missing or invalid.
    #[error("invalid target domain `{0}`; enter a valid domain such as example.com")]
    InvalidDomain(String),
    /// Platform reports no data directory.
    #[error("cannot determine platform application data directory")]
    NoDataDir,
    /// Scan ID was not found.
    #[error("scan `{0}` was not found")]
    NotFound(String),
    /// Filesystem I/O error.
    #[error("I/O error at `{}`: {source}", path.display())]
    Io {
        /// Affected path.
        path: PathBuf,
        /// Underlying OS error.
        source: std::io::Error,
    },
    /// DuckDB or metadata error.
    #[error("storage error: {0}")]
    Database(String),
    /// Export path extension is not recognized.
    #[error("cannot determine export format of `{}` (use .csv, .xlsx or .json)", .0.display())]
    UnknownExportFormat(PathBuf),
    /// The OctopusOn SEO account or its gateway failed.
    #[error("OctopusOn SEO account: {0}")]
    Cloud(String),
    /// A setting cannot be used.
    #[error("invalid setting: {0}")]
    InvalidSettings(String),
}

/// Event emitted during a running scan.
#[derive(Debug, Clone)]
pub enum FinderEvent {
    /// Scan entered a new lifecycle phase.
    Phase(ScanPhase),
    /// Scan directory & ID were reserved on disk.
    Started {
        /// Reserved scan ID.
        scan_id: String,
    },
    /// Progress counters updated.
    Progress(ScanProgress),
}

/// High-level workspace facade shared by `octopuson-index` CLI and the `apps/index-checker` desktop app.
#[derive(Debug, Clone)]
pub struct FinderWorkspace {
    store: FinderStore,
    cloud: Arc<cloud::CloudState>,
}

impl FinderWorkspace {
    /// Opens the workspace at `data_dir` or the platform default directory. The OctopusOn SEO
    /// account is the one every OctopusOn SEO program on this computer shares.
    ///
    /// # Errors
    /// Returns [`FinderError::NoDataDir`] if no default directory exists, and
    /// [`FinderError::InvalidSettings`] for an unusable `OCTOPUSSEO_CLOUD_URL`.
    pub fn resolve(data_dir: Option<PathBuf>) -> Result<Self, FinderError> {
        let store = match data_dir {
            Some(dir) => FinderStore::new(dir),
            None => FinderStore::platform_default()?,
        };
        let api =
            octopuson_cloud::api_base(std::env::var(octopuson_cloud::API_VARIABLE).ok().as_deref())
                .map_err(|error| FinderError::InvalidSettings(error.to_string()))?;
        // One account file for every OctopusOn SEO program: connecting once connects them all.
        let tokens = TokenStore::shared(api)
            .map_err(|error| FinderError::InvalidSettings(error.to_string()))?;
        Ok(Self::with_store(store).with_tokens(tokens))
    }

    /// A workspace over `store` whose account token lives in its data folder until
    /// [`Self::with_tokens`] says otherwise (tests, embedding).
    pub fn with_store(store: FinderStore) -> Self {
        let tokens = TokenStore::in_dir(&data_dir_of(&store), octopuson_cloud::DEFAULT_API);
        Self {
            store,
            cloud: Arc::new(cloud::CloudState::new(tokens)),
        }
    }

    /// Keeps the OctopusOn SEO account token in `tokens`.
    #[must_use]
    pub fn with_tokens(mut self, tokens: TokenStore) -> Self {
        self.cloud = Arc::new(cloud::CloudState::new(tokens));
        self
    }

    /// Underlying store.
    pub const fn store(&self) -> &FinderStore {
        &self.store
    }

    /// The data folder (the scans live in its `scans` folder).
    pub fn data_dir(&self) -> PathBuf {
        data_dir_of(&self.store)
    }

    /// Runs a full discovery + verification scan and stores the results in DuckDB.
    ///
    /// # Errors
    /// Returns an error if the domain is invalid or storage fails.
    pub fn run_scan(
        &self,
        config: ScanConfig,
        control: &ScanControl,
        on_event: impl FnMut(FinderEvent) + Send,
    ) -> Result<ScanMeta, FinderError> {
        let timeout = Duration::from_secs(u64::from(config.timeout_seconds.clamp(5, 60)));
        let transport = self.api_transport(timeout)?;
        self.run_scan_with(config, control, &transport, on_event)
    }

    /// [`Self::run_scan`] with `transport` for the paid search APIs (tests, embedding). Sources
    /// on OctopusOn SEO credits send their requests to the gateway over it; the charges are
    /// recorded in the scan's [`ScanMeta::credits`].
    ///
    /// # Errors
    /// Returns an error if the domain is invalid, the settings cannot be read or storage fails.
    pub fn run_scan_with(
        &self,
        config: ScanConfig,
        control: &ScanControl,
        transport: &dyn HttpTransport,
        mut on_event: impl FnMut(FinderEvent) + Send,
    ) -> Result<ScanMeta, FinderError> {
        on_event(FinderEvent::Phase(ScanPhase::Starting));
        let Some(base_host) = config.normalized_host() else {
            return Err(FinderError::InvalidDomain(config.domain));
        };
        let plan = self.credit_plan(&config.api, transport)?;

        let (scan_id, dir, mut meta) = self.store.reserve_scan(&base_host, &config)?;
        on_event(FinderEvent::Started { scan_id });

        let mut progress = ScanProgress::default();
        on_event(FinderEvent::Phase(ScanPhase::Discovering));

        let credit_transport = self.credit_transport(&plan, transport);
        let mut api = serp::ApiCalls::new(
            &credit_transport,
            plan.engines,
            CreditUsage {
                sources: plan.sources,
                ..CreditUsage::default()
            },
        );
        let search_config = ScanConfig {
            api: plan.api,
            ..config.clone()
        };
        progress.credits.clone_from(api.usage());
        let discovered = serp::discover_indexed_urls(
            &search_config,
            &base_host,
            &mut api,
            control,
            &mut progress,
            &mut |p| {
                on_event(FinderEvent::Progress(p.clone()));
            },
        );
        meta.credits.clone_from(api.usage());

        let discovered_count = u32::try_from(discovered.len()).unwrap_or(u32::MAX);
        progress.discovered_urls = discovered_count;
        on_event(FinderEvent::Progress(progress.clone()));

        on_event(FinderEvent::Phase(ScanPhase::Verifying));
        let mut verified =
            verify::verify_urls(&discovered, &config, control, &mut progress, &mut |p| {
                on_event(FinderEvent::Progress(p.clone()));
            });
        matcher::enrich_smart_redirect_targets(&mut verified);

        on_event(FinderEvent::Phase(ScanPhase::Saving));
        let final_status = if control.is_stopped() {
            "stopped"
        } else {
            "completed"
        };
        let saved = self.store.finish_scan(
            &dir,
            meta,
            final_status,
            progress.queries_completed,
            discovered_count,
            &verified,
        )?;
        on_event(FinderEvent::Phase(if control.is_stopped() {
            ScanPhase::Stopped
        } else {
            ScanPhase::Completed
        }));
        Ok(saved)
    }

    /// Re-verifies broken URLs (or a single URL) in an existing scan and updates its DuckDB store.
    ///
    /// # Errors
    /// Returns an error if the scan cannot be loaded or updated.
    pub fn reverify_scan(
        &self,
        scan_id: &str,
        url_filter: Option<&str>,
    ) -> Result<ScanOverview, FinderError> {
        compare::reverify_scan(&self.store, scan_id, url_filter)
    }

    /// Compares two saved scans (`base_id` vs `target_id`) and returns resolved, new, and persistent issues.
    ///
    /// # Errors
    /// Returns an error if either scan cannot be loaded.
    pub fn compare_scans(
        &self,
        base_id: &str,
        target_id: &str,
    ) -> Result<ScanComparison, FinderError> {
        compare::compare_scans(&self.store, base_id, target_id)
    }
}

/// The data folder of a store (the parent of its `scans` folder).
fn data_dir_of(store: &FinderStore) -> PathBuf {
    store
        .path()
        .parent()
        .map_or_else(|| store.path().to_owned(), Path::to_path_buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn store_persists_queries_and_compares_scans() {
        let temp = tempfile::tempdir().unwrap();
        let ws = FinderWorkspace::resolve(Some(temp.path().to_path_buf())).unwrap();
        let config = ScanConfig {
            domain: "acme.com".to_owned(),
            ..ScanConfig::default()
        };
        let (id1, dir1, meta1) = ws.store().reserve_scan("acme.com", &config).unwrap();
        let rows1 = vec![
            VerifiedUrl {
                url: "https://acme.com/eski-urun".to_owned(),
                engines: "google,yandex".to_owned(),
                serp_title: "Eski Ürün".to_owned(),
                serp_snippet: "Stokta olmayan ürün".to_owned(),
                found_by_query: "site:acme.com".to_owned(),
                initial_status: 404,
                final_status: 404,
                redirect_hops: 0,
                final_url: "https://acme.com/eski-urun".to_owned(),
                response_time_ms: 42,
                content_type: "text/html".to_owned(),
                live_title: "404 Not Found".to_owned(),
                h1: "Sayfa Bulunamadı".to_owned(),
                word_count: 8,
                canonical: String::new(),
                noindex: false,
                category: FindingCategory::Error404,
                reason: "http_404".to_owned(),
                suggested_target: "https://acme.com/".to_owned(),
                error_message: String::new(),
            },
            VerifiedUrl {
                url: "https://acme.com/ince-sayfa".to_owned(),
                engines: "google".to_owned(),
                serp_title: "İnce Sayfa".to_owned(),
                serp_snippet: "Kısa açıklama".to_owned(),
                found_by_query: "site:acme.com".to_owned(),
                initial_status: 200,
                final_status: 200,
                redirect_hops: 0,
                final_url: "https://acme.com/ince-sayfa".to_owned(),
                response_time_ms: 55,
                content_type: "text/html".to_owned(),
                live_title: "İnce Sayfa".to_owned(),
                h1: "Başlık".to_owned(),
                word_count: 14,
                canonical: "https://acme.com/ince-sayfa".to_owned(),
                noindex: false,
                category: FindingCategory::Thin200,
                reason: "thin_content_200".to_owned(),
                suggested_target: "https://acme.com/".to_owned(),
                error_message: String::new(),
            },
        ];
        let finished1 = ws
            .store()
            .finish_scan(&dir1, meta1, "completed", 4, 2, &rows1)
            .unwrap();
        assert_eq!(finished1.count_404, 1);
        assert_eq!(finished1.count_thin_200, 1);

        let overview = ws.store().overview(&id1).unwrap();
        assert_eq!(overview.top_issues.len(), 2);
        assert_eq!(overview.engines.len(), 2);

        let rules = ws.store().generate_redirect_rules(&id1, "nginx").unwrap();
        assert!(rules.contains("rewrite ^/eski\\-urun$ https://acme.com/ permanent;"));

        // Second scan where /eski-urun is resolved (200 OK) and /yeni-kirik is a new 404
        let (id2, dir2, meta2) = ws.store().reserve_scan("acme.com", &config).unwrap();
        let mut resolved_row = rows1[0].clone();
        resolved_row.initial_status = 301;
        resolved_row.final_status = 200;
        resolved_row.category = FindingCategory::Healthy200;
        resolved_row.reason = "healthy_200".to_owned();

        let mut new_404_row = rows1[0].clone();
        new_404_row.url = "https://acme.com/yeni-kirik".to_owned();

        ws.store()
            .finish_scan(
                &dir2,
                meta2,
                "completed",
                4,
                3,
                &[resolved_row, rows1[1].clone(), new_404_row],
            )
            .unwrap();

        let diff = ws.compare_scans(&id1, &id2).unwrap();
        assert_eq!(diff.resolved_count, 1);
        assert_eq!(diff.new_issue_count, 1);
        assert_eq!(diff.persistent_issue_count, 1);

        let rev_diff = ws.compare_scans(&id2, &id1).unwrap();
        assert!(rev_diff.rows.iter().any(|r| r.target_status == 0));
    }
}
