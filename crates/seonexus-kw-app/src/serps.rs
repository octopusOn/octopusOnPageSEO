//! Result pages for clustering: the top results of chosen keywords through the Keyword
//! Miner's own provider chain (DataForSEO, Serper, SerpApi, the Yandex Search API, and the
//! attended browser when the user adds it), reused while fresh, within each provider's
//! monthly budget; the site's ranking pages are kept for mapping.

use std::collections::{HashMap, HashSet};

use seonexus_core::http::HttpTransport;
use seonexus_keywords::{Level, result_key, site_domain};
use seonexus_kw_store::{
    AttemptOutcome, AttemptRecord, ClusteringRow, KwStore, RunKind, RunStatus, SerpRecord,
    SitePageRecord,
};
use seonexus_serp::provider::{
    self, Attempt, BROWSER_ID, BrowserPacing, ChainFailure, ProviderError, SearchContext,
    SearchEvent, SerpProvider, SpecProvider, YandexCloud, builtin_specs, search_chain,
};
use seonexus_serp::{Device, Engine, FeatureKind, SerpQuery, SerpSnapshot};
use serde::{Deserialize, Serialize};

use crate::runner::{CheckPage, Failure, Progress, RunControl, RunSummary, usable};
use crate::{KeywordWorkspace, KwAppError, ago, month, now};

/// Source of the site's pages read from result pages.
pub const SERP_SOURCE: &str = "serp";
/// Results compared per keyword.
const DEPTH: u16 = 10;
/// Keywords in a row without a result page before a run stops.
const MAX_FAILURES_IN_A_ROW: u32 = 3;

/// Which keywords get result pages.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct SerpRequest {
    /// Engine (the project's primary engine when absent).
    pub engine: Option<String>,
    /// `desktop` or `mobile`.
    pub device: String,
    /// Only the most searched keywords.
    pub top: Option<u32>,
    /// Only the most searched members of each cluster of the newest clustering.
    pub per_cluster: Option<u32>,
    /// Fetch again even when a fresh page is stored.
    pub refresh: bool,
}

impl Default for SerpRequest {
    fn default() -> Self {
        Self {
            engine: None,
            device: "desktop".to_owned(),
            top: None,
            per_cluster: None,
            refresh: false,
        }
    }
}

/// What fetching result pages will cost.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SerpEstimate {
    /// Keywords that need a page.
    pub keywords: u32,
    /// Chosen keywords with a fresh page already.
    pub cached: u32,
    /// The first usable provider.
    pub provider: Option<String>,
    /// Estimated US dollars on it.
    pub cost_usd: f64,
    /// Providers of the chain that cannot be used, with why.
    pub skipped: Vec<(String, String)>,
}

/// What fetching result pages did.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SerpOutcome {
    /// Requests, cost and problems; `found` counts the pages stored.
    #[serde(flatten)]
    pub run: RunSummary,
    /// Pages of the site seen on the stored result pages.
    pub site_pages: u64,
}

/// The usable providers of a chain, and the others with why they cannot be used.
type Chain<'a> = (Vec<&'a dyn SerpProvider>, Vec<(String, String)>);

/// What to do after one keyword.
enum Next {
    Continue,
    Stop,
    Cancel,
}

/// The engine of an id.
fn engine_of(id: &str) -> Result<Engine, KwAppError> {
    match id {
        "google" => Ok(Engine::Google),
        "yandex" => Ok(Engine::Yandex),
        "bing" => Ok(Engine::Bing),
        other => Err(KwAppError::Invalid(format!("unknown engine `{other}`"))),
    }
}

fn device_of(id: &str) -> Result<Device, KwAppError> {
    match id {
        "desktop" => Ok(Device::Desktop),
        "mobile" => Ok(Device::Mobile),
        other => Err(KwAppError::Invalid(format!(
            "unknown device `{other}` (desktop or mobile)"
        ))),
    }
}

/// The built-in result-page APIs.
fn catalog() -> Vec<Box<dyn SerpProvider>> {
    let mut providers: Vec<Box<dyn SerpProvider>> = builtin_specs()
        .unwrap_or_default()
        .iter()
        .filter_map(|spec| spec.compile().ok())
        .map(|compiled| Box::new(SpecProvider::new(compiled)) as Box<dyn SerpProvider>)
        .collect();
    providers.push(Box::new(YandexCloud));
    providers
}

/// What a stored page keeps of a snapshot.
fn record(keyword_id: i64, snapshot: &SerpSnapshot) -> SerpRecord {
    let texts = |kind: FeatureKind| -> Vec<String> {
        snapshot
            .features
            .iter()
            .filter(|feature| feature.kind == kind)
            .flat_map(|feature| feature.items.iter().filter_map(|item| item.text.clone()))
            .collect()
    };
    let mut features: Vec<String> = snapshot
        .features
        .iter()
        .map(|feature| feature.kind.as_str().to_owned())
        .collect();
    features.sort();
    features.dedup();
    SerpRecord {
        keyword_id,
        engine: snapshot.query.engine.as_str().to_owned(),
        device: snapshot.query.device.as_str().to_owned(),
        provider: snapshot.provider.clone(),
        fetched_at: snapshot.fetched_at.clone(),
        urls: snapshot
            .organic
            .iter()
            .take(usize::from(DEPTH))
            .map(|result| result.url.clone())
            .collect(),
        site_only: snapshot.organic.iter().any(|result| result.site_only),
        features,
        questions: texts(FeatureKind::PeopleAlsoAsk),
        related: texts(FeatureKind::RelatedSearches),
        raw_path: None,
    }
}

fn outcome_of(attempt: &Attempt) -> AttemptOutcome {
    match &attempt.outcome {
        provider::AttemptOutcome::Success => AttemptOutcome::Success,
        provider::AttemptOutcome::Skipped => AttemptOutcome::Skipped,
        provider::AttemptOutcome::Failed(code)
            if matches!(code.as_str(), "auth" | "quota" | "rate-limited" | "blocked") =>
        {
            AttemptOutcome::Refused
        }
        provider::AttemptOutcome::Failed(_) => AttemptOutcome::Failed,
    }
}

/// Pages of the site on every stored result page of an engine and device, by keyword.
fn site_pages(
    store: &mut KwStore,
    project_id: i64,
    site: &str,
    engine: &str,
    device: &str,
) -> Result<Vec<SitePageRecord>, KwAppError> {
    let ids: Vec<i64> = store.key_index(project_id)?.into_values().collect();
    let mut pages = Vec::new();
    for serp in store.serps(project_id, &ids, engine, device)? {
        if serp.site_only {
            continue;
        }
        let mut seen = HashSet::new();
        for (rank, url) in serp.urls.iter().enumerate() {
            if result_key(url, Level::Site).as_deref() == Some(site) && seen.insert(url.clone()) {
                pages.push(SitePageRecord {
                    keyword_id: serp.keyword_id,
                    url: url.clone(),
                    source: SERP_SOURCE.to_owned(),
                    clicks: None,
                    impressions: None,
                    position: u32::try_from(rank + 1).ok().map(f64::from),
                    period: serp.fetched_at.chars().take(10).collect(),
                });
            }
        }
    }
    Ok(pages)
}

impl KeywordWorkspace {
    /// The result-page providers of an engine: the APIs, and the browser when the engine's
    /// chain lists it.
    fn serp_catalog(&self, engine: &str) -> Result<Vec<Box<dyn SerpProvider>>, KwAppError> {
        let settings = self.settings()?;
        let mut providers = catalog();
        if settings
            .serp
            .chains
            .get(engine)
            .is_some_and(|chain| chain.iter().any(|id| id == BROWSER_ID))
        {
            providers.push(self.browser_provider(&settings)?);
        }
        Ok(providers)
    }

    /// The chosen keywords (id and text) and how many of them have a fresh page already.
    fn serp_keywords(
        &self,
        project_id: i64,
        request: &SerpRequest,
        engine: &str,
    ) -> Result<(Vec<(i64, String)>, u32), KwAppError> {
        let settings = self.settings()?;
        let store = self.store()?;
        let mut inputs = store.cluster_inputs(project_id, engine, &request.device)?;
        // Most searched first; unknown volumes last.
        inputs.sort_by(|a, b| b.volume.cmp(&a.volume).then_with(|| a.id.cmp(&b.id)));
        let mut chosen: Vec<(i64, String)> =
            match (request.per_cluster, store.latest_clustering(project_id)?) {
                (Some(per_cluster), Some(clustering)) => {
                    let wanted: HashSet<i64> = {
                        let rank: HashMap<i64, usize> = inputs
                            .iter()
                            .enumerate()
                            .map(|(index, input)| (input.id, index))
                            .collect();
                        let mut by_cluster: HashMap<u32, Vec<i64>> = HashMap::new();
                        for member in store.member_records(clustering.id)? {
                            if rank.contains_key(&member.keyword_id) {
                                by_cluster
                                    .entry(member.number)
                                    .or_default()
                                    .push(member.keyword_id);
                            }
                        }
                        by_cluster
                            .into_values()
                            .flat_map(|mut ids| {
                                ids.sort_by_key(|id| rank[id]);
                                ids.truncate(per_cluster as usize);
                                ids
                            })
                            .collect()
                    };
                    inputs
                        .iter()
                        .filter(|input| wanted.contains(&input.id))
                        .map(|input| (input.id, input.keyword.clone()))
                        .collect()
                }
                _ => inputs
                    .iter()
                    .map(|input| (input.id, input.keyword.clone()))
                    .collect(),
            };
        if let Some(top) = request.top {
            chosen.truncate(top as usize);
        }
        let before = chosen.len();
        if !request.refresh {
            let fresh_after = ago(i64::from(settings.serp.max_age_days) * 86_400);
            let ages = store.serp_ages(project_id, engine, &request.device)?;
            chosen.retain(|(id, _)| ages.get(id).is_none_or(|fetched| *fetched < fresh_after));
        }
        let cached = u32::try_from(before - chosen.len()).unwrap_or(u32::MAX);
        Ok((chosen, cached))
    }

    /// Providers of the engine's chain that can be used now, and why the others cannot.
    fn serp_chain<'a>(
        &self,
        providers: &'a [Box<dyn SerpProvider>],
        engine: &str,
        query: &SerpQuery,
    ) -> Result<Chain<'a>, KwAppError> {
        let settings = self.settings()?;
        let mut chain = Vec::new();
        let mut skipped = Vec::new();
        for id in settings.serp.chains.get(engine).into_iter().flatten() {
            let Some(provider) = providers.iter().find(|provider| provider.id() == id) else {
                skipped.push((id.clone(), "unknown provider".to_owned()));
                continue;
            };
            let reason = if provider.capabilities().supports(query) {
                match usable(self, &settings, id) {
                    Err(reason) => Some(reason),
                    Ok(()) if id == BROWSER_ID => {
                        self.browser_unusable(&settings, engine, query.pages())?
                    }
                    Ok(()) => None,
                }
            } else {
                Some(format!(
                    "does not return {} {} results",
                    query.device.as_str(),
                    engine
                ))
            };
            let reason = reason.or_else(|| {
                let budget = settings.budgets.get(id)?;
                let spent = self.store().ok()?.spent(id, &month()).ok()?;
                (spent + provider.estimate(query).usd > *budget)
                    .then(|| format!("the monthly budget of {budget} USD is reached"))
            });
            match reason {
                Some(reason) => skipped.push((id.clone(), reason)),
                None => chain.push(provider.as_ref()),
            }
        }
        Ok((chain, skipped))
    }

    fn query_for(
        &self,
        project_id: i64,
        engine: &str,
        device: &str,
        keyword: &str,
    ) -> Result<SerpQuery, KwAppError> {
        let project = self.project(project_id)?;
        let mut query = SerpQuery::new(engine_of(engine)?, keyword)
            .localised(&project.spec.country, &project.spec.language)
            .with_depth(DEPTH);
        query.device = device_of(device)?;
        Ok(query)
    }

    /// The keywords, provider and cost of fetching result pages.
    ///
    /// # Errors
    /// [`KwAppError::Invalid`] for an unknown engine or device; settings and store errors.
    pub fn estimate_serps(
        &self,
        project_id: i64,
        request: &SerpRequest,
    ) -> Result<SerpEstimate, KwAppError> {
        let engine = self.engine_for(project_id, request.engine.as_deref())?;
        let (keywords, cached) = self.serp_keywords(project_id, request, &engine)?;
        self.warm_catalog(&self.settings()?)?;
        let providers = self.serp_catalog(&engine)?;
        let query = self.query_for(project_id, &engine, &request.device, "kahve")?;
        let (chain, skipped) = self.serp_chain(&providers, &engine, &query)?;
        let count = u32::try_from(keywords.len()).unwrap_or(u32::MAX);
        Ok(SerpEstimate {
            keywords: count,
            cached,
            provider: chain.first().map(|provider| provider.id().to_owned()),
            cost_usd: chain.first().map_or(0.0, |provider| {
                let settings = self.settings().unwrap_or_default();
                provider.estimate(&query).usd
                    * self.price_factor(&settings, provider.id())
                    * f64::from(count)
            }),
            skipped,
        })
    }

    fn engine_for(&self, project_id: i64, engine: Option<&str>) -> Result<String, KwAppError> {
        let project = self.project(project_id)?;
        let engine = engine
            .map(str::to_owned)
            .or_else(|| project.spec.engines.first().cloned())
            .unwrap_or_else(|| "google".to_owned());
        engine_of(&engine)?;
        Ok(engine)
    }

    /// Fetches result pages with the user's network settings.
    ///
    /// # Errors
    /// See [`KeywordWorkspace::fetch_serps_with`].
    pub fn fetch_serps(
        &self,
        project_id: i64,
        request: &SerpRequest,
        control: &RunControl<'_>,
    ) -> Result<SerpOutcome, KwAppError> {
        let transport = self.transport(&self.settings()?)?;
        self.fetch_serps_with(project_id, request, &transport, control)
    }

    /// Fetches the top results of the chosen keywords, one keyword at a time through the
    /// chain, and stores them with the site's ranking pages.
    ///
    /// # Errors
    /// [`KwAppError::Invalid`] when no keyword needs a page or no provider can be used;
    /// store errors.
    pub fn fetch_serps_with(
        &self,
        project_id: i64,
        request: &SerpRequest,
        transport: &dyn HttpTransport,
        control: &RunControl<'_>,
    ) -> Result<SerpOutcome, KwAppError> {
        let engine = self.engine_for(project_id, request.engine.as_deref())?;
        let (keywords, _) = self.serp_keywords(project_id, request, &engine)?;
        if keywords.is_empty() {
            return Err(KwAppError::Invalid(
                "every chosen keyword has a fresh result page; pass refresh to fetch again"
                    .to_owned(),
            ));
        }
        let providers = self.serp_catalog(&engine)?;
        let planned = u32::try_from(keywords.len()).unwrap_or(u32::MAX);
        let run_id =
            self.store()?
                .begin_run(RunKind::Serp, Some(project_id), planned, 0.0, &now())?;
        let mut summary = RunSummary {
            run_id,
            status: RunStatus::Done,
            requests: 0,
            cost_usd: 0.0,
            found: 0,
            failures: Vec::new(),
        };
        let mut progress = Progress {
            run_id,
            done: 0,
            planned,
            step: engine.clone(),
            found: 0,
            waiting_until: None,
            check: None,
        };
        let mut in_a_row = 0;
        for (index, (keyword_id, keyword)) in keywords.iter().enumerate() {
            if (control.cancelled)() {
                summary.status = RunStatus::Cancelled;
                break;
            }
            let query = self.query_for(project_id, &engine, &request.device, keyword)?;
            match self.fetch_one(
                run_id,
                &providers,
                &engine,
                &query,
                *keyword_id,
                transport,
                control,
                &progress,
                &mut summary,
                &mut in_a_row,
            )? {
                Next::Continue => {}
                Next::Stop => break,
                Next::Cancel => {
                    summary.status = RunStatus::Cancelled;
                    break;
                }
            }
            progress.done = u32::try_from(index + 1).unwrap_or(u32::MAX);
            progress.found = summary.found;
            (control.progress)(&progress);
        }
        if summary.status != RunStatus::Cancelled && !summary.failures.is_empty() {
            summary.status = if summary.found == 0 {
                RunStatus::Failed
            } else {
                RunStatus::Partial
            };
        }
        let site_pages = self.refresh_serp_pages(project_id, &engine, &request.device)?;
        let error = summary
            .failures
            .first()
            .map(|failure| failure.error.clone());
        self.store()?.finish_run(
            run_id,
            summary.status,
            u32::try_from(summary.found).unwrap_or(u32::MAX),
            &now(),
            error.as_deref(),
        )?;
        Ok(SerpOutcome {
            run: summary,
            site_pages,
        })
    }

    /// Fetches the result page of one keyword through the chain and stores it. A check page
    /// in the browser is reported while it waits for the person; one left uncleared stops the
    /// run.
    #[allow(clippy::too_many_arguments)]
    fn fetch_one(
        &self,
        run_id: i64,
        providers: &[Box<dyn SerpProvider>],
        engine: &str,
        query: &SerpQuery,
        keyword_id: i64,
        transport: &dyn HttpTransport,
        control: &RunControl<'_>,
        progress: &Progress,
        summary: &mut RunSummary,
        in_a_row: &mut u32,
    ) -> Result<Next, KwAppError> {
        let (chain, skipped) = self.serp_chain(providers, engine, query)?;
        if chain.is_empty() {
            summary.failures.push(Failure {
                provider: "serp".to_owned(),
                step: engine.to_owned(),
                error: skipped
                    .iter()
                    .map(|(id, reason)| format!("{id}: {reason}"))
                    .collect::<Vec<_>>()
                    .join("; "),
            });
            return Ok(Next::Stop);
        }
        let secrets = |owner: &str, name: &str| self.secret(owner, name);
        let events = |event: &SearchEvent| match event {
            SearchEvent::Challenge { kind, url, .. } => {
                let timeout = BrowserPacing::default().challenge_timeout.as_secs();
                let mut waiting = progress.clone();
                waiting.check = Some(CheckPage {
                    kind: *kind,
                    url: url.clone(),
                    until: ago(-i64::try_from(timeout).unwrap_or(i64::MAX)),
                });
                (control.progress)(&waiting);
            }
            SearchEvent::Resumed { .. } => (control.progress)(progress),
            SearchEvent::Waiting { .. } | SearchEvent::Page { .. } => {}
        };
        let context = SearchContext {
            transport,
            secrets: &secrets,
            fetched_at: now(),
            cancelled: control.cancelled,
            events: &events,
        };
        let result = search_chain(&chain, query, &context);
        // On OctopusOn SEO credits the price is what the gateway charged.
        let charged = transport.take_charge_usd();
        match result {
            Ok((fetch, attempts)) => {
                let cost_usd = charged.unwrap_or_else(|| fetch.cost_usd());
                self.record_chain(run_id, engine, &attempts, Some((fetch.requests, cost_usd)))?;
                summary.requests += fetch.requests;
                summary.cost_usd += cost_usd;
                self.store()?
                    .put_serps(&[record(keyword_id, &fetch.snapshot)])?;
                summary.found += 1;
                *in_a_row = 0;
                Ok(Next::Continue)
            }
            Err(ChainFailure { error, attempts }) => {
                self.record_chain(run_id, engine, &attempts, None)?;
                if matches!(error, ProviderError::Cancelled) {
                    return Ok(Next::Cancel);
                }
                // Nobody cleared the check page: the next keyword would only wait again.
                let blocked = matches!(error, ProviderError::Blocked(_));
                let message = error.to_string();
                if !summary
                    .failures
                    .iter()
                    .any(|failure| failure.error == message)
                {
                    summary.failures.push(Failure {
                        provider: attempts
                            .last()
                            .map_or_else(|| "serp".to_owned(), |attempt| attempt.provider.clone()),
                        step: query.keyword.clone(),
                        error: message,
                    });
                }
                *in_a_row += 1;
                Ok(if blocked || *in_a_row >= MAX_FAILURES_IN_A_ROW {
                    Next::Stop
                } else {
                    Next::Continue
                })
            }
        }
    }

    fn record_chain(
        &self,
        run_id: i64,
        engine: &str,
        attempts: &[Attempt],
        success: Option<(u32, f64)>,
    ) -> Result<(), KwAppError> {
        let settings = self.settings()?;
        let store = self.store()?;
        for attempt in attempts {
            let outcome = outcome_of(attempt);
            let (requests, cost_usd) = match (outcome, success) {
                (AttemptOutcome::Success, Some(fetch)) => fetch,
                (AttemptOutcome::Skipped, _) => (0, 0.0),
                _ => (1, 0.0),
            };
            store.record_attempt(&AttemptRecord {
                run_id: Some(run_id),
                provider: attempt.provider.clone(),
                attempted_at: now(),
                outcome,
                requests,
                items: u32::from(outcome == AttemptOutcome::Success),
                cost_usd,
                error: attempt.message.clone(),
                engine: Some(engine.to_owned()),
                via: crate::runner::via_label(self.via(&settings, &attempt.provider)),
            })?;
        }
        Ok(())
    }

    /// Rebuilds the site's pages seen on stored result pages; returns how many there are.
    ///
    /// # Errors
    /// Store errors.
    pub fn refresh_serp_pages(
        &self,
        project_id: i64,
        engine: &str,
        device: &str,
    ) -> Result<u64, KwAppError> {
        let project = self.project(project_id)?;
        let Some(site) = project.spec.site.as_deref().and_then(site_domain) else {
            return Ok(0);
        };
        let mut store = self.store()?;
        let pages = site_pages(&mut store, project_id, &site, engine, device)?;
        store.put_site_pages(project_id, SERP_SOURCE, &pages)?;
        Ok(u64::try_from(pages.len()).unwrap_or(u64::MAX))
    }

    /// Maps the newest clustering again with the current site pages and result pages,
    /// without changing its clusters.
    ///
    /// # Errors
    /// [`KwAppError::Invalid`] for a project without clusterings; store errors.
    pub fn remap(&self, project_id: i64) -> Result<ClusteringRow, KwAppError> {
        crate::cluster::remap(self, project_id)
    }
}

#[cfg(test)]
#[path = "serps_tests.rs"]
mod tests;
