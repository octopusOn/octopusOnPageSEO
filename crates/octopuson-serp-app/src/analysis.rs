//! Keyword analysis: search, crawl the pages that rank, look their sites up, score.

use std::collections::BTreeMap;
use std::sync::LazyLock;

use octopuson_core::Timestamp;
use octopuson_domain_intel::{Day, registrable};
use octopuson_serp::provider::{
    DataForSeoVolume, KeywordVolume, SearchContext, SearchEvent, VolumeProvider, VolumeQuery,
    Wordstat,
};
use octopuson_serp::{Engine, SerpSnapshot, find_rank};
use octopuson_serp_score::{
    AnalysisInput, KeywordAnalysis, PageSignals, SiteSignals, TargetInput, analyze,
};
use octopuson_serp_store::{AnalysisRecord, AnalysisSummary, PageScoreRow};
use octopuson_spider_app::{CrawlControl, CrawlProgress};
use regex::Regex;
use serde::{Deserialize, Serialize};
use url::Url;

use crate::crawl::{CrawledPage, PageCrawlOptions};
use crate::domains::{DomainFacts, DomainOptions};
use crate::{SearchRequest, SerpAppError, SerpWorkspace};

/// Ranking pages, the user's page and the crawls they came from.
type CrawledResults = (Vec<PageSignals>, Option<PageSignals>, Vec<String>);

/// Results crawled by default.
pub const DEFAULT_PAGES: u16 = 10;

/// An analysis to run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnalyzeRequest {
    /// The search (with the user's site as `target`, if any).
    pub search: SearchRequest,
    /// How many results to crawl (1–20).
    #[serde(default = "default_pages")]
    pub pages: u16,
    /// How pages are crawled.
    #[serde(default)]
    pub crawl: PageCrawlOptions,
    /// The user's page to compare, when it does not rank yet.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_url: Option<String>,
    /// Ask a volume source for monthly searches.
    #[serde(default)]
    pub volume: bool,
    /// Ask DataForSEO for backlink summaries (paid).
    #[serde(default)]
    pub backlinks: bool,
}

const fn default_pages() -> u16 {
    DEFAULT_PAGES
}

/// Progress of an analysis.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "step", rename_all = "kebab-case")]
pub enum AnalysisEvent {
    /// Searching.
    Search {
        /// What the provider reported.
        event: SearchEvent,
    },
    /// Crawling the results.
    Crawl {
        /// URLs done.
        done: u64,
        /// URLs found.
        total: u64,
    },
    /// Looking the domains up.
    Domains {
        /// Domains to look up.
        count: usize,
    },
    /// Monthly searches.
    Volume,
    /// Scoring.
    Scoring,
}

/// The scoring inputs of an analysis, kept so it can be scored again.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnalysisInputs {
    /// Crawled pages.
    pub pages: Vec<PageSignals>,
    /// Sites by domain.
    pub sites: BTreeMap<String, SiteSignals>,
    /// Whether Tranco ranks were available.
    pub tranco: bool,
    /// Monthly searches.
    pub volume: Option<u64>,
    /// The user's site.
    pub target: Option<TargetInput>,
}

/// A finished analysis.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnalysisOutcome {
    /// Stored analysis id.
    pub analysis_id: i64,
    /// Stored snapshot id.
    pub snapshot_id: i64,
    /// The result page.
    pub snapshot: SerpSnapshot,
    /// Scores and recommendations.
    pub analysis: KeywordAnalysis,
    /// Facts about the domains.
    pub domains: Vec<DomainFacts>,
    /// Monthly searches, if asked and available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub volume: Option<KeywordVolume>,
    /// Crawls the pages came from.
    pub crawl_ids: Vec<String>,
    /// US dollars spent on the search.
    pub cost_usd: f64,
}

static DATES: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    [
        r#""dateModified"\s*:\s*"(\d{4}-\d{2}-\d{2})"#,
        r#"(?i)property=["']article:modified_time["'][^>]*content=["'](\d{4}-\d{2}-\d{2})"#,
        r#"(?i)content=["'](\d{4}-\d{2}-\d{2})[^"']*["'][^>]*property=["']article:modified_time"#,
        r#""datePublished"\s*:\s*"(\d{4}-\d{2}-\d{2})"#,
        r#"(?i)property=["']article:published_time["'][^>]*content=["'](\d{4}-\d{2}-\d{2})"#,
    ]
    .iter()
    .filter_map(|pattern| Regex::new(pattern).ok())
    .collect()
});

fn today() -> Option<Day> {
    Day::parse_iso(&Timestamp::now().to_string())
}

/// Days since the page says it was modified (else published).
fn days_since_modified(html: &str) -> Option<u32> {
    let date = DATES
        .iter()
        .find_map(|pattern| pattern.captures(html).and_then(|captures| captures.get(1)))
        .and_then(|date| Day::parse_iso(date.as_str()))?;
    let days = today()?.days() - date.days();
    u32::try_from(days.max(0)).ok()
}

/// What the scores need from a crawled page.
pub(crate) fn page_signals(page: &CrawledPage, position: u16) -> PageSignals {
    let html = page.html.as_deref().unwrap_or_default();
    let url = Url::parse(&page.url).ok();
    let facts = url
        .as_ref()
        .map(|url| octopuson_html::parse_page(html, url, &["googlebot"]))
        .unwrap_or_default();
    let lower = html.to_ascii_lowercase();
    PageSignals {
        position,
        url: page.url.clone(),
        domain: url
            .as_ref()
            .and_then(Url::host_str)
            .map(registrable)
            .unwrap_or_default(),
        status: page.status,
        indexable: matches!(page.indexability.as_str(), "indexable" | "canonicalised"),
        title: facts.title().map(str::to_owned),
        meta_description: facts.meta_descriptions.first().cloned(),
        text: octopuson_text::extract(html),
        schema_types: facts
            .structured_data
            .iter()
            .flat_map(|data| data.types.iter().cloned())
            .collect(),
        images: u32::try_from(facts.images.len()).unwrap_or(u32::MAX),
        images_with_alt: u32::try_from(
            facts
                .images
                .iter()
                .filter(|image| {
                    image
                        .alt
                        .as_deref()
                        .is_some_and(|alt| !alt.trim().is_empty())
                })
                .count(),
        )
        .unwrap_or(u32::MAX),
        has_video: lower.contains("<video")
            || lower.contains("youtube.com/embed")
            || lower.contains("player.vimeo.com")
            || html.contains("VideoObject"),
        days_since_modified: days_since_modified(html),
        findings: page.findings.clone(),
        https: url.as_ref().is_some_and(|url| url.scheme() == "https"),
        hsts: page.hsts,
        response_ms: page.response_ms,
    }
}

fn site_signals(facts: &DomainFacts) -> SiteSignals {
    SiteSignals {
        domain: facts.domain.clone(),
        open_page_rank: facts.open_page_rank,
        referring_domains: facts.referring_domains,
        tranco_rank: facts.tranco_rank,
        age_years: facts.age_years,
        https: false,
        hsts: false,
        technical: None,
    }
}

fn record_of(
    snapshot_id: i64,
    snapshot: &SerpSnapshot,
    analysis: &KeywordAnalysis,
) -> Result<AnalysisRecord, SerpAppError> {
    let query = &snapshot.query;
    Ok(AnalysisRecord {
        id: 0,
        snapshot_id,
        keyword: query.keyword.clone(),
        engine: query.engine,
        country: query.country.clone(),
        language: query.language.clone(),
        device: query.device.as_str().to_owned(),
        created_at: Timestamp::now().to_string(),
        target_domain: analysis.target.as_ref().map(|target| target.domain.clone()),
        difficulty: analysis.difficulty.score,
        band: serde_json::to_value(analysis.band)
            .ok()
            .and_then(|band| band.as_str().map(str::to_owned))
            .unwrap_or_default(),
        intent: serde_json::to_value(analysis.intent.primary)
            .ok()
            .and_then(|intent| intent.as_str().map(str::to_owned))
            .unwrap_or_default(),
        opportunity: analysis
            .target
            .as_ref()
            .map(|target| target.opportunity.score),
        volume: analysis.volume,
        models: to_json(&analysis.models)?,
        result: to_json(analysis)?,
        inputs: String::new(),
        domains: None,
        crawl_ids: None,
        pages: analysis
            .pages
            .iter()
            .chain(
                analysis
                    .target
                    .iter()
                    .filter_map(|target| target.page.as_ref()),
            )
            .map(|page| PageScoreRow {
                position: page.position,
                url: page.url.clone(),
                domain: page.domain.clone(),
                content: page.content.as_ref().map(|content| content.score),
                technical: page.technical.as_ref().map(|technical| technical.score),
                site: page.site.as_ref().map(|site| site.score),
                strength: page.strength,
                words: u32::try_from(page.words).unwrap_or(u32::MAX),
            })
            .collect(),
    })
}

/// A stored JSON document.
fn to_json<T: Serialize + ?Sized>(value: &T) -> Result<String, SerpAppError> {
    serde_json::to_string(value).map_err(|error| SerpAppError::InvalidSettings(error.to_string()))
}

impl SerpWorkspace {
    /// Analyses a keyword: searches it, crawls the top results (and the user's page), looks
    /// their sites up, scores everything and stores the analysis.
    ///
    /// # Errors
    /// [`SerpAppError::Search`] when no provider returned results; crawl, lookup and storage
    /// failures otherwise. Pages and domains that fail are scored without the missing parts.
    pub fn analyze(
        &self,
        request: &AnalyzeRequest,
        control: &CrawlControl,
        events: &dyn Fn(&AnalysisEvent),
    ) -> Result<AnalysisOutcome, SerpAppError> {
        let cancelled = || control.is_stopped();
        let outcome = self.search(&request.search, &cancelled, &|event| {
            events(&AnalysisEvent::Search {
                event: event.clone(),
            });
        })?;
        let snapshot = outcome.snapshot;
        let depth = request.pages.clamp(1, 20);
        // Results whose page the engine hid are scored on their site only: crawling the
        // site's home page would score the wrong page.
        let mut urls: Vec<String> = snapshot
            .top(depth)
            .filter(|result| !result.site_only)
            .map(|result| result.url.clone())
            .collect();
        let ranked_page = outcome.rank.as_ref().filter(|rank| {
            snapshot
                .organic
                .iter()
                .any(|result| result.position == rank.position && !result.site_only)
        });
        let target_url = request
            .target_url
            .clone()
            .or_else(|| ranked_page.map(|rank| rank.url.clone()));
        if let Some(url) = &request.target_url
            && !urls.contains(url)
        {
            urls.push(url.clone());
        }
        let (pages, target_page, crawl_ids) = self.crawl_results(
            &snapshot,
            &urls,
            target_url.as_deref(),
            request.crawl,
            control,
            events,
        )?;

        let target_domain = request
            .search
            .target
            .as_ref()
            .map(|target| registrable(target.value.split('/').next().unwrap_or_default()));
        let mut hosts: Vec<String> = snapshot
            .top(depth.max(10))
            .map(|result| result.host.clone())
            .collect();
        hosts.extend(target_domain.clone());
        events(&AnalysisEvent::Domains { count: hosts.len() });
        let domains = self.domain_facts(
            &hosts,
            DomainOptions {
                refresh: false,
                backlinks: request.backlinks,
            },
            &cancelled,
        )?;
        let sites: BTreeMap<String, SiteSignals> = domains
            .iter()
            .map(|facts| (facts.domain.clone(), site_signals(facts)))
            .collect();

        let volume = if request.volume {
            events(&AnalysisEvent::Volume);
            self.volume(&snapshot).ok().flatten()
        } else {
            None
        };
        let tranco = self.store()?.dataset("tranco")?.is_some();
        let target = target_domain.map(|domain| TargetInput {
            site: sites.get(&domain).cloned().unwrap_or_else(|| SiteSignals {
                domain: domain.clone(),
                ..SiteSignals::default()
            }),
            page: target_page,
            position: outcome.rank.as_ref().map(|rank| rank.position),
            domain,
        });
        let inputs = AnalysisInputs {
            pages,
            sites,
            tranco,
            volume: volume.as_ref().and_then(KeywordVolume::best),
            target,
        };
        events(&AnalysisEvent::Scoring);
        let analysis = score(&snapshot, &inputs);
        let mut record = record_of(outcome.snapshot_id, &snapshot, &analysis)?;
        record.inputs = to_json(&inputs)?;
        record.domains = Some(to_json(&domains)?);
        record.crawl_ids = Some(to_json(&crawl_ids)?);
        let analysis_id = self.store()?.save_analysis(&record)?;
        Ok(AnalysisOutcome {
            analysis_id,
            snapshot_id: outcome.snapshot_id,
            snapshot,
            analysis,
            domains,
            volume,
            crawl_ids,
            cost_usd: outcome.cost_usd,
        })
    }

    /// Crawls the result URLs (and the user's page) and reads their signals.
    fn crawl_results(
        &self,
        snapshot: &SerpSnapshot,
        urls: &[String],
        target_url: Option<&str>,
        options: PageCrawlOptions,
        control: &CrawlControl,
        events: &dyn Fn(&AnalysisEvent),
    ) -> Result<CrawledResults, SerpAppError> {
        let mut on_progress = |progress: &CrawlProgress| {
            events(&AnalysisEvent::Crawl {
                done: u64::from(progress.crawled),
                total: u64::try_from(progress.discovered).unwrap_or(u64::MAX),
            });
        };
        let crawl = self.crawl_pages(urls, options, control, &mut on_progress)?;
        let positions: BTreeMap<&str, u16> = snapshot
            .organic
            .iter()
            .map(|result| (result.url.as_str(), result.position))
            .collect();
        let mut pages = Vec::new();
        let mut target_page = None;
        for page in &crawl.pages {
            let position = positions.get(page.requested.as_str()).copied().unwrap_or(0);
            let signals = page_signals(page, position);
            if target_url == Some(page.requested.as_str()) {
                target_page = Some(signals.clone());
            }
            if position > 0 {
                pages.push(signals);
            }
        }

        Ok((pages, target_page, crawl.crawl_ids))
    }

    /// Scores a stored analysis again with the current models, without new requests.
    ///
    /// # Errors
    /// [`SerpAppError`] for unknown ids and unreadable stored data.
    pub fn rescore(&self, analysis_id: i64) -> Result<KeywordAnalysis, SerpAppError> {
        let mut store = self.store()?;
        let stored = store.analysis(analysis_id)?;
        let snapshot = store.snapshot(stored.snapshot_id)?;
        let inputs: AnalysisInputs = serde_json::from_str(&stored.inputs)
            .map_err(|error| SerpAppError::InvalidSettings(format!("stored inputs: {error}")))?;
        let analysis = score(&snapshot, &inputs);
        let mut record = record_of(stored.snapshot_id, &snapshot, &analysis)?;
        record.id = analysis_id;
        store.update_analysis(&record)?;
        Ok(analysis)
    }

    /// A stored analysis with its snapshot.
    ///
    /// # Errors
    /// [`SerpAppError`] for unknown ids and unreadable stored data.
    pub fn analysis(
        &self,
        analysis_id: i64,
    ) -> Result<(KeywordAnalysis, SerpSnapshot, Vec<DomainFacts>), SerpAppError> {
        let store = self.store()?;
        let stored = store.analysis(analysis_id)?;
        let analysis = serde_json::from_str(&stored.result)
            .map_err(|error| SerpAppError::InvalidSettings(format!("stored analysis: {error}")))?;
        let domains = stored
            .domains
            .as_deref()
            .map(serde_json::from_str)
            .transpose()
            .map_err(|error| SerpAppError::InvalidSettings(format!("stored domains: {error}")))?
            .unwrap_or_default();
        Ok((analysis, store.snapshot(stored.snapshot_id)?, domains))
    }

    /// Stored analyses, newest first.
    ///
    /// # Errors
    /// Store errors.
    pub fn analyses(
        &self,
        keyword: Option<&str>,
        limit: usize,
    ) -> Result<Vec<AnalysisSummary>, SerpAppError> {
        Ok(self.store()?.analyses(keyword, limit)?)
    }

    /// Monthly searches of a snapshot's keyword from the first source that is set up:
    /// Wordstat for Yandex, Google Ads data (DataForSEO) otherwise.
    ///
    /// # Errors
    /// [`SerpAppError`] when the source fails.
    pub fn volume(&self, snapshot: &SerpSnapshot) -> Result<Option<KeywordVolume>, SerpAppError> {
        let query = &snapshot.query;
        let providers: Vec<(&dyn VolumeProvider, &str, &str, &str)> = match query.engine {
            Engine::Yandex => vec![(
                &Wordstat,
                "yandex-cloud",
                "api_key",
                "https://searchapi.api.cloud.yandex.net",
            )],
            Engine::Google | Engine::Bing => {
                vec![(
                    &DataForSeoVolume,
                    "dataforseo",
                    "credentials",
                    "https://api.dataforseo.com",
                )]
            }
        };
        let settings = self.settings()?;
        let transport = self.transport(&settings)?;
        for (provider, owner, secret, origin) in providers {
            // The user's own key, or the placeholder on OctopusOn SEO credits.
            if self
                .source_secret(&settings, owner, secret, origin)
                .is_none()
            {
                continue;
            }
            let keywords = vec![query.keyword.clone()];
            let secrets = |provider: &str, name: &str| {
                let origin = if provider == "yandex-cloud" {
                    "https://searchapi.api.cloud.yandex.net"
                } else {
                    "https://api.dataforseo.com"
                };
                self.source_secret(&settings, provider, name, origin)
            };
            let context = SearchContext {
                transport: &transport,
                secrets: &secrets,
                fetched_at: Timestamp::now().to_string(),
                cancelled: &|| false,
                events: &|_| {},
            };
            let volumes = provider
                .volumes(
                    &VolumeQuery {
                        keywords: &keywords,
                        country: &query.country,
                        language: &query.language,
                        yandex_region: query
                            .location
                            .as_ref()
                            .and_then(|location| location.yandex_lr),
                    },
                    &context,
                )
                .map_err(|error| SerpAppError::InvalidProvider(error.to_string()))?;
            return Ok(volumes.into_iter().next());
        }
        Ok(None)
    }
}

/// Scores stored inputs.
fn score(snapshot: &SerpSnapshot, inputs: &AnalysisInputs) -> KeywordAnalysis {
    analyze(&AnalysisInput {
        snapshot,
        pages: &inputs.pages,
        sites: &inputs.sites,
        tranco: inputs.tranco,
        volume: inputs.volume,
        target: inputs.target.clone(),
    })
}

/// The rank of a domain in a snapshot (re-exported for front ends).
pub fn rank_of(snapshot: &SerpSnapshot, target: &octopuson_serp::Target) -> Option<u16> {
    find_rank(snapshot, target).map(|rank| rank.position)
}
