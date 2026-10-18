use std::fmt;

use serde::{Deserialize, Serialize};

/// What a bot is used for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BotCategory {
    /// Search engine crawlers (Googlebot, Bingbot, YandexBot, …).
    Search,
    /// Crawlers collecting AI training data (GPTBot, ClaudeBot, CCBot, …).
    AiTraining,
    /// Crawlers indexing for AI search products (OAI-SearchBot, PerplexityBot, …).
    AiSearch,
    /// Fetchers acting on a user's request in an AI assistant (ChatGPT-User, Claude-User, …).
    AiAssistant,
    /// Advertising quality and landing-page checks.
    Ads,
    /// SEO tools and backlink indexes.
    Seo,
    /// Link previews of social networks and messengers.
    Social,
    /// Uptime and performance monitoring.
    Monitoring,
    /// Everything else, including unidentified crawlers and HTTP libraries.
    Other,
}

impl BotCategory {
    /// All categories.
    pub const ALL: [Self; 9] = [
        Self::Search,
        Self::AiTraining,
        Self::AiSearch,
        Self::AiAssistant,
        Self::Ads,
        Self::Seo,
        Self::Social,
        Self::Monitoring,
        Self::Other,
    ];

    /// Stable machine name, e.g. `ai-training`.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Search => "search",
            Self::AiTraining => "ai-training",
            Self::AiSearch => "ai-search",
            Self::AiAssistant => "ai-assistant",
            Self::Ads => "ads",
            Self::Seo => "seo",
            Self::Social => "social",
            Self::Monitoring => "monitoring",
            Self::Other => "other",
        }
    }

    /// Looks a category up by name.
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|c| c.name() == name)
    }

    /// Whether the category belongs to AI products.
    pub const fn is_ai(self) -> bool {
        matches!(self, Self::AiTraining | Self::AiSearch | Self::AiAssistant)
    }
}

impl fmt::Display for BotCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_round_trip_and_match_serde() {
        for category in BotCategory::ALL {
            assert_eq!(BotCategory::from_name(category.name()), Some(category));
            let json = serde_json::to_string(&category).unwrap();
            assert_eq!(json, format!("\"{}\"", category.name()));
        }
    }
}
