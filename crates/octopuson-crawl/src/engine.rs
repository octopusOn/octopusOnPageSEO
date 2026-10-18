//! The crawl loop: one coordinator owns the frontier, worker threads fetch and parse.

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Mutex, PoisonError};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use octopuson_html::{Hreflang, LinkKind, PageFacts, decode_html, parse_directives, parse_page};
use serde::{Deserialize, Serialize};
use url::Url;

use crate::activity::{ActivityKind, ActivityState, ActivityTracker, CrawlActivity, UrlActivity};
use crate::config::{CrawlMode, CrawlPlan, RobotsPolicy};
use crate::control::CrawlControl;
use crate::fetch::{FetchError, FetchRequest, FetchResponse, Fetcher, follows_location};
use crate::link_header::parse_link_header;
use crate::rate::RateLimiter;
use crate::record::{
    ContentKind, CrawlLinkKind, Indexability, LinkRecord, RobotsRecord, SiteFileRecord,
    SitemapRecord, UrlId, UrlRecord,
};
use crate::robots::RobotsCache;
use crate::scope::is_http_url;
use crate::sitemap::parse_sitemap;

/// How often progress is reported while waiting.
const PROGRESS_INTERVAL: Duration = Duration::from_millis(250);

/// Largest sitemap body read (50 MB uncompressed is the protocol limit).
const SITEMAP_BYTES: u64 = 50 * 1024 * 1024;

/// Largest well-known file kept (`/llms.txt`).
const SITE_FILE_BYTES: u64 = 1024 * 1024;

/// Receives everything a crawl produces, in the order it is produced.
pub trait CrawlSink {
    /// Why the sink cannot take more; it stops the crawl.
    type Error;

    /// A fetched or robots-blocked URL with the references it contains.
    ///
    /// # Errors
    /// The record cannot be stored.
    fn url(&mut self, record: &UrlRecord, links: &[LinkRecord]) -> Result<(), Self::Error>;

    /// The robots.txt of an origin, once per origin.
    ///
    /// # Errors
    /// The record cannot be stored.
    fn robots(&mut self, record: &RobotsRecord) -> Result<(), Self::Error>;

    /// A fetched sitemap.
    ///
    /// # Errors
    /// The record cannot be stored.
    fn sitemap(&mut self, record: &SitemapRecord) -> Result<(), Self::Error>;

    /// A well-known file of a start origin (`/llms.txt`).
    ///
    /// # Errors
    /// The record cannot be stored.
    fn site_file(&mut self, record: &SiteFileRecord) -> Result<(), Self::Error> {
        let _ = record;
        Ok(())
    }

    /// Counters, a few times per second.
    fn progress(&mut self, progress: &CrawlProgress) {
        let _ = progress;
    }

    /// Bounded URL activity, immediately before the corresponding progress report.
    fn activity(&mut self, activity: &CrawlActivity) {
        let _ = activity;
    }
}

/// Counters of a running crawl.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct CrawlProgress {
    /// URLs fetched or blocked so far.
    pub crawled: u32,
    /// URLs waiting to be fetched.
    pub queued: usize,
    /// Requests running.
    pub in_flight: usize,
    /// Distinct URLs seen (crawled or not).
    pub discovered: usize,
    /// URLs answered with 4xx or 5xx, or not at all.
    pub errors: u32,
    /// Sitemaps read.
    pub sitemaps: u32,
    /// Milliseconds since the crawl started.
    pub elapsed_ms: u64,
    /// No new requests start until resumed.
    pub paused: bool,
}

/// How a crawl ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct CrawlSummary {
    /// Final counters.
    pub progress: CrawlProgress,
    /// [`CrawlControl::stop`] ended the crawl early.
    pub stopped: bool,
    /// The URL limit ended the crawl with URLs left to fetch.
    pub limit_reached: bool,
}

/// Runs a [`CrawlPlan`] through a [`Fetcher`].
pub struct Crawler<'a> {
    plan: &'a CrawlPlan,
    fetcher: &'a dyn Fetcher,
    renderer: Option<&'a dyn crate::Renderer>,
}

impl<'a> Crawler<'a> {
    /// A crawler for `plan` fetching through `fetcher`.
    pub fn new(plan: &'a CrawlPlan, fetcher: &'a dyn Fetcher) -> Self {
        Self {
            plan,
            fetcher,
            renderer: None,
        }
    }

    /// Uses a renderer for JavaScript mode; HTTP-only mode never calls it.
    #[must_use]
    pub fn with_renderer(mut self, renderer: &'a dyn crate::Renderer) -> Self {
        self.renderer = Some(renderer);
        self
    }

    /// Crawls until the frontier is empty, the URL limit is reached or `control` stops it,
    /// handing every result to `sink` on the calling thread.
    ///
    /// # Errors
    /// The first error of `sink`; the crawl stops there.
    pub fn run<S: CrawlSink>(
        &self,
        sink: &mut S,
        control: &CrawlControl,
    ) -> Result<CrawlSummary, S::Error> {
        let threads = self.plan.config.speed.threads;
        let shared = Shared {
            robots: RobotsCache::new(self.plan.robots_tokens()),
            limiter: RateLimiter::new(self.plan.config.speed.max_requests_per_second),
        };
        let (job_tx, job_rx) = mpsc::channel::<(u64, Job)>();
        let job_rx = Mutex::new(job_rx);
        let (outcome_tx, outcome_rx) = mpsc::channel::<(u64, Outcome)>();
        let summary = thread::scope(|scope| {
            for _ in 0..threads {
                let outcome_tx = outcome_tx.clone();
                let (job_rx, shared) = (&job_rx, &shared);
                scope.spawn(move || {
                    loop {
                        let job = job_rx.lock().unwrap_or_else(PoisonError::into_inner).recv();
                        let Ok((id, job)) = job else {
                            break;
                        };
                        let outcome = if control.is_stopped() {
                            Outcome::Skipped(None)
                        } else {
                            self.work(shared, job, control)
                        };
                        if outcome_tx.send((id, outcome)).is_err() {
                            break;
                        }
                    }
                });
            }
            drop(outcome_tx);
            Coordinator::new(self.plan, threads).run(job_tx, &outcome_rx, sink, control)
        })?;
        for record in shared.robots.take_deferred() {
            sink.robots(&record)?;
        }
        Ok(summary)
    }

    fn work(&self, shared: &Shared, job: Job, control: &CrawlControl) -> Outcome {
        match job {
            Job::Fetch(task) => self.fetch(shared, task, control),
            Job::Robots(url) => {
                let (cell, record) = shared.robots.host(self.fetcher, &url);
                let sitemaps = cell
                    .get()
                    .map(|host| host.sitemaps().to_vec())
                    .unwrap_or_default();
                Outcome::Robots {
                    origin: url,
                    record,
                    sitemaps,
                }
            }
            Job::Sitemap(url) => {
                if shared.limiter.wait(control) {
                    Outcome::Sitemap(self.sitemap(&url))
                } else {
                    Outcome::Skipped(None)
                }
            }
            Job::SiteFile(url) => {
                if shared.limiter.wait(control) {
                    Outcome::SiteFile(self.site_file(&url))
                } else {
                    Outcome::Skipped(None)
                }
            }
        }
    }

    fn sitemap(&self, url: &Url) -> SitemapRecord {
        let mut record = SitemapRecord {
            url: url.to_string(),
            status: None,
            redirect_location: None,
            redirect_url: None,
            kind: None,
            error: None,
            entries: Vec::new(),
            children: Vec::new(),
        };
        let request = FetchRequest {
            url: url.as_str(),
            body_limit: SITEMAP_BYTES,
        };
        match self.fetcher.fetch(&request) {
            Err(error) => record.error = Some(error.to_string()),
            Ok(response) => {
                record.status = Some(response.status);
                record.redirect_location = response.header("location").map(str::to_owned);
                if (200..300).contains(&response.status) {
                    let parsed = if response.truncated {
                        Err("truncated sitemap response".into())
                    } else {
                        parse_sitemap(&response.body)
                    };
                    match parsed {
                        Ok(parsed) => {
                            record.kind = Some(parsed.kind);
                            record.entries = parsed.entries;
                            record.children = parsed.children;
                        }
                        Err(error) => record.error = Some(error),
                    }
                } else if follows_location(response.status) {
                    Self::sitemap_redirect(&mut record, url);
                } else {
                    record.error = Some(format!("HTTP {}", response.status));
                }
            }
        }
        record
    }

    fn sitemap_redirect(record: &mut SitemapRecord, url: &Url) {
        let Some(location) = record
            .redirect_location
            .as_deref()
            .filter(|raw| !raw.trim().is_empty())
        else {
            record.error = Some("missing sitemap redirect Location".into());
            return;
        };
        let Ok(mut target) = url.join(location.trim()) else {
            record.error = Some("invalid sitemap redirect Location".into());
            return;
        };
        if is_http_url(&target) {
            target.set_fragment(None);
        } else {
            record.error = Some("unsupported sitemap redirect scheme".into());
        }
        record.redirect_url = Some(target.into());
    }

    fn site_file(&self, url: &Url) -> SiteFileRecord {
        let request = FetchRequest {
            url: url.as_str(),
            body_limit: SITE_FILE_BYTES,
        };
        let mut record = SiteFileRecord {
            url: url.to_string(),
            status: None,
            content_type: None,
            text: String::new(),
            error: None,
        };
        match self.fetcher.fetch(&request) {
            Err(error) => record.error = Some(error.code().to_owned()),
            Ok(response) => {
                record.status = Some(response.status);
                record.content_type = response.header("content-type").map(str::to_owned);
                if (200..300).contains(&response.status) {
                    record.text = String::from_utf8_lossy(&response.body).into_owned();
                }
            }
        }
        record
    }

    fn fetch(&self, shared: &Shared, task: Task, control: &CrawlControl) -> Outcome {
        let config = &self.plan.config;
        let mut record = UrlRecord::new(task.id, task.url.to_string(), task.depth, task.internal);
        let mut robots = None;
        if task.internal && config.robots != RobotsPolicy::Ignore {
            let (verdict, fetched) = shared.robots.check(self.fetcher, &task.url);
            robots = fetched;
            if !verdict.allowed {
                record.blocked_by_robots = true;
                record.robots_rule = verdict.rule;
                if config.robots == RobotsPolicy::Respect {
                    record.crawled_at_ms = now_ms();
                    record.indexability = Indexability::BlockedByRobots;
                    return Outcome::Fetched {
                        task,
                        record: Box::new(record),
                        found: Vec::new(),
                        robots,
                    };
                }
            }
        }

        if !shared.limiter.wait(control) {
            return Outcome::Skipped(robots);
        }
        let body_limit = if task.kind == TaskKind::Check && !task.internal {
            0
        } else {
            config.limits.max_body_bytes
        };
        let request = FetchRequest {
            url: task.url.as_str(),
            body_limit,
        };
        let response = self.fetcher.fetch(&request);
        record.crawled_at_ms = now_ms();
        let mut found = Vec::new();
        match response {
            Err(error) => record.error = Some(error.code().to_owned()),
            Ok(response) => {
                self.describe(&mut record, &response, &task, &mut found, control, shared);
            }
        }
        record.indexability = indexability(&record);
        Outcome::Fetched {
            task,
            record: Box::new(record),
            found,
            robots,
        }
    }

    /// Fills `record` from `response` and collects the references it contains.
    fn describe(
        &self,
        record: &mut UrlRecord,
        response: &FetchResponse,
        task: &Task,
        found: &mut Vec<Found>,
        control: &CrawlControl,
        shared: &Shared,
    ) {
        let config = &self.plan.config;
        let tokens = self.plan.robots_tokens();
        record.status = Some(response.status);
        record.content_type = response.header("content-type").map(str::to_owned);
        record.content_kind = ContentKind::of(record.content_type.as_deref());
        let declared = response
            .header("content-length")
            .and_then(|len| len.trim().parse().ok());
        record.size_bytes = if response.body.is_empty() || response.truncated {
            declared.unwrap_or(response.body.len() as u64)
        } else {
            response.body.len() as u64
        };
        record.truncated = response.truncated;
        record.response_ms = u32::try_from(response.response_time.as_millis()).unwrap_or(u32::MAX);
        record.http_version = Some(response.http_version.clone());
        let robots_tags: Vec<&str> = response.header_values("x-robots-tag").collect();
        record.x_robots_tag = (!robots_tags.is_empty()).then(|| robots_tags.join("; "));
        record.last_modified = response.header("last-modified").map(str::to_owned);
        record.content_encoding = response.header("content-encoding").map(str::to_owned);
        record.cache_control = response.header("cache-control").map(str::to_owned);
        record.hsts = response.header("strict-transport-security").is_some();
        if config.storage.headers {
            record.headers.clone_from(&response.headers);
        }
        for value in response.header_values("x-robots-tag") {
            record.directives.merge(&parse_directives(value, tokens));
        }

        if follows_location(response.status)
            && let Some(target) = response
                .header("location")
                .filter(|value| !value.trim().is_empty())
                .and_then(|l| task.url.join(l.trim()).ok())
        {
            // The saved chain must point to the same HTTP identity the frontier fetches.
            let target = if is_http_url(&target) {
                self.plan.scope.normalize(target)
            } else {
                target
            };
            record.redirect_url = Some(target.to_string());
            found.push(Found::new(target, CrawlLinkKind::Redirect, String::new()));
        }

        for link in parse_link_header(response.header_values("link"), &task.url) {
            let Ok(url) = Url::parse(&link.url) else {
                continue;
            };
            if link.rel.iter().any(|rel| rel == "canonical") {
                if record.header_canonical.is_none() {
                    record.header_canonical = Some(link.url.clone());
                }
                found.push(Found::new(
                    url.clone(),
                    CrawlLinkKind::Canonical,
                    String::new(),
                ));
            }
            if link.rel.iter().any(|rel| rel == "alternate") {
                for lang in link.hreflang {
                    record.header_hreflang.push(Hreflang {
                        lang: lang.clone(),
                        url: link.url.clone(),
                    });
                    found.push(Found::new(url.clone(), CrawlLinkKind::Hreflang, lang));
                }
            }
        }

        let parse = task.kind == TaskKind::Page
            && record.content_kind == ContentKind::Html
            && (200..300).contains(&response.status);
        if parse {
            let raw = decode_html(&response.body, record.content_type.as_deref());
            let (html, base) =
                if task.internal && config.rendering.mode == crate::RenderMode::Javascript {
                    self.render(record, response, task, raw, control, shared)
                } else {
                    (raw, task.url.clone())
                };
            let facts = parse_page(&html, &base, tokens);
            if let Some(rendering) = record.rendering.as_mut() {
                rendering.rendered_link_count = rendering.rendered.then(|| hyperlink_count(&facts));
                if base != task.url {
                    found.push(Found::new(
                        base.clone(),
                        CrawlLinkKind::Redirect,
                        String::new(),
                    ));
                }
            }
            record.directives.merge(&facts.directives);
            found.extend(references(&facts));
            if config.schema_validation && task.internal {
                validate_schema(record, &html, &base, &facts);
            }
            if config.storage.html && task.internal {
                record.html = Some(html);
            }
            record.page = Some(facts);
        }
    }
    fn render(
        &self,
        record: &mut UrlRecord,
        response: &FetchResponse,
        task: &Task,
        raw: String,
        control: &CrawlControl,
        shared: &Shared,
    ) -> (String, Url) {
        let before = parse_page(&raw, &task.url, self.plan.robots_tokens());
        let mut raw_directives = record.directives.clone();
        raw_directives.merge(&before.directives);
        let started = Instant::now();
        let mut report = crate::RenderRecord {
            rendered: false,
            url: None,
            error: None,
            elapsed_ms: 0,
            truncated: false,
            raw_title: before.titles.first().cloned(),
            raw_meta_description: before.meta_descriptions.first().cloned(),
            raw_canonical: before
                .canonicals
                .first()
                .cloned()
                .or_else(|| record.header_canonical.clone()),
            raw_noindex: Some(raw_directives.noindex),
            raw_nofollow: Some(raw_directives.nofollow),
            raw_word_count: before.word_count,
            raw_link_count: hyperlink_count(&before),
            rendered_link_count: None,
            raw_html: self.plan.config.storage.html.then(|| raw.clone()),
        };
        let allow_request = |raw: &str, kind: crate::BrowserRequest, deadline: Instant| {
            if control.is_stopped() || Instant::now() >= deadline {
                return false;
            }
            let Ok(url) = Url::parse(raw) else {
                return false;
            };
            if !matches!(url.scheme(), "http" | "https") {
                return false;
            }
            let internal = self.plan.scope.is_internal(&url);
            if kind == crate::BrowserRequest::Document
                && (!internal || self.plan.scope.check(&url).is_err())
            {
                return false;
            }
            if internal
                && self.plan.config.robots == RobotsPolicy::Respect
                && !shared.robots.check_resource(self.fetcher, &url).allowed
            {
                return false;
            }
            kind == crate::BrowserRequest::CachedResource
                || shared.limiter.wait_until(deadline, control)
        };
        let result = self
            .renderer
            .ok_or_else(|| FetchError::Other("JavaScript renderer is not configured".into()))
            .and_then(|renderer| {
                renderer.render(&crate::RenderRequest {
                    url: task.url.as_str(),
                    response,
                    body_limit: self.plan.config.limits.max_body_bytes,
                    control,
                    request_allowed: Some(&allow_request),
                })
            });
        let output = match result {
            Ok(rendered) => {
                let base = Url::parse(&rendered.url)
                    .ok()
                    .filter(|url| matches!(url.scheme(), "http" | "https"))
                    .unwrap_or_else(|| task.url.clone());
                report.rendered = true;
                report.url = Some(base.to_string());
                report.truncated = rendered.truncated;
                (rendered.html, base)
            }
            Err(error) => {
                report.error = Some(error.to_string());
                (raw, task.url.clone())
            }
        };
        report.elapsed_ms = u32::try_from(started.elapsed().as_millis()).unwrap_or(u32::MAX);
        record.rendering = Some(report);
        output
    }
}

fn validate_schema(record: &mut UrlRecord, html: &str, base: &Url, facts: &PageFacts) {
    if record.truncated
        || record
            .rendering
            .as_ref()
            .is_some_and(|r| !r.rendered || r.truncated)
    {
        record.schema_error = Some("incomplete-document".into());
    } else {
        match octopuson_html::schema::validate_page(html, base.as_str(), facts) {
            Ok(report) => record.schema_report = Some(report),
            Err(_) => record.schema_error = Some("input-limit".into()),
        }
    }
}

fn hyperlink_count(facts: &PageFacts) -> u32 {
    u32::try_from(
        facts
            .links
            .iter()
            .filter(|link| matches!(link.kind, LinkKind::Anchor | LinkKind::Area))
            .count(),
    )
    .unwrap_or(u32::MAX)
}

/// Everything the page references, in document order per kind.
fn references(facts: &PageFacts) -> Vec<Found> {
    let mut found = Vec::with_capacity(facts.links.len() + 8);
    for link in &facts.links {
        let Ok(url) = Url::parse(&link.url) else {
            continue;
        };
        let kind = match link.kind {
            LinkKind::Anchor | LinkKind::Area => CrawlLinkKind::Hyperlink,
            LinkKind::Image => CrawlLinkKind::Image,
            LinkKind::Script => CrawlLinkKind::Script,
            LinkKind::Stylesheet => CrawlLinkKind::Stylesheet,
            LinkKind::Iframe => CrawlLinkKind::Iframe,
            LinkKind::Media => CrawlLinkKind::Media,
        };
        found.push(Found {
            url,
            kind,
            text: link.anchor.clone(),
            name: link.name.clone(),
            nofollow: link.nofollow,
            ugc: link.ugc,
            sponsored: link.sponsored,
        });
    }
    let mut push = |target: Option<&str>, kind: CrawlLinkKind, text: &str| {
        if let Some(url) = target.and_then(|t| Url::parse(t).ok()) {
            found.push(Found::new(url, kind, text.to_owned()));
        }
    };
    for canonical in &facts.canonicals {
        push(Some(canonical), CrawlLinkKind::Canonical, "");
    }
    for alternate in &facts.hreflang {
        push(
            Some(&alternate.url),
            CrawlLinkKind::Hreflang,
            &alternate.lang,
        );
    }
    push(facts.rel_next.as_deref(), CrawlLinkKind::Next, "");
    push(facts.rel_prev.as_deref(), CrawlLinkKind::Prev, "");
    push(facts.amp_html.as_deref(), CrawlLinkKind::Amp, "");
    let refresh = facts
        .meta_refresh
        .as_ref()
        .and_then(|refresh| refresh.url.as_deref());
    push(refresh, CrawlLinkKind::MetaRefresh, "");
    found
}

fn indexability(record: &UrlRecord) -> Indexability {
    if record.blocked_by_robots {
        return Indexability::BlockedByRobots;
    }
    match record.status {
        Some(status) if follows_location(status) => Indexability::Redirected,
        Some(400..=499) => Indexability::ClientError,
        Some(500..=599) => Indexability::ServerError,
        Some(200..=299) => {
            let canonical = record
                .page
                .as_ref()
                .and_then(|page| page.canonicals.first())
                .or(record.header_canonical.as_ref());
            if record.directives.noindex {
                Indexability::Noindex
            } else if canonical.is_some_and(|canonical| *canonical != record.url) {
                Indexability::Canonicalised
            } else {
                Indexability::Indexable
            }
        }
        Some(_) => Indexability::OtherHttp,
        None => Indexability::NoResponse,
    }
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| {
            i64::try_from(elapsed.as_millis()).unwrap_or(i64::MAX)
        })
}

/// State the workers share.
struct Shared {
    robots: RobotsCache,
    limiter: RateLimiter,
}

/// Work for a worker thread.
enum Job {
    Fetch(Task),
    /// Read the robots.txt of this URL's origin and report its sitemaps.
    Robots(Url),
    Sitemap(Url),
    /// Fetch a well-known file such as `/llms.txt`.
    SiteFile(Url),
}

impl Job {
    fn activity(&self) -> (String, ActivityKind) {
        match self {
            Self::Fetch(task) => (
                task.url.to_string(),
                if !task.internal {
                    ActivityKind::External
                } else if task.kind == TaskKind::Page {
                    ActivityKind::Page
                } else {
                    ActivityKind::Resource
                },
            ),
            Self::Robots(url) => (
                url.join("/robots.txt")
                    .unwrap_or_else(|_| url.clone())
                    .to_string(),
                ActivityKind::Robots,
            ),
            Self::Sitemap(url) => (url.to_string(), ActivityKind::Sitemap),
            Self::SiteFile(url) => (url.to_string(), ActivityKind::SiteFile),
        }
    }
}

/// A result for the coordinator.
enum Outcome {
    Fetched {
        task: Task,
        record: Box<UrlRecord>,
        found: Vec<Found>,
        robots: Option<RobotsRecord>,
    },
    Robots {
        origin: Url,
        record: Option<RobotsRecord>,
        sitemaps: Vec<String>,
    },
    Sitemap(SitemapRecord),
    SiteFile(SiteFileRecord),
    /// The crawl stopped before its HTTP request began. Keep any completed robots response.
    Skipped(Option<RobotsRecord>),
}

impl Outcome {
    fn describe_activity(&self, entry: &mut UrlActivity) {
        match self {
            Self::Fetched { record, .. } => {
                entry.status = record.status;
                entry.title = record
                    .page
                    .as_ref()
                    .and_then(|page| page.titles.first().cloned());
                entry.error.clone_from(&record.error);
                entry.response_ms = record.status.map(|_| record.response_ms);
                entry.size_bytes = record.status.map(|_| record.size_bytes);
                if record.blocked_by_robots && record.status.is_none() {
                    entry.state = ActivityState::Blocked;
                }
                if let Some(rendering) = &record.rendering {
                    entry.rendered = rendering.rendered;
                    entry.render_error.clone_from(&rendering.error);
                }
            }
            Self::Robots {
                record: Some(record),
                ..
            } => {
                entry.status = record.http_status;
                entry.error = (record.http_status.is_none()
                    && record.status == crate::RobotsStatus::Unreachable)
                    .then(|| "robots-unreachable".into());
            }
            Self::Sitemap(record) => {
                entry.status = record.status;
                entry.error.clone_from(&record.error);
            }
            Self::SiteFile(record) => {
                entry.status = record.status;
                entry.error.clone_from(&record.error);
            }
            Self::Skipped(_) => entry.state = ActivityState::Cancelled,
            Self::Robots { record: None, .. } => {}
        }
    }
}

/// A URL to fetch.
struct Task {
    id: UrlId,
    url: Url,
    depth: Option<u32>,
    kind: TaskKind,
    internal: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TaskKind {
    /// A document whose HTML is parsed and whose links are followed.
    Page,
    /// A resource or external URL whose response is recorded only.
    Check,
}

/// A reference found in a response.
struct Found {
    url: Url,
    kind: CrawlLinkKind,
    text: String,
    name: Option<octopuson_html::LinkName>,
    nofollow: bool,
    ugc: bool,
    sponsored: bool,
}

impl Found {
    fn new(url: Url, kind: CrawlLinkKind, text: String) -> Self {
        Self {
            url,
            kind,
            text,
            name: None,
            nofollow: false,
            ugc: false,
            sponsored: false,
        }
    }
}

/// Owns the frontier on the calling thread.
struct Coordinator<'a> {
    plan: &'a CrawlPlan,
    threads: usize,
    ids: HashMap<String, UrlId>,
    /// Whether each id was queued (ids are dense).
    queued: Vec<bool>,
    queue: VecDeque<Task>,
    /// Robots and sitemap jobs, which run before page fetches.
    side_jobs: VecDeque<Job>,
    /// Internal URLs only sitemaps list, crawled once the link graph is exhausted.
    backlog: VecDeque<Url>,
    sitemaps_seen: HashSet<String>,
    in_flight: usize,
    dispatched: u32,
    progress: CrawlProgress,
    activity: ActivityTracker,
    next_activity: u64,
}

impl<'a> Coordinator<'a> {
    fn new(plan: &'a CrawlPlan, threads: usize) -> Self {
        Self {
            plan,
            threads,
            ids: HashMap::new(),
            queued: Vec::new(),
            queue: VecDeque::new(),
            side_jobs: VecDeque::new(),
            backlog: VecDeque::new(),
            sitemaps_seen: HashSet::new(),
            in_flight: 0,
            dispatched: 0,
            progress: CrawlProgress::default(),
            activity: ActivityTracker::default(),
            next_activity: 0,
        }
    }

    fn run<S: CrawlSink>(
        mut self,
        jobs: Sender<(u64, Job)>,
        outcomes: &Receiver<(u64, Outcome)>,
        sink: &mut S,
        control: &CrawlControl,
    ) -> Result<CrawlSummary, S::Error> {
        let started = Instant::now();
        self.seed();
        let mut last_report: Option<Instant> = None;
        loop {
            let paused = control.is_paused();
            if !control.is_stopped() && !paused {
                while self.in_flight < self.threads {
                    let Some(job) = self.next_job() else {
                        break;
                    };
                    let (url, kind) = job.activity();
                    let id = self.next_activity;
                    self.next_activity += 1;
                    if jobs.send((id, job)).is_err() {
                        break;
                    }
                    self.activity.start(id, url, kind, elapsed_ms(started));
                    self.in_flight += 1;
                }
            }
            if self.in_flight == 0 && (control.is_stopped() || !paused) {
                break;
            }
            if last_report.is_none_or(|last| last.elapsed() >= PROGRESS_INTERVAL) {
                sink.activity(&self.activity.snapshot(elapsed_ms(started)));
                sink.progress(&self.snapshot(started, control));
                last_report = Some(Instant::now());
            }
            match outcomes.recv_timeout(PROGRESS_INTERVAL) {
                Ok((id, outcome)) => {
                    self.in_flight -= 1;
                    self.activity.finish(id, elapsed_ms(started), |entry| {
                        outcome.describe_activity(entry);
                    });
                    self.handle(outcome, sink)?;
                }
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => break,
            }
        }
        drop(jobs);
        let progress = self.snapshot(started, control);
        sink.activity(&self.activity.snapshot(elapsed_ms(started)));
        sink.progress(&progress);
        let limit_reached =
            !self.budget_left() && (!self.queue.is_empty() || !self.backlog.is_empty());
        Ok(CrawlSummary {
            progress,
            stopped: control.is_stopped(),
            limit_reached,
        })
    }

    fn seed(&mut self) {
        let plan = self.plan;
        let spider = plan.config.mode == CrawlMode::Spider;
        for url in &plan.start {
            let (id, _) = self.id_of(url.as_str());
            let internal = plan.scope.is_internal(url);
            self.enqueue(id, url.clone(), Some(0), TaskKind::Page, internal);
        }
        if !spider {
            return;
        }
        // One URL per start origin, for its robots.txt, sitemaps and well-known files.
        let mut seen = HashSet::new();
        let origins: Vec<&Url> = plan
            .start
            .iter()
            .filter(|url| seen.insert(url.origin().ascii_serialization()))
            .collect();
        for origin in &origins {
            if let Ok(llms) = origin.join("/llms.txt") {
                self.side_jobs.push_back(Job::SiteFile(llms));
            }
        }
        if !plan.config.follow.sitemaps {
            return;
        }
        for raw in &plan.config.follow.sitemap_urls {
            if let Ok(url) = Url::parse(raw.trim()) {
                self.add_sitemap(url);
            }
        }
        for origin in origins {
            self.side_jobs.push_back(Job::Robots(origin.clone()));
        }
    }

    fn next_job(&mut self) -> Option<Job> {
        if let Some(job) = self.side_jobs.pop_front() {
            return Some(job);
        }
        if !self.budget_left() {
            return None;
        }
        if self.queue.is_empty() && self.in_flight == 0 {
            self.release_backlog();
        }
        let task = self.queue.pop_front()?;
        self.dispatched += 1;
        Some(Job::Fetch(task))
    }

    fn budget_left(&self) -> bool {
        let max = self.plan.config.limits.max_urls;
        max == 0 || self.dispatched < max
    }

    /// Queues the sitemap-only URLs no link reached.
    fn release_backlog(&mut self) {
        while let Some(url) = self.backlog.pop_front() {
            let (id, _) = self.id_of(url.as_str());
            self.enqueue(id, url, None, TaskKind::Page, true);
        }
    }

    fn handle<S: CrawlSink>(&mut self, outcome: Outcome, sink: &mut S) -> Result<(), S::Error> {
        match outcome {
            Outcome::Fetched {
                task,
                record,
                found,
                robots,
            } => {
                if let Some(robots) = robots {
                    sink.robots(&robots)?;
                }
                self.progress.crawled += 1;
                if !record.blocked_by_robots && record.status.is_none_or(|status| status >= 400) {
                    self.progress.errors += 1;
                }
                let links = self.follow(&task, &record, found);
                sink.url(&record, &links)?;
            }
            Outcome::Robots {
                origin,
                record,
                sitemaps,
            } => {
                if let Some(record) = record {
                    sink.robots(&record)?;
                }
                let mut listed = sitemaps
                    .iter()
                    .filter_map(|s| Url::parse(s.trim()).ok())
                    .filter(is_http_url)
                    .peekable();
                let configured = self
                    .plan
                    .config
                    .follow
                    .sitemap_urls
                    .iter()
                    .any(|raw| Url::parse(raw.trim()).is_ok_and(|url| is_http_url(&url)));
                if listed.peek().is_none() && !configured {
                    if let Ok(fallback) = origin.join("/sitemap.xml") {
                        self.add_sitemap(fallback);
                    }
                } else {
                    for url in listed {
                        self.add_sitemap(url);
                    }
                }
            }
            Outcome::Sitemap(record) => {
                self.progress.sitemaps += 1;
                if record.error.is_none()
                    && let Some(target) = record
                        .redirect_url
                        .as_deref()
                        .and_then(|raw| Url::parse(raw).ok())
                {
                    self.add_sitemap(target);
                }
                for child in &record.children {
                    if let Ok(url) = Url::parse(child.trim()) {
                        self.add_sitemap(url);
                    }
                }
                for entry in &record.entries {
                    let Some(url) = self.plan.normalize_http_url(&entry.url) else {
                        continue;
                    };
                    if self.plan.scope.is_internal(&url) {
                        self.backlog.push_back(url);
                    }
                }
                sink.sitemap(&record)?;
            }
            Outcome::SiteFile(record) => sink.site_file(&record)?,
            Outcome::Skipped(robots) => {
                if let Some(record) = robots {
                    sink.robots(&record)?;
                }
            }
        }
        Ok(())
    }

    fn add_sitemap(&mut self, mut url: Url) {
        if !is_http_url(&url) {
            return;
        }
        // A fragment never reaches HTTP. Keep path escapes and query parameters intact.
        url.set_fragment(None);
        let max = self.plan.config.limits.max_sitemaps as usize;
        if self.sitemaps_seen.len() < max && self.sitemaps_seen.insert(url.to_string()) {
            self.side_jobs.push_back(Job::Sitemap(url));
        }
    }

    /// Turns the references of a fetched URL into link records and queues those to follow.
    fn follow(&mut self, task: &Task, record: &UrlRecord, found: Vec<Found>) -> Vec<LinkRecord> {
        let page_nofollow = record.directives.nofollow;
        let mut links = Vec::with_capacity(found.len());
        for (position, item) in found.into_iter().enumerate() {
            let url = self.plan.scope.normalize(item.url);
            let internal = self.plan.scope.is_internal(&url);
            let (target, _) = self.id_of(url.as_str());
            let follow = self.decide(task, item.kind, internal, item.nofollow, page_nofollow);
            links.push(LinkRecord {
                source: task.id,
                target,
                target_url: url.to_string(),
                kind: item.kind,
                text: item.text,
                name: item.name,
                nofollow: item.nofollow,
                ugc: item.ugc,
                sponsored: item.sponsored,
                internal,
                position: u32::try_from(position).unwrap_or(u32::MAX),
            });
            if let Some(kind) = follow {
                let depth = if item.kind == CrawlLinkKind::Redirect {
                    task.depth
                } else {
                    task.depth.map(|depth| depth + 1)
                };
                self.enqueue(target, url, depth, kind, internal);
            }
        }
        links
    }

    /// Whether a reference is fetched, and how.
    fn decide(
        &self,
        task: &Task,
        kind: CrawlLinkKind,
        internal: bool,
        nofollow: bool,
        page_nofollow: bool,
    ) -> Option<TaskKind> {
        let config = &self.plan.config;
        let follow = &config.follow;
        // External URLs are checked, never crawled.
        let external = || {
            (follow.check_external && (!nofollow || follow.external_nofollow))
                .then_some(TaskKind::Check)
        };
        if kind == CrawlLinkKind::Redirect {
            return if internal {
                Some(TaskKind::Page)
            } else {
                external()
            };
        }
        if config.mode == CrawlMode::List || task.kind != TaskKind::Page {
            return None;
        }
        let wanted = match kind {
            CrawlLinkKind::Hyperlink => {
                (!page_nofollow || follow.nofollow_pages)
                    && (!nofollow || follow.internal_nofollow || !internal)
            }
            CrawlLinkKind::Image => follow.images,
            CrawlLinkKind::Stylesheet => follow.stylesheets,
            CrawlLinkKind::Script => follow.scripts,
            CrawlLinkKind::Iframe | CrawlLinkKind::Media => follow.media,
            CrawlLinkKind::Canonical => follow.canonicals,
            CrawlLinkKind::Hreflang => follow.hreflang,
            CrawlLinkKind::Next | CrawlLinkKind::Prev => follow.pagination,
            CrawlLinkKind::MetaRefresh => follow.meta_refresh,
            CrawlLinkKind::Amp => follow.amp,
            CrawlLinkKind::Redirect => true,
        };
        if !wanted {
            return None;
        }
        if !internal {
            return external();
        }
        Some(if kind.is_resource() {
            TaskKind::Check
        } else {
            TaskKind::Page
        })
    }

    /// Queues `url` unless it already was, it is out of scope or too deep.
    fn enqueue(&mut self, id: UrlId, url: Url, depth: Option<u32>, kind: TaskKind, internal: bool) {
        // Keep observed references, but unsupported schemes must never spend HTTP slots.
        if !is_http_url(&url) {
            return;
        }
        let index = id as usize;
        if self.queued.get(index).copied().unwrap_or(false) {
            return;
        }
        let start = depth == Some(0);
        if internal && !start {
            if self.plan.scope.check(&url).is_err() {
                return;
            }
            let max_depth = self.plan.config.limits.max_depth;
            if kind == TaskKind::Page
                && depth.zip(max_depth).is_some_and(|(depth, max)| depth > max)
            {
                return;
            }
        }
        if self.queued.len() <= index {
            self.queued.resize(index + 1, false);
        }
        self.queued[index] = true;
        self.queue.push_back(Task {
            id,
            url,
            depth,
            kind,
            internal,
        });
    }

    /// The identifier of `url`, assigned on first sight; `true` when new.
    fn id_of(&mut self, url: &str) -> (UrlId, bool) {
        if let Some(&id) = self.ids.get(url) {
            return (id, false);
        }
        let id = UrlId::try_from(self.ids.len()).unwrap_or(UrlId::MAX);
        self.ids.insert(url.to_owned(), id);
        (id, true)
    }

    fn snapshot(&self, started: Instant, control: &CrawlControl) -> CrawlProgress {
        CrawlProgress {
            queued: self.queue.len(),
            in_flight: self.in_flight,
            discovered: self.ids.len(),
            elapsed_ms: elapsed_ms(started),
            paused: control.is_paused(),
            ..self.progress
        }
    }
}

fn elapsed_ms(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests;
