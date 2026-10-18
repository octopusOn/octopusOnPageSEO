//! Crawl settings and their validated form.

use std::time::Duration;

use regex::Regex;
use serde::{Deserialize, Serialize};
use url::Url;

use crate::scope::ScopeRules;

/// Everything a crawl can be told. Deserializes from partial JSON: missing fields keep their
/// defaults.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CrawlConfig {
    /// Spider mode: where the crawl starts. List mode: every URL to fetch.
    pub start_urls: Vec<String>,
    /// Whether links are followed.
    pub mode: CrawlMode,
    /// Which crawler the requests identify as.
    pub agent: AgentPreset,
    /// User-agent header overriding the preset's.
    pub user_agent: Option<String>,
    /// `Accept-Language` header, if any.
    pub accept_language: Option<String>,
    /// How robots.txt is honored.
    pub robots: RobotsPolicy,
    /// Which references are followed.
    pub follow: Follow,
    /// How far and how much is crawled.
    pub limits: Limits,
    /// Which URLs are in scope and how they are rewritten.
    pub urls: UrlRules,
    /// Parallelism and politeness.
    pub speed: Speed,
    /// What is kept beyond the extracted facts.
    pub storage: Storage,
    /// Optional execution of JavaScript in a local browser.
    pub rendering: Rendering,
    /// Validate structured-data properties during this crawl and include confirmed errors in scoring.
    pub schema_validation: bool,
}

impl Default for CrawlConfig {
    fn default() -> Self {
        Self {
            start_urls: Vec::new(),
            mode: CrawlMode::Spider,
            agent: AgentPreset::Octopus,
            user_agent: None,
            accept_language: None,
            robots: RobotsPolicy::Respect,
            follow: Follow::default(),
            limits: Limits::default(),
            urls: UrlRules::default(),
            speed: Speed::default(),
            storage: Storage::default(),
            rendering: Rendering::default(),
            schema_validation: false,
        }
    }
}

/// Whether links are followed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CrawlMode {
    /// Follow links from the start URLs.
    Spider,
    /// Fetch only the given URLs (and the redirects they lead to).
    List,
}

/// The crawler the requests identify as. The robots.txt group obeyed follows the preset, so
/// a site can be seen the way Googlebot sees it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AgentPreset {
    /// OctopusOn SEO Crawler's own user agent.
    Octopus,
    /// Googlebot Smartphone.
    GooglebotSmartphone,
    /// Googlebot Desktop.
    GooglebotDesktop,
    /// Bingbot.
    Bingbot,
    /// A desktop Chrome browser (obeys the `*` robots.txt group).
    Chrome,
}

impl AgentPreset {
    /// The `User-Agent` header.
    pub fn user_agent(self) -> &'static str {
        match self {
            Self::Octopus => {
                "Mozilla/5.0 (compatible; octopusSEO-Spider/0.1; +https://octopuson.com/spider)"
            }
            Self::GooglebotSmartphone => {
                "Mozilla/5.0 (Linux; Android 6.0.1; Nexus 5X Build/MMB29P) AppleWebKit/537.36 \
                 (KHTML, like Gecko) Chrome/138.0.0.0 Mobile Safari/537.36 \
                 (compatible; Googlebot/2.1; +http://www.google.com/bot.html)"
            }
            Self::GooglebotDesktop => {
                "Mozilla/5.0 (compatible; Googlebot/2.1; +http://www.google.com/bot.html)"
            }
            Self::Bingbot => {
                "Mozilla/5.0 (compatible; bingbot/2.0; +http://www.bing.com/bingbot.htm)"
            }
            Self::Chrome => {
                "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 \
                 (KHTML, like Gecko) Chrome/138.0.0.0 Safari/537.36"
            }
        }
    }

    /// Robots tokens, most specific first, used for robots.txt groups, robots meta tags and
    /// `X-Robots-Tag` scopes.
    pub fn robots_tokens(self) -> &'static [&'static str] {
        match self {
            Self::Octopus => &["octopusseo-spider", "octopusseo"],
            Self::GooglebotSmartphone | Self::GooglebotDesktop => &["googlebot"],
            Self::Bingbot => &["bingbot"],
            Self::Chrome => &[],
        }
    }
}

/// How robots.txt is honored.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RobotsPolicy {
    /// Blocked URLs are reported but not fetched.
    Respect,
    /// Blocked URLs are fetched and flagged.
    IgnoreButReport,
    /// robots.txt is not read.
    Ignore,
}

/// Which references the crawler follows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
#[allow(clippy::struct_excessive_bools)]
pub struct Follow {
    /// Follow internal `rel="nofollow"` links.
    pub internal_nofollow: bool,
    /// Check external `rel="nofollow"` links.
    pub external_nofollow: bool,
    /// Follow links of pages whose robots directives say `nofollow`.
    pub nofollow_pages: bool,
    /// Fetch external URLs once to report their status (they are never crawled further).
    pub check_external: bool,
    /// Check images.
    pub images: bool,
    /// Check stylesheets.
    pub stylesheets: bool,
    /// Check scripts.
    pub scripts: bool,
    /// Check iframes and audio/video sources.
    pub media: bool,
    /// Crawl canonical targets.
    pub canonicals: bool,
    /// Crawl hreflang alternates.
    pub hreflang: bool,
    /// Crawl `rel="next"` / `rel="prev"` targets.
    pub pagination: bool,
    /// Crawl meta refresh targets.
    pub meta_refresh: bool,
    /// Crawl AMP versions.
    pub amp: bool,
    /// Read sitemaps from robots.txt (or `/sitemap.xml`) and crawl the URLs no link reaches.
    pub sitemaps: bool,
    /// Further sitemaps to read.
    pub sitemap_urls: Vec<String>,
}

impl Default for Follow {
    fn default() -> Self {
        Self {
            internal_nofollow: false,
            external_nofollow: false,
            nofollow_pages: false,
            check_external: true,
            images: true,
            stylesheets: true,
            scripts: true,
            media: false,
            canonicals: true,
            hreflang: true,
            pagination: true,
            meta_refresh: true,
            amp: true,
            sitemaps: true,
            sitemap_urls: Vec::new(),
        }
    }
}

/// How far and how much is crawled.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Limits {
    /// URLs fetched at most (pages, resources and external checks); 0 = no limit.
    pub max_urls: u32,
    /// Clicks from a start URL at most; `None` = no limit.
    pub max_depth: Option<u32>,
    /// Longer URLs are not followed.
    pub max_url_length: usize,
    /// URLs with more query parameters are not followed; `None` = no limit.
    pub max_query_params: Option<usize>,
    /// Response bodies are cut after this many bytes.
    pub max_body_bytes: u64,
    /// Seconds before a request is abandoned.
    pub timeout_seconds: u32,
    /// Sitemaps read at most (indexes count too).
    pub max_sitemaps: u32,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_urls: 0,
            max_depth: None,
            max_url_length: 2048,
            max_query_params: None,
            max_body_bytes: 16 * 1024 * 1024,
            timeout_seconds: 20,
            max_sitemaps: 500,
        }
    }
}

/// Which URLs are in scope and how they are rewritten before crawling.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
#[allow(clippy::struct_excessive_bools)]
pub struct UrlRules {
    /// Subdomains of the start host count as internal.
    pub subdomains: bool,
    /// Only URLs inside the start URL's folder are crawled.
    pub start_folder_only: bool,
    /// Regexes a URL must match one of to be crawled (empty = all).
    pub include: Vec<String>,
    /// Regexes excluding URLs from the crawl.
    pub exclude: Vec<String>,
    /// Query parameters removed from every URL (case-insensitive names).
    pub strip_params: Vec<String>,
    /// Remove every query string.
    pub strip_all_params: bool,
}

impl Default for UrlRules {
    fn default() -> Self {
        Self {
            subdomains: false,
            start_folder_only: false,
            include: Vec::new(),
            exclude: crate::DEFAULT_URL_EXCLUSIONS
                .iter()
                .map(|pattern| (*pattern).to_owned())
                .collect(),
            strip_params: Vec::new(),
            strip_all_params: false,
        }
    }
}

/// Parallelism and politeness.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Speed {
    /// Parallel requests, from 1 to 64.
    pub threads: usize,
    /// Finite, nonnegative requests per second at most; 0 = no limit.
    pub max_requests_per_second: f64,
}

impl Default for Speed {
    fn default() -> Self {
        Self {
            threads: 5,
            max_requests_per_second: 0.0,
        }
    }
}

/// What is kept beyond the extracted facts.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Storage {
    /// Keep every response header.
    pub headers: bool,
    /// Keep the HTML of internal pages.
    pub html: bool,
}

/// Which representation is analyzed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RenderMode {
    /// Analyze the server-returned HTML only.
    #[default]
    Html,
    /// Execute JavaScript and analyze the resulting DOM.
    Javascript,
}

/// Optional local Chrome/Chromium/Edge rendering settings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Rendering {
    /// Server HTML or browser-rendered DOM.
    pub mode: RenderMode,
    /// Minimum settling time after `DOMContentLoaded`, in milliseconds.
    pub wait_ms: u32,
    /// Time allowed for an active browser page; excludes waiting for a browser slot.
    pub timeout_seconds: u32,
    /// Concurrent browser pages; independent of HTTP worker count.
    pub concurrency: usize,
    /// Reuse public, immutable JavaScript/CSS resources within this crawl.
    pub cache_resources: bool,
    /// Explicit browser executable; None discovers an installed browser.
    pub browser_path: Option<String>,
    /// Explicit browser layout; None retains the selected agent's existing defaults.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub viewport: Option<Viewport>,
}

impl Default for Rendering {
    fn default() -> Self {
        Self {
            mode: RenderMode::Html,
            wait_ms: 1000,
            timeout_seconds: 30,
            concurrency: 2,
            cache_resources: true,
            browser_path: None,
            viewport: None,
        }
    }
}

/// Browser viewport in CSS pixels, independent of the HTTP user-agent identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Viewport {
    /// Layout width, from 240 to 3840 CSS pixels.
    pub width: u32,
    /// Layout height, from 240 to 3840 CSS pixels.
    pub height: u32,
    /// Chrome mobile layout, including viewport meta-tag handling; not touch emulation.
    #[serde(default)]
    pub mobile: bool,
}

impl Viewport {
    /// Whether both dimensions fit the application's bounded rendering sizes.
    pub fn is_valid(self) -> bool {
        (240..=3840).contains(&self.width) && (240..=3840).contains(&self.height)
    }
}

impl Rendering {
    /// Explicit layout or the historical 412×915 smartphone / 1280×800 desktop defaults.
    pub fn effective_viewport(&self, agent: AgentPreset) -> Viewport {
        self.viewport.unwrap_or_else(|| {
            let mobile = agent == AgentPreset::GooglebotSmartphone;
            Viewport {
                width: if mobile { 412 } else { 1280 },
                height: if mobile { 915 } else { 800 },
                mobile,
            }
        })
    }
}

/// An invalid setting.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ConfigError {
    /// No start URL was given.
    #[error("no start URL")]
    NoStartUrl,
    /// HTTP worker count is outside the supported range.
    #[error("concurrent requests must be between 1 and 64")]
    InvalidConcurrency,
    /// The request ceiling is negative or nonfinite.
    #[error("requests per second must be finite and nonnegative (0 means unlimited)")]
    InvalidRequestRate,
    /// Rendering concurrency, wait or timeout is out of bounds.
    #[error(
        "rendering requires 1–8 browser pages, a 1–120 second timeout and a wait shorter than that timeout"
    )]
    InvalidRendering,
    /// An explicit rendering viewport exceeds the supported dimensions.
    #[error("rendering viewport width and height must be between 240 and 3840 CSS pixels")]
    InvalidViewport,
    /// A start URL is not an absolute http(s) URL.
    #[error("`{0}` is not an http(s) URL")]
    InvalidUrl(String),
    /// An include or exclude pattern is not a valid regex.
    #[error("invalid pattern `{pattern}`: {message}")]
    InvalidPattern {
        /// The pattern as written.
        pattern: String,
        /// Why it does not compile.
        message: String,
    },
}

/// A validated [`CrawlConfig`], ready to run.
#[derive(Debug, Clone)]
pub struct CrawlPlan {
    pub(crate) config: CrawlConfig,
    pub(crate) start: Vec<Url>,
    pub(crate) scope: ScopeRules,
}

impl CrawlPlan {
    /// Validates start URLs, patterns, HTTP speed and rendering settings.
    ///
    /// # Errors
    /// The first invalid setting.
    pub fn new(config: CrawlConfig) -> Result<Self, ConfigError> {
        if !(1..=64).contains(&config.speed.threads) {
            return Err(ConfigError::InvalidConcurrency);
        }
        let rate = config.speed.max_requests_per_second;
        if !rate.is_finite() || rate < 0.0 {
            return Err(ConfigError::InvalidRequestRate);
        }
        let rendering = &config.rendering;
        if rendering.concurrency == 0
            || rendering.concurrency > 8
            || !(1..=120).contains(&rendering.timeout_seconds)
            || rendering.wait_ms >= rendering.timeout_seconds.saturating_mul(1000)
        {
            return Err(ConfigError::InvalidRendering);
        }
        if rendering
            .viewport
            .is_some_and(|viewport| !viewport.is_valid())
        {
            return Err(ConfigError::InvalidViewport);
        }
        let mut start = Vec::new();
        for raw in &config.start_urls {
            let raw = raw.trim();
            if raw.is_empty() {
                continue;
            }
            let url =
                parse_start_url(raw).ok_or_else(|| ConfigError::InvalidUrl(raw.to_owned()))?;
            start.push(url);
        }
        if start.is_empty() {
            return Err(ConfigError::NoStartUrl);
        }
        let compile = |patterns: &[String]| {
            patterns
                .iter()
                .filter(|p| !p.trim().is_empty())
                .map(|pattern| {
                    Regex::new(pattern).map_err(|error| ConfigError::InvalidPattern {
                        pattern: pattern.clone(),
                        message: error.to_string(),
                    })
                })
                .collect::<Result<Vec<_>, _>>()
        };
        let include = compile(&config.urls.include)?;
        let exclude = compile(&config.urls.exclude)?;
        let scope = ScopeRules::new(&config, &start, include, exclude);
        let start = start.into_iter().map(|url| scope.normalize(url)).collect();
        Ok(Self {
            config,
            start,
            scope,
        })
    }

    /// The settings.
    pub fn config(&self) -> &CrawlConfig {
        &self.config
    }

    /// Normalized HTTP page identity, independent of whether scope permits a fetch.
    /// Sitemap source files intentionally use a different identity that retains queries.
    pub fn normalize_http_url(&self, raw: &str) -> Option<Url> {
        let url = Url::parse(raw.trim()).ok()?;
        crate::scope::is_http_url(&url).then(|| self.scope.normalize(url))
    }

    /// The `User-Agent` header requests send.
    pub fn user_agent(&self) -> &str {
        self.config
            .user_agent
            .as_deref()
            .unwrap_or_else(|| self.config.agent.user_agent())
    }

    /// Per-request timeout.
    pub fn timeout(&self) -> Duration {
        Duration::from_secs(u64::from(self.config.limits.timeout_seconds.max(1)))
    }

    pub(crate) fn robots_tokens(&self) -> &'static [&'static str] {
        self.config.agent.robots_tokens()
    }
}

/// `example.com` and `//example.com` become `https://example.com/`.
fn parse_start_url(raw: &str) -> Option<Url> {
    let candidate = if raw.contains("://") {
        raw.to_owned()
    } else {
        format!("https://{}", raw.trim_start_matches('/'))
    };
    let mut url = Url::parse(&candidate).ok()?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        return None;
    }
    url.set_fragment(None);
    Some(url)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_start_urls_and_patterns() {
        let config = CrawlConfig {
            start_urls: vec!["example.com".into(), " ".into()],
            ..CrawlConfig::default()
        };
        let plan = CrawlPlan::new(config).unwrap();
        assert_eq!(plan.start[0].as_str(), "https://example.com/");

        let empty = CrawlConfig::default();
        assert_eq!(CrawlPlan::new(empty).unwrap_err(), ConfigError::NoStartUrl);

        let ftp = CrawlConfig {
            start_urls: vec!["ftp://example.com/".into()],
            ..CrawlConfig::default()
        };
        assert!(matches!(
            CrawlPlan::new(ftp),
            Err(ConfigError::InvalidUrl(_))
        ));

        let mut bad = CrawlConfig {
            start_urls: vec!["https://example.com/".into()],
            ..CrawlConfig::default()
        };
        bad.urls.exclude = vec!["(".into()];
        assert!(matches!(
            CrawlPlan::new(bad),
            Err(ConfigError::InvalidPattern { .. })
        ));
    }

    #[test]
    fn validates_rendering_bounds() {
        let mut config = CrawlConfig {
            start_urls: vec!["https://example.com/".into()],
            ..CrawlConfig::default()
        };
        config.rendering.concurrency = 0;
        assert_eq!(
            CrawlPlan::new(config.clone()).unwrap_err(),
            ConfigError::InvalidRendering
        );
        config.rendering.concurrency = 2;
        config.rendering.timeout_seconds = 1;
        config.rendering.wait_ms = 1000;
        assert_eq!(
            CrawlPlan::new(config.clone()).unwrap_err(),
            ConfigError::InvalidRendering
        );
        config.rendering.wait_ms = 0;
        assert!(CrawlPlan::new(config).is_ok());
    }

    #[test]
    fn partial_json_keeps_defaults() {
        let config: CrawlConfig =
            serde_json::from_str(r#"{"start_urls":["https://a.com"],"speed":{"threads":2}}"#)
                .unwrap();
        assert_eq!(config.speed.threads, 2);
        assert!(config.follow.check_external);
        assert_eq!(config.limits.timeout_seconds, 20);
        assert_eq!(config.rendering.mode, RenderMode::Html);
        assert_eq!(config.rendering.concurrency, 2);
        assert_eq!(config.rendering.viewport, None);
    }

    #[test]
    fn viewport_defaults_preserve_agent_layout_and_explicit_settings_win() {
        let rendering = Rendering::default();
        assert_eq!(
            rendering.effective_viewport(AgentPreset::GooglebotSmartphone),
            Viewport {
                width: 412,
                height: 915,
                mobile: true
            }
        );
        for agent in [
            AgentPreset::Octopus,
            AgentPreset::Chrome,
            AgentPreset::GooglebotDesktop,
            AgentPreset::Bingbot,
        ] {
            assert_eq!(
                rendering.effective_viewport(agent),
                Viewport {
                    width: 1280,
                    height: 800,
                    mobile: false
                }
            );
        }
        let explicit = Viewport {
            width: 768,
            height: 1024,
            mobile: false,
        };
        let rendering = Rendering {
            viewport: Some(explicit),
            ..rendering
        };
        assert_eq!(
            rendering.effective_viewport(AgentPreset::GooglebotSmartphone),
            explicit
        );
    }

    #[test]
    fn rejects_unbounded_viewports_before_creating_a_crawl() {
        let mut config = CrawlConfig {
            start_urls: vec!["https://example.com/".into()],
            ..CrawlConfig::default()
        };
        for size in [0, 239, 3841, u32::MAX] {
            for (width, height) in [(size, 800), (1280, size)] {
                config.rendering.viewport = Some(Viewport {
                    width,
                    height,
                    mobile: true,
                });
                assert_eq!(
                    CrawlPlan::new(config.clone()).unwrap_err(),
                    ConfigError::InvalidViewport
                );
            }
        }
        for size in [240, 390, 1280, 3840] {
            config.rendering.viewport = Some(Viewport {
                width: size,
                height: size,
                mobile: false,
            });
            assert!(CrawlPlan::new(config.clone()).is_ok());
        }
    }

    #[test]
    fn viewport_json_round_trips_and_old_configs_do_not_gain_an_explicit_override() {
        let config: CrawlConfig = serde_json::from_str(r#"{"rendering":{"mode":"javascript","viewport":{"width":390,"height":844,"mobile":true}}}"#).unwrap();
        assert_eq!(
            config.rendering.viewport,
            Some(Viewport {
                width: 390,
                height: 844,
                mobile: true
            })
        );
        let encoded = serde_json::to_string(&config).unwrap();
        assert_eq!(
            serde_json::from_str::<CrawlConfig>(&encoded).unwrap(),
            config
        );
        let legacy: CrawlConfig =
            serde_json::from_str(r#"{"rendering":{"mode":"javascript"}}"#).unwrap();
        assert!(
            !serde_json::to_value(legacy).unwrap()["rendering"]
                .as_object()
                .unwrap()
                .contains_key("viewport")
        );
        assert!(
            !serde_json::from_str::<Viewport>(r#"{"width":800,"height":600}"#)
                .unwrap()
                .mobile
        );
        assert!(serde_json::from_str::<Viewport>(r#"{"width":800}"#).is_err());
    }

    #[test]
    fn rejects_invalid_speed_instead_of_silently_changing_it() {
        let mut config = CrawlConfig {
            start_urls: vec!["https://example.com/".into()],
            ..CrawlConfig::default()
        };
        for threads in [0, 65, usize::MAX] {
            config.speed.threads = threads;
            assert_eq!(
                CrawlPlan::new(config.clone()).unwrap_err(),
                ConfigError::InvalidConcurrency,
                "threads={threads}"
            );
        }
        config.speed.threads = 5;
        for rate in [-1.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            config.speed.max_requests_per_second = rate;
            assert_eq!(
                CrawlPlan::new(config.clone()).unwrap_err(),
                ConfigError::InvalidRequestRate,
                "rate={rate}"
            );
        }
        for threads in [1, 64] {
            config.speed.threads = threads;
            for rate in [0.0, 0.1, 5.0, f64::from_bits(1), f64::MAX] {
                config.speed.max_requests_per_second = rate;
                assert!(CrawlPlan::new(config.clone()).is_ok(), "rate={rate}");
            }
        }
    }
}
