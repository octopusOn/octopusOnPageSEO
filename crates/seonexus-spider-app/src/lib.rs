//! Use cases of the OctopusOn SEO Crawler, shared by the `octopuson-crawler` CLI and the desktop app.
//!
//! [`SpiderWorkspace`] wires the crawl engine to the HTTP adapter and the crawl store: it
//! runs crawls into new saved crawls, lists and opens them, and exports their tabs to CSV,
//! XLSX or JSON.

mod exports;
mod observe;
pub mod opportunities;
mod resources;
mod schema_test;
pub use schema_test::{SchemaTestMode, SchemaTestRequest, SchemaTestResult, test_schema};
mod sitemaps;
pub use observe::{CrawlEvent, CrawlPhase};
pub use resources::resident_memory_bytes;
pub use seonexus_crawl_store::{SitemapPlan, SitemapSelection};
pub use sitemaps::{SitemapExport, SitemapFormat};

use std::path::{Path, PathBuf};

use exports::export_tab;
pub use seonexus_audit::{Catalog, catalog};
pub use seonexus_crawl::{
    ActivityKind, ActivityState, AgentPreset, ConfigError, CrawlActivity, CrawlConfig,
    CrawlControl, CrawlMode, CrawlPlan, CrawlProgress, RenderMode, Rendering, RobotsPolicy,
    UrlActivity, Viewport,
};
use seonexus_crawl::{
    CrawlSink, Crawler, Fetcher, LinkRecord, RobotsRecord, SitemapRecord, UrlRecord,
};
pub use seonexus_crawl_store::ContentMatch;
pub use seonexus_crawl_store::CrawlStorage;
pub use seonexus_crawl_store::{
    ComparisonFilter, ComparisonRequest, ComparisonSummary, CrawlComparison, FindingChange,
    FindingComparisonRequest, FindingComparisonSummary, FindingRuleCount,
};
use seonexus_crawl_store::{CrawlLibrary, CrawlWriter};
pub use seonexus_crawl_store::{CrawlMeta, CrawlReader, CrawlStatus, CrawlStoreError, CrawlTab};
pub use seonexus_crawl_store::{
    CrawlOverview, LinkDirection, LinkScope, PageRequest, TableExportRequest, TablePage, UrlDetail,
};
pub use seonexus_crawl_store::{DuplicateGroup, DuplicateKind, DuplicateScope};
pub use seonexus_crawl_store::{RedirectHop, RedirectOutcome, RedirectTrace};
pub use seonexus_crawl_store::{RemediationDetail, RemediationRequest};
use seonexus_net::CrawlFetcher;
pub use seonexus_report::ExportFormat;

/// A Spider use case failed.
#[derive(Debug, thiserror::Error)]
pub enum SpiderError {
    /// The crawl settings are invalid.
    #[error(transparent)]
    Config(#[from] ConfigError),
    /// The crawl store failed.
    #[error(transparent)]
    Store(#[from] CrawlStoreError),
    /// Browser rendering could not start.
    #[error(transparent)]
    Rendering(#[from] seonexus_render::RenderError),
    /// An export file could not be created.
    #[error("cannot create `{}`: {source}", path.display())]
    Output {
        /// The export file.
        path: PathBuf,
        /// The underlying error.
        source: std::io::Error,
    },
    /// The export path has no known extension and no format was given.
    #[error("cannot tell the export format of `{}` (use .csv, .xlsx or .json)", .0.display())]
    UnknownFormat(PathBuf),
}

/// One exported tab.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportedTab {
    /// The tab.
    pub tab: CrawlTab,
    /// The file written.
    pub path: PathBuf,
    /// Rows written.
    pub rows: u64,
}

/// The saved crawls of one data directory.
#[derive(Debug, Clone)]
pub struct SpiderWorkspace {
    library: CrawlLibrary,
}

impl SpiderWorkspace {
    /// Crawls under `data_dir`, or under the platform data directory.
    ///
    /// # Errors
    /// The platform reports no home directory.
    pub fn resolve(data_dir: Option<PathBuf>) -> Result<Self, SpiderError> {
        let library = match data_dir {
            Some(dir) => CrawlLibrary::new(dir),
            None => CrawlLibrary::platform_default()?,
        };
        Ok(Self { library })
    }

    /// Where crawls are stored.
    pub fn crawls_dir(&self) -> &Path {
        self.library.path()
    }

    /// Crawls `config` over HTTP into a new saved crawl, reporting progress to `on_progress`
    /// on the calling thread. `control` pauses or stops it from another thread.
    ///
    /// # Errors
    /// Invalid settings or storage failures; network failures are recorded per URL instead.
    pub fn crawl(
        &self,
        config: CrawlConfig,
        control: &CrawlControl,
        mut on_progress: impl FnMut(&CrawlProgress),
    ) -> Result<CrawlMeta, SpiderError> {
        self.crawl_observed(config, control, |event| {
            if let CrawlEvent::Progress(progress) = event {
                on_progress(progress);
            }
        })
    }

    /// Crawls with phase, identity, counters and bounded URL activity observations.
    ///
    /// # Errors
    /// Invalid configuration, browser startup or storage failures.
    pub fn crawl_observed(
        &self,
        config: CrawlConfig,
        control: &CrawlControl,
        mut on_event: impl FnMut(&CrawlEvent<'_>),
    ) -> Result<CrawlMeta, SpiderError> {
        on_event(&CrawlEvent::Phase(CrawlPhase::Starting));
        let plan = CrawlPlan::new(config)?;
        let fetcher = CrawlFetcher::new(
            plan.user_agent(),
            plan.timeout(),
            plan.config().accept_language.clone(),
        );
        let renderer = if plan.config().rendering.mode == RenderMode::Javascript {
            Some(seonexus_render::ChromeRenderer::launch(&plan)?)
        } else {
            None
        };
        self.crawl_observed_with(
            &plan,
            &fetcher,
            renderer
                .as_ref()
                .map(|renderer| renderer as &dyn seonexus_crawl::Renderer),
            control,
            on_event,
        )
    }

    /// Like [`crawl`](Self::crawl) with any [`Fetcher`] (tests, alternative HTTP adapters).
    ///
    /// # Errors
    /// Storage failures.
    pub fn crawl_with(
        &self,
        plan: &CrawlPlan,
        fetcher: &dyn Fetcher,
        control: &CrawlControl,
        on_progress: impl FnMut(&CrawlProgress),
    ) -> Result<CrawlMeta, SpiderError> {
        self.crawl_with_renderer(plan, fetcher, None, control, on_progress)
    }

    /// Runs the same use case with an injected renderer (including deterministic tests).
    ///
    /// # Errors
    /// Storage failures.
    pub fn crawl_with_renderer(
        &self,
        plan: &CrawlPlan,
        fetcher: &dyn Fetcher,
        renderer: Option<&dyn seonexus_crawl::Renderer>,
        control: &CrawlControl,
        mut on_progress: impl FnMut(&CrawlProgress),
    ) -> Result<CrawlMeta, SpiderError> {
        self.crawl_observed_with(plan, fetcher, renderer, control, |event| {
            if let CrawlEvent::Progress(progress) = event {
                on_progress(progress);
            }
        })
    }

    /// Observed crawl with injected network and browser adapters for integration tests.
    ///
    /// # Errors
    /// Storage failures.
    pub fn crawl_observed_with(
        &self,
        plan: &CrawlPlan,
        fetcher: &dyn Fetcher,
        renderer: Option<&dyn seonexus_crawl::Renderer>,
        control: &CrawlControl,
        mut on_event: impl FnMut(&CrawlEvent<'_>),
    ) -> Result<CrawlMeta, SpiderError> {
        let writer = self.library.create(plan)?;
        on_event(&CrawlEvent::Created(writer.meta()));
        on_event(&CrawlEvent::Phase(CrawlPhase::Crawling));
        let mut sink = ProgressSink { writer, on_event };
        let mut crawler = Crawler::new(plan, fetcher);
        if let Some(renderer) = renderer {
            crawler = crawler.with_renderer(renderer);
        }
        match crawler.run(&mut sink, control) {
            Ok(summary) => {
                (sink.on_event)(&CrawlEvent::Phase(CrawlPhase::Finalizing));
                Ok(sink.writer.finish(&summary)?)
            }
            Err(error) => {
                let _ = sink.writer.fail(&error.to_string());
                Err(error.into())
            }
        }
    }

    /// Saved crawls, newest first.
    ///
    /// # Errors
    /// The crawls directory cannot be read.
    pub fn crawls(&self) -> Result<Vec<CrawlMeta>, SpiderError> {
        Ok(self.library.list()?)
    }

    /// Measures saved crawl files without opening or modifying their databases.
    ///
    /// # Errors
    /// The crawls directory cannot be listed. Individual measurement failures have no size.
    pub fn crawl_storage(&self) -> Result<Vec<CrawlStorage>, SpiderError> {
        Ok(self.library.storage()?)
    }

    /// The metadata of crawl `id`.
    ///
    /// # Errors
    /// Unknown crawl.
    pub fn meta(&self, id: &str) -> Result<CrawlMeta, SpiderError> {
        Ok(self.library.meta(id)?)
    }

    /// Opens crawl `id` read-only.
    ///
    /// # Errors
    /// Unknown crawl or database errors.
    pub fn open(&self, id: &str) -> Result<CrawlReader, SpiderError> {
        Ok(self.library.open(id)?)
    }

    /// Read-only score-based URL witnesses from a finished compatible archive.
    ///
    /// # Errors
    /// Invalid archives, unknown checks, selections or database failures.
    pub fn remediation(
        &self,
        id: &str,
        request: &RemediationRequest,
    ) -> Result<RemediationDetail, SpiderError> {
        Ok(self.library.open_archive(id)?.remediation(request)?)
    }

    /// Compares two finished archives by exact stored URL, without changing either archive.
    ///
    /// # Errors
    /// Invalid, running or incompatible archives; database failures.
    pub fn compare(&self, baseline: &str, current: &str) -> Result<CrawlComparison, SpiderError> {
        Ok(self.library.compare(baseline, current)?)
    }

    /// Streams all comparison matches and before/after fields to an atomic export.
    ///
    /// # Errors
    /// Invalid archives, filters, output path or database/output failures.
    pub fn export_comparison(
        &self,
        baseline: &str,
        current: &str,
        request: &ComparisonRequest,
        path: &Path,
    ) -> Result<u64, SpiderError> {
        let comparison = self.compare(baseline, current)?;
        exports::write_rows(path, None, |sink| comparison.export(request, sink))
    }

    /// Streams all matching historical finding pairs to an atomic CSV/XLSX/JSON export.
    ///
    /// # Errors
    /// Invalid archives, filters, database or output failures.
    pub fn export_finding_comparison(
        &self,
        baseline: &str,
        current: &str,
        request: &FindingComparisonRequest,
        path: &Path,
    ) -> Result<u64, SpiderError> {
        let comparison = self.compare(baseline, current)?;
        exports::write_rows(path, None, |sink| comparison.export_findings(request, sink))
    }

    /// Deletes crawl `id`.
    ///
    /// # Errors
    /// Unknown crawl or file system errors.
    pub fn delete(&self, id: &str) -> Result<(), SpiderError> {
        Ok(self.library.delete(id)?)
    }

    /// Writes `tab` of crawl `id` to `path`, in `format` or the one its extension names.
    ///
    /// # Errors
    /// Unknown crawl or format, database or file errors.
    pub fn export(
        &self,
        id: &str,
        tab: CrawlTab,
        path: &Path,
        format: Option<ExportFormat>,
    ) -> Result<ExportedTab, SpiderError> {
        let reader = self.open(id)?;
        export_tab(&reader, tab, path, format)
    }

    /// Exports all matching rows, preserving filters, sorting and selected column order.
    ///
    /// # Errors
    /// Unknown crawl/selection/format, database or file errors. Existing output is preserved on failure.
    pub fn export_selection(
        &self,
        id: &str,
        selection: &TableExportRequest,
        path: &Path,
        format: Option<ExportFormat>,
    ) -> Result<ExportedTab, SpiderError> {
        let reader = self.open(id)?;
        exports::export_selection(&reader, selection, path, format)
    }

    /// Writes every tab of crawl `id` that has rows into `dir` as `<tab>.<ext>`.
    ///
    /// # Errors
    /// Unknown crawl, database or file errors.
    pub fn export_all(
        &self,
        id: &str,
        dir: &Path,
        format: ExportFormat,
    ) -> Result<Vec<ExportedTab>, SpiderError> {
        let reader = self.open(id)?;
        std::fs::create_dir_all(dir).map_err(|source| SpiderError::Output {
            path: dir.to_owned(),
            source,
        })?;
        let mut exported = Vec::new();
        for tab in CrawlTab::ALL {
            if reader.count(tab)? == 0 {
                continue;
            }
            let path = dir.join(format!("{}.{}", tab.name(), format.extension()));
            exported.push(export_tab(&reader, tab, &path, Some(format))?);
        }
        Ok(exported)
    }
}

/// Forwards to the crawl writer and reports progress to a callback.
struct ProgressSink<F> {
    writer: CrawlWriter,
    on_event: F,
}

impl<F: FnMut(&CrawlEvent<'_>)> CrawlSink for ProgressSink<F> {
    type Error = CrawlStoreError;

    fn site_file(&mut self, record: &seonexus_crawl::SiteFileRecord) -> Result<(), Self::Error> {
        self.writer.site_file(record)
    }

    fn url(&mut self, record: &UrlRecord, links: &[LinkRecord]) -> Result<(), Self::Error> {
        self.writer.url(record, links)
    }

    fn robots(&mut self, record: &RobotsRecord) -> Result<(), Self::Error> {
        self.writer.robots(record)
    }

    fn sitemap(&mut self, record: &SitemapRecord) -> Result<(), Self::Error> {
        self.writer.sitemap(record)
    }

    fn progress(&mut self, progress: &CrawlProgress) {
        self.writer.progress(progress);
        (self.on_event)(&CrawlEvent::Progress(progress));
    }

    fn activity(&mut self, activity: &CrawlActivity) {
        (self.on_event)(&CrawlEvent::Activity(activity));
    }
}

#[cfg(test)]
mod tests {
    use seonexus_crawl::{FetchError, FetchRequest, FetchResponse};

    use super::*;

    struct OnePage;

    impl Fetcher for OnePage {
        fn fetch(&self, request: &FetchRequest<'_>) -> Result<FetchResponse, FetchError> {
            Ok(if request.url == "https://example.com/" {
                FetchResponse::new(200, &[("Content-Type", "text/html")], "<title>Home</title>")
            } else {
                FetchResponse::new(404, &[], "")
            })
        }
    }

    #[test]
    fn observations_identify_the_crawl_and_finalize_stopped_results() {
        let dir = tempfile::tempdir().unwrap();
        let workspace = SpiderWorkspace::resolve(Some(dir.path().to_owned())).unwrap();
        let plan = CrawlPlan::new(CrawlConfig {
            start_urls: vec!["https://example.com/".into()],
            ..CrawlConfig::default()
        })
        .unwrap();
        let control = CrawlControl::new();
        control.stop();
        let mut created = None;
        let mut phases = Vec::new();
        let meta = workspace
            .crawl_observed_with(&plan, &OnePage, None, &control, |event| match event {
                CrawlEvent::Created(meta) => created = Some(meta.id.clone()),
                CrawlEvent::Phase(phase) => phases.push(*phase),
                _ => {}
            })
            .unwrap();
        assert_eq!(created.as_deref(), Some(meta.id.as_str()));
        assert_eq!(phases, [CrawlPhase::Crawling, CrawlPhase::Finalizing]);
        assert_eq!(meta.status, CrawlStatus::Stopped);
        assert_eq!(
            workspace.meta(&meta.id).unwrap().status,
            CrawlStatus::Stopped
        );
    }

    #[test]
    fn empty_filtered_exports_keep_headers_and_report_zero_rows() {
        let dir = tempfile::tempdir().unwrap();
        let workspace = SpiderWorkspace::resolve(Some(dir.path().to_owned())).unwrap();
        let plan = CrawlPlan::new(CrawlConfig {
            start_urls: vec!["https://example.com/".into()],
            ..CrawlConfig::default()
        })
        .unwrap();
        let meta = workspace
            .crawl_with(&plan, &OnePage, &CrawlControl::new(), |_| {})
            .unwrap();
        let selection = TableExportRequest {
            request: PageRequest {
                tab: "internal".into(),
                search: "no-such-address".into(),
                ..PageRequest::default()
            },
            columns: vec!["title".into(), "address".into()],
        };
        for format in ExportFormat::ALL {
            let path = dir.path().join(format!("empty.{}", format.extension()));
            assert_eq!(
                workspace
                    .export_selection(&meta.id, &selection, &path, None)
                    .unwrap()
                    .rows,
                0
            );
            if format == ExportFormat::Csv {
                assert_eq!(std::fs::read_to_string(&path).unwrap(), "title,address\r\n");
            }
            if format == ExportFormat::Json {
                assert_eq!(
                    std::fs::read_to_string(&path)
                        .unwrap()
                        .split_whitespace()
                        .collect::<String>(),
                    "[]"
                );
            }
            assert!(path.is_file());
        }
    }

    #[test]
    fn crawls_lists_and_exports() {
        let dir = tempfile::tempdir().unwrap();
        let workspace = SpiderWorkspace::resolve(Some(dir.path().to_owned())).unwrap();
        let config = CrawlConfig {
            start_urls: vec!["https://example.com/".into()],
            ..CrawlConfig::default()
        };
        let plan = CrawlPlan::new(config).unwrap();
        let mut reports = 0;
        let meta = workspace
            .crawl_with(&plan, &OnePage, &CrawlControl::new(), |_| reports += 1)
            .unwrap();
        assert!(reports > 0);
        assert_eq!(meta.status, CrawlStatus::Completed);
        assert_eq!(workspace.crawls().unwrap()[0].id, meta.id);

        let csv = dir.path().join("internal.csv");
        let exported = workspace
            .export(&meta.id, CrawlTab::Internal, &csv, None)
            .unwrap();
        assert_eq!(exported.rows, 1);
        let text = std::fs::read_to_string(&csv).unwrap();
        assert!(text.starts_with("address,content,status_code"));
        assert!(text.contains("https://example.com/,html,200"));

        for format in ExportFormat::ALL {
            let path = dir.path().join(format!("filtered.{}", format.extension()));
            std::fs::write(&path, "previous report").unwrap();
            let selection = TableExportRequest {
                request: PageRequest {
                    tab: "internal".into(),
                    search: "EXAMPLE".into(),
                    offset: 9999,
                    limit: 1,
                    ..PageRequest::default()
                },
                columns: vec!["title".into(), "address".into()],
            };
            assert_eq!(
                workspace
                    .export_selection(&meta.id, &selection, &path, None)
                    .unwrap()
                    .rows,
                1
            );
            assert!(std::fs::metadata(&path).unwrap().len() > 20);
            if format == ExportFormat::Csv {
                assert!(
                    std::fs::read_to_string(path)
                        .unwrap()
                        .starts_with("title,address\r\nHome,https://example.com/")
                );
            }
        }

        let out = dir.path().join("all");
        let all = workspace
            .export_all(&meta.id, &out, ExportFormat::Xlsx)
            .unwrap();
        assert!(all.iter().any(|tab| tab.tab == CrawlTab::Robots));
        assert!(out.join("internal.xlsx").is_file());

        let unknown = workspace.export(&meta.id, CrawlTab::Internal, Path::new("x.txt"), None);
        assert!(matches!(unknown, Err(SpiderError::UnknownFormat(_))));
    }

    #[test]
    fn comparison_exports_share_filters_ignore_pagination_and_preserve_outputs_on_failure() {
        let dir = tempfile::tempdir().unwrap();
        let workspace = SpiderWorkspace::resolve(Some(dir.path().to_owned())).unwrap();
        let plan = CrawlPlan::new(CrawlConfig {
            start_urls: vec!["https://example.com/".into()],
            ..CrawlConfig::default()
        })
        .unwrap();
        let baseline = workspace
            .crawl_with(&plan, &OnePage, &CrawlControl::new(), |_| {})
            .unwrap();
        let current = workspace
            .crawl_with(&plan, &OnePage, &CrawlControl::new(), |_| {})
            .unwrap();
        let request = ComparisonRequest {
            filter: ComparisonFilter::Unchanged,
            offset: 999,
            limit: 1,
            ..ComparisonRequest::default()
        };
        for format in ExportFormat::ALL {
            let path = dir
                .path()
                .join(format!("comparison.{}", format.extension()));
            assert_eq!(
                workspace
                    .export_comparison(&baseline.id, &current.id, &request, &path)
                    .unwrap(),
                1
            );
            let output = std::fs::read(&path).unwrap();
            if format == ExportFormat::Xlsx {
                assert!(output.starts_with(b"PK"));
            } else {
                let text = String::from_utf8(output).unwrap();
                assert!(text.contains("before_title"));
                assert!(text.contains("after_title"));
                assert!(text.contains("Home"));
                assert!(text.contains("unchanged"));
            }
            std::fs::write(&path, "existing report").unwrap();
            let invalid = ComparisonRequest {
                field: "title) OR true".into(),
                ..request.clone()
            };
            assert!(
                workspace
                    .export_comparison(&baseline.id, &current.id, &invalid, &path)
                    .is_err()
            );
            assert_eq!(std::fs::read_to_string(&path).unwrap(), "existing report");
        }
    }

    #[test]
    fn historical_finding_exports_stream_the_selected_scope_and_preserve_existing_reports() {
        let dir = tempfile::tempdir().unwrap();
        let workspace = SpiderWorkspace::resolve(Some(dir.path().to_owned())).unwrap();
        let plan = CrawlPlan::new(CrawlConfig {
            start_urls: vec!["https://example.com/".into()],
            ..CrawlConfig::default()
        })
        .unwrap();
        let baseline = workspace
            .crawl_with(&plan, &OnePage, &CrawlControl::new(), |_| {})
            .unwrap();
        let current = workspace
            .crawl_with(&plan, &OnePage, &CrawlControl::new(), |_| {})
            .unwrap();
        let request = FindingComparisonRequest {
            change: FindingChange::Persisting,
            priority: "high".into(),
            offset: 9999,
            limit: 1,
            ..FindingComparisonRequest::default()
        };
        let expected = workspace
            .compare(&baseline.id, &current.id)
            .unwrap()
            .findings_page(&request)
            .unwrap()
            .total;
        assert!(expected > 0);
        for format in ExportFormat::ALL {
            let path = dir
                .path()
                .join(format!("finding-comparison.{}", format.extension()));
            assert_eq!(
                workspace
                    .export_finding_comparison(&baseline.id, &current.id, &request, &path)
                    .unwrap(),
                expected
            );
            let bytes = std::fs::read(&path).unwrap();
            if format == ExportFormat::Xlsx {
                assert!(bytes.starts_with(b"PK"));
            } else {
                let text = String::from_utf8(bytes).unwrap();
                for value in [
                    "issue_id",
                    "finding_change",
                    "before_finding",
                    "after_finding",
                    "persisting",
                ] {
                    assert!(text.contains(value));
                }
            }
            std::fs::write(&path, "existing report").unwrap();
            let invalid = FindingComparisonRequest {
                priority: "unknown' OR true".into(),
                ..request.clone()
            };
            assert!(
                workspace
                    .export_finding_comparison(&baseline.id, &current.id, &invalid, &path)
                    .is_err()
            );
            assert_eq!(std::fs::read_to_string(&path).unwrap(), "existing report");
        }
    }
}

#[cfg(test)]
mod anchor_name_tests;
#[cfg(test)]
mod base_resolution_tests;
#[cfg(test)]
mod canonical_tests;
#[cfg(test)]
mod header_link_tests;
#[cfg(test)]
mod header_scope_tests;
#[cfg(test)]
mod hreflang_eligibility_tests;
#[cfg(test)]
mod hreflang_header_tests;
#[cfg(test)]
mod html_encoding_tests;
#[cfg(test)]
mod http_scope_tests;
#[cfg(test)]
mod render_comparison_tests;
#[cfg(test)]
mod render_tests;
#[cfg(test)]
mod responsive_image_tests;
#[cfg(test)]
mod robots_encoding_tests;
#[cfg(test)]
mod robots_group_tests;
#[cfg(test)]
mod robots_status_tests;
#[cfg(test)]
mod schema_crawl_tests;
#[cfg(test)]
mod sitemap_identity_tests;
#[cfg(test)]
mod sitemap_page_tests;
#[cfg(test)]
mod sitemap_redirect_tests;
#[cfg(test)]
mod snippet_tests;
#[cfg(test)]
mod stop_rate_tests;
#[cfg(test)]
mod template_tests;
