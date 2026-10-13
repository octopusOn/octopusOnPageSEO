//! Which robots.txt `User-agent` groups a bot obeys.
//!
//! The product token usually equals the bot's name (`GPTBot`, `ClaudeBot`); the exceptions
//! below follow the operators' documentation: Google's specialised crawlers fall back to the
//! `Googlebot` group, AdsBot and Mediapartners ignore the `*` group, Applebot falls back to
//! `Googlebot` rules, and every Yandex robot obeys the `Yandex` group.

use crate::catalog::BotDef;
use crate::category::BotCategory;

/// Tokens of a bot, most specific first, and whether it falls back to the `*` group.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RobotsTokens {
    /// Lowercase product tokens.
    pub tokens: Vec<String>,
    /// Whether the `*` group applies when no group names the bot.
    pub uses_default: bool,
    /// Fetches triggered by a user (assistants answering a question, feed readers); their
    /// operators may not apply robots.txt to them.
    pub user_triggered: bool,
}

/// robots.txt identity of `bot`.
pub fn robots_tokens(bot: &BotDef) -> RobotsTokens {
    let fixed: &[&str] = match bot.id {
        "googlebot-smartphone" | "googlebot-desktop" | "google-inspectiontool" => &["googlebot"],
        "googlebot-image" => &["googlebot-image", "googlebot"],
        "googlebot-video" => &["googlebot-video", "googlebot"],
        "googlebot-news" => &["googlebot-news", "googlebot"],
        "google-storebot" => &["storebot-google"],
        "googleother-image" => &["googleother-image", "googleother"],
        "googleother-video" => &["googleother-video", "googleother"],
        "adsbot-google-mobile" => &["adsbot-google-mobile", "adsbot-google"],
        "adidxbot" | "bingpreview" | "microsoftpreview" => &["bingbot"],
        "applebot" => &["applebot", "googlebot"],
        "yandexbot" => &["yandexbot", "yandex"],
        "yandex-images" => &["yandeximages", "yandex"],
        "yandex-other" => &["yandex"],
        "naver-yeti" => &["yeti"],
        "yahoo-slurp" => &["slurp"],
        "screaming-frog" => &["screaming frog seo spider"],
        _ => &[],
    };
    let mut tokens: Vec<String> = fixed.iter().map(|t| (*t).to_owned()).collect();
    if tokens.is_empty() {
        tokens.push(bot.name.to_ascii_lowercase().replace(' ', "-"));
        if bot.id != tokens[0] {
            tokens.push(bot.id.to_owned());
        }
    }
    RobotsTokens {
        tokens,
        uses_default: !matches!(
            bot.id,
            "adsbot-google" | "adsbot-google-mobile" | "mediapartners-google"
        ),
        user_triggered: bot.category == BotCategory::AiAssistant
            || matches!(
                bot.id,
                "feedfetcher-google" | "google-read-aloud" | "google-site-verification"
            ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::find;

    fn tokens(id: &str) -> Vec<String> {
        robots_tokens(find(id).unwrap()).tokens
    }

    #[test]
    fn tokens_follow_operator_documentation() {
        assert_eq!(tokens("gptbot"), ["gptbot"]);
        assert_eq!(tokens("chatgpt-user"), ["chatgpt-user"]);
        assert_eq!(tokens("googlebot-image"), ["googlebot-image", "googlebot"]);
        assert_eq!(tokens("applebot"), ["applebot", "googlebot"]);
        assert!(!robots_tokens(find("adsbot-google").unwrap()).uses_default);
        assert!(robots_tokens(find("chatgpt-user").unwrap()).user_triggered);
        assert!(!robots_tokens(find("gptbot").unwrap()).user_triggered);
    }
}
