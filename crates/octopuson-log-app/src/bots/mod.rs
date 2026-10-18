//! Bot use-cases: the bot catalog, crawler IP lists and (re-)verification of stored hits.
//!
//! Classification happens during import; see ADR 0005 for the design.

mod adapters;
mod custom;
mod ranges;

pub(crate) use adapters::{BotContext, bot_context, classifier, tag};
pub use custom::{list_custom_bots, parse_custom_bots, save_custom_bots};
pub use octopuson_botverify::CustomBot;
pub use octopuson_botverify::ranges::RangeSnapshot;
pub use ranges::{
    RangeDownload, RangeSourceStatus, RangeUpdateReport, SourceFailure, bot_range_status,
    download_custom_ranges, download_ranges, update_bot_ranges,
};

use std::collections::BTreeMap;
use std::io;
use std::net::IpAddr;
use std::path::PathBuf;

use octopuson_botverify::BotCategory;
use octopuson_botverify::catalog::BotDef;
use octopuson_store::{Catalog, SiteStore, StoreError, VerifyScope};
use serde::Serialize;

use crate::workspace::Workspace;

/// Errors of the bot use-cases.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum BotError {
    /// No bot has this id.
    #[error("unknown bot `{0}`")]
    UnknownBot(String),
    /// Custom bot definitions are invalid or unreadable.
    #[error("invalid custom bots: {0}")]
    InvalidCustomBots(String),
    /// The text is not an IP address.
    #[error("`{0}` is not an IP address")]
    InvalidIp(String),
    /// Storage failure, unknown or busy site.
    #[error(transparent)]
    Store(#[from] StoreError),
    /// A file could not be written.
    #[error("cannot write `{path}`")]
    Write {
        /// File involved.
        path: PathBuf,
        /// Underlying error.
        #[source]
        source: io::Error,
    },
}

/// A known bot as shown to users.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BotInfo {
    /// Stable id used in queries (`bot:<id>`).
    pub id: &'static str,
    /// Display name.
    pub name: &'static str,
    /// Operator.
    pub operator: &'static str,
    /// Category name.
    pub category: &'static str,
    /// How hits can be verified: `ip-list`, `dns`, `ip-list+dns` or `none`.
    pub verification: &'static str,
    /// Defined by the user (see [`save_custom_bots`]).
    pub custom: bool,
}

impl From<&'static BotDef> for BotInfo {
    fn from(bot: &'static BotDef) -> Self {
        let verification = match (!bot.ip_lists.is_empty(), !bot.dns_suffixes.is_empty()) {
            (true, true) => "ip-list+dns",
            (true, false) => "ip-list",
            (false, true) => "dns",
            (false, false) => "none",
        };
        Self {
            id: bot.id,
            name: bot.name,
            operator: bot.operator,
            category: bot.category.name(),
            verification,
            custom: octopuson_botverify::catalog::find(bot.id).is_none(),
        }
    }
}

/// Every known bot (custom bots first) in matching order, optionally limited to one category.
pub fn list_bots(workspace: &Workspace, category: Option<&str>) -> Vec<BotInfo> {
    let category = category.and_then(BotCategory::from_name);
    classifier(workspace)
        .bots()
        .iter()
        .filter(|bot| category.is_none_or(|c| bot.category == c))
        .map(BotInfo::from)
        .collect()
}

/// Names of all bot categories.
pub fn bot_categories() -> Vec<&'static str> {
    BotCategory::ALL.iter().map(|c| c.name()).collect()
}

/// The bot a user-agent string belongs to (custom bots included).
pub fn identify_user_agent(workspace: &Workspace, user_agent: &str) -> Option<BotInfo> {
    classifier(workspace)
        .classify(user_agent)
        .map(BotInfo::from)
}

/// Result of [`verify_ip`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct IpCheck {
    /// Checked address.
    pub ip: String,
    /// Claimed bot.
    pub bot: BotInfo,
    /// `verified`, `spoofed` or `unverified`.
    pub verdict: &'static str,
    /// How the verdict was reached.
    pub method: &'static str,
}

/// Checks whether `ip` belongs to the operator of `bot_id`.
///
/// # Errors
/// [`BotError::UnknownBot`] or [`BotError::InvalidIp`].
pub fn verify_ip(
    workspace: &Workspace,
    ip: &str,
    bot_id: &str,
    dns: bool,
) -> Result<IpCheck, BotError> {
    let bot = classifier(workspace)
        .find(bot_id)
        .ok_or_else(|| BotError::UnknownBot(bot_id.to_owned()))?;
    let address: IpAddr = ip
        .trim()
        .parse()
        .map_err(|_| BotError::InvalidIp(ip.to_owned()))?;
    let context = bot_context(workspace, dns);
    let result = context.verifier.inner().verify(address, bot);
    Ok(IpCheck {
        ip: address.to_string(),
        bot: BotInfo::from(bot),
        verdict: result.verdict.name(),
        method: result.method.name(),
    })
}

/// Input of [`verify_site_bots`].
#[derive(Debug, Clone)]
pub struct VerifySiteRequest {
    /// Site name.
    pub site: String,
    /// Use reverse/forward DNS for bots without (matching) IP lists.
    pub dns: bool,
    /// Check every stored verdict, not only provisional ones (after updating IP lists).
    pub all: bool,
}

/// Result of [`verify_site_bots`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VerifySiteReport {
    /// Site name.
    pub site: String,
    /// Whether DNS checks ran.
    pub dns_used: bool,
    /// DNS was requested but did not respond.
    pub dns_unavailable: bool,
    /// User agents whose bot changed because the catalog was updated.
    pub reclassified_user_agents: u64,
    /// `(ip, bot)` pairs checked.
    pub checked: u64,
    /// Pairs whose verdict or method changed.
    pub changed: u64,
    /// Bots whose rollups were rebuilt.
    pub rebuilt_bots: Vec<String>,
    /// Stored pairs per verdict after the run.
    pub verdicts: BTreeMap<String, u64>,
}

/// Verifies the bots of a site again, e.g. with DNS or after updating IP lists.
///
/// # Errors
/// See [`BotError`]; a busy site is reported as a store error.
pub fn verify_site_bots(
    workspace: &Workspace,
    request: &VerifySiteRequest,
) -> Result<VerifySiteReport, BotError> {
    let site = Catalog::open(workspace.data_dir())?.site_by_name(request.site.trim())?;
    let mut store = SiteStore::open(workspace.data_dir(), site.id, workspace.settings())?;
    let context = bot_context(workspace, request.dns);
    let reclassified = store.reclassify_user_agents(&context.classifier, &context.verifier)?;
    let scope = if request.all {
        VerifyScope::All
    } else {
        VerifyScope::Pending
    };
    let outcome = store.verify_bots(&context.verifier, scope)?;
    Ok(VerifySiteReport {
        site: site.name,
        dns_used: context.verifier.uses_dns(),
        dns_unavailable: context.dns_unavailable,
        reclassified_user_agents: reclassified.changed_user_agents,
        checked: outcome.checked,
        changed: outcome.changed,
        rebuilt_bots: outcome.rebuilt_bots,
        verdicts: store.verdict_counts()?.into_iter().collect(),
    })
}

#[cfg(test)]
mod tests;
