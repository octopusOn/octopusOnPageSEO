//! Trying providers in order.

use serde::{Deserialize, Serialize};

use super::{ProviderError, SearchContext, SerpFetch, SerpProvider};
use crate::SerpQuery;

/// How one provider of a chain fared.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AttemptOutcome {
    /// It returned the snapshot.
    Success,
    /// It was not tried because the query is outside its capabilities.
    Skipped,
    /// It failed with the error code (`auth`, `quota`, ...).
    Failed(String),
}

/// One provider's attempt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Attempt {
    /// Provider id.
    pub provider: String,
    /// What happened.
    pub outcome: AttemptOutcome,
    /// Error message, secrets redacted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// Every provider failed, or one hit an error that must not fall back.
#[derive(Debug, Clone, PartialEq)]
pub struct ChainFailure {
    /// The error that ended the chain (the last one, or the challenge).
    pub error: ProviderError,
    /// Every attempt.
    pub attempts: Vec<Attempt>,
}

/// Runs `query` on the first provider that can and does answer.
///
/// Providers outside the query's capabilities are skipped. Errors another provider may not
/// have (credentials, quota, rate limits, network, HTTP errors, unreadable responses) move on
/// to the next one; a challenge or a cancellation ends the chain, so a person decides what to
/// do. One snapshot always comes from a single provider.
///
/// # Errors
/// [`ChainFailure`] with every attempt when no provider returned a snapshot.
pub fn search_chain(
    providers: &[&dyn SerpProvider],
    query: &SerpQuery,
    context: &SearchContext<'_>,
) -> Result<(SerpFetch, Vec<Attempt>), ChainFailure> {
    let mut attempts = Vec::new();
    let mut last = ProviderError::Unsupported(format!(
        "no provider is set up for {} {} results",
        query.device.as_str(),
        query.engine
    ));
    for provider in providers {
        if !provider.capabilities().supports(query) {
            attempts.push(Attempt {
                provider: provider.id().to_owned(),
                outcome: AttemptOutcome::Skipped,
                message: None,
            });
            continue;
        }
        match provider.search(query, context) {
            Ok(fetch) => {
                attempts.push(Attempt {
                    provider: provider.id().to_owned(),
                    outcome: AttemptOutcome::Success,
                    message: None,
                });
                return Ok((fetch, attempts));
            }
            Err(error) => {
                attempts.push(Attempt {
                    provider: provider.id().to_owned(),
                    outcome: AttemptOutcome::Failed(error.code().to_owned()),
                    message: Some(error.to_string()),
                });
                if !error.falls_back() {
                    return Err(ChainFailure { error, attempts });
                }
                last = error;
            }
        }
    }
    Err(ChainFailure {
        error: last,
        attempts,
    })
}

#[cfg(test)]
mod tests {
    use octopuson_core::http::testing::FakeTransport;

    use super::*;
    use crate::challenge::{Challenge, ChallengeKind};
    use crate::provider::{Capabilities, CostEstimate};
    use crate::{Device, Engine, SnapshotBuilder};

    struct Canned {
        id: &'static str,
        engines: Vec<Engine>,
        result: Result<(), ProviderError>,
    }

    impl SerpProvider for Canned {
        fn id(&self) -> &str {
            self.id
        }

        fn name(&self) -> &str {
            self.id
        }

        fn capabilities(&self) -> Capabilities {
            Capabilities {
                engines: self.engines.clone(),
                devices: vec![Device::Desktop],
                max_depth: 100,
                interactive: false,
            }
        }

        fn estimate(&self, _: &SerpQuery) -> CostEstimate {
            CostEstimate::default()
        }

        fn search(
            &self,
            query: &SerpQuery,
            context: &SearchContext<'_>,
        ) -> Result<SerpFetch, ProviderError> {
            self.result.clone().map(|()| SerpFetch {
                snapshot: SnapshotBuilder::new(query.clone(), self.id, context.fetched_at.clone())
                    .finish(),
                pages: Vec::new(),
                requests: 1,
                reported_cost_usd: None,
                estimated_cost_usd: 0.0,
            })
        }
    }

    fn run(providers: &[&dyn SerpProvider]) -> Result<(SerpFetch, Vec<Attempt>), ChainFailure> {
        let transport = FakeTransport::new();
        let context = SearchContext {
            transport: &transport,
            secrets: &|_, _| None,
            fetched_at: "2026-10-18T10:00:00Z".to_owned(),
            cancelled: &|| false,
            events: &|_| {},
        };
        search_chain(
            providers,
            &SerpQuery::new(Engine::Google, "kahve"),
            &context,
        )
    }

    #[test]
    fn skips_unsupported_providers_and_falls_back_on_errors() {
        let yandex_only = Canned {
            id: "yandex-only",
            engines: vec![Engine::Yandex],
            result: Ok(()),
        };
        let broke = Canned {
            id: "broke",
            engines: vec![Engine::Google],
            result: Err(ProviderError::Quota {
                message: "empty".to_owned(),
                retry_after: None,
            }),
        };
        let good = Canned {
            id: "good",
            engines: vec![Engine::Google],
            result: Ok(()),
        };
        let (fetch, attempts) = run(&[&yandex_only, &broke, &good]).unwrap();
        assert_eq!(fetch.snapshot.provider, "good");
        let outcomes: Vec<_> = attempts
            .iter()
            .map(|attempt| attempt.outcome.clone())
            .collect();
        assert_eq!(
            outcomes,
            [
                AttemptOutcome::Skipped,
                AttemptOutcome::Failed("quota".to_owned()),
                AttemptOutcome::Success
            ]
        );
    }

    #[test]
    fn stops_at_challenges_and_reports_the_last_error() {
        let blocked = Canned {
            id: "browser",
            engines: vec![Engine::Google],
            result: Err(ProviderError::Blocked(Challenge {
                kind: ChallengeKind::Captcha,
                reason: "sorry".to_owned(),
            })),
        };
        let good = Canned {
            id: "good",
            engines: vec![Engine::Google],
            result: Ok(()),
        };
        let failure = run(&[&blocked, &good]).unwrap_err();
        assert!(matches!(failure.error, ProviderError::Blocked(_)));
        assert_eq!(failure.attempts.len(), 1);

        let none = run(&[]).unwrap_err();
        assert!(matches!(none.error, ProviderError::Unsupported(_)));
    }
}
