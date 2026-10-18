//! Domain types, configuration and live state for the OctopusOn Index Checker.

use std::fmt;
use std::str::FromStr;
use std::sync::atomic::{AtomicU8, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

/// Search engine or index source that discovered a URL.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IndexEngine {
    /// Free Google Search (`site:` SERP).
    Google,
    /// Free Yandex Search (`site:` SERP).
    Yandex,
    /// Free Microsoft Bing Search (`site:` SERP).
    Bing,
    /// Free DuckDuckGo HTML Search (`site:` SERP).
    #[serde(rename = "duckduckgo", alias = "duck_duck_go", alias = "ddg")]
    DuckDuckGo,
    /// Free Internet Archive Wayback Machine CDX index.
    Wayback,
    /// Free Common Crawl CDX web index (`index.commoncrawl.org`).
    #[serde(rename = "commoncrawl", alias = "common_crawl", alias = "cc")]
    CommonCrawl,
    /// Live XML Sitemaps (`robots.txt` + `/sitemap.xml`).
    Sitemap,
    /// Google Custom Search JSON API.
    GoogleApi,
    /// Yandex Search / XML API.
    YandexApi,
    /// Serper.dev Google SERP API.
    SerperApi,
    /// SerpApi.com Search API (Google/Yandex/Bing).
    #[serde(rename = "serpapi", alias = "serp_api")]
    SerpApi,
    /// User-supplied seed / GSC URL list.
    SeedList,
}

impl IndexEngine {
    /// Machine identifier.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Google => "google",
            Self::Yandex => "yandex",
            Self::Bing => "bing",
            Self::DuckDuckGo => "duckduckgo",
            Self::Wayback => "wayback",
            Self::CommonCrawl => "commoncrawl",
            Self::Sitemap => "sitemap",
            Self::GoogleApi => "google_api",
            Self::YandexApi => "yandex_api",
            Self::SerperApi => "serper_api",
            Self::SerpApi => "serpapi",
            Self::SeedList => "seed_list",
        }
    }

    /// Default free search engines.
    pub fn default_free() -> Vec<Self> {
        vec![Self::Google, Self::Yandex, Self::Bing, Self::DuckDuckGo]
    }
}

impl fmt::Display for IndexEngine {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for IndexEngine {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "google" => Ok(Self::Google),
            "yandex" => Ok(Self::Yandex),
            "bing" => Ok(Self::Bing),
            "duckduckgo" | "ddg" => Ok(Self::DuckDuckGo),
            "wayback" | "archive" => Ok(Self::Wayback),
            "commoncrawl" | "cc" | "common_crawl" => Ok(Self::CommonCrawl),
            "sitemap" | "sitemaps" => Ok(Self::Sitemap),
            "google_api" | "google-api" => Ok(Self::GoogleApi),
            "yandex_api" | "yandex-api" => Ok(Self::YandexApi),
            "serper" | "serper_api" | "serper-api" => Ok(Self::SerperApi),
            "serpapi" | "serp_api" | "serp-api" => Ok(Self::SerpApi),
            "seed" | "seed_list" => Ok(Self::SeedList),
            other => Err(format!("unknown index engine `{other}`")),
        }
    }
}

/// Classification of an indexed URL after live HTTP and content verification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FindingCategory {
    /// HTTP 404 Not Found or 410 Gone (direct or after redirects).
    Error404,
    /// Other HTTP 4xx client errors (400, 401, 403, 429, 451, ...).
    Error4xx,
    /// HTTP 5xx server errors (500, 502, 503, 504, ...).
    Error5xx,
    /// Soft 404 (200 OK with "not found / bulunamadı" text or deep URL redirected to homepage).
    Soft404,
    /// HTTP 200 OK with very thin content (`word_count < min_word_count`).
    Thin200,
    /// Network / DNS / TLS / timeout failure while checking the URL.
    NetworkError,
    /// Healthy HTTP 200 OK page with sufficient content.
    Healthy200,
}

impl FindingCategory {
    /// Machine identifier stored in DuckDB and sent over IPC.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Error404 => "error_404",
            Self::Error4xx => "error_4xx",
            Self::Error5xx => "error_5xx",
            Self::Soft404 => "soft_404",
            Self::Thin200 => "thin_200",
            Self::NetworkError => "network_error",
            Self::Healthy200 => "healthy_200",
        }
    }

    /// Parses a stored category string.
    pub fn parse(value: &str) -> Self {
        match value {
            "error_404" => Self::Error404,
            "error_4xx" => Self::Error4xx,
            "error_5xx" => Self::Error5xx,
            "soft_404" => Self::Soft404,
            "thin_200" => Self::Thin200,
            "network_error" => Self::NetworkError,
            _ => Self::Healthy200,
        }
    }

    /// Whether this category represents an actionable issue.
    pub const fn is_issue(self) -> bool {
        !matches!(self, Self::Healthy200)
    }
}

/// Optional search API credentials configured by the user.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApiConfig {
    /// Google Custom Search JSON API key.
    #[serde(default)]
    pub google_api_key: String,
    /// Google Programmable Search Engine ID (`cx`).
    #[serde(default)]
    pub google_cx: String,
    /// Yandex Search / XML API key.
    #[serde(default)]
    pub yandex_api_key: String,
    /// Yandex Cloud folder ID or XML user name.
    #[serde(default)]
    pub yandex_user: String,
    /// Serper.dev API key for high-volume Google SERP queries.
    #[serde(default)]
    pub serper_api_key: String,
    /// SerpApi.com API key for Google/Yandex/Bing SERP queries.
    #[serde(default)]
    pub serpapi_key: String,
}

impl ApiConfig {
    /// Returns true if Google Custom Search API is configured.
    pub fn has_google_api(&self) -> bool {
        !self.google_api_key.trim().is_empty() && !self.google_cx.trim().is_empty()
    }

    /// Returns true if Yandex XML/Search API is configured.
    pub fn has_yandex_api(&self) -> bool {
        !self.yandex_api_key.trim().is_empty()
    }

    /// Returns true if Serper.dev API is configured.
    pub fn has_serper_api(&self) -> bool {
        !self.serper_api_key.trim().is_empty()
    }

    /// Returns true if SerpApi.com API is configured.
    pub fn has_serpapi(&self) -> bool {
        !self.serpapi_key.trim().is_empty()
    }
}

/// Configuration of an indexed 404 scan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[allow(clippy::struct_excessive_bools)]
pub struct ScanConfig {
    /// Target domain or root URL (e.g. `example.com` or `https://example.com`).
    pub domain: String,
    /// Include subdomains (`*.example.com`) when matching indexed URLs.
    #[serde(default)]
    pub include_subdomains: bool,
    /// Free search engines to query with `site:`.
    #[serde(default = "IndexEngine::default_free")]
    pub engines: Vec<IndexEngine>,
    /// Query Internet Archive Wayback Machine CDX API for historically indexed HTML URLs.
    #[serde(default)]
    pub use_wayback: bool,
    /// Query Common Crawl CDX index (`index.commoncrawl.org`) for indexed HTML URLs.
    #[serde(default)]
    pub use_commoncrawl: bool,
    /// Discover URLs listed in `robots.txt` and `/sitemap.xml` to catch broken sitemap entries.
    #[serde(default)]
    pub use_sitemaps: bool,
    /// Automatically expand `site:domain.com/subpath` queries from discovered top-level directories.
    #[serde(default)]
    pub expand_subpaths: bool,
    /// SERP result pages to fetch per search query (`1..=10`).
    #[serde(default = "default_pages_per_query")]
    pub pages_per_query: u16,
    /// Optional custom subpaths or keywords to append to `site:domain.com`.
    #[serde(default)]
    pub seed_queries: Vec<String>,
    /// Optional extra URLs pasted or imported from GSC / Yandex Webmaster.
    #[serde(default)]
    pub extra_urls: Vec<String>,
    /// Optional API credentials (Google Custom Search, Yandex XML, Serper.dev, SerpApi).
    #[serde(default)]
    pub api: ApiConfig,
    /// Report all 4xx client errors (400, 403, 410, 429, ...) in addition to 404.
    #[serde(default = "default_true")]
    pub detect_all_4xx: bool,
    /// Report 5xx server errors (500, 502, 503, 504).
    #[serde(default = "default_true")]
    pub detect_5xx: bool,
    /// Report 200 OK pages that have very thin content (`word_count < min_word_count`) or soft-404 signals.
    #[serde(default = "default_true")]
    pub detect_thin_200: bool,
    /// Minimum visible word count required for a 200 OK HTML page to be considered healthy.
    #[serde(default = "default_min_word_count")]
    pub min_word_count: u32,
    /// Concurrent HTTP verification workers (`1..=32`).
    #[serde(default = "default_concurrency")]
    pub concurrency: u16,
    /// Timeout per HTTP request in seconds (`3..=60`).
    #[serde(default = "default_timeout_seconds")]
    pub timeout_seconds: u16,
    /// Maximum redirect hops to follow when checking an indexed URL (`0..=10`).
    #[serde(default = "default_max_redirects")]
    pub max_redirects: u8,
    /// User-Agent string sent during live verification.
    #[serde(default = "default_user_agent")]
    pub user_agent: String,
}

const fn default_true() -> bool {
    true
}
const fn default_pages_per_query() -> u16 {
    3
}
const fn default_min_word_count() -> u32 {
    60
}
const fn default_concurrency() -> u16 {
    8
}
const fn default_timeout_seconds() -> u16 {
    15
}
const fn default_max_redirects() -> u8 {
    5
}
fn default_user_agent() -> String {
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36 (compatible; octopusSEO-Finder/0.1)".to_owned()
}

impl Default for ScanConfig {
    fn default() -> Self {
        Self {
            domain: String::new(),
            include_subdomains: false,
            engines: IndexEngine::default_free(),
            use_wayback: false,
            use_commoncrawl: false,
            use_sitemaps: false,
            expand_subpaths: false,
            pages_per_query: default_pages_per_query(),
            seed_queries: Vec::new(),
            extra_urls: Vec::new(),
            api: ApiConfig::default(),
            detect_all_4xx: true,
            detect_5xx: true,
            detect_thin_200: true,
            min_word_count: default_min_word_count(),
            concurrency: default_concurrency(),
            timeout_seconds: default_timeout_seconds(),
            max_redirects: default_max_redirects(),
            user_agent: default_user_agent(),
        }
    }
}

impl ScanConfig {
    /// Extracts the normalized registrable host (without `www.` or scheme) from `self.domain`.
    pub fn normalized_host(&self) -> Option<String> {
        let trimmed = self.domain.trim();
        if trimmed.is_empty() {
            return None;
        }
        let candidate = if trimmed.contains("://") {
            trimmed.to_owned()
        } else {
            format!("https://{trimmed}")
        };
        let parsed = url::Url::parse(&candidate).ok()?;
        let host = parsed.host_str()?.to_ascii_lowercase();
        let stripped = host.strip_prefix("www.").unwrap_or(&host).to_owned();
        if stripped.is_empty() || !stripped.contains('.') {
            return None;
        }
        Some(stripped)
    }

    /// Checks whether a URL belongs to the target domain scope.
    pub fn matches_domain(&self, raw_url: &str, base_host: &str) -> bool {
        let Ok(parsed) = url::Url::parse(raw_url) else {
            return false;
        };
        if !matches!(parsed.scheme(), "http" | "https") {
            return false;
        }
        let Some(host) = parsed.host_str() else {
            return false;
        };
        let host_lower = host.to_ascii_lowercase();
        let clean = host_lower.strip_prefix("www.").unwrap_or(&host_lower);
        if clean == base_host {
            return true;
        }
        if self.include_subdomains {
            let suffix = format!(".{base_host}");
            return clean.ends_with(&suffix);
        }
        false
    }
}

/// An indexed URL discovered from one or more search engines or index sources.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiscoveredUrl {
    /// Normalized HTTP(S) URL.
    pub url: String,
    /// Search engines / sources where this URL was found.
    pub engines: Vec<IndexEngine>,
    /// Title shown on the search engine result page (if available).
    pub serp_title: String,
    /// Snippet shown on the search engine result page (if available).
    pub serp_snippet: String,
    /// The search query or source label that first discovered this URL.
    pub found_by_query: String,
}

/// A verified indexed URL with its live HTTP response and content metrics.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerifiedUrl {
    /// Normalized indexed URL.
    pub url: String,
    /// Comma-separated list of engines that indexed this URL (`google,yandex`).
    pub engines: String,
    /// Title captured from SERP.
    pub serp_title: String,
    /// Snippet captured from SERP.
    pub serp_snippet: String,
    /// Query or source that discovered the URL.
    pub found_by_query: String,
    /// First HTTP status code received (e.g. `301`, `404`, `200`).
    pub initial_status: u16,
    /// Final HTTP status code after following redirects.
    pub final_status: u16,
    /// Number of redirect hops followed.
    pub redirect_hops: u8,
    /// Final destination URL after redirects (or equal to `url`).
    pub final_url: String,
    /// Total HTTP check duration in milliseconds.
    pub response_time_ms: u32,
    /// `Content-Type` header of the final response.
    pub content_type: String,
    /// Live `<title>` extracted from the HTML body.
    pub live_title: String,
    /// First `<h1>` extracted from the HTML body.
    pub h1: String,
    /// Visible word count of the HTML body.
    pub word_count: u32,
    /// Canonical URL declared on the live page.
    pub canonical: String,
    /// Whether `noindex` is set in meta robots or `X-Robots-Tag`.
    pub noindex: bool,
    /// Computed finding category.
    pub category: FindingCategory,
    /// Specific machine reason code (`http_404`, `http_410`, `redirect_to_404`, `thin_content_200`, `soft_404_phrase`, ...).
    pub reason: String,
    /// Suggested 301 redirect target URL when the page is broken.
    pub suggested_target: String,
    /// Network error details if the request failed.
    pub error_message: String,
}

/// Phase of a running scan job.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScanPhase {
    /// Preparing configuration and storage.
    Starting,
    /// Querying search engines, Wayback and APIs for indexed URLs.
    Discovering,
    /// Checking discovered URLs over HTTP and analyzing content.
    Verifying,
    /// Committing results to DuckDB.
    Saving,
    /// Completed normally.
    Completed,
    /// Stopped early by user.
    Stopped,
    /// Failed with an error.
    Failed,
}

/// Live progress snapshot of a running scan.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScanProgress {
    /// Queries executed across search engines / APIs.
    pub queries_completed: u32,
    /// Total planned queries (grows if subpath expansion adds queries).
    pub queries_total: u32,
    /// Unique in-scope indexed URLs discovered so far.
    pub discovered_urls: u32,
    /// URLs verified via HTTP so far.
    pub verified_urls: u32,
    /// 404 / 410 URLs found so far.
    pub found_404: u32,
    /// Other 4xx URLs found so far.
    pub found_4xx: u32,
    /// 5xx URLs found so far.
    pub found_5xx: u32,
    /// Thin 200 / Soft-404 URLs found so far.
    pub found_thin_soft404: u32,
    /// Currently active query or URL being checked.
    pub current_item: String,
    /// Live preview of discovered and verified URLs (up to 250 entries, prioritized by issues).
    #[serde(default)]
    pub live_rows: Vec<VerifiedUrl>,
    /// Search API requests paid with OctopusOn SEO credits so far.
    #[serde(default)]
    pub credits: CreditUsage,
}

/// Search API requests of a scan paid with OctopusOn SEO credits.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreditUsage {
    /// Sources that ran on credits (`serper`, `serpapi`).
    #[serde(default)]
    pub sources: Vec<String>,
    /// Requests the OctopusOn SEO gateway answered.
    #[serde(default)]
    pub requests: u32,
    /// What the gateway charged, in micro-dollars.
    #[serde(default)]
    pub charged_micros: u64,
    /// Why a source stopped running on credits during the scan (signed out, out of credits).
    #[serde(default)]
    pub stopped: Option<String>,
}

/// Summary metadata of a saved scan session (`scan.json`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScanMeta {
    /// Unique scan identifier.
    pub id: String,
    /// Normalized domain name.
    pub domain: String,
    /// Unix epoch milliseconds when scan started.
    pub created_at_ms: u64,
    /// Unix epoch milliseconds when scan finished.
    pub finished_at_ms: u64,
    /// Status (`running`, `completed`, `stopped`, `failed`).
    pub status: String,
    /// Configuration used for the scan.
    pub config: ScanConfig,
    /// Total search queries executed.
    pub queries_executed: u32,
    /// Total unique indexed URLs discovered.
    pub discovered_urls: u32,
    /// Total URLs verified over HTTP.
    pub verified_urls: u32,
    /// Count of HTTP 404 / 410 URLs.
    pub count_404: u32,
    /// Count of other HTTP 4xx URLs.
    pub count_4xx: u32,
    /// Count of HTTP 5xx URLs.
    pub count_5xx: u32,
    /// Count of Soft-404 URLs.
    pub count_soft_404: u32,
    /// Count of Thin 200 OK URLs (`word_count < min_word_count`).
    pub count_thin_200: u32,
    /// Count of network errors.
    pub count_network_error: u32,
    /// Count of healthy 200 OK URLs.
    pub count_healthy: u32,
    /// Search API requests paid with OctopusOn SEO credits and what they cost.
    #[serde(default)]
    pub credits: CreditUsage,
}

impl ScanMeta {
    /// Current UTC timestamp in milliseconds.
    pub fn now_ms() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
    }

    /// Health score (0..=100) based on the ratio of healthy indexed pages to total verified pages.
    pub fn health_score(&self) -> u8 {
        if self.verified_urls == 0 {
            return 100;
        }
        let issues = self
            .count_404
            .saturating_add(self.count_4xx)
            .saturating_add(self.count_5xx)
            .saturating_add(self.count_soft_404)
            .saturating_add(self.count_thin_200)
            .saturating_add(self.count_network_error);
        let healthy = self.verified_urls.saturating_sub(issues);
        let pct = (u64::from(healthy) * 100) / u64::from(self.verified_urls);
        u8::try_from(pct.min(100)).unwrap_or(100)
    }
}

/// Breakdown of discovered URLs by search engine / source.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EngineSummary {
    /// Engine identifier (`google`, `yandex`, `bing`, `duckduckgo`, `wayback`, ...).
    pub engine: String,
    /// Total URLs indexed by this engine.
    pub total_urls: u32,
    /// Broken / 404 / thin issue URLs on this engine.
    pub issue_urls: u32,
}

/// Complete overview returned to the UI and CLI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScanOverview {
    /// Scan metadata and counters.
    pub meta: ScanMeta,
    /// Health score (0..=100).
    pub health_score: u8,
    /// Per-engine discovery and issue breakdown.
    pub engines: Vec<EngineSummary>,
    /// Top-level directory / URL path clusters and issue concentration.
    #[serde(default)]
    pub clusters: Vec<crate::cluster::PathCluster>,
    /// Top critical 404 / broken URLs for immediate action.
    pub top_issues: Vec<VerifiedUrl>,
}

/// Filter and pagination request for the URL explorer table.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FinderPageRequest {
    /// Category filter: `all`, `issues`, `error_404`, `error_4xx`, `error_5xx`, `thin_soft_404`, `thin_200`, `soft_404`, `healthy_200`, `network_error`.
    #[serde(default)]
    pub category: String,
    /// Engine filter (`google`, `yandex`, `bing`, `duckduckgo`, `wayback`, or empty for all).
    #[serde(default)]
    pub engine: String,
    /// Substring search on URL, SERP title or live title.
    #[serde(default)]
    pub search: String,
    /// Maximum word count filter (optional).
    #[serde(default)]
    pub max_words: Option<u32>,
    /// Optional sort key (`priority`, `words_asc`, `words_desc`, `time_desc`, `status_desc`, `url_asc`).
    #[serde(default)]
    pub sort_by: String,
    /// Zero-based row offset.
    #[serde(default)]
    pub offset: u32,
    /// Page size (`1..=500`).
    #[serde(default = "default_limit")]
    pub limit: u32,
}

const fn default_limit() -> u32 {
    100
}

/// A paginated slice of verified URLs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FinderTablePage {
    /// Total rows matching the filter.
    pub total: u32,
    /// Rows for the current page.
    pub rows: Vec<VerifiedUrl>,
}

/// Thread-safe pause / resume / stop control for a running scan.
#[derive(Debug, Default)]
pub struct ScanControl {
    state: AtomicU8,
}

const STATE_RUNNING: u8 = 0;
const STATE_PAUSED: u8 = 1;
const STATE_STOPPED: u8 = 2;

impl ScanControl {
    /// Creates a new running control.
    pub fn new() -> Self {
        Self {
            state: AtomicU8::new(STATE_RUNNING),
        }
    }

    /// Pauses the scan.
    pub fn pause(&self) {
        let _ = self.state.compare_exchange(
            STATE_RUNNING,
            STATE_PAUSED,
            Ordering::SeqCst,
            Ordering::SeqCst,
        );
    }

    /// Resumes a paused scan.
    pub fn resume(&self) {
        let _ = self.state.compare_exchange(
            STATE_PAUSED,
            STATE_RUNNING,
            Ordering::SeqCst,
            Ordering::SeqCst,
        );
    }

    /// Stops the scan.
    pub fn stop(&self) {
        self.state.store(STATE_STOPPED, Ordering::SeqCst);
    }

    /// Whether the scan has been stopped.
    pub fn is_stopped(&self) -> bool {
        self.state.load(Ordering::SeqCst) == STATE_STOPPED
    }

    /// Whether the scan is currently paused.
    pub fn is_paused(&self) -> bool {
        self.state.load(Ordering::SeqCst) == STATE_PAUSED
    }

    /// Waits while paused; returns `false` if stopped.
    pub fn wait_if_paused(&self) -> bool {
        while self.is_paused() {
            std::thread::sleep(std::time::Duration::from_millis(120));
        }
        !self.is_stopped()
    }
}

/// Change status of an indexed URL between a baseline scan and a target scan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiffStatus {
    /// Was broken (`404`/`4xx`/`5xx`/`soft_404`/`thin_200`) in baseline scan, now `Healthy200`.
    Resolved,
    /// Newly broken in target scan (was `Healthy200` or not present in baseline).
    NewIssue,
    /// Broken in both baseline and target scans.
    PersistentIssue,
}

/// A single URL entry in a two-scan comparison report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComparedUrl {
    /// Normalized URL.
    pub url: String,
    /// Search engines that indexed the URL.
    pub engines: String,
    /// Change classification (`resolved`, `new_issue`, `persistent_issue`).
    pub diff: DiffStatus,
    /// Previous HTTP final status (0 if not in baseline scan).
    pub base_status: u16,
    /// Current HTTP final status in target scan.
    pub target_status: u16,
    /// Previous finding category (`healthy_200` if not in baseline).
    pub base_category: FindingCategory,
    /// Current finding category in target scan.
    pub target_category: FindingCategory,
    /// Suggested 301 redirect target URL.
    pub suggested_target: String,
}

/// Summary and row list comparing two scan sessions (`base_scan_id` vs `target_scan_id`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScanComparison {
    /// Baseline (older) scan ID.
    pub base_scan_id: String,
    /// Target (newer) scan ID.
    pub target_scan_id: String,
    /// Domain of the target scan.
    pub domain: String,
    /// Number of previously broken URLs that are now healthy (`200 OK`).
    pub resolved_count: u32,
    /// Number of newly broken URLs in the target scan.
    pub new_issue_count: u32,
    /// Number of URLs that remain broken across both scans.
    pub persistent_issue_count: u32,
    /// Detailed diff rows (up to 500 most actionable entries).
    pub rows: Vec<ComparedUrl>,
}
