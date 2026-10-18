//! Adapters from the bot domain (`octopuson-botverify`) to the store's ports.

use std::net::IpAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use octopuson_botverify::catalog::BotDef;
use octopuson_botverify::{Classifier, DnsError, DnsResolver, Verifier};
use octopuson_net::SystemDns;
use octopuson_store::{BotIpPair, BotIpVerdict, BotIpVerifier, BotTag, UserAgentClassifier};
use rayon::prelude::*;

use super::custom::custom_bots_or_none;
use super::ranges::load_snapshot;
use crate::workspace::Workspace;

/// Concurrent DNS lookups; they mostly wait on the network.
const DNS_THREADS: usize = 32;

/// Reverse lookups per operation. Sites attacked by botnets faking a crawler can show
/// hundreds of thousands of addresses; the rest stay pending for `bots verify`.
const DNS_LOOKUP_BUDGET: usize = 20_000;

/// System DNS with a limit on reverse lookups.
#[derive(Debug)]
struct BudgetedDns {
    inner: SystemDns,
    used: AtomicUsize,
}

impl DnsResolver for BudgetedDns {
    fn reverse(&self, ip: IpAddr) -> Result<Vec<String>, DnsError> {
        if self.used.fetch_add(1, Ordering::Relaxed) >= DNS_LOOKUP_BUDGET {
            return Err(DnsError::Failed("DNS lookup budget exhausted".into()));
        }
        self.inner.reverse(ip)
    }

    fn forward(&self, host: &str) -> Result<Vec<IpAddr>, DnsError> {
        self.inner.forward(host)
    }
}

/// Store tag of a catalog bot.
pub(crate) fn tag(bot: &'static BotDef) -> BotTag {
    BotTag {
        id: bot.id,
        category: bot.category.name(),
    }
}

/// The built-in catalog preceded by the workspace's custom bots.
pub(crate) fn classifier(workspace: &Workspace) -> Classifier {
    Classifier::with_custom(&custom_bots_or_none(workspace))
}

/// Classifies user agents with the catalog (and custom bots), in parallel.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct CatalogClassifier(pub(crate) Classifier);

impl UserAgentClassifier for CatalogClassifier {
    fn version(&self) -> u32 {
        self.0.version()
    }

    fn classify(&self, user_agents: &[String]) -> Vec<Option<BotTag>> {
        user_agents
            .par_iter()
            .map(|ua| self.0.classify(ua).map(tag))
            .collect()
    }
}

/// Verifies `(ip, bot)` pairs with IP lists and optionally DNS on a dedicated pool.
pub(crate) struct PairVerifier {
    verifier: Verifier,
    classifier: Classifier,
    dns_pool: Option<rayon::ThreadPool>,
}

impl std::fmt::Debug for PairVerifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PairVerifier")
            .field("verifier", &self.verifier)
            .finish_non_exhaustive()
    }
}

impl PairVerifier {
    /// Whether DNS checks are active.
    pub(crate) fn uses_dns(&self) -> bool {
        self.verifier.uses_dns()
    }

    /// The underlying domain verifier.
    pub(crate) fn inner(&self) -> &Verifier {
        &self.verifier
    }
}

impl BotIpVerifier for PairVerifier {
    fn verify(&self, pairs: &[BotIpPair]) -> Vec<BotIpVerdict> {
        let check = |pair: &BotIpPair| {
            let (Ok(ip), Some(bot)) = (pair.ip.parse::<IpAddr>(), self.classifier.find(&pair.bot))
            else {
                return BotIpVerdict {
                    verdict: "unverified",
                    method: "not-verifiable",
                };
            };
            let result = self.verifier.verify(ip, bot);
            BotIpVerdict {
                verdict: result.verdict.name(),
                method: result.method.name(),
            }
        };
        match &self.dns_pool {
            Some(pool) => pool.install(|| pairs.par_iter().map(check).collect()),
            None => pairs.par_iter().map(check).collect(),
        }
    }
}

/// Everything an operation needs to classify and verify bots.
#[derive(Debug)]
pub(crate) struct BotContext {
    pub(crate) classifier: CatalogClassifier,
    pub(crate) verifier: PairVerifier,
    /// DNS was requested but the resolver does not answer (offline).
    pub(crate) dns_unavailable: bool,
}

/// Loads IP lists and, when `dns` is requested and DNS works, enables DNS verification.
pub(crate) fn bot_context(workspace: &Workspace, dns: bool) -> BotContext {
    let custom = custom_bots_or_none(workspace);
    let classifier = Classifier::with_custom(&custom);
    let snapshot = load_snapshot(workspace).with_custom_ranges(&custom);
    let dns_available = dns && SystemDns.is_available();
    if dns && !dns_available {
        tracing::warn!("DNS does not respond; bots needing DNS verification stay unverified");
    }
    let resolver: Option<Arc<dyn DnsResolver>> = dns_available.then(|| {
        Arc::new(BudgetedDns {
            inner: SystemDns,
            used: AtomicUsize::new(0),
        }) as Arc<dyn DnsResolver>
    });
    let dns_pool = dns_available
        .then(|| {
            rayon::ThreadPoolBuilder::new()
                .num_threads(DNS_THREADS)
                .thread_name(|index| format!("dns-{index}"))
                .build()
                .ok()
        })
        .flatten();
    BotContext {
        classifier: CatalogClassifier(classifier),
        verifier: PairVerifier {
            verifier: Verifier::new(&snapshot, resolver),
            classifier,
            dns_pool,
        },
        dns_unavailable: dns && !dns_available,
    }
}
