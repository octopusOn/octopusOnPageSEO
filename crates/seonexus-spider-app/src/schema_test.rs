//! On-demand single-document validation, with no crawl archive or score mutations.

use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use seonexus_crawl::{
    CrawlConfig, CrawlControl, CrawlMode, CrawlPlan, CrawlSink, Crawler, Fetcher, LinkRecord,
    RenderMode, Renderer, Rendering, RobotsRecord, SitemapRecord, UrlRecord,
};
use seonexus_html::schema::{INPUT_LIMIT, SchemaReport, validate};
use serde::{Deserialize, Serialize};

static LIVE_TEST: Mutex<()> = Mutex::new(());

/// Explicit source selected by the user; opening the screen never fetches a website.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SchemaTestMode {
    /// Fetch one URL and its bounded redirect chain.
    Url,
    /// Pasted HTML; no network or JavaScript execution.
    Html,
    /// Pasted JSON-LD; no network or remote-context loading.
    Jsonld,
}

/// Optional, user-triggered test. Input HTML is kept in memory only.
#[derive(Debug, Clone, Deserialize)]
pub struct SchemaTestRequest {
    /// URL, HTML or JSON-LD according to `mode`.
    pub input: String,
    /// Explicit source.
    pub mode: SchemaTestMode,
    /// Browser settings for URL mode; ignored for pasted input.
    #[serde(default)]
    pub rendering: Rendering,
}

/// Live observation and local checks; not a Google eligibility verdict.
#[derive(Debug, Serialize)]
pub struct SchemaTestResult {
    /// Exact tested final URL, when a URL was fetched.
    pub url: Option<String>,
    /// Final observed HTTP status.
    pub status: Option<u16>,
    /// Was the analyzed source successfully rendered?
    pub rendered: bool,
    /// Browser failure; fallback HTML is not silently reported as a rendered test.
    pub render_error: Option<String>,
    /// Bounded redirect chain, in request order.
    pub redirects: Vec<String>,
    /// Observed crawl indexability, distinct from schema syntax validity.
    pub indexability: Option<String>,
    /// A partial response/DOM must not produce a conclusive schema result.
    pub truncated: bool,
    /// Request, robots, redirect-limit or non-HTML failure.
    pub error: Option<String>,
    /// Only available when an HTML document (or pasted source) was actually analyzed.
    pub report: Option<SchemaReport>,
    /// Time of the explicit test, independent of any selected crawl.
    pub tested_at_ms: u64,
}

/// Runs one explicit test without saving a crawl or changing existing findings.
///
/// # Errors
/// Invalid input, settings, browser startup or input-size limit.
pub fn test_schema(request: &SchemaTestRequest) -> Result<SchemaTestResult, String> {
    if !matches!(request.mode, SchemaTestMode::Url) {
        return pasted(request);
    }
    let _guard = LIVE_TEST
        .try_lock()
        .map_err(|_| "Another live Schema test is still running")?;
    let plan = plan(request)?;
    let fetcher = seonexus_net::CrawlFetcher::new(plan.user_agent(), plan.timeout(), None);
    let renderer = if plan.config().rendering.mode == RenderMode::Javascript {
        Some(seonexus_render::ChromeRenderer::launch(&plan).map_err(|e| e.to_string())?)
    } else {
        None
    };
    test_with(
        &plan,
        &fetcher,
        renderer.as_ref().map(|r| r as &dyn Renderer),
    )
}

fn result() -> SchemaTestResult {
    SchemaTestResult {
        url: None,
        status: None,
        rendered: false,
        render_error: None,
        redirects: Vec::new(),
        indexability: None,
        truncated: false,
        error: None,
        report: None,
        tested_at_ms: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
            .try_into()
            .unwrap_or(u64::MAX),
    }
}

fn pasted(request: &SchemaTestRequest) -> Result<SchemaTestResult, String> {
    let mut result = result();
    result.report = Some(validate(
        &request.input,
        matches!(request.mode, SchemaTestMode::Jsonld),
        "https://example.invalid/",
    )?);
    Ok(result)
}

fn plan(request: &SchemaTestRequest) -> Result<CrawlPlan, String> {
    if request.input.len() > 8192 {
        return Err("URL exceeds 8192 bytes".into());
    }
    let mut config = CrawlConfig {
        start_urls: vec![request.input.trim().into()],
        mode: CrawlMode::List,
        rendering: request.rendering.clone(),
        ..Default::default()
    };
    config.speed.threads = 1;
    config.rendering.concurrency = 1;
    config.limits.max_urls = 6;
    config.limits.max_body_bytes = INPUT_LIMIT as u64;
    config.follow.sitemaps = false;
    config.storage.html = true;
    CrawlPlan::new(config).map_err(|e| e.to_string())
}

#[derive(Default)]
struct Capture {
    last: Option<UrlRecord>,
    redirects: Vec<String>,
}
impl CrawlSink for Capture {
    type Error = String;
    fn url(&mut self, record: &UrlRecord, _: &[LinkRecord]) -> Result<(), String> {
        if record.redirect_url.is_some() {
            self.redirects.push(record.url.clone());
        }
        self.last = Some(record.clone());
        Ok(())
    }
    fn robots(&mut self, _: &RobotsRecord) -> Result<(), String> {
        Ok(())
    }
    fn sitemap(&mut self, _: &SitemapRecord) -> Result<(), String> {
        Ok(())
    }
}

fn test_with(
    plan: &CrawlPlan,
    fetcher: &dyn Fetcher,
    renderer: Option<&dyn Renderer>,
) -> Result<SchemaTestResult, String> {
    let mut capture = Capture::default();
    let mut crawler = Crawler::new(plan, fetcher);
    if let Some(renderer) = renderer {
        crawler = crawler.with_renderer(renderer);
    }
    crawler.run(&mut capture, &CrawlControl::new())?;
    let mut result = result();
    result.redirects = capture.redirects;
    let Some(record) = capture.last else {
        result.error = Some("No document fetched".into());
        return Ok(result);
    };
    result.url = Some(record.url.clone());
    result.status = record.status;
    result.indexability = Some(record.indexability.as_str().into());
    result.truncated = record.truncated;
    if let Some(rendering) = &record.rendering {
        result.rendered = rendering.rendered;
        if rendering.rendered
            && let Some(url) = &rendering.url
        {
            result.url = Some(url.clone());
        }
        result.render_error.clone_from(&rendering.error);
        result.truncated |= rendering.truncated;
    }
    result.error = record.error.clone().or_else(|| {
        if record.blocked_by_robots {
            Some("Blocked by robots.txt".into())
        } else if record.redirect_url.is_some() {
            Some("Redirect chain incomplete or exceeds 5 hops".into())
        } else if !record.internal {
            Some(
                "Redirect target is outside this site's scope; test the destination URL directly"
                    .into(),
            )
        } else if !record.status.is_some_and(|s| (200..300).contains(&s)) {
            Some("Document did not return a successful HTTP status".into())
        } else if record.html.is_none() {
            Some("Response is not an HTML document".into())
        } else {
            None
        }
    });
    // A failed render and truncated source remain unavailable, not valid or schema-missing.
    if result.error.is_none()
        && result.render_error.is_none()
        && !result.truncated
        && let Some(html) = &record.html
    {
        result.report = Some(validate(
            html,
            false,
            result.url.as_deref().unwrap_or(&record.url),
        )?);
    }
    Ok(result)
}

#[cfg(test)]
mod tests;
