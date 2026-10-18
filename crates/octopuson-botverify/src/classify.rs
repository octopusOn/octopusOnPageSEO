//! User-agent → bot classification.

use std::sync::{Mutex, PoisonError};

use crate::catalog::{BOTS, BotDef, CATALOG_VERSION};
use crate::custom::{CustomBot, leak_def};

/// Identifies bots from user-agent strings using the [catalog](crate::catalog), optionally
/// preceded by [custom bots](crate::custom).
///
/// Classification is pure and deterministic; callers cache results per distinct user agent.
#[derive(Debug, Clone, Copy)]
pub struct Classifier {
    bots: &'static [BotDef],
    version: u32,
}

impl Default for Classifier {
    fn default() -> Self {
        Self {
            bots: BOTS,
            version: CATALOG_VERSION,
        }
    }
}

/// Catalogs built for distinct custom bot sets, so repeated loads reuse them.
static INTERNED: Mutex<Vec<(u64, &'static [BotDef])>> = Mutex::new(Vec::new());

impl Classifier {
    /// The catalog preceded by `custom` (validated with
    /// [`validate_custom_bots`](crate::validate_custom_bots)), so custom bots win.
    pub fn with_custom(custom: &[CustomBot]) -> Self {
        if custom.is_empty() {
            return Self::default();
        }
        let mut key_source = CATALOG_VERSION.to_le_bytes().to_vec();
        key_source.extend(serde_json::to_vec(custom).unwrap_or_default());
        let key = fnv1a(&key_source);
        let mut interned = INTERNED.lock().unwrap_or_else(PoisonError::into_inner);
        let bots = if let Some((_, bots)) = interned.iter().find(|(k, _)| *k == key) {
            *bots
        } else {
            let all: Vec<BotDef> = custom
                .iter()
                .map(leak_def)
                .chain(BOTS.iter().copied())
                .collect();
            let bots: &'static [BotDef] = Box::leak(all.into_boxed_slice());
            interned.push((key, bots));
            bots
        };
        Self {
            bots,
            // Never equal to a plain catalog version, and stable across processes.
            version: u32::try_from(key >> 33).unwrap_or(0) | 0x8000_0000,
        }
    }

    /// Classification rules version; changes with the catalog and the custom bots, so stored
    /// classifications are recomputed.
    pub fn version(&self) -> u32 {
        self.version
    }

    /// Every bot in matching order.
    pub fn bots(&self) -> &'static [BotDef] {
        self.bots
    }

    /// Looks a bot up by id.
    pub fn find(&self, id: &str) -> Option<&'static BotDef> {
        self.bots.iter().find(|bot| bot.id == id)
    }

    /// The bot a user agent belongs to, or `None` for browsers and empty values.
    pub fn classify(&self, user_agent: &str) -> Option<&'static BotDef> {
        let user_agent = user_agent.trim();
        if user_agent.is_empty() || user_agent == "-" {
            return None;
        }
        let lower = user_agent.to_ascii_lowercase();
        self.bots.iter().find(|bot| matches(bot, &lower))
    }
}

/// 64-bit FNV-1a, a stable hash (unlike `std`'s) for the version number.
fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |hash, &byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

fn matches(bot: &BotDef, user_agent: &str) -> bool {
    bot.patterns
        .iter()
        .any(|tokens| tokens.iter().all(|token| user_agent.contains(token)))
        && !bot.excludes.iter().any(|token| user_agent.contains(token))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn custom_bots_precede_the_catalog_and_change_the_version() {
        let custom = crate::validate_custom_bots(vec![CustomBot {
            id: "acme-monitor".into(),
            category: "monitoring".into(),
            patterns: vec!["acmemonitor".into(), "googlebot/9".into()],
            ..CustomBot::default()
        }])
        .unwrap();
        let classifier = Classifier::with_custom(&custom);
        assert_eq!(
            classifier.classify("AcmeMonitor/2.0").map(|b| b.id),
            Some("acme-monitor")
        );
        // Matched before the built-in Googlebot entry.
        assert_eq!(
            classifier.classify("Googlebot/9.0").map(|b| b.id),
            Some("acme-monitor")
        );
        assert_eq!(
            classifier.classify("Googlebot/2.1").map(|b| b.id),
            Some("googlebot-desktop")
        );
        assert_eq!(
            classifier.find("acme-monitor").map(|b| b.name),
            Some("acme-monitor")
        );
        assert_ne!(classifier.version(), CATALOG_VERSION);
        // Interned: the same definitions give the same catalog and version.
        let again = Classifier::with_custom(&custom);
        assert!(std::ptr::eq(again.bots(), classifier.bots()));
        assert_eq!(again.version(), classifier.version());
        assert_eq!(Classifier::with_custom(&[]).version(), CATALOG_VERSION);
    }

    fn id(user_agent: &str) -> Option<&'static str> {
        Classifier::default().classify(user_agent).map(|bot| bot.id)
    }

    #[test]
    fn identifies_google_crawlers_precisely() {
        let cases = [
            (
                "Mozilla/5.0 (Linux; Android 6.0.1; Nexus 5X Build/MMB29P) AppleWebKit/537.36 \
                 (KHTML, like Gecko) Chrome/141.0.7390.122 Mobile Safari/537.36 \
                 (compatible; Googlebot/2.1; +http://www.google.com/bot.html)",
                "googlebot-smartphone",
            ),
            (
                "Mozilla/5.0 (compatible; Googlebot/2.1; +http://www.google.com/bot.html)",
                "googlebot-desktop",
            ),
            (
                "Mozilla/5.0 AppleWebKit/537.36 (KHTML, like Gecko; compatible; Googlebot/2.1; \
                 +http://www.google.com/bot.html) Chrome/141.0.7390.122 Safari/537.36",
                "googlebot-desktop",
            ),
            ("Googlebot-Image/1.0", "googlebot-image"),
            (
                "Mozilla/5.0 (compatible; Google-InspectionTool/1.0;)",
                "google-inspectiontool",
            ),
            (
                "Mozilla/5.0 (Linux; Android 5.0; SM-G920A) AppleWebKit (KHTML, like Gecko) \
                 Chrome Mobile Safari (compatible; AdsBot-Google-Mobile; \
                 +http://www.google.com/mobile/adsbot.html)",
                "adsbot-google-mobile",
            ),
            (
                "AdsBot-Google (+http://www.google.com/adsbot.html)",
                "adsbot-google",
            ),
        ];
        for (user_agent, expected) in cases {
            assert_eq!(id(user_agent), Some(expected), "{user_agent}");
        }
    }

    #[test]
    fn identifies_ai_and_other_crawlers() {
        let cases = [
            (
                "Mozilla/5.0 AppleWebKit/537.36 (KHTML, like Gecko; compatible; GPTBot/1.2; \
                 +https://openai.com/gptbot)",
                "gptbot",
            ),
            (
                "Mozilla/5.0 AppleWebKit/537.36 (KHTML, like Gecko); compatible; \
                 ChatGPT-User/1.0; +https://openai.com/bot",
                "chatgpt-user",
            ),
            (
                "Mozilla/5.0 AppleWebKit/537.36 (KHTML, like Gecko; compatible; ClaudeBot/1.0; \
                 +claudebot@anthropic.com)",
                "claudebot",
            ),
            (
                "Mozilla/5.0 (compatible; Claude-SearchBot/1.0)",
                "claude-searchbot",
            ),
            (
                "Mozilla/5.0 (compatible; PerplexityBot/1.0)",
                "perplexitybot",
            ),
            (
                "Mozilla/5.0 (compatible; bingbot/2.0; +http://www.bing.com/bingbot.htm)",
                "bingbot",
            ),
            (
                "Mozilla/5.0 (compatible; YandexBot/3.0; +http://yandex.com/bots)",
                "yandexbot",
            ),
            (
                "Mozilla/5.0 (compatible; AhrefsBot/7.0; +http://ahrefs.com/robot/)",
                "ahrefsbot",
            ),
            ("Screaming Frog SEO Spider/23.0", "screaming-frog"),
            ("facebookexternalhit/1.1", "facebookexternalhit"),
            ("python-requests/2.32.3", "http-library"),
            ("curl/8.7.1", "http-library"),
            (
                "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) \
                 HeadlessChrome/141.0.0.0 Safari/537.36",
                "headless-browser",
            ),
            (
                "FancyNewBot/0.1 (+https://example.com/bot)",
                "unknown-crawler",
            ),
        ];
        for (user_agent, expected) in cases {
            assert_eq!(id(user_agent), Some(expected), "{user_agent}");
        }
    }

    #[test]
    fn browsers_and_empty_values_are_not_bots() {
        for user_agent in [
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) \
             Chrome/141.0.0.0 Safari/537.36",
            "Mozilla/5.0 (iPhone; CPU iPhone OS 18_6 like Mac OS X) AppleWebKit/605.1.15 \
             (KHTML, like Gecko) Version/18.6 Mobile/15E148 Safari/604.1",
            "Mozilla/5.0 (Linux; Android 12; CUBOT KingKong 7) AppleWebKit/537.36 \
             (KHTML, like Gecko) Chrome/141.0.0.0 Mobile Safari/537.36",
            "",
            "-",
            "   ",
        ] {
            assert_eq!(id(user_agent), None, "{user_agent}");
        }
    }
}
