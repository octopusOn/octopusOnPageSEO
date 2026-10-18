//! Ports through which the application supplies bot classification and verification.
//!
//! The store only persists and aggregates the answers; deciding what a bot is and whether an
//! IP is genuine belongs to the domain (`octopuson-botverify`), wired up by the application.

/// Classification of a user agent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BotTag {
    /// Bot id, e.g. `googlebot-smartphone`.
    pub id: &'static str,
    /// Category name, e.g. `search`.
    pub category: &'static str,
}

/// Classifies user agents in bulk.
pub trait UserAgentClassifier: Sync {
    /// Version of the classification rules; a change triggers re-classification of stored
    /// user agents.
    fn version(&self) -> u32;

    /// One answer per input, in order.
    fn classify(&self, user_agents: &[String]) -> Vec<Option<BotTag>>;
}

/// An `(ip, bot)` pair to verify.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BotIpPair {
    /// Client IP as stored.
    pub ip: String,
    /// Bot id.
    pub bot: String,
}

/// Verdict for a [`BotIpPair`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BotIpVerdict {
    /// `verified`, `spoofed` or `unverified`.
    pub verdict: &'static str,
    /// How the verdict was reached, e.g. `ip-list`.
    pub method: &'static str,
}

/// Verifies `(ip, bot)` pairs in bulk; may use the network.
pub trait BotIpVerifier: Sync {
    /// One verdict per input, in order.
    fn verify(&self, pairs: &[BotIpPair]) -> Vec<BotIpVerdict>;
}

/// Verifier answering `unverified` for everything (tests, offline maintenance).
#[derive(Debug, Clone, Copy, Default)]
pub struct NoBotVerification;

impl BotIpVerifier for NoBotVerification {
    fn verify(&self, pairs: &[BotIpPair]) -> Vec<BotIpVerdict> {
        vec![
            BotIpVerdict {
                verdict: "unverified",
                method: "not-checked",
            };
            pairs.len()
        ]
    }
}
