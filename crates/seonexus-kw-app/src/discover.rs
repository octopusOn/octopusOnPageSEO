//! Discovery: new keywords from seeds and competitors through the paid sources, filtered,
//! and added with the source and seed that found them, their numbers and intents.

use std::collections::BTreeSet;
use std::sync::OnceLock;
use std::time::Duration;

use seonexus_core::http::HttpTransport;
use seonexus_keyword_sources::dataforseo::{DataForSeo, prices};
use seonexus_keyword_sources::google_ads::GoogleAds;
use seonexus_keyword_sources::serpapi::SerpApi;
use seonexus_keyword_sources::suggest::FreeSuggest;
use seonexus_keyword_sources::wordstat::Wordstat;
use seonexus_keyword_sources::{Fetched, FoundKeyword, Market, SourceContext, SourceError};
use seonexus_keywords::{Expansion, Filter, FilterSpec, expand, normalize};
use seonexus_kw_store::{
    Added, IntentRecord, KwStore, MetricRecord, MonthlyVolume, ProjectRow, RunKind,
};
use seonexus_text::Language;
use serde::{Deserialize, Serialize};

use crate::keywords::{fill_intents, prepare};
use crate::runner::{RunControl, RunSummary, Sender, Step, usable};
use crate::{KeywordWorkspace, KwAppError, month, now};

/// Most seeds one discovery takes.
pub const MAX_SEEDS: usize = 1_000;

/// A way to find keywords.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DiscoverySource {
    /// Wordstat's most searched phrases with the seed and its associations (Yandex).
    Wordstat,
    /// Google Ads keyword ideas through DataForSEO (20 seeds per request).
    AdsIdeas,
    /// DataForSEO Labs ideas: the same topic in other words (200 seeds per request).
    LabsIdeas,
    /// DataForSEO Labs "searches related to", two levels deep.
    Related,
    /// DataForSEO Labs suggestions: longer keywords containing the seed.
    Suggestions,
    /// The keywords competitors rank for (DataForSEO Labs).
    Competitors,
    /// Google autocomplete through DataForSEO, for the seed and its expansions.
    Autocomplete,
    /// Google autocomplete through SerpApi, for the seed and its expansions.
    SerpapiAutocomplete,
    /// Google Ads Keyword Planner ideas with the user's own account (20 seeds per request).
    GoogleAds,
    /// The free Google suggestion endpoint (off unless switched on), for the seed and its
    /// expansions.
    GoogleSuggest,
    /// The free Yandex suggestion endpoint (off unless switched on).
    YandexSuggest,
}

impl DiscoverySource {
    /// Every source, in run order.
    pub const ALL: [Self; 11] = [
        Self::GoogleAds,
        Self::GoogleSuggest,
        Self::YandexSuggest,
        Self::Wordstat,
        Self::AdsIdeas,
        Self::LabsIdeas,
        Self::Related,
        Self::Suggestions,
        Self::Competitors,
        Self::Autocomplete,
        Self::SerpapiAutocomplete,
    ];

    /// The keyword source recorded for what it finds.
    pub const fn id(self) -> &'static str {
        match self {
            Self::Wordstat => "wordstat",
            Self::AdsIdeas => "dataforseo-ads-ideas",
            Self::LabsIdeas => "dataforseo-ideas",
            Self::Related => "dataforseo-related",
            Self::Suggestions => "dataforseo-suggestions",
            Self::Competitors => "dataforseo-competitors",
            Self::Autocomplete => "dataforseo-autocomplete",
            Self::SerpapiAutocomplete => "serpapi-autocomplete",
            Self::GoogleAds => "google-ads-ideas",
            Self::GoogleSuggest => FreeSuggest::GOOGLE,
            Self::YandexSuggest => FreeSuggest::YANDEX,
        }
    }

    /// The provider it asks.
    pub const fn provider(self) -> &'static str {
        match self {
            Self::Wordstat => Wordstat::ID,
            Self::SerpapiAutocomplete => SerpApi::ID,
            Self::GoogleAds => GoogleAds::ID,
            Self::GoogleSuggest => FreeSuggest::GOOGLE,
            Self::YandexSuggest => FreeSuggest::YANDEX,
            _ => DataForSeo::ID,
        }
    }

    /// Source and engine of the numbers it brings, if any.
    const fn numbers(self) -> Option<(&'static str, &'static str)> {
        match self {
            Self::Wordstat => Some(("wordstat", "yandex")),
            Self::AdsIdeas => Some(("dataforseo-ads", "google")),
            Self::LabsIdeas | Self::Related | Self::Suggestions | Self::Competitors => {
                Some(("dataforseo-labs", "google"))
            }
            Self::GoogleAds => Some(("google-ads", "google")),
            Self::Autocomplete
            | Self::SerpapiAutocomplete
            | Self::GoogleSuggest
            | Self::YandexSuggest => None,
        }
    }
}

/// What to discover.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DiscoverRequest {
    /// Seed keywords.
    pub seeds: Vec<String>,
    /// Competitor domains; the project's competitors when empty.
    pub competitors: Vec<String>,
    /// Sources to ask.
    pub sources: Vec<DiscoverySource>,
    /// Expansions of each seed for the autocomplete sources.
    pub expansions: Vec<Expansion>,
    /// Results per Wordstat or Labs request.
    pub limit: u32,
    /// Which found keywords are kept.
    pub filter: FilterSpec,
}

impl Default for DiscoverRequest {
    fn default() -> Self {
        Self {
            seeds: Vec::new(),
            competitors: Vec::new(),
            sources: Vec::new(),
            expansions: Vec::new(),
            limit: 500,
            filter: FilterSpec::default(),
        }
    }
}

/// What a source of a discovery will cost.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SourceEstimate {
    /// The source.
    pub source: DiscoverySource,
    /// Its provider.
    pub provider: String,
    /// Requests.
    pub requests: u32,
    /// Estimated US dollars (Labs at the most results).
    pub cost_usd: f64,
    /// Whether it can run now.
    pub ready: bool,
    /// Why not.
    pub reason: Option<String>,
}

/// What a discovery will cost.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DiscoverEstimate {
    /// Per source.
    pub sources: Vec<SourceEstimate>,
    /// Requests of the sources that can run.
    pub requests: u32,
    /// US dollars of the sources that can run.
    pub cost_usd: f64,
    /// Hours Wordstat's hourly quota stretches the run over.
    pub wordstat_hours: f64,
}

/// What a discovery did.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DiscoverOutcome {
    /// Requests, cost, found keywords and problems.
    #[serde(flatten)]
    pub run: RunSummary,
    /// Found keywords the filter kept.
    pub kept: u64,
    /// New and known keywords among them.
    pub added: Added,
}

/// What one request asks.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Input {
    Seed(String),
    Seeds(Vec<String>),
    Domain(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Call {
    source: DiscoverySource,
    input: Input,
    /// The seed or domain recorded with what it finds.
    seed: String,
}

fn clean(texts: &[String]) -> Vec<String> {
    let mut seen = BTreeSet::new();
    texts
        .iter()
        .map(|text| text.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|text| !text.is_empty() && seen.insert(text.to_lowercase()))
        .collect()
}

/// The requests of a discovery, in source order.
fn plan(
    request: &DiscoverRequest,
    project: &ProjectRow,
    language: Language,
) -> Result<Vec<Call>, KwAppError> {
    let seeds = clean(&request.seeds);
    if seeds.len() > MAX_SEEDS {
        return Err(KwAppError::Invalid(format!(
            "use at most {MAX_SEEDS} seeds"
        )));
    }
    let competitors = clean(if request.competitors.is_empty() {
        &project.spec.competitors
    } else {
        &request.competitors
    });
    let sources: BTreeSet<DiscoverySource> = request.sources.iter().copied().collect();
    let mut calls = Vec::new();
    for source in sources {
        let single = |seed: &String| Call {
            source,
            input: Input::Seed(seed.clone()),
            seed: seed.clone(),
        };
        match source {
            DiscoverySource::Wordstat | DiscoverySource::Related | DiscoverySource::Suggestions => {
                calls.extend(seeds.iter().map(single));
            }
            DiscoverySource::AdsIdeas | DiscoverySource::LabsIdeas | DiscoverySource::GoogleAds => {
                let size = match source {
                    DiscoverySource::AdsIdeas => DataForSeo::ADS_SEEDS,
                    DiscoverySource::GoogleAds => GoogleAds::SEEDS,
                    _ => DataForSeo::LABS_SEEDS,
                };
                calls.extend(seeds.chunks(size).map(|chunk| Call {
                    source,
                    input: Input::Seeds(chunk.to_vec()),
                    seed: chunk.join(", "),
                }));
            }
            DiscoverySource::Competitors => {
                calls.extend(competitors.iter().map(|domain| Call {
                    source,
                    input: Input::Domain(domain.clone()),
                    seed: domain.clone(),
                }));
            }
            DiscoverySource::Autocomplete
            | DiscoverySource::SerpapiAutocomplete
            | DiscoverySource::GoogleSuggest
            | DiscoverySource::YandexSuggest => {
                for seed in &seeds {
                    let queries = std::iter::once(seed.clone()).chain(expand(
                        seed,
                        language,
                        &request.expansions,
                    ));
                    calls.extend(queries.map(|query| Call {
                        source,
                        input: Input::Seed(query),
                        seed: seed.clone(),
                    }));
                }
            }
        }
    }
    Ok(calls)
}

impl Call {
    fn cost(&self, limit: u32) -> f64 {
        match self.source {
            DiscoverySource::Wordstat => Wordstat::COST_PER_REQUEST_USD,
            DiscoverySource::AdsIdeas => prices::ADS_TASK,
            DiscoverySource::LabsIdeas
            | DiscoverySource::Related
            | DiscoverySource::Suggestions
            | DiscoverySource::Competitors => DataForSeo::labs_estimate(limit),
            DiscoverySource::Autocomplete => prices::AUTOCOMPLETE,
            DiscoverySource::SerpapiAutocomplete => SerpApi::COST_PER_REQUEST_USD,
            DiscoverySource::GoogleAds
            | DiscoverySource::GoogleSuggest
            | DiscoverySource::YandexSuggest => 0.0,
        }
    }

    fn send(
        &self,
        context: &SourceContext<'_>,
        market: &Market,
        limit: u32,
    ) -> Result<Fetched, SourceError> {
        let (seed, seeds, domain) = match &self.input {
            Input::Seed(seed) => (seed.as_str(), std::slice::from_ref(seed), ""),
            Input::Seeds(seeds) => ("", seeds.as_slice(), ""),
            Input::Domain(domain) => ("", &[][..], domain.as_str()),
        };
        match self.source {
            DiscoverySource::Wordstat => Wordstat::top_requests(context, seed, market, limit),
            DiscoverySource::AdsIdeas => DataForSeo::keywords_for_keywords(context, seeds, market),
            DiscoverySource::LabsIdeas => DataForSeo::keyword_ideas(context, seeds, market, limit),
            DiscoverySource::Related => {
                DataForSeo::related_keywords(context, seed, market, 2, limit)
            }
            DiscoverySource::Suggestions => {
                DataForSeo::keyword_suggestions(context, seed, market, limit)
            }
            DiscoverySource::Competitors => {
                DataForSeo::ranked_keywords(context, domain, market, limit)
            }
            DiscoverySource::Autocomplete => DataForSeo::autocomplete(context, seed, market),
            DiscoverySource::SerpapiAutocomplete => SerpApi::autocomplete(context, seed, market),
            DiscoverySource::GoogleSuggest => FreeSuggest::google(context, seed, market),
            DiscoverySource::YandexSuggest => FreeSuggest::yandex(context, seed, market),
            // Sent through the signed-in account instead (see `discover_with`).
            DiscoverySource::GoogleAds => Err(SourceError::Config(
                "Google Ads needs the signed-in account".to_owned(),
            )),
        }
    }
}

/// The market of a project.
pub(crate) fn market_of(project: &ProjectRow) -> Market {
    Market::new(&project.spec.country, &project.spec.language)
}

/// Stores the keywords one request found; returns how many the filter kept and the
/// addition.
#[allow(clippy::too_many_arguments)]
fn keep(
    store: &mut KwStore,
    project: &ProjectRow,
    language: Language,
    filter: &Filter,
    call: &Call,
    found: Vec<FoundKeyword>,
) -> Result<(u64, Added), KwAppError> {
    let kept: Vec<FoundKeyword> = found
        .into_iter()
        .filter(|keyword| filter.keeps(&keyword.keyword, keyword.volume))
        .collect();
    let (keywords, _) = prepare(
        kept.iter()
            .map(|keyword| (keyword.keyword.clone(), Some(call.seed.clone()))),
        language,
        call.source.id(),
    );
    let found_at = now();
    let added = store.add_keywords(project.id, &keywords, &found_at)?;
    let numbered = kept
        .iter()
        .any(|keyword| keyword.has_numbers() || keyword.intent.is_some());
    if let (true, Some((source, engine))) = (numbered, call.source.numbers()) {
        let index = store.key_index(project.id)?;
        let mut metrics = Vec::new();
        let mut intents = Vec::new();
        for keyword in &kept {
            let Some(id) =
                normalize(&keyword.keyword, language).and_then(|key| index.get(&key.key).copied())
            else {
                continue;
            };
            if keyword.has_numbers() {
                metrics.push(metric(
                    id,
                    source,
                    engine,
                    &project.spec.country,
                    keyword,
                    &found_at,
                ));
            }
            if let Some(primary) = &keyword.intent {
                intents.push(IntentRecord {
                    keyword_id: id,
                    primary: primary.clone(),
                    secondary: keyword.secondary_intent.clone(),
                    source: DataForSeo::ID.to_owned(),
                });
            }
        }
        store.put_metrics(&metrics)?;
        store.put_intents(&intents)?;
    }
    fill_intents(store, project.id, &project.spec.language)?;
    Ok((u64::try_from(kept.len()).unwrap_or(u64::MAX), added))
}

/// A stored number record of a found keyword.
pub(crate) fn metric(
    keyword_id: i64,
    source: &str,
    engine: &str,
    country: &str,
    keyword: &FoundKeyword,
    fetched_at: &str,
) -> MetricRecord {
    MetricRecord {
        keyword_id,
        source: source.to_owned(),
        engine: engine.to_owned(),
        country: country.to_owned(),
        volume: keyword.volume,
        volume_low: keyword.volume_low,
        volume_high: keyword.volume_high,
        cpc_usd: keyword.cpc_usd,
        competition: keyword.competition,
        difficulty: keyword.difficulty,
        monthly: keyword
            .monthly
            .iter()
            .map(|month| MonthlyVolume {
                month: month.month.clone(),
                volume: month.volume,
            })
            .collect(),
        fetched_at: fetched_at.to_owned(),
    }
}

impl KeywordWorkspace {
    /// The requests and cost of a discovery per source, and whether each can run.
    ///
    /// # Errors
    /// [`KwAppError::Invalid`] for too many seeds; settings and store errors.
    pub fn estimate_discovery(
        &self,
        project_id: i64,
        request: &DiscoverRequest,
    ) -> Result<DiscoverEstimate, KwAppError> {
        let settings = self.settings()?;
        self.warm_catalog(&settings)?;
        let project = self.project(project_id)?;
        let calls = plan(
            request,
            &project,
            Language::from_code(&project.spec.language),
        )?;
        let mut estimate = DiscoverEstimate {
            sources: Vec::new(),
            requests: 0,
            cost_usd: 0.0,
            wordstat_hours: 0.0,
        };
        let mut planned: BTreeSet<DiscoverySource> = request.sources.iter().copied().collect();
        for source in DiscoverySource::ALL {
            if !planned.remove(&source) {
                continue;
            }
            let own: Vec<&Call> = calls.iter().filter(|call| call.source == source).collect();
            let requests = u32::try_from(own.len()).unwrap_or(u32::MAX);
            let cost_usd: f64 = own.iter().map(|call| call.cost(request.limit)).sum::<f64>()
                * self.price_factor(&settings, source.provider());
            let mut reason = usable(self, &settings, source.provider()).err();
            if requests == 0 {
                reason = Some(if source == DiscoverySource::Competitors {
                    "no competitor domains".to_owned()
                } else {
                    "no seeds".to_owned()
                });
            }
            if reason.is_none()
                && let Some(budget) = settings.budgets.get(source.provider())
            {
                let spent = self.store()?.spent(source.provider(), &month())?;
                if spent + cost_usd > *budget {
                    reason = Some(format!(
                        "passes the monthly budget of {budget} USD (spent {spent:.2})"
                    ));
                }
            }
            let ready = reason.is_none();
            if ready {
                estimate.requests += requests;
                estimate.cost_usd += cost_usd;
                if source == DiscoverySource::Wordstat {
                    estimate.wordstat_hours =
                        f64::from(requests) / f64::from(settings.wordstat_hourly_quota);
                }
            }
            estimate.sources.push(SourceEstimate {
                source,
                provider: source.provider().to_owned(),
                requests,
                cost_usd,
                ready,
                reason,
            });
        }
        Ok(estimate)
    }

    /// Discovers keywords with the user's network settings.
    ///
    /// # Errors
    /// See [`KeywordWorkspace::discover_with`].
    pub fn discover(
        &self,
        project_id: i64,
        request: &DiscoverRequest,
        control: &RunControl<'_>,
    ) -> Result<DiscoverOutcome, KwAppError> {
        let transport = self.transport(&self.settings()?)?;
        self.discover_with(project_id, request, &transport, control)
    }

    /// Discovers keywords: asks every chosen source, keeps what passes the filter, and adds
    /// it to the project. A source that cannot be used is reported and skipped.
    ///
    /// # Errors
    /// [`KwAppError::Invalid`] for an empty request or an invalid filter; store errors.
    pub fn discover_with(
        &self,
        project_id: i64,
        request: &DiscoverRequest,
        transport: &dyn HttpTransport,
        control: &RunControl<'_>,
    ) -> Result<DiscoverOutcome, KwAppError> {
        let project = self.project(project_id)?;
        let language = Language::from_code(&project.spec.language);
        let filter =
            Filter::new(&request.filter).map_err(|error| KwAppError::Invalid(error.to_string()))?;
        let calls = plan(request, &project, language)?;
        if calls.is_empty() {
            return Err(KwAppError::Invalid(
                "give seeds (or competitors) and choose at least one source".to_owned(),
            ));
        }
        let market = market_of(&project);
        let limit = request.limit.clamp(1, Wordstat::MAX_PHRASES);
        let ads_token = OnceLock::new();
        let suggest_pause = Duration::from_millis(
            self.settings()?
                .suggest
                .pause_ms
                .max(crate::settings::SUGGEST_MIN_PAUSE_MS),
        );
        let mut steps: Vec<Step<'_>> = Vec::with_capacity(calls.len());
        for call in &calls {
            let market = market.clone();
            let (send, pause): (Sender<'_>, _) = if call.source == DiscoverySource::GoogleAds {
                let Input::Seeds(seeds) = &call.input else {
                    continue;
                };
                (
                    self.ads_sender(&ads_token, move |context, account| {
                        GoogleAds::keyword_ideas(context, account, seeds, &market)
                    })?,
                    Some(Duration::from_secs(1)),
                )
            } else {
                let free = matches!(
                    call.source,
                    DiscoverySource::GoogleSuggest | DiscoverySource::YandexSuggest
                );
                (
                    Box::new(move |context: &SourceContext<'_>| call.send(context, &market, limit)),
                    free.then_some(suggest_pause),
                )
            };
            steps.push(Step {
                provider: call.source.provider(),
                label: call.source.id(),
                cost_usd: call.cost(limit),
                pause,
                send,
            });
        }
        let mut kept = 0;
        let mut added = Added::default();
        let run = self.run_steps(
            project_id,
            RunKind::Discover,
            &steps,
            transport,
            control,
            |index, fetched| {
                let found = u64::try_from(fetched.keywords.len()).unwrap_or(u64::MAX);
                let mut store = self.store()?;
                let (count, addition) = keep(
                    &mut store,
                    &project,
                    language,
                    &filter,
                    &calls[index],
                    fetched.keywords,
                )?;
                kept += count;
                added.received += addition.received;
                added.added += addition.added;
                added.known += addition.known;
                Ok(found)
            },
        )?;
        Ok(DiscoverOutcome { run, kept, added })
    }
}

#[cfg(test)]
#[path = "discover_tests.rs"]
mod tests;
