//! Which AI crawlers a robots.txt lets in.
//!
//! The crawlers come from the bot catalog the Log Analyser verifies hits with, so both programs
//! agree on names, operators and the robots.txt groups each crawler obeys.

use seonexus_botverify::catalog::BOTS;
use seonexus_botverify::robots::robots_tokens;
use seonexus_botverify::{BotCategory, BotDef};
use seonexus_url::robots::Robots;
use serde::Serialize;

/// What an AI crawler fetches pages for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum AiPurpose {
    /// Indexing for AI search answers (OAI-SearchBot, PerplexityBot, Claude-SearchBot).
    Search,
    /// Reading a page because a user asked an assistant (ChatGPT-User, Claude-User).
    Assistant,
    /// Collecting training data (GPTBot, ClaudeBot, CCBot).
    Training,
}

impl AiPurpose {
    /// Lowercase name used in storage and exports.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Search => "search",
            Self::Assistant => "assistant",
            Self::Training => "training",
        }
    }

    fn of(category: BotCategory) -> Option<Self> {
        match category {
            BotCategory::AiSearch => Some(Self::Search),
            BotCategory::AiAssistant => Some(Self::Assistant),
            BotCategory::AiTraining => Some(Self::Training),
            _ => None,
        }
    }
}

/// A known AI crawler.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct AiCrawler {
    /// Catalog id, e.g. `oai-searchbot`.
    pub id: &'static str,
    /// Display name, e.g. `OAI-SearchBot`.
    pub name: &'static str,
    /// Company operating it.
    pub operator: &'static str,
    /// What it fetches pages for.
    pub purpose: AiPurpose,
}

impl AiCrawler {
    /// Every AI crawler of the bot catalog, in catalog order.
    pub fn all() -> impl Iterator<Item = (Self, &'static BotDef)> {
        BOTS.iter().filter_map(|bot| {
            AiPurpose::of(bot.category).map(|purpose| {
                (
                    Self {
                        id: bot.id,
                        name: bot.name,
                        operator: bot.operator,
                        purpose,
                    },
                    bot,
                )
            })
        })
    }
}

/// Whether one AI crawler may fetch the site's home page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AiAccess {
    /// The crawler.
    pub crawler: AiCrawler,
    /// robots.txt allows `/`.
    pub allowed: bool,
    /// The `User-agent` group it obeys (`*` for the default group); `None` without a group.
    pub group: Option<String>,
    /// The deciding rule as written (`disallow: /`), if any.
    pub rule: Option<String>,
}

/// Access of every AI crawler to `/` under `robots_txt` (an empty text allows everything).
pub fn ai_crawler_access(robots_txt: &str) -> Vec<AiAccess> {
    let robots = Robots::parse(robots_txt);
    AiCrawler::all()
        .map(|(crawler, bot)| {
            let identity = robots_tokens(bot);
            let tokens: Vec<&str> = identity.tokens.iter().map(String::as_str).collect();
            let rules = robots.group_for(&tokens, identity.uses_default);
            let decision = rules.decide("/");
            AiAccess {
                crawler,
                allowed: decision.allowed,
                group: rules.group.map(str::to_owned),
                rule: decision.rule.map(|rule| {
                    format!(
                        "{}: {}",
                        if rule.allow { "allow" } else { "disallow" },
                        rule.pattern
                    )
                }),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn access<'a>(all: &'a [AiAccess], id: &str) -> &'a AiAccess {
        all.iter().find(|a| a.crawler.id == id).unwrap()
    }

    #[test]
    fn reads_groups_per_crawler() {
        let all = ai_crawler_access(
            "User-agent: GPTBot\nDisallow: /\n\nUser-agent: *\nDisallow: /private/\n",
        );
        assert!(all.len() >= 10);
        let gpt = access(&all, "gptbot");
        assert!(!gpt.allowed);
        assert_eq!(gpt.group.as_deref(), Some("gptbot"));
        assert_eq!(gpt.rule.as_deref(), Some("disallow: /"));
        assert_eq!(gpt.crawler.purpose, AiPurpose::Training);

        let search = access(&all, "oai-searchbot");
        assert!(search.allowed);
        assert_eq!(search.group.as_deref(), Some("*"));
        assert_eq!(search.crawler.purpose, AiPurpose::Search);
        assert_eq!(
            access(&all, "claude-user").crawler.purpose,
            AiPurpose::Assistant
        );
    }

    #[test]
    fn empty_robots_allows_everyone() {
        assert!(
            ai_crawler_access("")
                .iter()
                .all(|a| a.allowed && a.group.is_none())
        );
        let blocked = ai_crawler_access("User-agent: *\nDisallow: /\n");
        assert!(blocked.iter().all(|a| !a.allowed));
    }

    #[test]
    fn ignored_records_preserve_shared_ai_crawler_rules() {
        let all = ai_crawler_access(
            "User-agent: GPTBot\nCrawl-delay: 10\nUser-agent: CCBot\nDisallow: /\n\
             User-agent: *\nAllow: /\n",
        );
        for id in ["gptbot", "ccbot"] {
            let crawler = access(&all, id);
            assert!(!crawler.allowed, "{id}");
            assert_eq!(crawler.group.as_deref(), Some(id));
            assert_eq!(crawler.rule.as_deref(), Some("disallow: /"));
        }
        assert!(access(&all, "oai-searchbot").allowed);
    }
}
