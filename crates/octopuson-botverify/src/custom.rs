//! User-defined bots: a crawler of the user's own (monitoring, an internal crawler, a partner's
//! bot) or a bot the built-in catalog does not know yet.
//!
//! Custom bots are matched before the built-in catalog, so they can also re-label a known
//! user agent. They are verified like built-in bots: by static IP ranges (`custom:<id>`), by a
//! downloaded IP list in the common `{"prefixes": [{"ipv4Prefix": …}]}` format
//! (`custom-url:<id>`, refreshed with the other lists) and/or by reverse DNS suffixes.

use serde::{Deserialize, Serialize};

use crate::catalog::{BOTS, BotDef};
use crate::category::BotCategory;
use crate::cidr::IpNet;

/// Longest accepted id, name or pattern.
const MAX_TEXT: usize = 200;

/// A user-defined bot as stored and edited.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct CustomBot {
    /// Stable id used in queries (`bot:<id>`): lowercase letters, digits and `-`.
    pub id: String,
    /// Display name.
    pub name: String,
    /// Operator (optional).
    pub operator: String,
    /// Category name, e.g. `seo` or `monitoring`.
    pub category: String,
    /// Case-insensitive substrings of the user agent; any of them identifies the bot.
    pub patterns: Vec<String>,
    /// Addresses or CIDR prefixes the bot crawls from.
    pub ip_ranges: Vec<String>,
    /// URL of a published IP list (`ipv4Prefix` / `ipv6Prefix` JSON).
    pub ip_list_url: Option<String>,
    /// Host name suffixes accepted by forward-confirmed reverse DNS, e.g. `crawler.example.com`.
    pub dns_suffixes: Vec<String>,
}

impl Default for CustomBot {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            operator: String::new(),
            category: BotCategory::Other.name().to_owned(),
            patterns: Vec::new(),
            ip_ranges: Vec::new(),
            ip_list_url: None,
            dns_suffixes: Vec::new(),
        }
    }
}

/// An invalid custom bot.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum CustomBotError {
    /// The id is empty or uses other characters than `a-z`, `0-9` and `-`.
    #[error("invalid bot id `{0}`: use lowercase letters, digits and -")]
    InvalidId(String),
    /// Two custom bots share an id.
    #[error("two custom bots use the id `{0}`")]
    DuplicateId(String),
    /// The id is a built-in bot's.
    #[error("`{0}` is a built-in bot id; choose another")]
    BuiltinId(String),
    /// No user-agent pattern.
    #[error("bot `{0}` needs at least one user-agent pattern")]
    NoPatterns(String),
    /// Unknown category.
    #[error("bot `{bot}`: unknown category `{category}`")]
    UnknownCategory {
        /// Bot id.
        bot: String,
        /// Given category.
        category: String,
    },
    /// An IP range is not an address or CIDR prefix.
    #[error("bot `{bot}`: `{value}` is not an IP address or CIDR prefix")]
    InvalidRange {
        /// Bot id.
        bot: String,
        /// Given value.
        value: String,
    },
    /// The IP list URL is not an http(s) URL.
    #[error("bot `{bot}`: `{value}` is not an http(s) URL")]
    InvalidUrl {
        /// Bot id.
        bot: String,
        /// Given value.
        value: String,
    },
    /// A DNS suffix is not a host name.
    #[error("bot `{bot}`: `{value}` is not a host name suffix")]
    InvalidSuffix {
        /// Bot id.
        bot: String,
        /// Given value.
        value: String,
    },
    /// A value is too long.
    #[error("bot `{0}`: a value is longer than {MAX_TEXT} characters")]
    TooLong(String),
}

/// IP list source id of a bot's static ranges.
pub fn static_source_id(bot_id: &str) -> String {
    format!("custom:{bot_id}")
}

/// IP list source id of a bot's downloaded list.
pub fn url_source_id(bot_id: &str) -> String {
    format!("custom-url:{bot_id}")
}

/// Checks and normalizes custom bots (trimmed, lower-cased patterns and suffixes, names
/// defaulting to the id).
///
/// # Errors
/// The first [`CustomBotError`] found.
pub fn validate_custom_bots(bots: Vec<CustomBot>) -> Result<Vec<CustomBot>, CustomBotError> {
    let mut seen = std::collections::HashSet::new();
    bots.into_iter()
        .map(|bot| {
            let bot = normalize(bot)?;
            if !seen.insert(bot.id.clone()) {
                return Err(CustomBotError::DuplicateId(bot.id));
            }
            Ok(bot)
        })
        .collect()
}

fn normalize(mut bot: CustomBot) -> Result<CustomBot, CustomBotError> {
    bot.id = bot.id.trim().to_ascii_lowercase();
    let id = bot.id.clone();
    let valid_id = !id.is_empty()
        && id.len() <= 64
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-');
    if !valid_id {
        return Err(CustomBotError::InvalidId(id));
    }
    if BOTS.iter().any(|known| known.id == id) {
        return Err(CustomBotError::BuiltinId(id));
    }
    let name = bot.name.trim().to_owned();
    bot.name = name;
    if bot.name.is_empty() {
        bot.name.clone_from(&id);
    }
    let operator = bot.operator.trim().to_owned();
    bot.operator = operator;
    bot.category = bot.category.trim().to_ascii_lowercase();
    if BotCategory::from_name(&bot.category).is_none() {
        return Err(CustomBotError::UnknownCategory {
            bot: id,
            category: bot.category,
        });
    }
    bot.patterns = cleaned(&bot.patterns, str::to_ascii_lowercase);
    if bot.patterns.is_empty() {
        return Err(CustomBotError::NoPatterns(id));
    }
    bot.ip_ranges = cleaned(&bot.ip_ranges, ToOwned::to_owned);
    if let Some(bad) = bot.ip_ranges.iter().find(|r| r.parse::<IpNet>().is_err()) {
        return Err(CustomBotError::InvalidRange {
            bot: id,
            value: bad.clone(),
        });
    }
    bot.ip_list_url = bot
        .ip_list_url
        .map(|url| url.trim().to_owned())
        .filter(|url| !url.is_empty());
    if let Some(url) = &bot.ip_list_url
        && !(url.starts_with("https://") || url.starts_with("http://"))
    {
        return Err(CustomBotError::InvalidUrl {
            bot: id,
            value: url.clone(),
        });
    }
    bot.dns_suffixes = cleaned(&bot.dns_suffixes, |value| {
        value
            .trim_start_matches('.')
            .trim_end_matches('.')
            .to_ascii_lowercase()
    });
    if let Some(bad) = bot.dns_suffixes.iter().find(|s| !is_suffix(s)) {
        return Err(CustomBotError::InvalidSuffix {
            bot: id,
            value: bad.clone(),
        });
    }
    let too_long = [&bot.name, &bot.operator]
        .into_iter()
        .chain(&bot.patterns)
        .chain(&bot.dns_suffixes)
        .any(|text| text.len() > MAX_TEXT);
    if too_long {
        return Err(CustomBotError::TooLong(id));
    }
    Ok(bot)
}

fn cleaned(values: &[String], map: impl Fn(&str) -> String) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    values
        .iter()
        .map(|value| map(value.trim()))
        .filter(|value| !value.is_empty() && seen.insert(value.clone()))
        .collect()
}

fn is_suffix(value: &str) -> bool {
    value.contains('.')
        && value.split('.').all(|label| {
            !label.is_empty()
                && label
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-')
        })
}

/// A catalog entry for a (validated) custom bot. Its strings live as long as the process:
/// definitions are interned by [`crate::Classifier::with_custom`], so editing bots repeatedly
/// only keeps the distinct versions.
pub(crate) fn leak_def(bot: &CustomBot) -> BotDef {
    fn leak(text: &str) -> &'static str {
        Box::leak(text.to_owned().into_boxed_str())
    }
    fn leak_list(items: impl Iterator<Item = String>) -> &'static [&'static str] {
        Box::leak(
            items
                .map(|item| leak(&item))
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        )
    }
    let patterns: Vec<&'static [&'static str]> = bot
        .patterns
        .iter()
        .map(|pattern| leak_list(std::iter::once(pattern.clone())))
        .collect();
    let mut lists = Vec::new();
    if !bot.ip_ranges.is_empty() {
        lists.push(static_source_id(&bot.id));
    }
    if bot.ip_list_url.is_some() {
        lists.push(url_source_id(&bot.id));
    }
    BotDef {
        id: leak(&bot.id),
        name: leak(&bot.name),
        operator: leak(&bot.operator),
        category: BotCategory::from_name(&bot.category).unwrap_or(BotCategory::Other),
        patterns: Box::leak(patterns.into_boxed_slice()),
        excludes: &[],
        ip_lists: leak_list(lists.into_iter()),
        dns_suffixes: leak_list(bot.dns_suffixes.iter().cloned()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bot(id: &str) -> CustomBot {
        CustomBot {
            id: id.into(),
            patterns: vec!["AcmeBot".into()],
            ..CustomBot::default()
        }
    }

    #[test]
    fn normalizes_valid_bots() {
        let bots = validate_custom_bots(vec![CustomBot {
            id: " Acme-Bot ".into(),
            category: "Monitoring".into(),
            patterns: vec![" AcmeBot/1 ".into(), "acmebot/1".into(), String::new()],
            ip_ranges: vec!["203.0.113.0/24".into(), "198.51.100.7".into()],
            ip_list_url: Some("  ".into()),
            dns_suffixes: vec![".Crawl.Acme.com.".into()],
            ..CustomBot::default()
        }])
        .unwrap();
        let bot = &bots[0];
        assert_eq!(bot.id, "acme-bot");
        assert_eq!(bot.name, "acme-bot");
        assert_eq!(bot.category, "monitoring");
        assert_eq!(bot.patterns, ["acmebot/1"]);
        assert_eq!(bot.ip_list_url, None);
        assert_eq!(bot.dns_suffixes, ["crawl.acme.com"]);
        let def = leak_def(bot);
        assert_eq!(def.ip_lists, ["custom:acme-bot"]);
        assert!(def.is_verifiable());
    }

    #[test]
    fn rejects_invalid_bots() {
        let error = |bot: CustomBot| validate_custom_bots(vec![bot]).unwrap_err();
        assert!(matches!(
            error(bot("Acme Bot")),
            CustomBotError::InvalidId(_)
        ));
        assert!(matches!(
            error(bot("googlebot-smartphone")),
            CustomBotError::BuiltinId(_)
        ));
        assert!(matches!(
            error(CustomBot {
                patterns: vec![],
                ..bot("x")
            }),
            CustomBotError::NoPatterns(_)
        ));
        assert!(matches!(
            error(CustomBot {
                category: "robots".into(),
                ..bot("x")
            }),
            CustomBotError::UnknownCategory { .. }
        ));
        assert!(matches!(
            error(CustomBot {
                ip_ranges: vec!["300.1.1.1".into()],
                ..bot("x")
            }),
            CustomBotError::InvalidRange { .. }
        ));
        assert!(matches!(
            error(CustomBot {
                ip_list_url: Some("ftp://x".into()),
                ..bot("x")
            }),
            CustomBotError::InvalidUrl { .. }
        ));
        assert!(matches!(
            error(CustomBot {
                dns_suffixes: vec!["not a host".into()],
                ..bot("x")
            }),
            CustomBotError::InvalidSuffix { .. }
        ));
        assert!(matches!(
            validate_custom_bots(vec![bot("x"), bot("x")]).unwrap_err(),
            CustomBotError::DuplicateId(_)
        ));
    }
}
