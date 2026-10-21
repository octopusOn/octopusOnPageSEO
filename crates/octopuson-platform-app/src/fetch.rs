//! Paid requests: kept answers first, an estimate before paying, the budget, then the request
//! with its cost recorded.
//!
//! Every module asks providers through [`Workspace::fetch`]: the answer comes from the cache
//! while it is fresh, otherwise the source is asked on the user's own key or on credits and the
//! answer is kept. Pages first show what is kept ([`Workspace::kept`]) and the estimate
//! ([`Workspace::estimate`]), and pay only when the user asks for fresh data. Questions that are
//! always asked fresh, like rank checks, run through a [`PaidRun`]: paid and recorded the same
//! way, their readings kept by the module instead of the answers.

use octopuson_cloud::Via;
use octopuson_core::http::HttpTransport;
use octopuson_platform::{CacheClass, CacheEntry, SpendRecord, cache_key, unix_now};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::router::ErrorCode;
use crate::sources::provider;
use crate::{AppError, Workspace, month, now};

/// One paid question.
#[derive(Debug, Clone, PartialEq)]
pub struct Paid<'a> {
    /// Module asking (`overview`).
    pub module: &'a str,
    /// Source id (`dataforseo`).
    pub provider: &'a str,
    /// Provider endpoint.
    pub endpoint: &'a str,
    /// How fast the data changes.
    pub class: CacheClass,
    /// The project it is for.
    pub project_id: Option<i64>,
    /// The request, as the provider gets it.
    pub task: Value,
    /// Provider list price, US dollars.
    pub estimate_usd: f64,
}

impl Paid<'_> {
    fn key(&self) -> String {
        cache_key(self.endpoint, &self.task)
    }
}

/// What a source answered.
#[derive(Debug, Clone, PartialEq)]
pub struct Reply {
    /// The provider's result list.
    pub result: Vec<Value>,
    /// What the provider says it cost, US dollars.
    pub cost_usd: f64,
    /// What the gateway charged in credits, US dollars.
    pub charged_usd: Option<f64>,
}

impl From<octopuson_dataforseo::Answer> for Reply {
    fn from(answer: octopuson_dataforseo::Answer) -> Self {
        Self {
            result: answer.result,
            cost_usd: answer.cost_usd,
            charged_usd: answer.charged_usd,
        }
    }
}

/// How a request reaches the source: the transport and the key the client gets.
pub struct Route<'a> {
    /// Sends provider requests (through the gateway on credits).
    pub transport: &'a dyn HttpTransport,
    /// How it is paid.
    pub via: Via,
    credential: &'a (dyn Fn(&str) -> Option<String> + Sync),
}

impl Route<'_> {
    /// The value of a key of the source: the user's own, or the placeholder on credits.
    ///
    /// # Errors
    /// [`AppError::NoSource`] when the key is not available.
    pub fn key(&self, name: &str) -> Result<String, AppError> {
        (self.credential)(name)
            .ok_or_else(|| AppError::NoSource(format!("the key `{name}` is not entered")))
    }
}

/// What a kept answer is.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Kept {
    /// RFC 3339.
    pub fetched_at: String,
    /// Still fresh.
    pub fresh: bool,
}

/// Why a question cannot be asked now.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Blocked {
    /// [`ErrorCode::NoSource`] or [`ErrorCode::Budget`], for pages to explain in their language.
    pub code: ErrorCode,
    /// For people.
    pub message: String,
}

/// What a question would cost now.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Estimate {
    /// Provider list price, US dollars.
    pub usd: f64,
    /// Credits, when the source runs on credits.
    pub credits: Option<f64>,
    /// How it would be paid; `None` when the source cannot be used.
    pub via: Option<Via>,
    /// The kept answer, if any.
    pub kept: Option<Kept>,
    /// Why it cannot be asked now (no key, no credits, over budget).
    pub blocked: Option<Blocked>,
}

/// Data with where it came from.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Fetched<T> {
    /// The data.
    pub data: T,
    /// RFC 3339.
    pub fetched_at: String,
    /// Still fresh.
    pub fresh: bool,
    /// Taken from the kept answers, nothing paid now.
    pub cached: bool,
    /// What was paid now, US dollars (0 from the cache).
    pub spent_usd: f64,
    /// How it was paid when fetched: `own` or `credit`.
    pub via: String,
}

const fn via_name(via: Via) -> &'static str {
    match via {
        Via::Own => "own",
        Via::Credit => "credit",
    }
}

impl Workspace {
    /// Why a source cannot pay `usd` more this month, if it cannot.
    fn over_budget(&self, provider_id: &str, usd: f64) -> Result<Option<String>, AppError> {
        let settings = self.settings()?;
        let Some(budget) = settings.budgets.get(provider_id).copied() else {
            return Ok(None);
        };
        let spent = self.store()?.spent_in(provider_id, &month())?;
        Ok((spent + usd > budget + 1e-9).then(|| {
            format!(
                "the monthly budget of {} (${budget:.2}) would be passed: ${spent:.2} spent",
                provider(provider_id).map_or(provider_id, |definition| definition.name)
            )
        }))
    }

    /// The kept answer of a question, fresh or stale, read with `parse`.
    ///
    /// # Errors
    /// Storage errors.
    pub fn kept<T>(
        &self,
        paid: &Paid<'_>,
        parse: impl FnOnce(&[Value]) -> T,
    ) -> Result<Option<Fetched<T>>, AppError> {
        let Some(entry) = self.store()?.cached(&paid.key())? else {
            return Ok(None);
        };
        Ok(Some(from_entry(&entry, parse)))
    }

    /// What a question would cost now, and whether it can be asked.
    ///
    /// # Errors
    /// Settings or storage errors.
    pub fn estimate(&self, paid: &Paid<'_>) -> Result<Estimate, AppError> {
        let mut estimate = self.estimate_spend(paid.provider, paid.estimate_usd)?;
        estimate.kept = self.store()?.cached(&paid.key())?.map(|entry| Kept {
            fresh: entry.is_fresh(unix_now()),
            fetched_at: entry.fetched_at,
        });
        Ok(estimate)
    }

    /// What spending `usd` of list price on a source would cost now, and whether it can be
    /// spent; nothing is kept for it.
    ///
    /// # Errors
    /// Settings or storage errors.
    pub fn estimate_spend(&self, provider_id: &str, usd: f64) -> Result<Estimate, AppError> {
        let settings = self.settings()?;
        self.warm_catalog(&settings)?;
        let via = self.via(&settings, provider_id);
        let credits = match via {
            Some(Via::Credit) => self.credits_per_usd(provider_id).map(|rate| usd * rate),
            _ => None,
        };
        let blocked = match via {
            None => Some(Blocked {
                code: ErrorCode::NoSource,
                message: no_source(provider_id),
            }),
            Some(_) => self.over_budget(provider_id, usd)?.map(|message| Blocked {
                code: ErrorCode::Budget,
                message,
            }),
        };
        Ok(Estimate {
            usd,
            credits,
            via,
            kept: None,
            blocked,
        })
    }

    /// The answer of a question: kept while fresh (unless `force`), else asked with `ask` and
    /// kept, its cost recorded.
    ///
    /// # Errors
    /// [`AppError::NoSource`] without a key or credits, [`AppError::Budget`] over the monthly
    /// budget, or what `ask` returns.
    pub fn fetch<T>(
        &self,
        paid: &Paid<'_>,
        force: bool,
        ask: impl FnOnce(&Route<'_>) -> Result<Reply, AppError>,
        parse: impl FnOnce(&[Value]) -> T,
    ) -> Result<Fetched<T>, AppError> {
        let key = paid.key();
        if !force
            && let Some(entry) = self.store()?.cached(&key)?
            && entry.is_fresh(unix_now())
        {
            return Ok(from_entry(&entry, parse));
        }
        let settings = self.settings()?;
        self.warm_catalog(&settings)?;
        let via = self
            .via(&settings, paid.provider)
            .ok_or_else(|| AppError::NoSource(no_source(paid.provider)))?;
        if let Some(reason) = self.over_budget(paid.provider, paid.estimate_usd)? {
            return Err(AppError::Budget(reason));
        }
        let transport = self.transport(&settings)?;
        let credential = |name: &str| self.credential(via, paid.provider, name);
        let route = Route {
            transport: transport.as_ref(),
            via,
            credential: &credential,
        };
        // The database is not held while the provider answers.
        let reply = ask(&route)?;
        let spent = reply.charged_usd.unwrap_or(reply.cost_usd);
        let fetched_at = now();
        let entry = CacheEntry {
            body: Value::Array(reply.result),
            fetched_at: fetched_at.clone(),
            expires_at: unix_now() + paid.class.ttl_seconds(),
            cost_usd: spent,
            via: via_name(via).to_owned(),
        };
        let store = self.store()?;
        store.record_spend(&spend_of(paid, via, spent, fetched_at))?;
        store.keep(&key, paid.endpoint, paid.class, paid.project_id, &entry)?;
        let mut fetched = from_entry(&entry, parse);
        fetched.cached = false;
        fetched.spent_usd = spent;
        Ok(fetched)
    }
}

impl Workspace {
    /// What a question made of several paid parts would cost now, all asked of one source.
    ///
    /// # Errors
    /// [`AppError::Invalid`] without parts; settings or storage errors.
    pub fn estimate_many(&self, parts: &[Paid<'_>]) -> Result<Estimate, AppError> {
        let first = parts
            .first()
            .ok_or_else(|| AppError::Invalid("nothing to ask".to_owned()))?;
        let mut estimate = self.estimate(first)?;
        let usd: f64 = parts.iter().map(|part| part.estimate_usd).sum();
        estimate.usd = usd;
        estimate.credits = match estimate.via {
            Some(Via::Credit) => self.credits_per_usd(first.provider).map(|rate| usd * rate),
            _ => None,
        };
        let store = self.store()?;
        let mut kept: Option<Kept> = estimate.kept.clone();
        for part in &parts[1..] {
            kept = match (kept, store.cached(&part.key())?) {
                (Some(known), Some(entry)) => Some(Kept {
                    fresh: known.fresh && entry.is_fresh(unix_now()),
                    fetched_at: known.fetched_at.min(entry.fetched_at),
                }),
                _ => None,
            };
        }
        drop(store);
        estimate.kept = kept;
        if estimate.via.is_some() {
            estimate.blocked = self
                .over_budget(first.provider, usd)?
                .map(|message| Blocked {
                    code: ErrorCode::Budget,
                    message,
                });
        }
        Ok(estimate)
    }

    /// The kept answers of every part, when each is kept (fresh or stale).
    ///
    /// # Errors
    /// Storage errors.
    pub fn kept_many(
        &self,
        parts: &[Paid<'_>],
    ) -> Result<Option<Fetched<Vec<Vec<Value>>>>, AppError> {
        let mut answers: Vec<Fetched<Vec<Value>>> = Vec::with_capacity(parts.len());
        for part in parts {
            match self.kept(part, <[Value]>::to_vec)? {
                Some(answer) => answers.push(answer),
                None => return Ok(None),
            }
        }
        Ok(Some(combine(answers)))
    }

    /// The answers of every part, each kept while fresh (unless `force`) or asked with `ask`.
    ///
    /// # Errors
    /// The first part's failure; parts asked before it stay kept and paid.
    pub fn fetch_many(
        &self,
        parts: &[Paid<'_>],
        force: bool,
        ask: impl Fn(&Route<'_>, &Paid<'_>) -> Result<Reply, AppError>,
    ) -> Result<Fetched<Vec<Vec<Value>>>, AppError> {
        if parts.is_empty() {
            return Err(AppError::Invalid("nothing to ask".to_owned()));
        }
        let total: f64 = parts.iter().map(|part| part.estimate_usd).sum();
        if let Some(reason) = self.over_budget(parts[0].provider, total)? {
            return Err(AppError::Budget(reason));
        }
        let mut answers: Vec<Fetched<Vec<Value>>> = Vec::with_capacity(parts.len());
        for part in parts {
            answers.push(self.fetch(part, force, |route| ask(route, part), <[Value]>::to_vec)?);
        }
        Ok(combine(answers))
    }
}

/// A fresh answer, paid now and not kept (its reading is).
#[derive(Debug, Clone, PartialEq)]
pub struct Answered {
    /// The provider's result list.
    pub result: Vec<Value>,
    /// What was paid, US dollars.
    pub spent_usd: f64,
    /// RFC 3339.
    pub fetched_at: String,
}

/// Many fresh questions to one source in a row (rank checks): the source and the transport
/// are settled once; each question is paid within the month's budget and recorded.
pub struct PaidRun<'w> {
    workspace: &'w Workspace,
    via: Via,
    transport: Box<dyn HttpTransport>,
}

impl std::fmt::Debug for PaidRun<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PaidRun")
            .field("via", &self.via)
            .finish_non_exhaustive()
    }
}

impl Workspace {
    /// Starts a run of fresh questions to `provider_id`.
    ///
    /// # Errors
    /// [`AppError::NoSource`] without a key or credits; settings errors.
    pub fn paid_run(&self, provider_id: &str) -> Result<PaidRun<'_>, AppError> {
        let settings = self.settings()?;
        self.warm_catalog(&settings)?;
        let via = self
            .via(&settings, provider_id)
            .ok_or_else(|| AppError::NoSource(no_source(provider_id)))?;
        Ok(PaidRun {
            workspace: self,
            via,
            transport: self.transport(&settings)?,
        })
    }
}

impl PaidRun<'_> {
    /// How the run is paid.
    pub const fn via(&self) -> Via {
        self.via
    }

    /// Asks one question fresh with `ask`, within the month's budget, and records what it cost.
    ///
    /// # Errors
    /// [`AppError::Budget`] over the monthly budget, or what `ask` returns.
    pub fn ask(
        &self,
        paid: &Paid<'_>,
        ask: impl FnOnce(&Route<'_>) -> Result<Reply, AppError>,
    ) -> Result<Answered, AppError> {
        let workspace = self.workspace;
        if let Some(reason) = workspace.over_budget(paid.provider, paid.estimate_usd)? {
            return Err(AppError::Budget(reason));
        }
        let credential = |name: &str| workspace.credential(self.via, paid.provider, name);
        let route = Route {
            transport: self.transport.as_ref(),
            via: self.via,
            credential: &credential,
        };
        let reply = ask(&route)?;
        let spent = reply.charged_usd.unwrap_or(reply.cost_usd);
        let fetched_at = now();
        workspace
            .store()?
            .record_spend(&spend_of(paid, self.via, spent, fetched_at.clone()))?;
        Ok(Answered {
            result: reply.result,
            spent_usd: spent,
            fetched_at,
        })
    }
}

fn spend_of(paid: &Paid<'_>, via: Via, spent: f64, at: String) -> SpendRecord {
    SpendRecord {
        at,
        project_id: paid.project_id,
        module: paid.module.to_owned(),
        endpoint: paid.endpoint.to_owned(),
        provider: paid.provider.to_owned(),
        via: via_name(via).to_owned(),
        requests: 1,
        cost_usd: spent,
    }
}

/// Several answers as one: the oldest time, fresh only when all are, the sum spent.
fn combine(answers: Vec<Fetched<Vec<Value>>>) -> Fetched<Vec<Vec<Value>>> {
    let fetched_at = answers
        .iter()
        .map(|answer| answer.fetched_at.clone())
        .min()
        .unwrap_or_default();
    let fresh = answers.iter().all(|answer| answer.fresh);
    let cached = answers.iter().all(|answer| answer.cached);
    let spent_usd = answers.iter().map(|answer| answer.spent_usd).sum();
    let via = answers
        .iter()
        .find(|answer| !answer.cached)
        .or_else(|| answers.first())
        .map(|answer| answer.via.clone())
        .unwrap_or_default();
    Fetched {
        data: answers.into_iter().map(|answer| answer.data).collect(),
        fetched_at,
        fresh,
        cached,
        spent_usd,
        via,
    }
}

fn no_source(provider_id: &str) -> String {
    let name = provider(provider_id).map_or(provider_id, |definition| definition.name);
    format!("{name}: enter your own key or connect your OctopusOn SEO account to use credits")
}

fn from_entry<T>(entry: &CacheEntry, parse: impl FnOnce(&[Value]) -> T) -> Fetched<T> {
    let items = entry.body.as_array().map_or(&[][..], Vec::as_slice);
    Fetched {
        data: parse(items),
        fetched_at: entry.fetched_at.clone(),
        fresh: entry.is_fresh(unix_now()),
        cached: true,
        spent_usd: 0.0,
        via: entry.via.clone(),
    }
}
