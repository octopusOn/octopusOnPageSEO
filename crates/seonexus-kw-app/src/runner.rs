//! Runs a list of paid requests: one at a time, the store released while waiting for the
//! network, each provider's monthly budget and Wordstat's hourly quota kept, every attempt
//! recorded with its cost, a provider that refuses the key or is out of balance left out
//! for the rest of the run, and a rate limit waited out once.

use std::collections::BTreeSet;
use std::time::Duration;

use seonexus_core::http::HttpTransport;
use seonexus_keyword_sources::suggest::FreeSuggest;
use seonexus_keyword_sources::wordstat::Wordstat;
use seonexus_keyword_sources::{Fetched, SourceContext, SourceError};
use seonexus_kw_store::{AttemptOutcome, AttemptRecord, RunKind, RunStatus};
pub use seonexus_serp::challenge::ChallengeKind;
use serde::{Deserialize, Serialize};

use crate::settings::Settings;
use crate::sources::provider;
use crate::{KeywordWorkspace, KwAppError, ago, month, now};

/// Longest wait for a rate limit, seconds.
const MAX_RATE_WAIT: u64 = 300;

/// Sends one request.
pub(crate) type Sender<'a> = Box<dyn Fn(&SourceContext<'_>) -> Result<Fetched, SourceError> + 'a>;

/// What happens to a step before it is sent.
enum Gate {
    Go,
    Skip,
    Cancel,
}

/// One request of a run.
pub(crate) struct Step<'a> {
    /// Provider id.
    pub provider: &'static str,
    /// What it asks (source id), for progress.
    pub label: &'static str,
    /// Estimated price, US dollars.
    pub cost_usd: f64,
    /// Pause after it (Google Ads takes one request a second).
    pub pause: Option<Duration>,
    /// Sends it.
    pub send: Sender<'a>,
}

/// How a run is going.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Progress {
    /// The run.
    pub run_id: i64,
    /// Requests finished or skipped.
    pub done: u32,
    /// Requests planned.
    pub planned: u32,
    /// What the current request asks.
    pub step: String,
    /// Keywords found so far.
    pub found: u64,
    /// Waiting for the Wordstat quota until about this time (RFC 3339).
    pub waiting_until: Option<String>,
    /// A check page in the browser window that waits for the person.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub check: Option<CheckPage>,
}

/// A consent page or CAPTCHA the engine shows in the browser window: the person clears it
/// there, and the run goes on by itself.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckPage {
    /// Consent, CAPTCHA or blocked.
    pub kind: ChallengeKind,
    /// The search address.
    pub url: String,
    /// The wait ends about this time (RFC 3339); the keyword then fails.
    pub until: String,
}

/// Cancellation, progress and waiting of a run.
#[derive(Clone, Copy)]
pub struct RunControl<'a> {
    /// Whether the user stopped the run.
    pub cancelled: &'a (dyn Fn() -> bool + Sync),
    /// Called after every request and while waiting.
    pub progress: &'a (dyn Fn(&Progress) + Sync),
    /// Waits (tests pass a function that returns at once).
    pub sleep: &'a (dyn Fn(Duration) + Sync),
}

impl std::fmt::Debug for RunControl<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RunControl").finish_non_exhaustive()
    }
}

/// A provider that stopped answering during a run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Failure {
    /// Provider id.
    pub provider: String,
    /// What it asked.
    pub step: String,
    /// Why.
    pub error: String,
}

/// What a run did.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RunSummary {
    /// The run.
    pub run_id: i64,
    /// How it ended.
    pub status: RunStatus,
    /// Requests sent.
    pub requests: u32,
    /// US dollars spent.
    pub cost_usd: f64,
    /// Keywords found.
    pub found: u64,
    /// Problems, at most one per provider and kind.
    pub failures: Vec<Failure>,
}

/// Whether a provider can be used: switched on and every key available.
pub(crate) fn usable(
    workspace: &KeywordWorkspace,
    settings: &Settings,
    provider_id: &str,
) -> Result<(), String> {
    let definition = provider(provider_id).map_err(|error| error.to_string())?;
    if settings.disabled.contains(definition.id) {
        return Err(format!("{} is switched off", definition.name));
    }
    if definition.secrets_of == crate::sources::GOOGLE && settings.google.client_id.is_none() {
        return Err("the Google OAuth client id is not set".to_owned());
    }
    if let Some(on) = match definition.id {
        FreeSuggest::GOOGLE => Some(settings.suggest.google),
        FreeSuggest::YANDEX => Some(settings.suggest.yandex),
        _ => None,
    } {
        if !on || settings.suggest.accepted_at.is_none() {
            return Err(format!(
                "{} are off; switch them on in the settings after reading the terms warning",
                definition.name
            ));
        }
        let store = workspace.store().map_err(|error| error.to_string())?;
        let today = crate::now().chars().take(10).collect::<String>();
        if store
            .refused_on(definition.id, &today)
            .map_err(|error| error.to_string())?
        {
            return Err(format!(
                "{} refused a request today; stopped until tomorrow",
                definition.name
            ));
        }
        let used = store
            .requests_on(definition.id, &today)
            .map_err(|error| error.to_string())?;
        if used >= u64::from(settings.suggest.daily_limit) {
            return Err(format!(
                "the daily limit of {} {} requests is reached",
                settings.suggest.daily_limit, definition.name
            ));
        }
    }
    if let Some(name) = definition
        .secrets
        .iter()
        .find(|name| workspace.secret(definition.secrets_of, name).is_none())
    {
        return Err(format!("{} key `{name}` is not set", definition.name));
    }
    Ok(())
}

/// How an attempt was paid, as the store records it.
pub(crate) fn via_label(via: Option<seonexus_cloud::Via>) -> Option<String> {
    via.map(|via| {
        match via {
            seonexus_cloud::Via::Own => "own",
            seonexus_cloud::Via::Credit => "credit",
        }
        .to_owned()
    })
}

/// Adds a problem unless the same provider already reported it.
fn push_failure(summary: &mut RunSummary, step: &Step<'_>, error: String) {
    if !summary
        .failures
        .iter()
        .any(|failure| failure.provider == step.provider && failure.error == error)
    {
        summary.failures.push(Failure {
            provider: step.provider.to_owned(),
            step: step.label.to_owned(),
            error,
        });
    }
}

impl KeywordWorkspace {
    /// Waits until Wordstat's hourly quota has room; `false` when the user stopped the run.
    fn wait_for_wordstat(
        &self,
        settings: &Settings,
        control: &RunControl<'_>,
        progress: &mut Progress,
    ) -> Result<bool, KwAppError> {
        loop {
            let used = self.store()?.requests_since(Wordstat::ID, &ago(3_600))?;
            if used < u64::from(settings.wordstat_hourly_quota) {
                progress.waiting_until = None;
                return Ok(true);
            }
            progress.waiting_until = Some(ago(-60));
            (control.progress)(progress);
            for _ in 0..60 {
                if (control.cancelled)() {
                    return Ok(false);
                }
                (control.sleep)(Duration::from_secs(1));
            }
        }
    }

    /// Runs `steps`; `keep` stores what each one returned and says how many keywords it
    /// found.
    pub(crate) fn run_steps(
        &self,
        project_id: i64,
        kind: RunKind,
        steps: &[Step<'_>],
        transport: &dyn HttpTransport,
        control: &RunControl<'_>,
        mut keep: impl FnMut(usize, Fetched) -> Result<u64, KwAppError>,
    ) -> Result<RunSummary, KwAppError> {
        let settings = self.settings()?;
        let planned = u32::try_from(steps.len()).unwrap_or(u32::MAX);
        let estimate: f64 = steps.iter().map(|step| step.cost_usd).sum();
        let run_id = self
            .store()?
            .begin_run(kind, Some(project_id), planned, estimate, &now())?;
        let mut summary = RunSummary {
            run_id,
            status: RunStatus::Done,
            requests: 0,
            cost_usd: 0.0,
            found: 0,
            failures: Vec::new(),
        };
        let mut stopped: BTreeSet<&str> = BTreeSet::new();
        let mut progress = Progress {
            run_id,
            done: 0,
            planned,
            step: String::new(),
            found: 0,
            waiting_until: None,
            check: None,
        };
        let mut succeeded = 0_u32;
        for (index, step) in steps.iter().enumerate() {
            progress.done = u32::try_from(index).unwrap_or(u32::MAX);
            step.label.clone_into(&mut progress.step);
            if (control.cancelled)() {
                summary.status = RunStatus::Cancelled;
                break;
            }
            if stopped.contains(step.provider) {
                continue;
            }
            match self.gate(step, &settings, control, &mut progress, &mut summary)? {
                Gate::Go => {}
                Gate::Skip => {
                    stopped.insert(step.provider);
                    continue;
                }
                Gate::Cancel => {
                    summary.status = RunStatus::Cancelled;
                    break;
                }
            }
            let Some(result) = self.send(run_id, step, transport, control)? else {
                summary.status = RunStatus::Cancelled;
                break;
            };
            self.record(run_id, step, &result)?;
            match result {
                Ok(fetched) => {
                    summary.requests += fetched.requests;
                    summary.cost_usd += fetched.cost_usd;
                    let found = keep(index, fetched)?;
                    summary.found += found;
                    progress.found = summary.found;
                    succeeded += 1;
                }
                Err(error) => {
                    if matches!(
                        error,
                        SourceError::Auth(_)
                            | SourceError::Config(_)
                            | SourceError::Quota { .. }
                            | SourceError::RateLimited { .. }
                    ) {
                        stopped.insert(step.provider);
                    }
                    push_failure(&mut summary, step, error.to_string());
                }
            }
            progress.done = u32::try_from(index + 1).unwrap_or(u32::MAX);
            (control.progress)(&progress);
            if let Some(pause) = step.pause {
                (control.sleep)(pause);
            }
        }
        if summary.status != RunStatus::Cancelled && !summary.failures.is_empty() {
            summary.status = if succeeded == 0 {
                RunStatus::Failed
            } else {
                RunStatus::Partial
            };
        }
        let error = summary
            .failures
            .first()
            .map(|failure| format!("{}: {}", failure.provider, failure.error));
        self.store()?.finish_run(
            run_id,
            summary.status,
            progress.done,
            &now(),
            error.as_deref(),
        )?;
        Ok(summary)
    }

    /// Whether a step may be sent: its provider usable, its budget left, Wordstat's quota
    /// free.
    fn gate(
        &self,
        step: &Step<'_>,
        settings: &Settings,
        control: &RunControl<'_>,
        progress: &mut Progress,
        summary: &mut RunSummary,
    ) -> Result<Gate, KwAppError> {
        if let Err(reason) = usable(self, settings, step.provider) {
            push_failure(summary, step, reason);
            return Ok(Gate::Skip);
        }
        if let Some(budget) = settings.budgets.get(step.provider) {
            let spent = self.store()?.spent(step.provider, &month())?;
            if spent + step.cost_usd > *budget {
                push_failure(
                    summary,
                    step,
                    format!(
                        "the monthly budget of {budget} USD would be passed (spent {spent:.2})"
                    ),
                );
                return Ok(Gate::Skip);
            }
        }
        if step.provider == Wordstat::ID && !self.wait_for_wordstat(settings, control, progress)? {
            return Ok(Gate::Cancel);
        }
        Ok(Gate::Go)
    }

    /// Sends a step, waiting out one rate limit; `None` when the user stopped the run
    /// meanwhile. The attempts are recorded.
    fn send(
        &self,
        run_id: i64,
        step: &Step<'_>,
        transport: &dyn HttpTransport,
        control: &RunControl<'_>,
    ) -> Result<Option<Result<Fetched, SourceError>>, KwAppError> {
        // On OctopusOn SEO credits the price is what the gateway charged, not the estimate.
        let metered =
            |result: Result<Fetched, SourceError>| match (result, transport.take_charge_usd()) {
                (Ok(mut fetched), Some(charge)) => {
                    fetched.cost_usd = charge;
                    Ok(fetched)
                }
                (result, _) => result,
            };
        let mut result = metered(self.with_sources(transport, |context| (step.send)(context)));
        if let Err(SourceError::RateLimited { retry_after }) = &result {
            let wait = retry_after.unwrap_or(60).min(MAX_RATE_WAIT);
            self.record(run_id, step, &result)?;
            (control.sleep)(Duration::from_secs(wait));
            if (control.cancelled)() {
                return Ok(None);
            }
            result = metered(self.with_sources(transport, |context| (step.send)(context)));
        }
        Ok(Some(result))
    }

    fn record(
        &self,
        run_id: i64,
        step: &Step<'_>,
        result: &Result<Fetched, SourceError>,
    ) -> Result<(), KwAppError> {
        let (outcome, requests, items, cost_usd, error) = match result {
            Ok(fetched) => (
                AttemptOutcome::Success,
                fetched.requests,
                u32::try_from(fetched.keywords.len()).unwrap_or(u32::MAX),
                fetched.cost_usd,
                None,
            ),
            Err(error) => (
                match error {
                    SourceError::Auth(_)
                    | SourceError::Quota { .. }
                    | SourceError::RateLimited { .. } => AttemptOutcome::Refused,
                    SourceError::Config(_) => AttemptOutcome::Skipped,
                    _ => AttemptOutcome::Failed,
                },
                u32::from(!matches!(error, SourceError::Config(_))),
                0,
                0.0,
                Some(error.to_string()),
            ),
        };
        let via = via_label(self.via(&self.settings()?, step.provider));
        self.store()?.record_attempt(&AttemptRecord {
            run_id: Some(run_id),
            provider: step.provider.to_owned(),
            attempted_at: now(),
            outcome,
            requests,
            items,
            cost_usd,
            error,
            engine: None,
            via,
        })?;
        Ok(())
    }
}
