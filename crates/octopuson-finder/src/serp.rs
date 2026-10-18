//! Search engine result page (SERP), Wayback CDX, and API discovery adapters.

use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
use std::time::Duration;

use base64::Engine as _;
use octopuson_core::http::{HttpRequest, HttpTransport};
use scraper::{Html, Selector};
use ureq::Agent;
use url::Url;

use crate::model::{
    CreditUsage, DiscoveredUrl, FindingCategory, IndexEngine, ScanConfig, ScanControl,
    ScanProgress, VerifiedUrl,
};

/// Browser-like Accept header for SERP requests.
const ACCEPT_HTML: &str =
    "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,*/*;q=0.8";

/// Serper.dev search endpoint.
pub(crate) const SERPER_SEARCH: &str = "https://google.serper.dev/search";

/// Raw item extracted from one SERP or API page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawSerpHit {
    /// Target URL.
    pub url: String,
    /// Result title on the SERP.
    pub title: String,
    /// Result snippet on the SERP.
    pub snippet: String,
}

/// The paid search APIs of a scan that can run on OctopusOn SEO credits (Serper, SerpApi).
///
/// Their requests go over the [`HttpTransport`] port instead of the scan's own agent: in the
/// apps that is a `octopuson_cloud::CreditTransport`, which sends the requests of sources on
/// credits to the gateway and leaves the others alone. The charges it reports are added up
/// here.
pub struct ApiCalls<'a> {
    transport: &'a dyn HttpTransport,
    on_credit: Vec<IndexEngine>,
    refused: Vec<IndexEngine>,
    usage: CreditUsage,
}

impl std::fmt::Debug for ApiCalls<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ApiCalls")
            .field("on_credit", &self.on_credit)
            .field("refused", &self.refused)
            .field("usage", &self.usage)
            .finish_non_exhaustive()
    }
}

impl<'a> ApiCalls<'a> {
    /// API calls over `transport`; `usage.sources` names the sources of `on_credit`.
    pub fn new(
        transport: &'a dyn HttpTransport,
        on_credit: Vec<IndexEngine>,
        usage: CreditUsage,
    ) -> Self {
        Self {
            transport,
            on_credit,
            refused: Vec::new(),
            usage,
        }
    }

    /// What the sources on credits used so far.
    pub const fn usage(&self) -> &CreditUsage {
        &self.usage
    }

    /// Sends one API page request of `engine` and returns the body of a successful answer.
    /// When the gateway refuses a source on credits (signed out, out of credits, a spending
    /// limit), the source is not asked again in this scan.
    pub(crate) fn send(&mut self, engine: IndexEngine, request: &HttpRequest) -> Option<String> {
        if self.refused.contains(&engine) {
            return None;
        }
        let response = self.transport.send(request).ok();
        let credit = self.on_credit.contains(&engine);
        if credit && response.is_some() {
            self.usage.requests = self.usage.requests.saturating_add(1);
        }
        if let Some(usd) = self.transport.take_charge_usd() {
            self.usage.charged_micros = self.usage.charged_micros.saturating_add(micros(usd));
        }
        let response = response?;
        if credit && matches!(response.status, 401 | 402) {
            let message = serde_json::from_slice::<serde_json::Value>(&response.body)
                .ok()
                .and_then(|body| body.get("message")?.as_str().map(str::to_owned))
                .unwrap_or_else(|| format!("status {}", response.status));
            self.usage.stopped = Some(format!("{engine}: {message}"));
            self.refused.push(engine);
            return None;
        }
        response.is_success().then(|| response.text())
    }
}

/// US dollars as whole micro-dollars.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn micros(usd: f64) -> u64 {
    // The transport only reports finite, non-negative charges.
    (usd * 1_000_000.0).round().max(0.0) as u64
}

/// Discovers indexed URLs for `config` across configured free search engines, Wayback, APIs and seed lists.
/// Serper and SerpApi requests go through `api`.
pub fn discover_indexed_urls(
    config: &ScanConfig,
    base_host: &str,
    api: &mut ApiCalls<'_>,
    control: &ScanControl,
    progress: &mut ScanProgress,
    on_progress: &mut dyn FnMut(&ScanProgress),
) -> Vec<DiscoveredUrl> {
    let mut catalog: BTreeMap<String, DiscoveredUrl> = BTreeMap::new();
    ingest_seed_urls(config, base_host, &mut catalog);
    sync_live_preview(&catalog, base_host, progress);
    on_progress(progress);

    let timeout = Duration::from_secs(u64::from(config.timeout_seconds.clamp(5, 60)));
    let agent = build_serp_agent(&config.user_agent, timeout);
    let queries = build_initial_queries(config, base_host);

    let pages = config.pages_per_query.clamp(1, 10);
    let active_api_count = u32::from(config.api.has_google_api())
        + u32::from(config.api.has_yandex_api())
        + u32::from(config.api.has_serper_api())
        + u32::from(config.api.has_serpapi());
    let engine_steps = (u32::try_from(config.engines.len()).unwrap_or(0) + active_api_count)
        .saturating_mul(u32::from(pages));
    progress.queries_total = u32::try_from(queries.len())
        .unwrap_or(1)
        .saturating_mul(engine_steps.max(1))
        .saturating_add(u32::from(config.use_wayback))
        .saturating_add(u32::from(config.use_commoncrawl))
        .saturating_add(u32::from(config.use_sitemaps));
    on_progress(progress);

    if config.use_sitemaps && control.wait_if_paused() {
        progress.current_item = format!("sitemap:{base_host}");
        on_progress(progress);
        let hits = crate::archive::fetch_sitemap_urls(&agent, base_host);
        ingest_serp_hits(
            hits,
            IndexEngine::Sitemap,
            "sitemap:xml",
            config,
            base_host,
            &mut catalog,
        );
        progress.queries_completed = progress.queries_completed.saturating_add(1);
        sync_live_preview(&catalog, base_host, progress);
        on_progress(progress);
    }

    // Execute initial SERP & API queries using the attended browser runner (octopuson-render + octopuson-serp).
    let mut browser = crate::browser_serp::SerpBrowserRunner::new();
    run_query_batch(
        &mut browser,
        &agent,
        api,
        &queries,
        pages,
        config,
        base_host,
        control,
        progress,
        &mut catalog,
        on_progress,
    );

    // Smart subpath expansion: discover top-level folders and query `site:domain.com/<folder>`.
    if config.expand_subpaths && control.wait_if_paused() {
        let expanded_queries = derive_subpath_queries(&catalog, base_host, &queries, 4);
        if !expanded_queries.is_empty() {
            let expanded_pages = pages.min(2);
            let expanded_engine_steps = (u32::try_from(config.engines.len()).unwrap_or(0)
                + active_api_count)
                .saturating_mul(u32::from(expanded_pages));
            let extra_steps = u32::try_from(expanded_queries.len())
                .unwrap_or(0)
                .saturating_mul(expanded_engine_steps.max(1));
            progress.queries_total = progress.queries_total.saturating_add(extra_steps);
            on_progress(progress);
            run_query_batch(
                &mut browser,
                &agent,
                api,
                &expanded_queries,
                expanded_pages,
                config,
                base_host,
                control,
                progress,
                &mut catalog,
                on_progress,
            );
        }
    }

    // Archive presence check only: tag whether already-discovered URLs exist in Wayback / Common Crawl
    // without polluting the scan with old historical-only 404 URLs.
    run_archive_presence_check(
        &agent,
        config,
        base_host,
        control,
        progress,
        &mut catalog,
        on_progress,
    );

    catalog.into_values().collect()
}

fn sync_live_preview(
    catalog: &BTreeMap<String, DiscoveredUrl>,
    base_host: &str,
    progress: &mut ScanProgress,
) {
    progress.discovered_urls = u32::try_from(catalog.len()).unwrap_or(u32::MAX);
    let default_target = format!("https://{base_host}/");
    progress.live_rows = catalog
        .values()
        .take(250)
        .map(|item| VerifiedUrl {
            url: item.url.clone(),
            engines: item
                .engines
                .iter()
                .map(|e| e.as_str())
                .collect::<Vec<_>>()
                .join(","),
            serp_title: item.serp_title.clone(),
            serp_snippet: item.serp_snippet.clone(),
            found_by_query: item.found_by_query.clone(),
            initial_status: 0,
            final_status: 0,
            redirect_hops: 0,
            final_url: item.url.clone(),
            response_time_ms: 0,
            content_type: String::new(),
            live_title: "HTTP kontrolü bekleniyor...".to_owned(),
            h1: String::new(),
            word_count: 0,
            canonical: String::new(),
            noindex: false,
            category: FindingCategory::Healthy200,
            reason: "checking".to_owned(),
            suggested_target: default_target.clone(),
            error_message: String::new(),
        })
        .collect();
}

fn run_archive_presence_check(
    agent: &Agent,
    config: &ScanConfig,
    base_host: &str,
    control: &ScanControl,
    progress: &mut ScanProgress,
    catalog: &mut BTreeMap<String, DiscoveredUrl>,
    on_progress: &mut dyn FnMut(&ScanProgress),
) {
    if config.use_wayback && control.wait_if_paused() {
        progress.current_item = format!("wayback:{base_host}");
        on_progress(progress);
        if !catalog.is_empty() {
            let hits = fetch_wayback_cdx(agent, base_host, config.include_subdomains);
            mark_existing_archive_hits(hits, IndexEngine::Wayback, config, base_host, catalog);
            sync_live_preview(catalog, base_host, progress);
        }
        progress.queries_completed = progress.queries_completed.saturating_add(1);
        on_progress(progress);
    }

    if config.use_commoncrawl && control.wait_if_paused() {
        progress.current_item = format!("commoncrawl:{base_host}");
        on_progress(progress);
        if !catalog.is_empty() {
            let hits =
                crate::archive::fetch_commoncrawl_cdx(agent, base_host, config.include_subdomains);
            mark_existing_archive_hits(hits, IndexEngine::CommonCrawl, config, base_host, catalog);
            sync_live_preview(catalog, base_host, progress);
        }
        progress.queries_completed = progress.queries_completed.saturating_add(1);
        on_progress(progress);
    }
}

fn mark_existing_archive_hits(
    hits: Vec<RawSerpHit>,
    engine: IndexEngine,
    config: &ScanConfig,
    base_host: &str,
    catalog: &mut BTreeMap<String, DiscoveredUrl>,
) {
    for hit in hits {
        let Some(normalized) = normalize_discovered_url(&hit.url, config, base_host) else {
            continue;
        };
        if let Some(existing) = catalog.get_mut(&normalized) {
            if !existing.engines.contains(&engine) {
                existing.engines.push(engine);
            }
            if existing.serp_snippet.is_empty() && !hit.snippet.is_empty() {
                existing.serp_snippet = hit.snippet;
            }
        }
    }
}

pub(crate) fn ingest_seed_urls(
    config: &ScanConfig,
    base_host: &str,
    catalog: &mut BTreeMap<String, DiscoveredUrl>,
) {
    for raw_entry in &config.extra_urls {
        for line in raw_entry.lines() {
            let Some((candidate, seed_title)) = parse_seed_line(line, base_host) else {
                continue;
            };
            if let Some(normalized) = normalize_discovered_url(&candidate, config, base_host) {
                record_hit(
                    catalog,
                    normalized,
                    IndexEngine::SeedList,
                    seed_title,
                    String::new(),
                    "seed:list",
                );
            }
        }
    }
}

fn parse_seed_line(line: &str, base_host: &str) -> Option<(String, String)> {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with('#') {
        return None;
    }
    let cols: Vec<&str> = trimmed
        .split([',', ';', '\t'])
        .map(|c| c.trim().trim_matches('"').trim_matches('\''))
        .filter(|c| !c.is_empty())
        .collect();
    let first = cols.first().copied()?;
    if matches!(
        first.to_ascii_lowercase().as_str(),
        "url" | "urls" | "page" | "sayfa" | "address" | "id"
    ) && cols.len() <= 2
    {
        return None;
    }

    let mut url_candidate = None;
    let mut title_candidate = String::new();

    for col in cols {
        if url_candidate.is_none()
            && (col.starts_with("http://")
                || col.starts_with("https://")
                || col.starts_with('/')
                || col.starts_with(base_host))
        {
            let full = if col.contains("://") {
                col.to_owned()
            } else if col.starts_with('/') {
                format!("https://{base_host}{col}")
            } else {
                format!("https://{col}")
            };
            url_candidate = Some(full);
        } else if title_candidate.is_empty() && is_descriptive_seed_title(col) {
            col.clone_into(&mut title_candidate);
        }
    }

    url_candidate.map(|u| (u, title_candidate))
}

fn is_descriptive_seed_title(col: &str) -> bool {
    if col.len() < 3 || col.starts_with("http") || col.starts_with('/') {
        return false;
    }
    // Skip pure numbers or YYYY-MM-DD dates
    !col.chars()
        .all(|c| c.is_ascii_digit() || matches!(c, '-' | '.' | ':' | ' '))
}

fn build_initial_queries(config: &ScanConfig, base_host: &str) -> Vec<String> {
    let mut queries = vec![format!("site:{base_host}")];
    for seed in &config.seed_queries {
        let clean = seed.trim().trim_matches('/');
        if !clean.is_empty() {
            let q = if clean.contains(' ') || clean.contains(':') || clean.starts_with('-') {
                format!("site:{base_host} {clean}")
            } else {
                format!("site:{base_host}/{clean}")
            };
            if !queries.contains(&q) {
                queries.push(q);
            }
        }
    }
    queries
}

#[allow(clippy::too_many_arguments)]
fn run_query_batch(
    browser: &mut crate::browser_serp::SerpBrowserRunner,
    agent: &Agent,
    api: &mut ApiCalls<'_>,
    queries: &[String],
    pages: u16,
    config: &ScanConfig,
    base_host: &str,
    control: &ScanControl,
    progress: &mut ScanProgress,
    catalog: &mut BTreeMap<String, DiscoveredUrl>,
    on_progress: &mut dyn FnMut(&ScanProgress),
) {
    for query in queries {
        if !control.wait_if_paused() {
            return;
        }
        for &engine in &config.engines {
            for page in 0..pages {
                if !control.wait_if_paused() {
                    return;
                }
                progress.current_item = format!("{engine}: {query} (p.{})", page + 1);
                on_progress(progress);

                let hits = browser
                    .fetch_page(engine, query, page, control, progress, on_progress)
                    .unwrap_or_else(|| fetch_free_engine_page(agent, engine, query, page));
                let added = ingest_serp_hits(hits, engine, query, config, base_host, catalog);
                progress.queries_completed = progress.queries_completed.saturating_add(1);
                if added == 0 {
                    let skipped = u32::from(pages.saturating_sub(page + 1));
                    progress.queries_completed = progress.queries_completed.saturating_add(skipped);
                    sync_live_preview(catalog, base_host, progress);
                    on_progress(progress);
                    break;
                }
                sync_live_preview(catalog, base_host, progress);
                on_progress(progress);
            }
        }
        run_api_queries_for_query(
            agent,
            api,
            query,
            pages,
            config,
            base_host,
            control,
            progress,
            catalog,
            on_progress,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn run_api_queries_for_query(
    agent: &Agent,
    api: &mut ApiCalls<'_>,
    query: &str,
    pages: u16,
    config: &ScanConfig,
    base_host: &str,
    control: &ScanControl,
    progress: &mut ScanProgress,
    catalog: &mut BTreeMap<String, DiscoveredUrl>,
    on_progress: &mut dyn FnMut(&ScanProgress),
) {
    if config.api.has_google_api() {
        paginate_api_engine(
            IndexEngine::GoogleApi,
            query,
            pages,
            config,
            base_host,
            control,
            progress,
            catalog,
            on_progress,
            |page| {
                fetch_google_custom_search(
                    agent,
                    &config.api.google_api_key,
                    &config.api.google_cx,
                    query,
                    page,
                )
            },
        );
    }

    if config.api.has_yandex_api() {
        paginate_api_engine(
            IndexEngine::YandexApi,
            query,
            pages,
            config,
            base_host,
            control,
            progress,
            catalog,
            on_progress,
            |page| {
                fetch_yandex_xml_api(
                    agent,
                    &config.api.yandex_user,
                    &config.api.yandex_api_key,
                    query,
                    page,
                )
            },
        );
    }

    if config.api.has_serper_api() {
        paginate_api_engine(
            IndexEngine::SerperApi,
            query,
            pages,
            config,
            base_host,
            control,
            progress,
            catalog,
            on_progress,
            |page| fetch_serper_api(api, &config.api.serper_api_key, query, page),
        );
        progress.credits.clone_from(api.usage());
    }

    if config.api.has_serpapi() {
        paginate_api_engine(
            IndexEngine::SerpApi,
            query,
            pages,
            config,
            base_host,
            control,
            progress,
            catalog,
            on_progress,
            |page| crate::archive::fetch_serpapi(api, &config.api.serpapi_key, query, page),
        );
        progress.credits.clone_from(api.usage());
    }
}

#[allow(clippy::too_many_arguments)]
fn paginate_api_engine(
    engine: IndexEngine,
    query: &str,
    pages: u16,
    config: &ScanConfig,
    base_host: &str,
    control: &ScanControl,
    progress: &mut ScanProgress,
    catalog: &mut BTreeMap<String, DiscoveredUrl>,
    on_progress: &mut dyn FnMut(&ScanProgress),
    mut fetch_page: impl FnMut(u16) -> Vec<RawSerpHit>,
) {
    for page in 0..pages {
        if !control.wait_if_paused() {
            return;
        }
        progress.current_item = format!("{}: {query} (p.{})", engine.as_str(), page + 1);
        on_progress(progress);
        let hits = fetch_page(page);
        let count = hits.len();
        ingest_serp_hits(hits, engine, query, config, base_host, catalog);
        progress.queries_completed = progress.queries_completed.saturating_add(1);
        if count == 0 {
            let skipped = u32::from(pages.saturating_sub(page + 1));
            progress.queries_completed = progress.queries_completed.saturating_add(skipped);
            sync_live_preview(catalog, base_host, progress);
            on_progress(progress);
            break;
        }
        sync_live_preview(catalog, base_host, progress);
        on_progress(progress);
    }
}

pub(crate) fn url_encode(value: &str) -> String {
    url::form_urlencoded::byte_serialize(value.as_bytes()).collect()
}

fn ingest_serp_hits(
    hits: Vec<RawSerpHit>,
    engine: IndexEngine,
    query: &str,
    config: &ScanConfig,
    base_host: &str,
    catalog: &mut BTreeMap<String, DiscoveredUrl>,
) -> usize {
    let mut added = 0usize;
    for hit in hits {
        if let Some(normalized) = normalize_discovered_url(&hit.url, config, base_host) {
            record_hit(catalog, normalized, engine, hit.title, hit.snippet, query);
            added += 1;
        }
    }
    added
}

fn build_serp_agent(user_agent: &str, timeout: Duration) -> Agent {
    let config = Agent::config_builder()
        .user_agent(user_agent)
        .accept(ACCEPT_HTML)
        .timeout_global(Some(timeout))
        .http_status_as_error(false)
        .max_redirects(3)
        .build();
    Agent::new_with_config(config)
}

fn get_text(agent: &Agent, url: &str) -> Option<String> {
    let mut response = agent
        .get(url)
        .header("Accept-Language", "tr-TR,tr;q=0.9,en-US;q=0.8,en;q=0.7")
        .call()
        .ok()?;
    if response.status().as_u16() >= 400 {
        return None;
    }
    let mut bytes = Vec::new();
    let reader = response
        .body_mut()
        .with_config()
        .limit(4 * 1024 * 1024)
        .reader();
    reader.take(4 * 1024 * 1024).read_to_end(&mut bytes).ok()?;
    Some(String::from_utf8_lossy(&bytes).into_owned())
}

fn fetch_free_engine_page(
    agent: &Agent,
    engine: IndexEngine,
    query: &str,
    page: u16,
) -> Vec<RawSerpHit> {
    let encoded: String = url::form_urlencoded::byte_serialize(query.as_bytes()).collect();
    match engine {
        IndexEngine::Google => {
            let start = u32::from(page) * 100;
            let url = format!(
                "https://www.google.com/search?q={encoded}&num=100&start={start}&hl=tr&ie=UTF-8"
            );
            let mut hits = get_text(agent, &url)
                .map(|html| parse_google_serp(&html))
                .unwrap_or_default();
            if hits.is_empty() {
                let start_10 = u32::from(page) * 10;
                let fallback_url = format!(
                    "https://www.google.com/search?q={encoded}&start={start_10}&hl=tr&ie=UTF-8"
                );
                hits = get_text(agent, &fallback_url)
                    .map(|html| parse_google_serp(&html))
                    .unwrap_or_default();
            }
            hits
        }
        IndexEngine::Yandex => {
            let url_tr = format!("https://yandex.com.tr/search/?text={encoded}&numdoc=50&p={page}");
            let mut hits = get_text(agent, &url_tr)
                .map(|html| parse_yandex_serp(&html))
                .unwrap_or_default();
            if hits.is_empty() {
                let url_com =
                    format!("https://yandex.com/search/?text={encoded}&numdoc=50&p={page}");
                hits = get_text(agent, &url_com)
                    .map(|html| parse_yandex_serp(&html))
                    .unwrap_or_default();
            }
            hits
        }
        IndexEngine::Bing => {
            let first = u32::from(page) * 50 + 1;
            let url = format!("https://www.bing.com/search?q={encoded}&count=50&first={first}");
            get_text(agent, &url)
                .map(|html| parse_bing_serp(&html))
                .unwrap_or_default()
        }
        IndexEngine::DuckDuckGo => {
            let offset = u32::from(page) * 30;
            let url = format!("https://html.duckduckgo.com/html/?q={encoded}&s={offset}");
            get_text(agent, &url)
                .map(|html| parse_duckduckgo_serp(&html))
                .unwrap_or_default()
        }
        _ => Vec::new(),
    }
}

/// Parses Google HTML SERP and extracts outbound result URLs, titles and snippets.
pub fn parse_google_serp(html: &str) -> Vec<RawSerpHit> {
    let document = Html::parse_document(html);
    let Ok(anchor_sel) = Selector::parse("a[href]") else {
        return Vec::new();
    };
    let h3_sel = Selector::parse("h3").ok();
    let mut results = Vec::new();
    let mut seen = BTreeSet::new();

    for anchor in document.select(&anchor_sel) {
        let Some(href) = anchor.value().attr("href") else {
            continue;
        };
        let Some(target) = unwrap_google_href(href) else {
            continue;
        };
        if is_search_engine_internal(&target) || !seen.insert(target.clone()) {
            continue;
        }
        let title = h3_sel
            .as_ref()
            .and_then(|sel| anchor.select(sel).next())
            .map(|el| collapse_whitespace(&el.text().collect::<String>()))
            .filter(|t| !t.is_empty())
            .unwrap_or_else(|| collapse_whitespace(&anchor.text().collect::<String>()));

        let snippet = extract_card_snippet(anchor, &title);

        results.push(RawSerpHit {
            url: target,
            title,
            snippet,
        });
    }
    results
}

fn extract_card_snippet(anchor: scraper::ElementRef<'_>, title: &str) -> String {
    let Ok(snippet_sel) = Selector::parse(
        ".VwiC3b, .b_caption p, .result__snippet, .OrganicTextContentSpan, .TextContainer",
    ) else {
        return String::new();
    };
    for ancestor in anchor
        .ancestors()
        .take(4)
        .filter_map(scraper::ElementRef::wrap)
    {
        for el in ancestor.select(&snippet_sel) {
            let text = collapse_whitespace(&el.text().collect::<String>());
            if !text.is_empty() && text != title {
                return text.chars().take(280).collect();
            }
        }
    }
    String::new()
}

fn unwrap_google_href(href: &str) -> Option<String> {
    if let Some(rest) = href.strip_prefix("/url?") {
        for (k, v) in url::form_urlencoded::parse(rest.as_bytes()) {
            if (k == "q" || k == "url") && (v.starts_with("http://") || v.starts_with("https://")) {
                return Some(v.into_owned());
            }
        }
        return None;
    }
    if href.starts_with("http://") || href.starts_with("https://") {
        return Some(href.to_owned());
    }
    None
}

/// Parses Yandex HTML SERP and extracts outbound result URLs, titles and snippets.
pub fn parse_yandex_serp(html: &str) -> Vec<RawSerpHit> {
    let document = Html::parse_document(html);
    let Ok(anchor_sel) = Selector::parse("a[href]") else {
        return Vec::new();
    };
    let mut results = Vec::new();
    let mut seen = BTreeSet::new();

    for anchor in document.select(&anchor_sel) {
        let Some(href) = anchor.value().attr("href") else {
            continue;
        };
        if !(href.starts_with("http://") || href.starts_with("https://")) {
            continue;
        }
        if is_search_engine_internal(href) || !seen.insert(href.to_owned()) {
            continue;
        }
        let title = collapse_whitespace(&anchor.text().collect::<String>());
        let snippet = extract_card_snippet(anchor, &title);
        results.push(RawSerpHit {
            url: href.to_owned(),
            title,
            snippet,
        });
    }
    results
}

/// Parses Bing HTML SERP (including `bing.com/ck/a` `u=a1...` base64 redirect URLs).
pub fn parse_bing_serp(html: &str) -> Vec<RawSerpHit> {
    let document = Html::parse_document(html);
    let Ok(anchor_sel) = Selector::parse("li.b_algo h2 a[href], a[href]") else {
        return Vec::new();
    };
    let mut results = Vec::new();
    let mut seen = BTreeSet::new();

    for anchor in document.select(&anchor_sel) {
        let Some(href) = anchor.value().attr("href") else {
            continue;
        };
        let Some(target) = unwrap_bing_href(href) else {
            continue;
        };
        if is_search_engine_internal(&target) || !seen.insert(target.clone()) {
            continue;
        }
        let title = collapse_whitespace(&anchor.text().collect::<String>());
        let snippet = extract_card_snippet(anchor, &title);
        results.push(RawSerpHit {
            url: target,
            title,
            snippet,
        });
    }
    results
}

fn unwrap_bing_href(href: &str) -> Option<String> {
    if !(href.starts_with("http://") || href.starts_with("https://")) {
        return None;
    }
    if let Ok(parsed) = Url::parse(href)
        && let Some(host) = parsed.host_str()
        && host.contains("bing.com")
        && parsed.path().starts_with("/ck/a")
    {
        for (k, v) in parsed.query_pairs() {
            if k == "u"
                && let Some(encoded) = v.strip_prefix("a1")
                && let Ok(bytes) = base64::engine::general_purpose::URL_SAFE_NO_PAD
                    .decode(encoded)
                    .or_else(|_| base64::engine::general_purpose::STANDARD.decode(encoded))
                && let Ok(decoded) = String::from_utf8(bytes)
                && (decoded.starts_with("http://") || decoded.starts_with("https://"))
            {
                return Some(decoded);
            }
        }
        return None;
    }
    Some(href.to_owned())
}

/// Parses DuckDuckGo HTML SERP (including `//duckduckgo.com/l/?uddg=...` redirect URLs).
pub fn parse_duckduckgo_serp(html: &str) -> Vec<RawSerpHit> {
    let document = Html::parse_document(html);
    let Ok(anchor_sel) = Selector::parse("a.result__a[href], a[href]") else {
        return Vec::new();
    };
    let mut results = Vec::new();
    let mut seen = BTreeSet::new();

    for anchor in document.select(&anchor_sel) {
        let Some(href) = anchor.value().attr("href") else {
            continue;
        };
        let Some(target) = unwrap_ddg_href(href) else {
            continue;
        };
        if is_search_engine_internal(&target) || !seen.insert(target.clone()) {
            continue;
        }
        let title = collapse_whitespace(&anchor.text().collect::<String>());
        let snippet = extract_card_snippet(anchor, &title);
        results.push(RawSerpHit {
            url: target,
            title,
            snippet,
        });
    }
    results
}

fn unwrap_ddg_href(href: &str) -> Option<String> {
    if let Some((_, query)) = href.split_once("uddg=") {
        let param_end = query.find('&').unwrap_or(query.len());
        let raw_val = &query[..param_end];
        for (k, v) in url::form_urlencoded::parse(format!("u={raw_val}").as_bytes()) {
            if k == "u" && (v.starts_with("http://") || v.starts_with("https://")) {
                return Some(v.into_owned());
            }
        }
    }
    if href.starts_with("http://") || href.starts_with("https://") {
        return Some(href.to_owned());
    }
    None
}

/// Queries the free Internet Archive Wayback Machine CDX API for historically indexed HTML URLs.
pub fn fetch_wayback_cdx(
    agent: &Agent,
    base_host: &str,
    include_subdomains: bool,
) -> Vec<RawSerpHit> {
    let target = if include_subdomains {
        format!("*.{base_host}/*")
    } else {
        format!("{base_host}/*")
    };
    let encoded: String = url::form_urlencoded::byte_serialize(target.as_bytes()).collect();
    let url = format!(
        "https://web.archive.org/cdx/search/cdx?url={encoded}&output=json&fl=original,statuscode,mimetype&filter=mimetype:text/html&filter=statuscode:200&collapse=urlkey&limit=1000"
    );
    let Some(body) = get_text(agent, &url) else {
        return Vec::new();
    };
    parse_wayback_cdx_json(&body)
}

/// Parses Wayback CDX JSON output (`[["original","statuscode","mimetype"], ["https://...", "200", "text/html"]]`).
pub fn parse_wayback_cdx_json(json_text: &str) -> Vec<RawSerpHit> {
    let Ok(rows) = serde_json::from_str::<Vec<Vec<String>>>(json_text) else {
        return Vec::new();
    };
    let mut hits = Vec::new();
    for (idx, row) in rows.into_iter().enumerate() {
        let Some(first) = row.first() else {
            continue;
        };
        if idx == 0 && first == "original" {
            continue;
        }
        if first.starts_with("http://") || first.starts_with("https://") {
            let status = row.get(1).cloned().unwrap_or_default();
            hits.push(RawSerpHit {
                url: first.clone(),
                title: String::new(),
                snippet: if status.is_empty() {
                    "Wayback CDX archive".to_owned()
                } else {
                    format!("Archived HTTP {status}")
                },
            });
        }
    }
    hits
}

/// Google Custom Search with the user's key. It stays on the scan's agent: it cannot run on
/// OctopusOn SEO credits (see the `cloud` module).
fn fetch_google_custom_search(
    agent: &Agent,
    api_key: &str,
    cx: &str,
    query: &str,
    page: u16,
) -> Vec<RawSerpHit> {
    let start = u32::from(page) * 10 + 1;
    let q_enc: String = url::form_urlencoded::byte_serialize(query.as_bytes()).collect();
    let key_enc: String = url::form_urlencoded::byte_serialize(api_key.trim().as_bytes()).collect();
    let cx_enc: String = url::form_urlencoded::byte_serialize(cx.trim().as_bytes()).collect();
    let url = format!(
        "https://customsearch.googleapis.com/customsearch/v1?key={key_enc}&cx={cx_enc}&q={q_enc}&num=10&start={start}"
    );
    let Some(body) = get_text(agent, &url) else {
        return Vec::new();
    };
    parse_google_cse_json(&body)
}

/// Parses Google Custom Search JSON response.
pub fn parse_google_cse_json(body: &str) -> Vec<RawSerpHit> {
    let Ok(val) = serde_json::from_str::<serde_json::Value>(body) else {
        return Vec::new();
    };
    let Some(items) = val.get("items").and_then(serde_json::Value::as_array) else {
        return Vec::new();
    };
    let mut hits = Vec::new();
    for item in items {
        let Some(link) = item.get("link").and_then(serde_json::Value::as_str) else {
            continue;
        };
        let title = item
            .get("title")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_owned();
        let snippet = item
            .get("snippet")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_owned();
        hits.push(RawSerpHit {
            url: link.to_owned(),
            title,
            snippet,
        });
    }
    hits
}

/// The Yandex XML endpoint with the user's user name and key. It stays on the scan's agent: the
/// gateway's Yandex connector serves Search API v2, not this endpoint (see the `cloud` module).
fn fetch_yandex_xml_api(
    agent: &Agent,
    user: &str,
    api_key: &str,
    query: &str,
    page: u16,
) -> Vec<RawSerpHit> {
    let q_enc: String = url::form_urlencoded::byte_serialize(query.as_bytes()).collect();
    let user_enc: String = url::form_urlencoded::byte_serialize(user.trim().as_bytes()).collect();
    let key_enc: String = url::form_urlencoded::byte_serialize(api_key.trim().as_bytes()).collect();
    let url = format!(
        "https://yandex.com.tr/search/xml?user={user_enc}&key={key_enc}&query={q_enc}&page={page}&groupby=attr%3Dd.mode%3Ddeep.groups-on-page%3D100.docs-in-group%3D1"
    );
    let Some(body) = get_text(agent, &url) else {
        return Vec::new();
    };
    parse_yandex_xml(&body)
}

/// Parses Yandex XML search response (`<doc><url>...</url><title>...</title></doc>`).
pub fn parse_yandex_xml(xml: &str) -> Vec<RawSerpHit> {
    let document = Html::parse_document(xml);
    let Ok(doc_sel) = Selector::parse("doc") else {
        return Vec::new();
    };
    let url_sel = Selector::parse("url").ok();
    let title_sel = Selector::parse("title").ok();
    let passage_sel = Selector::parse("passage").ok();
    let mut hits = Vec::new();

    for doc in document.select(&doc_sel) {
        let Some(url) = url_sel
            .as_ref()
            .and_then(|s| doc.select(s).next())
            .map(|el| collapse_whitespace(&el.text().collect::<String>()))
            .filter(|u| u.starts_with("http://") || u.starts_with("https://"))
        else {
            continue;
        };
        let title = title_sel
            .as_ref()
            .and_then(|s| doc.select(s).next())
            .map(|el| collapse_whitespace(&el.text().collect::<String>()))
            .unwrap_or_default();
        let snippet = passage_sel
            .as_ref()
            .and_then(|s| doc.select(s).next())
            .map(|el| collapse_whitespace(&el.text().collect::<String>()))
            .unwrap_or_default();
        hits.push(RawSerpHit {
            url,
            title,
            snippet,
        });
    }
    hits
}

fn fetch_serper_api(
    api: &mut ApiCalls<'_>,
    api_key: &str,
    query: &str,
    page: u16,
) -> Vec<RawSerpHit> {
    let payload = serde_json::json!({
        "q": query,
        "num": 100,
        "page": page + 1,
        "gl": "tr",
        "hl": "tr"
    });
    let request = HttpRequest::post(SERPER_SEARCH, payload.to_string().into_bytes())
        .header("X-API-KEY", api_key.trim())
        .header("Content-Type", "application/json");
    api.send(IndexEngine::SerperApi, &request)
        .map(|body| parse_serper_json(&body))
        .unwrap_or_default()
}

/// Parses Serper.dev JSON response (`organic[].link`, `title`, `snippet`).
pub fn parse_serper_json(body: &str) -> Vec<RawSerpHit> {
    let Ok(val) = serde_json::from_str::<serde_json::Value>(body) else {
        return Vec::new();
    };
    let Some(items) = val.get("organic").and_then(serde_json::Value::as_array) else {
        return Vec::new();
    };
    let mut hits = Vec::new();
    for item in items {
        let Some(link) = item.get("link").and_then(serde_json::Value::as_str) else {
            continue;
        };
        let title = item
            .get("title")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_owned();
        let snippet = item
            .get("snippet")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_owned();
        hits.push(RawSerpHit {
            url: link.to_owned(),
            title,
            snippet,
        });
    }
    hits
}

fn is_search_engine_internal(raw_url: &str) -> bool {
    let Ok(parsed) = Url::parse(raw_url) else {
        return true;
    };
    let Some(host) = parsed.host_str() else {
        return true;
    };
    let lower = host.to_ascii_lowercase();
    [
        "google.",
        "gstatic.com",
        "googleapis.com",
        "googleusercontent.com",
        "yandex.",
        "ya.ru",
        "yastatic.net",
        "bing.com",
        "microsoft.com",
        "duckduckgo.com",
        "archive.org",
    ]
    .iter()
    .any(|pattern| lower.contains(pattern))
}

fn normalize_discovered_url(raw_url: &str, config: &ScanConfig, base_host: &str) -> Option<String> {
    let mut parsed = Url::parse(raw_url.trim()).ok()?;
    if !config.matches_domain(parsed.as_str(), base_host) {
        return None;
    }
    parsed.set_fragment(None);
    // Remove default ports (:80 / :443) that Wayback CDX often includes.
    if (parsed.scheme() == "http" && parsed.port() == Some(80))
        || (parsed.scheme() == "https" && parsed.port() == Some(443))
    {
        let _ = parsed.set_port(None);
    }
    strip_tracking_query_params(&mut parsed);
    // Skip obvious static binary assets so verification focuses on indexable pages.
    let path_lower = parsed.path().to_ascii_lowercase();
    if [
        ".jpg", ".jpeg", ".png", ".gif", ".webp", ".svg", ".ico", ".css", ".js", ".woff", ".woff2",
        ".ttf", ".mp4", ".mp3", ".zip", ".gz",
    ]
    .iter()
    .any(|ext| path_lower.ends_with(ext))
    {
        return None;
    }
    Some(parsed.into())
}

fn strip_tracking_query_params(parsed: &mut Url) {
    if parsed.query().is_none() {
        return;
    }
    let retained: Vec<(String, String)> = parsed
        .query_pairs()
        .filter_map(|(k, v)| {
            let key_lower = k.to_ascii_lowercase();
            let is_tracking = key_lower.starts_with("utm_")
                || matches!(
                    key_lower.as_str(),
                    "gclid"
                        | "fbclid"
                        | "yclid"
                        | "msclkid"
                        | "srsltid"
                        | "_ga"
                        | "_gl"
                        | "mc_cid"
                        | "mc_eid"
                );
            (!is_tracking).then(|| (k.into_owned(), v.into_owned()))
        })
        .collect();
    if retained.is_empty() {
        parsed.set_query(None);
    } else {
        let mut pairs = parsed.query_pairs_mut();
        pairs.clear();
        for (k, v) in retained {
            pairs.append_pair(&k, &v);
        }
    }
}

fn is_synthetic_snippet(s: &str) -> bool {
    s.starts_with("Archived HTTP")
        || s.starts_with("Common Crawl CDX")
        || s.starts_with("XML Sitemap")
        || s == "Wayback CDX archive"
}

fn record_hit(
    catalog: &mut BTreeMap<String, DiscoveredUrl>,
    url: String,
    engine: IndexEngine,
    title: String,
    snippet: String,
    query: &str,
) {
    let entry = catalog.entry(url.clone()).or_insert_with(|| DiscoveredUrl {
        url,
        engines: Vec::new(),
        serp_title: String::new(),
        serp_snippet: String::new(),
        found_by_query: query.to_owned(),
    });
    if !entry.engines.contains(&engine) {
        entry.engines.push(engine);
    }
    if entry.serp_title.is_empty() && !title.is_empty() {
        entry.serp_title = title;
    }
    if !snippet.is_empty()
        && (entry.serp_snippet.is_empty()
            || (is_synthetic_snippet(&entry.serp_snippet) && !is_synthetic_snippet(&snippet)))
    {
        entry.serp_snippet = snippet;
    }
    if query.starts_with("site:") && !entry.found_by_query.starts_with("site:") {
        query.clone_into(&mut entry.found_by_query);
    }
}

/// Derives `site:domain.com/<segment>` queries from the most common top-level directory segments.
pub fn derive_subpath_queries(
    catalog: &BTreeMap<String, DiscoveredUrl>,
    base_host: &str,
    existing_queries: &[String],
    max_queries: usize,
) -> Vec<String> {
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for url_str in catalog.keys() {
        let Ok(parsed) = Url::parse(url_str) else {
            continue;
        };
        let mut segments = parsed.path().split('/').filter(|s| !s.is_empty());
        let Some(first) = segments.next() else {
            continue;
        };
        // Only treat as a directory segment if there is at least a second segment or no file extension.
        if segments.next().is_some()
            && first.len() >= 2
            && first.len() <= 28
            && !first.contains('.')
            && first
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            *counts.entry(first.to_ascii_lowercase()).or_insert(0) += 1;
        }
    }

    let mut ranked: Vec<(String, usize)> = counts.into_iter().collect();
    ranked.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

    let mut result = Vec::new();
    for (segment, _) in ranked {
        let q = format!("site:{base_host}/{segment}");
        if !existing_queries.contains(&q) {
            result.push(q);
            if result.len() >= max_queries {
                break;
            }
        }
    }
    result
}

fn collapse_whitespace(input: &str) -> String {
    input.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_google_serp_links_and_titles() {
        let html = r#"
            <html><body>
                <div class="g">
                    <a href="/url?q=https://acme.com/eski-urun-404&amp;sa=U">
                        <h3>Eski Ürün Sayfası - Acme</h3>
                    </a>
                    <div class="VwiC3b">2024 sezonuna ait keten gömlek ürün açıklaması.</div>
                </div>
                <div class="g">
                    <a href="https://acme.com/blog/seo-rehberi">
                        <h3>SEO Rehberi</h3>
                    </a>
                </div>
            </body></html>
        "#;
        let hits = parse_google_serp(html);
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].url, "https://acme.com/eski-urun-404");
        assert_eq!(hits[0].title, "Eski Ürün Sayfası - Acme");
        assert_eq!(
            hits[0].snippet,
            "2024 sezonuna ait keten gömlek ürün açıklaması."
        );
        assert_eq!(hits[1].url, "https://acme.com/blog/seo-rehberi");
    }

    #[test]
    fn unwraps_bing_ck_base64_links() {
        let encoded = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .encode("https://acme.com/kampanya/2024");
        let html = format!(
            r#"<ul><li class="b_algo"><h2><a href="https://www.bing.com/ck/a?!&&p=123&u=a1{encoded}">Kampanya 2024</a></h2></li></ul>"#
        );
        let hits = parse_bing_serp(&html);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].url, "https://acme.com/kampanya/2024");
        assert_eq!(hits[0].title, "Kampanya 2024");
    }

    #[test]
    fn unwraps_duckduckgo_uddg_links() {
        let html = r#"
            <div class="result">
                <a class="result__a" href="//duckduckgo.com/l/?uddg=https%3A%2F%2Facme.com%2Fsilinen-sayfa&amp;rut=abc">Silinen Sayfa</a>
            </div>
        "#;
        let hits = parse_duckduckgo_serp(html);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].url, "https://acme.com/silinen-sayfa");
        assert_eq!(hits[0].title, "Silinen Sayfa");
    }

    #[test]
    fn parses_wayback_cdx_json_rows() {
        let json = r#"[
            ["original","statuscode","mimetype"],
            ["https://acme.com:443/urun/101","200","text/html"],
            ["https://acme.com/iletisim","200","text/html"]
        ]"#;
        let hits = parse_wayback_cdx_json(json);
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].url, "https://acme.com:443/urun/101");
    }

    #[test]
    fn builds_subpath_and_dork_operator_queries() {
        let cfg = ScanConfig {
            domain: "acme.com".to_owned(),
            seed_queries: vec![
                "urunler".to_owned(),
                "intitle:404".to_owned(),
                "inurl:kampanya".to_owned(),
                "-inurl:tag".to_owned(),
            ],
            ..ScanConfig::default()
        };
        let queries = build_initial_queries(&cfg, "acme.com");
        assert_eq!(
            queries,
            vec![
                "site:acme.com",
                "site:acme.com/urunler",
                "site:acme.com intitle:404",
                "site:acme.com inurl:kampanya",
                "site:acme.com -inurl:tag",
            ]
        );
        let parsed = parse_seed_line(
            "\"404\",\"https://acme.com/p/sku-98\",\"Yazlık Keten Gömlek\",\"2026-10-01\"",
            "acme.com",
        );
        assert_eq!(
            parsed,
            Some((
                "https://acme.com/p/sku-98".to_owned(),
                "Yazlık Keten Gömlek".to_owned()
            ))
        );

        let cleaned = normalize_discovered_url(
            "https://acme.com/urun/101?id=7&utm_source=google&srsltid=AfmBOoq123",
            &cfg,
            "acme.com",
        );
        assert_eq!(cleaned.as_deref(), Some("https://acme.com/urun/101?id=7"));

        let mut cat = BTreeMap::new();
        record_hit(
            &mut cat,
            "https://acme.com/urun/101".to_owned(),
            IndexEngine::Sitemap,
            String::new(),
            "XML Sitemap <loc>".to_owned(),
            "sitemap:xml",
        );
        record_hit(
            &mut cat,
            "https://acme.com/urun/101".to_owned(),
            IndexEngine::Google,
            "Ürün 101".to_owned(),
            "Gerçek Google SERP açıklaması".to_owned(),
            "site:acme.com",
        );
        let item = &cat["https://acme.com/urun/101"];
        assert_eq!(item.serp_snippet, "Gerçek Google SERP açıklaması");
        assert_eq!(item.found_by_query, "site:acme.com");
    }
}
