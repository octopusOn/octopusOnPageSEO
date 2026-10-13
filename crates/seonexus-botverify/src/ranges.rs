//! Operators' published IP lists: sources, lenient parsing and snapshots.
//!
//! Every operator uses the same JSON shape, `{"prefixes": [{"ipv4Prefix": "…"}, …]}`, but some
//! embed it in HTML or omit the prefix length, so parsing extracts every `ipv4Prefix` /
//! `ipv6Prefix` string value instead of requiring an exact document structure.

use std::collections::{BTreeMap, HashMap};

use memchr::memmem;
use serde::{Deserialize, Serialize};

use crate::cidr::{IpNet, IpRangeSet};

/// Ids of the known IP list sources.
pub mod source {
    /// Googlebot and other common Google crawlers.
    pub const GOOGLE_COMMON: &str = "google-common";
    /// Google special-case crawlers (AdsBot, …).
    pub const GOOGLE_SPECIAL: &str = "google-special";
    /// Google user-triggered fetchers.
    pub const GOOGLE_USER: &str = "google-user";
    /// Google user-triggered fetchers on Google-owned IPs.
    pub const GOOGLE_USER_GOOGLE: &str = "google-user-google";
    /// Bingbot.
    pub const BING: &str = "bing";
    /// OpenAI GPTBot.
    pub const OPENAI_GPTBOT: &str = "openai-gptbot";
    /// OpenAI OAI-SearchBot.
    pub const OPENAI_SEARCHBOT: &str = "openai-searchbot";
    /// OpenAI ChatGPT-User.
    pub const OPENAI_CHATGPT_USER: &str = "openai-chatgpt-user";
    /// Anthropic's crawlers.
    pub const ANTHROPIC: &str = "anthropic";
    /// PerplexityBot.
    pub const PERPLEXITY_BOT: &str = "perplexity-bot";
    /// Perplexity-User.
    pub const PERPLEXITY_USER: &str = "perplexity-user";
    /// Applebot.
    pub const APPLE: &str = "apple";
    /// DuckDuckBot.
    pub const DUCKDUCKGO_BOT: &str = "duckduckgo-bot";
    /// DuckAssistBot.
    pub const DUCKDUCKGO_ASSIST: &str = "duckduckgo-assist";
    /// Common Crawl CCBot.
    pub const COMMONCRAWL: &str = "commoncrawl";
    /// MistralAI-User.
    pub const MISTRAL_USER: &str = "mistral-user";
    /// AhrefsBot and AhrefsSiteAudit.
    pub const AHREFS: &str = "ahrefs";
    /// Amazonbot.
    pub const AMAZONBOT: &str = "amazonbot";
}

/// Where an operator publishes an IP list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RangeSource {
    /// Stable id referenced by [`BotDef::ip_lists`](crate::BotDef::ip_lists).
    pub id: &'static str,
    /// Operator name.
    pub operator: &'static str,
    /// Download URL.
    pub url: &'static str,
}

/// Every known IP list.
pub const SOURCES: &[RangeSource] = &[
    RangeSource {
        id: source::GOOGLE_COMMON,
        operator: "Google",
        url: "https://developers.google.com/static/search/apis/ipranges/googlebot.json",
    },
    RangeSource {
        id: source::GOOGLE_SPECIAL,
        operator: "Google",
        url: "https://developers.google.com/static/search/apis/ipranges/special-crawlers.json",
    },
    RangeSource {
        id: source::GOOGLE_USER,
        operator: "Google",
        url: "https://developers.google.com/static/search/apis/ipranges/user-triggered-fetchers.json",
    },
    RangeSource {
        id: source::GOOGLE_USER_GOOGLE,
        operator: "Google",
        url: "https://developers.google.com/static/search/apis/ipranges/user-triggered-fetchers-google.json",
    },
    RangeSource {
        id: source::BING,
        operator: "Microsoft",
        url: "https://www.bing.com/toolbox/bingbot.json",
    },
    RangeSource {
        id: source::OPENAI_GPTBOT,
        operator: "OpenAI",
        url: "https://openai.com/gptbot.json",
    },
    RangeSource {
        id: source::OPENAI_SEARCHBOT,
        operator: "OpenAI",
        url: "https://openai.com/searchbot.json",
    },
    RangeSource {
        id: source::OPENAI_CHATGPT_USER,
        operator: "OpenAI",
        url: "https://openai.com/chatgpt-user.json",
    },
    RangeSource {
        id: source::ANTHROPIC,
        operator: "Anthropic",
        url: "https://claude.com/crawling/bots.json",
    },
    RangeSource {
        id: source::PERPLEXITY_BOT,
        operator: "Perplexity",
        url: "https://www.perplexity.com/perplexitybot.json",
    },
    RangeSource {
        id: source::PERPLEXITY_USER,
        operator: "Perplexity",
        url: "https://www.perplexity.com/perplexity-user.json",
    },
    RangeSource {
        id: source::APPLE,
        operator: "Apple",
        url: "https://search.developer.apple.com/applebot.json",
    },
    RangeSource {
        id: source::DUCKDUCKGO_BOT,
        operator: "DuckDuckGo",
        url: "https://duckduckgo.com/duckduckbot.json",
    },
    RangeSource {
        id: source::DUCKDUCKGO_ASSIST,
        operator: "DuckDuckGo",
        url: "https://duckduckgo.com/duckassistbot.json",
    },
    RangeSource {
        id: source::COMMONCRAWL,
        operator: "Common Crawl",
        url: "https://index.commoncrawl.org/ccbot.json",
    },
    RangeSource {
        id: source::MISTRAL_USER,
        operator: "Mistral AI",
        url: "https://mistral.ai/mistralai-user-ips.json",
    },
    RangeSource {
        id: source::AHREFS,
        operator: "Ahrefs",
        url: "https://api.ahrefs.com/v3/public/crawler-ip-ranges",
    },
    RangeSource {
        id: source::AMAZONBOT,
        operator: "Amazon",
        url: "https://developer.amazon.com/amazonbot/live-ip-addresses/",
    },
];

/// Errors while reading IP lists.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum RangeError {
    /// The document contains no prefixes (probably an error page or a changed format).
    #[error("the document contains no IP prefixes")]
    NoPrefixes,
    /// A prefix value is malformed.
    #[error(transparent)]
    InvalidPrefix(#[from] crate::cidr::IpNetError),
    /// A snapshot file is not valid JSON of the expected shape.
    #[error("invalid IP range snapshot: {0}")]
    InvalidSnapshot(String),
}

/// Extracts every `ipv4Prefix` / `ipv6Prefix` value of a published list.
///
/// # Errors
/// [`RangeError::NoPrefixes`] for documents without any, [`RangeError::InvalidPrefix`] when a
/// value is not an address or CIDR prefix.
pub fn parse_prefix_list(text: &str) -> Result<Vec<IpNet>, RangeError> {
    let mut prefixes = Vec::new();
    for key in ["ipv4Prefix\"", "ipv6Prefix\""] {
        for start in memmem::find_iter(text.as_bytes(), key.as_bytes()) {
            if let Some(value) = string_value(&text[start + key.len()..]) {
                prefixes.push(value.parse::<IpNet>()?);
            }
        }
    }
    if prefixes.is_empty() {
        return Err(RangeError::NoPrefixes);
    }
    Ok(prefixes)
}

/// The string after `: "` at the start of `rest` (whitespace allowed around the colon).
fn string_value(rest: &str) -> Option<&str> {
    let rest = rest.trim_start().strip_prefix(':')?.trim_start();
    let rest = rest.strip_prefix('"')?;
    rest.find('"').map(|end| &rest[..end])
}

/// Prefixes of one source as downloaded at a point in time.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceRanges {
    /// Where the list was downloaded from.
    pub url: String,
    /// Download time, seconds since the Unix epoch.
    pub fetched_at: i64,
    /// CIDR prefixes.
    pub prefixes: Vec<String>,
}

/// All IP lists known at some point; stored as JSON.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RangeSnapshot {
    /// Lists by source id.
    pub sources: BTreeMap<String, SourceRanges>,
}

const BUNDLED_JSON: &str = include_str!("../data/ip-ranges.json");

impl RangeSnapshot {
    /// The snapshot compiled into the application (see `cargo xtask bot-ranges`).
    pub fn bundled() -> Self {
        // Validated by `bundled_snapshot_is_valid`; an empty snapshot only disables
        // list-based verification.
        Self::from_json(BUNDLED_JSON).unwrap_or_default()
    }

    /// Parses a snapshot file.
    ///
    /// # Errors
    /// [`RangeError::InvalidSnapshot`].
    pub fn from_json(text: &str) -> Result<Self, RangeError> {
        serde_json::from_str(text).map_err(|error| RangeError::InvalidSnapshot(error.to_string()))
    }

    /// Serializes the snapshot (one prefix per line keeps diffs reviewable).
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }

    /// Records a freshly downloaded list.
    pub fn insert(&mut self, source: &RangeSource, fetched_at: i64, prefixes: &[IpNet]) {
        self.sources.insert(
            source.id.to_owned(),
            SourceRanges {
                url: source.url.to_owned(),
                fetched_at,
                prefixes: prefixes.iter().map(ToString::to_string).collect(),
            },
        );
    }

    /// Records a list that is not one of [`SOURCES`] (custom bots).
    pub fn insert_custom(&mut self, id: String, url: String, fetched_at: i64, prefixes: &[IpNet]) {
        self.sources.insert(
            id,
            SourceRanges {
                url,
                fetched_at,
                prefixes: prefixes.iter().map(ToString::to_string).collect(),
            },
        );
    }

    /// Adds the static IP ranges of custom bots (validated) as `custom:<id>` lists. They never
    /// count as outdated.
    #[must_use]
    pub fn with_custom_ranges(mut self, bots: &[crate::CustomBot]) -> Self {
        for bot in bots.iter().filter(|bot| !bot.ip_ranges.is_empty()) {
            self.sources.insert(
                crate::custom::static_source_id(&bot.id),
                SourceRanges {
                    url: String::new(),
                    fetched_at: i64::MAX,
                    prefixes: bot.ip_ranges.clone(),
                },
            );
        }
        self
    }

    /// Combines two snapshots, keeping the more recent list of every source.
    #[must_use]
    pub fn merged_with(mut self, other: Self) -> Self {
        for (id, ranges) in other.sources {
            match self.sources.get(&id) {
                Some(existing) if existing.fetched_at >= ranges.fetched_at => {}
                _ => {
                    self.sources.insert(id, ranges);
                }
            }
        }
        self
    }

    /// Download time of the oldest list, seconds since the Unix epoch.
    pub fn oldest_fetch(&self) -> Option<i64> {
        self.sources.values().map(|s| s.fetched_at).min()
    }

    /// Lookup sets by source id; malformed prefixes are ignored.
    pub fn range_sets(&self) -> HashMap<String, IpRangeSet> {
        self.sources
            .iter()
            .map(|(id, ranges)| {
                let networks = ranges.prefixes.iter().filter_map(|p| p.parse().ok());
                (id.clone(), IpRangeSet::new(networks))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_published_json_html_and_bare_addresses() {
        let json = r#"{"creationTime": "x", "prefixes": [{"ipv4Prefix": "66.249.64.0/27"},
            {"ipv6Prefix" : "2001:4860:4801:10::/64"}]}"#;
        let parsed = parse_prefix_list(json).unwrap();
        assert_eq!(parsed.len(), 2);
        let html = r#"<script>var x = {"ipv4Prefix": "100.24.86.21"};</script>"#;
        assert_eq!(
            parse_prefix_list(html).unwrap()[0].to_string(),
            "100.24.86.21/32"
        );
        assert_eq!(
            parse_prefix_list("<html>404</html>"),
            Err(RangeError::NoPrefixes)
        );
        assert!(matches!(
            parse_prefix_list(r#"{"ipv4Prefix": "nope"}"#),
            Err(RangeError::InvalidPrefix(_))
        ));
    }

    #[test]
    fn merge_keeps_newest_list_per_source() {
        let net: IpNet = "10.0.0.0/8".parse().unwrap();
        let source = &SOURCES[0];
        let mut old = RangeSnapshot::default();
        old.insert(source, 100, &[net]);
        let mut new = RangeSnapshot::default();
        new.insert(source, 200, &[]);
        new.insert(&SOURCES[1], 50, &[net]);

        let merged = old.clone().merged_with(new.clone());
        assert_eq!(merged.sources[source.id].fetched_at, 200);
        assert_eq!(merged.oldest_fetch(), Some(50));
        let reverse = new.merged_with(old);
        assert_eq!(reverse.sources[source.id].fetched_at, 200);
        let json = merged.to_json();
        assert_eq!(RangeSnapshot::from_json(&json).unwrap(), merged);
    }

    #[test]
    fn bundled_snapshot_is_valid() {
        let snapshot = RangeSnapshot::from_json(BUNDLED_JSON).unwrap();
        for (id, ranges) in &snapshot.sources {
            assert!(SOURCES.iter().any(|s| s.id == id), "unknown source {id}");
            for prefix in &ranges.prefixes {
                assert!(prefix.parse::<IpNet>().is_ok(), "{id}: {prefix}");
            }
        }
    }
}
