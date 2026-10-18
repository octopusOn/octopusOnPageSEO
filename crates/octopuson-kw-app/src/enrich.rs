//! Enrichment: Google Ads volumes, difficulty and intent for keywords a project already has,
//! asked from DataForSEO in batches of a thousand.

use octopuson_core::http::HttpTransport;
use std::sync::OnceLock;
use std::time::Duration;

use octopuson_keyword_sources::dataforseo::{DataForSeo, prices};
use octopuson_keyword_sources::google_ads::GoogleAds;
use octopuson_keyword_sources::{Fetched, Market, SourceContext};
use octopuson_keywords::normalize;
use octopuson_kw_store::{IntentRecord, ProjectRow, RunKind};
use octopuson_text::Language;
use serde::{Deserialize, Serialize};

use crate::discover::{market_of, metric};
use crate::runner::{RunControl, RunSummary, Sender, Step, usable};
use crate::{KeywordWorkspace, KwAppError, now};

/// Source of difficulty-only numbers (kept apart so they never replace a volume).
pub const DIFFICULTY_SOURCE: &str = "dataforseo-kd";
/// Source of Google Ads numbers.
pub const VOLUME_SOURCE: &str = "dataforseo-ads";

/// A number asked for known keywords.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Enrichment {
    /// Google Ads volume, cost per click, competition and monthly searches.
    Volume,
    /// Keyword difficulty.
    Difficulty,
    /// Search intent.
    Intent,
}

impl Enrichment {
    /// Every number, in run order.
    pub const ALL: [Self; 3] = [Self::Volume, Self::Difficulty, Self::Intent];
}

/// Where volumes come from.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum VolumeSource {
    /// Google Ads data through DataForSEO (paid, 1,000 keywords per request).
    #[default]
    Dataforseo,
    /// The user's own Google Ads account (10,000 keywords per request).
    GoogleAds,
}

/// What to ask for the project's active keywords.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct EnrichRequest {
    /// Numbers to ask.
    pub asks: Vec<Enrichment>,
    /// Where volumes come from (difficulty and intent come from DataForSEO).
    pub volume_source: VolumeSource,
    /// Only keywords that lack the number.
    pub only_missing: bool,
}

impl Default for EnrichRequest {
    fn default() -> Self {
        Self {
            asks: vec![Enrichment::Volume],
            volume_source: VolumeSource::Dataforseo,
            only_missing: true,
        }
    }
}

/// Source of volumes from the user's Google Ads account.
pub const ADS_VOLUME_SOURCE: &str = "google-ads";

/// What an enrichment will cost.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EnrichEstimate {
    /// Keywords asked for volume, difficulty and intent.
    pub keywords: [u32; 3],
    /// Requests.
    pub requests: u32,
    /// Estimated US dollars.
    pub cost_usd: f64,
    /// Why it cannot run, if it cannot.
    pub reason: Option<String>,
}

/// Batches of keyword texts per kind.
fn batches(
    workspace: &KeywordWorkspace,
    project_id: i64,
    request: &EnrichRequest,
) -> Result<Vec<(Enrichment, Vec<String>)>, KwAppError> {
    let store = workspace.store()?;
    let inputs = store.cluster_inputs(project_id, "google", "desktop")?;
    let mut batches = Vec::new();
    for ask in Enrichment::ALL {
        if !request.asks.contains(&ask) {
            continue;
        }
        let texts: Vec<String> = inputs
            .iter()
            .filter(|input| {
                !request.only_missing
                    || match ask {
                        Enrichment::Volume => input.volume.is_none(),
                        Enrichment::Difficulty => input.difficulty.is_none(),
                        Enrichment::Intent => input.intent.is_none(),
                    }
            })
            .map(|input| input.keyword.clone())
            .collect();
        let size = if ask == Enrichment::Volume && request.volume_source == VolumeSource::GoogleAds
        {
            GoogleAds::BATCH
        } else {
            DataForSeo::BATCH
        };
        batches.extend(texts.chunks(size).map(|chunk| (ask, chunk.to_vec())));
    }
    Ok(batches)
}

fn cost(ask: Enrichment, keywords: usize, source: VolumeSource) -> f64 {
    match ask {
        Enrichment::Volume if source == VolumeSource::GoogleAds => 0.0,
        Enrichment::Volume => prices::ADS_TASK,
        Enrichment::Difficulty | Enrichment::Intent => {
            DataForSeo::labs_estimate(u32::try_from(keywords).unwrap_or(u32::MAX))
        }
    }
}

const fn provider_of(ask: Enrichment, source: VolumeSource) -> &'static str {
    match (ask, source) {
        (Enrichment::Volume, VolumeSource::GoogleAds) => GoogleAds::ID,
        _ => DataForSeo::ID,
    }
}

impl KeywordWorkspace {
    /// The keywords, requests and cost of an enrichment.
    ///
    /// # Errors
    /// Settings and store errors.
    pub fn estimate_enrichment(
        &self,
        project_id: i64,
        request: &EnrichRequest,
    ) -> Result<EnrichEstimate, KwAppError> {
        let settings = self.settings()?;
        self.warm_catalog(&settings)?;
        let batches = batches(self, project_id, request)?;
        let mut keywords = [0_u32; 3];
        for (ask, texts) in &batches {
            let slot = match ask {
                Enrichment::Volume => 0,
                Enrichment::Difficulty => 1,
                Enrichment::Intent => 2,
            };
            keywords[slot] += u32::try_from(texts.len()).unwrap_or(u32::MAX);
        }
        Ok(EnrichEstimate {
            keywords,
            requests: u32::try_from(batches.len()).unwrap_or(u32::MAX),
            cost_usd: batches
                .iter()
                .map(|(ask, texts)| {
                    cost(*ask, texts.len(), request.volume_source)
                        * self.price_factor(&settings, provider_of(*ask, request.volume_source))
                })
                .sum(),
            reason: batches
                .iter()
                .map(|(ask, _)| provider_of(*ask, request.volume_source))
                .find_map(|provider| usable(self, &settings, provider).err()),
        })
    }

    /// Enriches with the user's network settings.
    ///
    /// # Errors
    /// See [`KeywordWorkspace::enrich_with`].
    pub fn enrich(
        &self,
        project_id: i64,
        request: &EnrichRequest,
        control: &RunControl<'_>,
    ) -> Result<RunSummary, KwAppError> {
        let transport = self.transport(&self.settings()?)?;
        self.enrich_with(project_id, request, &transport, control)
    }

    /// Asks DataForSEO for the chosen numbers of the project's active keywords and stores
    /// them: volumes as `dataforseo-ads`, difficulty as `dataforseo-kd`, intents replacing
    /// those read from the words.
    ///
    /// # Errors
    /// [`KwAppError::Invalid`] when nothing is to be asked; store errors.
    pub fn enrich_with(
        &self,
        project_id: i64,
        request: &EnrichRequest,
        transport: &dyn HttpTransport,
        control: &RunControl<'_>,
    ) -> Result<RunSummary, KwAppError> {
        let project = self.project(project_id)?;
        let language = Language::from_code(&project.spec.language);
        let market: Market = market_of(&project);
        let batches = batches(self, project_id, request)?;
        if batches.is_empty() {
            return Err(KwAppError::Invalid(
                "no keyword lacks the chosen numbers".to_owned(),
            ));
        }
        let ads_token = OnceLock::new();
        let source = request.volume_source;
        let mut steps: Vec<Step<'_>> = Vec::with_capacity(batches.len());
        for (ask, texts) in &batches {
            let market = market.clone();
            let ask = *ask;
            let ads = ask == Enrichment::Volume && source == VolumeSource::GoogleAds;
            let send: Sender<'_> = if ads {
                self.ads_sender(&ads_token, move |context, account| {
                    GoogleAds::historical_metrics(context, account, texts, &market)
                })?
            } else {
                Box::new(move |context: &SourceContext<'_>| match ask {
                    Enrichment::Volume => DataForSeo::search_volume(context, texts, &market),
                    Enrichment::Difficulty => {
                        DataForSeo::keyword_difficulty(context, texts, &market)
                    }
                    Enrichment::Intent => DataForSeo::search_intent(context, texts, &market),
                })
            };
            steps.push(Step {
                provider: provider_of(ask, source),
                label: match ask {
                    Enrichment::Volume if ads => ADS_VOLUME_SOURCE,
                    Enrichment::Volume => VOLUME_SOURCE,
                    Enrichment::Difficulty => DIFFICULTY_SOURCE,
                    Enrichment::Intent => "dataforseo-intent",
                },
                cost_usd: cost(ask, texts.len(), source),
                pause: ads.then_some(Duration::from_secs(1)),
                send,
            });
        }
        self.run_steps(
            project_id,
            RunKind::Enrich,
            &steps,
            transport,
            control,
            |index, fetched: Fetched| {
                let (ask, _) = &batches[index];
                store_numbers(self, &project, language, *ask, source, &fetched)
            },
        )
    }
}

/// Stores the numbers of one enrichment request; returns how many records it wrote.
fn store_numbers(
    workspace: &KeywordWorkspace,
    project: &ProjectRow,
    language: Language,
    ask: Enrichment,
    source: VolumeSource,
    fetched: &Fetched,
) -> Result<u64, KwAppError> {
    let mut store = workspace.store()?;
    let ids = store.key_index(project.id)?;
    let fetched_at = now();
    let volume_source = if source == VolumeSource::GoogleAds {
        ADS_VOLUME_SOURCE
    } else {
        VOLUME_SOURCE
    };
    let mut metrics = Vec::new();
    let mut intents = Vec::new();
    for keyword in &fetched.keywords {
        let Some(id) = normalize(&keyword.keyword, language)
            .and_then(|normalized| ids.get(&normalized.key).copied())
        else {
            continue;
        };
        let record = |source| {
            metric(
                id,
                source,
                "google",
                &project.spec.country,
                keyword,
                &fetched_at,
            )
        };
        match ask {
            Enrichment::Volume if keyword.has_numbers() => metrics.push(record(volume_source)),
            Enrichment::Difficulty if keyword.difficulty.is_some() => {
                metrics.push(record(DIFFICULTY_SOURCE));
            }
            Enrichment::Intent => {
                if let Some(primary) = &keyword.intent {
                    intents.push(IntentRecord {
                        keyword_id: id,
                        primary: primary.clone(),
                        secondary: keyword.secondary_intent.clone(),
                        source: DataForSeo::ID.to_owned(),
                    });
                }
            }
            _ => {}
        }
    }
    store.put_metrics(&metrics)?;
    store.put_intents(&intents)?;
    Ok(u64::try_from(metrics.len() + intents.len()).unwrap_or(u64::MAX))
}
