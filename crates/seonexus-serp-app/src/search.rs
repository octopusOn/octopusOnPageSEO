//! Searching through the provider chain.

use std::collections::HashMap;
use std::path::PathBuf;

use seonexus_cloud::{SourceMode, Via};
use seonexus_core::Timestamp;
use seonexus_core::http::HttpTransport;
use seonexus_serp::provider::{
    Attempt, AttemptOutcome, CostEstimate, ProviderError, RawPage, SearchContext, SearchEvent,
    SerpProvider, search_chain,
};
use seonexus_serp::{Rank, SerpQuery, SerpSnapshot, Target, find_rank};
use seonexus_serp_store::{RunKind, RunStatus};
use serde::{Deserialize, Serialize};

use crate::browser::SharedBrowser;
use crate::files::write_atomically;
use crate::providers::{self, BROWSER, Entry, ProviderKind, load_custom};
use crate::secrets::SecretState;
use crate::settings::Settings;
use crate::{SerpAppError, SerpWorkspace};

/// One search.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SearchRequest {
    /// The query.
    pub query: SerpQuery,
    /// Providers to try in order, instead of the engine's chain.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub providers: Option<Vec<String>>,
    /// A site whose rank to report.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<Target>,
}

/// What a search returned.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SearchOutcome {
    /// Id of the stored snapshot.
    pub snapshot_id: i64,
    /// The results.
    pub snapshot: SerpSnapshot,
    /// Every provider attempt, including skipped ones.
    pub attempts: Vec<Attempt>,
    /// US dollars spent.
    pub cost_usd: f64,
    /// Requests sent.
    pub requests: u32,
    /// Rank of the target, if one was given and it ranks.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rank: Option<Rank>,
}

/// What a provider of the chain would cost for a query.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProviderEstimate {
    /// Provider id.
    pub provider: String,
    /// Whether it can be used now.
    pub ready: bool,
    /// Why not, if it cannot.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// Expected requests and price.
    pub estimate: CostEstimate,
}

pub(crate) fn now() -> String {
    let micros = Timestamp::now().as_unix_micros();
    Timestamp::from_unix_micros(micros - micros.rem_euclid(1_000_000)).to_string()
}

impl SerpWorkspace {
    /// Expected cost of a query on each provider of its chain, and which can be used.
    ///
    /// # Errors
    /// [`SerpAppError`] for unreadable settings, provider files or store.
    pub fn estimate(&self, request: &SearchRequest) -> Result<Vec<ProviderEstimate>, SerpAppError> {
        let settings = self.settings()?;
        self.warm_catalog(&settings)?;
        let ids = chain_ids(request, &settings);
        let entries = self.entries(&settings, &ids)?;
        let month = &now()[..7];
        let store = self.store()?;
        ids.into_iter()
            .map(|id| {
                let entry = entries.iter().find(|entry| entry.provider.id() == id);
                // On OctopusOn SEO credits the price is the provider's times the multiplier.
                let estimate = entry
                    .map(|entry| {
                        let mut estimate = entry.provider.estimate(&request.query);
                        estimate.usd *= self.price_factor(&settings, entry);
                        estimate
                    })
                    .unwrap_or_default();
                let reason = match entry {
                    None => Some(format!("unknown provider `{id}`")),
                    Some(entry) => self.unavailable(
                        entry,
                        &request.query,
                        &settings,
                        &store,
                        month,
                        estimate.usd,
                    )?,
                };
                Ok(ProviderEstimate {
                    provider: id,
                    ready: reason.is_none(),
                    reason,
                    estimate,
                })
            })
            .collect()
    }

    /// Providers of the catalog, with the browser added when the chain uses it.
    fn entries(&self, settings: &Settings, ids: &[String]) -> Result<Vec<Entry>, SerpAppError> {
        let (mut entries, _) =
            providers::catalog(load_custom(&self.data_dir().join("providers.json"))?);
        if ids.iter().any(|id| id == BROWSER) {
            entries.push(Entry {
                provider: Box::new(SharedBrowser(self.browser_provider(settings)?)),
                kind: ProviderKind::Browser,
                secrets: Vec::new(),
                origins: Vec::new(),
                per_request_usd: 0.0,
                notes: None,
            });
        }
        Ok(entries)
    }

    /// Why a provider cannot run a query now, if it cannot.
    fn unavailable(
        &self,
        entry: &Entry,
        query: &SerpQuery,
        settings: &Settings,
        store: &seonexus_serp_store::SerpStore,
        month: &str,
        estimate_usd: f64,
    ) -> Result<Option<String>, SerpAppError> {
        let id = entry.provider.id();
        if settings.disabled.contains(id) {
            return Ok(Some("switched off".to_owned()));
        }
        if !entry.provider.capabilities().supports(query) {
            return Ok(Some(format!(
                "does not return {} {} results",
                query.device.as_str(),
                query.engine
            )));
        }
        if id == BROWSER {
            if seonexus_render::find_browser(settings.browser.executable.as_deref()).is_none() {
                return Ok(Some("Chrome or Edge was not found".to_owned()));
            }
            let today = &now()[..10];
            let used = store.requests_on(BROWSER, query.engine.as_str(), today)?;
            if used + u64::from(query.pages()) > u64::from(settings.browser.daily_cap) {
                return Ok(Some(format!(
                    "daily limit of {} {} pages reached in browser mode",
                    settings.browser.daily_cap, query.engine
                )));
            }
        }
        let origins = entry.origin_refs();
        let mut missing = None;
        for secret in &entry.secrets {
            let reason = match self.secrets().state(id, &secret.name, &origins)? {
                SecretState::Saved | SecretState::Environment => continue,
                SecretState::Unset => format!("secret `{}` is not set", secret.name),
                SecretState::Rebind => format!(
                    "secret `{}` was saved for another address; enter it again",
                    secret.name
                ),
            };
            missing.get_or_insert(reason);
        }
        // Without the own key a source can run on OctopusOn SEO credits.
        if self.entry_via(settings, entry)?.is_none() {
            return Ok(Some(match (self.source_mode(settings, id), missing) {
                (SourceMode::Credit, _) => {
                    "OctopusOn SEO credits are not available for it (sign in, or it is not offered)"
                        .to_owned()
                }
                (_, Some(reason)) => reason,
                (_, None) => "it cannot be used now".to_owned(),
            }));
        }
        if let Some(budget) = settings.budgets.get(id) {
            let spent = store.spent(id, month)?;
            if spent + estimate_usd > *budget {
                return Ok(Some(format!(
                    "monthly budget of {budget:.2} USD reached ({spent:.2} USD spent)"
                )));
            }
        }
        Ok(None)
    }

    /// Providers of a chain that can run a query now, and attempts for the others.
    fn usable<'a>(
        &self,
        entries: &'a [Entry],
        ids: Vec<String>,
        query: &SerpQuery,
        settings: &Settings,
        store: &seonexus_serp_store::SerpStore,
        month: &str,
    ) -> Result<(Vec<&'a Entry>, Vec<Attempt>), SerpAppError> {
        let mut skipped: Vec<Attempt> = Vec::new();
        let mut chain: Vec<&Entry> = Vec::new();
        for id in ids {
            let entry = entries.iter().find(|entry| entry.provider.id() == id);
            let reason = match entry {
                None => Some(format!("unknown provider `{id}`")),
                Some(entry) => {
                    let estimate = entry.provider.estimate(query).usd;
                    self.unavailable(entry, query, settings, store, month, estimate)?
                }
            };
            match (entry, reason) {
                (Some(entry), None) => chain.push(entry),
                (_, reason) => skipped.push(Attempt {
                    provider: id,
                    outcome: AttemptOutcome::Skipped,
                    message: reason,
                }),
            }
        }
        Ok((chain, skipped))
    }

    /// Runs a search through the provider chain and stores the snapshot.
    ///
    /// # Errors
    /// [`SerpAppError::Search`] with every attempt when no provider returned results (the
    /// attempts are stored too); [`SerpAppError`] for invalid queries and storage failures.
    pub fn search(
        &self,
        request: &SearchRequest,
        cancelled: &dyn Fn() -> bool,
        events: &dyn Fn(&SearchEvent),
    ) -> Result<SearchOutcome, SerpAppError> {
        let settings = self.settings()?;
        let transport = self.transport(&settings)?;
        self.search_with(request, &transport, cancelled, events)
    }

    /// [`Self::search`] through a given transport.
    ///
    /// # Errors
    /// As [`Self::search`].
    pub fn search_with(
        &self,
        request: &SearchRequest,
        transport: &dyn HttpTransport,
        cancelled: &dyn Fn() -> bool,
        events: &dyn Fn(&SearchEvent),
    ) -> Result<SearchOutcome, SerpAppError> {
        self.search_in(request, transport, cancelled, events, None)
    }

    /// Searches within an existing run (rank checks), or in a run of its own.
    pub(crate) fn search_in(
        &self,
        request: &SearchRequest,
        transport: &dyn HttpTransport,
        cancelled: &dyn Fn() -> bool,
        events: &dyn Fn(&SearchEvent),
        within: Option<(i64, i64)>,
    ) -> Result<SearchOutcome, SerpAppError> {
        request
            .query
            .validate()
            .map_err(|error| SerpAppError::InvalidQuery(error.to_string()))?;
        let settings = self.settings()?;
        let ids = chain_ids(request, &settings);
        let entries = self.entries(&settings, &ids)?;
        let started = now();
        let month = started[..7].to_owned();
        // The store is not held while providers are asked: a browser search can wait minutes
        // for the person, and the desktop app's views read the store meanwhile.
        let store = self.store()?;
        let (chain, skipped) =
            self.usable(&entries, ids, &request.query, &settings, &store, &month)?;
        let estimated: f64 = chain
            .first()
            .map_or(0.0, |entry| entry.provider.estimate(&request.query).usd);
        let (run, keyword_id) = match within {
            Some((run, keyword_id)) => (run, Some(keyword_id)),
            None => (
                store.begin_run(RunKind::Search, None, 1, estimated, &started)?,
                None,
            ),
        };
        drop(store);
        let origins: HashMap<&str, Vec<&str>> = chain
            .iter()
            .map(|entry| (entry.provider.id(), entry.origin_refs()))
            .collect();
        let on_credit = self.on_credit(&settings, &chain)?;
        // Sources on credits get a placeholder; the gateway adds OctopusOn SEO's key.
        let secrets = |provider: &str, name: &str| {
            if on_credit.contains(&provider) {
                return Some(Self::credit_secret(provider, name));
            }
            let origins = origins.get(provider).map(Vec::as_slice).unwrap_or_default();
            self.secrets().get(provider, name, origins).ok().flatten()
        };
        let context = SearchContext {
            transport,
            secrets: &secrets,
            fetched_at: started.clone(),
            cancelled,
            events,
        };
        let providers: Vec<&dyn SerpProvider> =
            chain.iter().map(|entry| entry.provider.as_ref()).collect();
        let result = search_chain(&providers, &request.query, &context);
        // On OctopusOn SEO credits the price is what the gateway charged.
        let charged = transport.take_charge_usd();
        match result {
            Ok((mut fetch, attempts)) => {
                fetch.reported_cost_usd = charged.or(fetch.reported_cost_usd);
                let mut all = skipped;
                all.extend(attempts);
                let mut store = self.store()?;
                let id = store.save_fetch(Some(run), keyword_id, &fetch, &all)?;
                store.set_attempt_via(id, via_of(&on_credit, &fetch.snapshot.provider))?;
                if within.is_none() {
                    store.finish_run(run, RunStatus::Done, &now(), None)?;
                }
                let rank = request
                    .target
                    .as_ref()
                    .and_then(|target| find_rank(&fetch.snapshot, target));
                Ok(SearchOutcome {
                    snapshot_id: id,
                    cost_usd: fetch.cost_usd(),
                    requests: fetch.requests,
                    snapshot: fetch.snapshot,
                    attempts: all,
                    rank,
                })
            }
            Err(failure) => {
                let mut all = skipped;
                all.extend(failure.attempts);
                let store = self.store()?;
                store.record_attempts(Some(run), &all, &started)?;
                let status = if matches!(failure.error, ProviderError::Cancelled) {
                    RunStatus::Cancelled
                } else {
                    RunStatus::Failed
                };
                if within.is_none() {
                    store.finish_run(run, status, &now(), Some(&failure.error.to_string()))?;
                }
                let diagnostic = match &failure.error {
                    ProviderError::Unreadable { page, .. } => {
                        Some(self.save_diagnostic(&started, request.query.engine, page)?)
                    }
                    _ => None,
                };
                Err(SerpAppError::Search {
                    error: failure.error,
                    attempts: all,
                    diagnostic,
                })
            }
        }
    }
}

impl SerpWorkspace {
    /// Ids of the chain's sources that run on OctopusOn SEO credits now.
    fn on_credit<'a>(
        &self,
        settings: &Settings,
        chain: &[&'a Entry],
    ) -> Result<Vec<&'a str>, SerpAppError> {
        let mut ids = Vec::new();
        for entry in chain {
            if self.entry_via(settings, entry)? == Some(Via::Credit) {
                ids.push(entry.provider.id());
            }
        }
        Ok(ids)
    }

    /// Saves a page no result could be read from, for fixing the parser selectors.
    fn save_diagnostic(
        &self,
        at: &str,
        engine: seonexus_serp::Engine,
        page: &RawPage,
    ) -> Result<PathBuf, SerpAppError> {
        let stamp: String = at.chars().filter(char::is_ascii_digit).take(14).collect();
        let path = self
            .data_dir()
            .join("diagnostics")
            .join(format!("{stamp}-{engine}-p{}.html", page.page));
        write_atomically(&path, &page.body)?;
        Ok(path)
    }
}

/// How a snapshot's provider was paid, as the store records it.
fn via_of(on_credit: &[&str], provider: &str) -> &'static str {
    if on_credit.contains(&provider) {
        "credit"
    } else {
        "own"
    }
}

fn chain_ids(request: &SearchRequest, settings: &Settings) -> Vec<String> {
    request
        .providers
        .clone()
        .unwrap_or_else(|| settings.chain(request.query.engine))
}
