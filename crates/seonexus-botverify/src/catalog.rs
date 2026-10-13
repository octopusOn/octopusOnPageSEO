//! Known bots: identity, user-agent patterns and verification sources.
//!
//! Order matters: the first bot whose pattern matches wins, so specific crawlers precede the
//! generic ones of the same operator and the catch-all entries come last.

use crate::category::BotCategory;
use crate::ranges::source;

/// Bumped whenever patterns or ids change, so stored classifications are recomputed.
pub const CATALOG_VERSION: u32 = 1;

/// A known bot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BotDef {
    /// Stable machine id stored with hits, e.g. `googlebot-smartphone`.
    pub id: &'static str,
    /// Display name.
    pub name: &'static str,
    /// Company or project operating the bot.
    pub operator: &'static str,
    /// What the bot is used for.
    pub category: BotCategory,
    /// Alternatives of lowercase substrings that must all occur in the user agent.
    pub patterns: &'static [&'static [&'static str]],
    /// Lowercase substrings that rule this bot out even if a pattern matches.
    pub excludes: &'static [&'static str],
    /// Ids of published IP lists (see [`ranges::SOURCES`](crate::ranges::SOURCES)).
    pub ip_lists: &'static [&'static str],
    /// Host name suffixes accepted by forward-confirmed reverse DNS.
    pub dns_suffixes: &'static [&'static str],
}

impl BotDef {
    /// Whether the operator publishes any means of verification.
    pub const fn is_verifiable(&self) -> bool {
        !self.ip_lists.is_empty() || !self.dns_suffixes.is_empty()
    }
}

/// Looks a bot up by id.
pub fn find(id: &str) -> Option<&'static BotDef> {
    BOTS.iter().find(|bot| bot.id == id)
}

const NONE: &[&str] = &[];

const GOOGLE: &str = "Google";
const GOOGLE_LISTS: &[&str] = &[
    source::GOOGLE_COMMON,
    source::GOOGLE_SPECIAL,
    source::GOOGLE_USER,
    source::GOOGLE_USER_GOOGLE,
];
const GOOGLE_DNS: &[&str] = &["googlebot.com", "google.com", "googleusercontent.com"];

const MICROSOFT: &str = "Microsoft";
const BING_LISTS: &[&str] = &[source::BING];
const BING_DNS: &[&str] = &["search.msn.com"];

const OPENAI: &str = "OpenAI";
const OPENAI_LISTS: &[&str] = &[
    source::OPENAI_GPTBOT,
    source::OPENAI_SEARCHBOT,
    source::OPENAI_CHATGPT_USER,
];

const ANTHROPIC: &str = "Anthropic";
const ANTHROPIC_LISTS: &[&str] = &[source::ANTHROPIC];

const PERPLEXITY: &str = "Perplexity";
const PERPLEXITY_LISTS: &[&str] = &[source::PERPLEXITY_BOT, source::PERPLEXITY_USER];

const DUCKDUCKGO: &str = "DuckDuckGo";
const DUCKDUCKGO_LISTS: &[&str] = &[source::DUCKDUCKGO_BOT, source::DUCKDUCKGO_ASSIST];

const AMAZON: &str = "Amazon";
const AMAZON_LISTS: &[&str] = &[source::AMAZONBOT];
const AMAZON_DNS: &[&str] = &["crawl.amazonbot.amazon"];

const META: &str = "Meta";
const YANDEX: &str = "Yandex";
const YANDEX_DNS: &[&str] = &["yandex.ru", "yandex.net", "yandex.com"];
const AHREFS: &str = "Ahrefs";
const AHREFS_LISTS: &[&str] = &[source::AHREFS];
const AHREFS_DNS: &[&str] = &["ahrefs.com", "ahrefs.net"];
const SEMRUSH: &str = "Semrush";
const MOZ: &str = "Moz";

/// Shorthand for the common case of a bot without exclusions.
const fn bot(
    id: &'static str,
    name: &'static str,
    operator: &'static str,
    category: BotCategory,
    patterns: &'static [&'static [&'static str]],
    ip_lists: &'static [&'static str],
    dns_suffixes: &'static [&'static str],
) -> BotDef {
    BotDef {
        id,
        name,
        operator,
        category,
        patterns,
        excludes: NONE,
        ip_lists,
        dns_suffixes,
    }
}

use BotCategory::{Ads, AiAssistant, AiSearch, AiTraining, Monitoring, Other, Search, Seo, Social};

/// Every known bot in matching order.
#[rustfmt::skip]
pub const BOTS: &[BotDef] = &[
    // Google — specific crawlers before Googlebot itself.
    bot("google-inspectiontool", "Google-InspectionTool", GOOGLE, Search, &[&["google-inspectiontool"]], GOOGLE_LISTS, GOOGLE_DNS),
    bot("googlebot-image", "Googlebot Image", GOOGLE, Search, &[&["googlebot-image"]], GOOGLE_LISTS, GOOGLE_DNS),
    bot("googlebot-video", "Googlebot Video", GOOGLE, Search, &[&["googlebot-video"]], GOOGLE_LISTS, GOOGLE_DNS),
    bot("googlebot-news", "Googlebot News", GOOGLE, Search, &[&["googlebot-news"]], GOOGLE_LISTS, GOOGLE_DNS),
    bot("google-storebot", "Storebot-Google", GOOGLE, Search, &[&["storebot-google"]], GOOGLE_LISTS, GOOGLE_DNS),
    bot("googleother-image", "GoogleOther-Image", GOOGLE, Other, &[&["googleother-image"]], GOOGLE_LISTS, GOOGLE_DNS),
    bot("googleother-video", "GoogleOther-Video", GOOGLE, Other, &[&["googleother-video"]], GOOGLE_LISTS, GOOGLE_DNS),
    bot("googleother", "GoogleOther", GOOGLE, Other, &[&["googleother"]], GOOGLE_LISTS, GOOGLE_DNS),
    bot("google-cloudvertexbot", "Google-CloudVertexBot", GOOGLE, AiSearch, &[&["google-cloudvertexbot"]], GOOGLE_LISTS, GOOGLE_DNS),
    bot("adsbot-google-mobile", "AdsBot-Google-Mobile", GOOGLE, Ads, &[&["adsbot-google-mobile"]], GOOGLE_LISTS, GOOGLE_DNS),
    bot("adsbot-google", "AdsBot-Google", GOOGLE, Ads, &[&["adsbot-google"]], GOOGLE_LISTS, GOOGLE_DNS),
    bot("mediapartners-google", "Mediapartners-Google", GOOGLE, Ads, &[&["mediapartners-google"]], GOOGLE_LISTS, GOOGLE_DNS),
    bot("google-safety", "Google-Safety", GOOGLE, Other, &[&["google-safety"]], GOOGLE_LISTS, GOOGLE_DNS),
    bot("apis-google", "APIs-Google", GOOGLE, Other, &[&["apis-google"]], GOOGLE_LISTS, GOOGLE_DNS),
    bot("feedfetcher-google", "FeedFetcher-Google", GOOGLE, Other, &[&["feedfetcher-google"]], GOOGLE_LISTS, GOOGLE_DNS),
    bot("google-read-aloud", "Google-Read-Aloud", GOOGLE, Other, &[&["google-read-aloud"]], GOOGLE_LISTS, GOOGLE_DNS),
    bot("google-site-verification", "Google-Site-Verification", GOOGLE, Other, &[&["google-site-verification"]], GOOGLE_LISTS, GOOGLE_DNS),
    bot("googleproducer", "GoogleProducer", GOOGLE, Other, &[&["googleproducer"]], GOOGLE_LISTS, GOOGLE_DNS),
    bot("google-pagerenderer", "Google-PageRenderer", GOOGLE, Other, &[&["google-pagerenderer"]], GOOGLE_LISTS, GOOGLE_DNS),
    bot("googlebot-smartphone", "Googlebot Smartphone", GOOGLE, Search, &[&["googlebot/", "mobile"], &["googlebot/", "android"]], GOOGLE_LISTS, GOOGLE_DNS),
    bot("googlebot-desktop", "Googlebot Desktop", GOOGLE, Search, &[&["googlebot"]], GOOGLE_LISTS, GOOGLE_DNS),
    // Microsoft
    bot("adidxbot", "AdIdxBot", MICROSOFT, Ads, &[&["adidxbot"]], BING_LISTS, BING_DNS),
    bot("bingpreview", "BingPreview", MICROSOFT, Other, &[&["bingpreview"]], BING_LISTS, BING_DNS),
    bot("microsoftpreview", "MicrosoftPreview", MICROSOFT, Social, &[&["microsoftpreview"]], BING_LISTS, BING_DNS),
    bot("bingbot", "Bingbot", MICROSOFT, Search, &[&["bingbot"], &["msnbot"]], BING_LISTS, BING_DNS),
    // OpenAI
    bot("oai-searchbot", "OAI-SearchBot", OPENAI, AiSearch, &[&["oai-searchbot"]], OPENAI_LISTS, NONE),
    bot("chatgpt-user", "ChatGPT-User", OPENAI, AiAssistant, &[&["chatgpt-user"]], OPENAI_LISTS, NONE),
    bot("gptbot", "GPTBot", OPENAI, AiTraining, &[&["gptbot"]], OPENAI_LISTS, NONE),
    // Anthropic
    bot("claude-searchbot", "Claude-SearchBot", ANTHROPIC, AiSearch, &[&["claude-searchbot"]], ANTHROPIC_LISTS, NONE),
    bot("claude-user", "Claude-User", ANTHROPIC, AiAssistant, &[&["claude-user"]], ANTHROPIC_LISTS, NONE),
    bot("claudebot", "ClaudeBot", ANTHROPIC, AiTraining, &[&["claudebot"], &["anthropic-ai"], &["claude-web"]], ANTHROPIC_LISTS, NONE),
    // Perplexity
    bot("perplexity-user", "Perplexity-User", PERPLEXITY, AiAssistant, &[&["perplexity-user"]], PERPLEXITY_LISTS, NONE),
    bot("perplexitybot", "PerplexityBot", PERPLEXITY, AiSearch, &[&["perplexitybot"]], PERPLEXITY_LISTS, NONE),
    // Other AI
    bot("mistralai-user", "MistralAI-User", "Mistral AI", AiAssistant, &[&["mistralai-user"]], &[source::MISTRAL_USER], NONE),
    bot("ccbot", "CCBot", "Common Crawl", AiTraining, &[&["ccbot"]], &[source::COMMONCRAWL], NONE),
    bot("meta-externalagent", "meta-externalagent", META, AiTraining, &[&["meta-externalagent"]], NONE, NONE),
    bot("meta-externalfetcher", "meta-externalfetcher", META, AiAssistant, &[&["meta-externalfetcher"]], NONE, NONE),
    bot("meta-webindexer", "meta-webindexer", META, AiSearch, &[&["meta-webindexer"]], NONE, NONE),
    bot("amzn-searchbot", "Amzn-SearchBot", AMAZON, AiSearch, &[&["amzn-searchbot"]], NONE, NONE),
    bot("amzn-user", "Amzn-User", AMAZON, AiAssistant, &[&["amzn-user"]], NONE, NONE),
    bot("amazonbot", "Amazonbot", AMAZON, AiTraining, &[&["amazonbot"]], AMAZON_LISTS, AMAZON_DNS),
    bot("bytespider", "Bytespider", "ByteDance", AiTraining, &[&["bytespider"]], NONE, NONE),
    bot("tiktokspider", "TikTokSpider", "ByteDance", AiTraining, &[&["tiktokspider"]], NONE, NONE),
    bot("cohere-training-data-crawler", "cohere-training-data-crawler", "Cohere", AiTraining, &[&["cohere-training-data-crawler"]], NONE, NONE),
    bot("cohere-ai", "cohere-ai", "Cohere", AiAssistant, &[&["cohere-ai"]], NONE, NONE),
    bot("ai2bot", "AI2Bot", "Allen Institute for AI", AiTraining, &[&["ai2bot"]], NONE, NONE),
    bot("diffbot", "Diffbot", "Diffbot", AiTraining, &[&["diffbot"]], NONE, NONE),
    bot("youbot", "YouBot", "You.com", AiSearch, &[&["youbot"]], NONE, NONE),
    bot("timpibot", "Timpibot", "Timpi", AiTraining, &[&["timpibot"]], NONE, NONE),
    bot("omgili", "omgili", "Webz.io", AiTraining, &[&["omgili"]], NONE, NONE),
    bot("imagesiftbot", "ImagesiftBot", "ImageSift", AiTraining, &[&["imagesiftbot"]], NONE, NONE),
    bot("pangubot", "PanguBot", "Huawei", AiTraining, &[&["pangubot"]], NONE, NONE),
    // Search engines
    bot("applebot", "Applebot", "Apple", Search, &[&["applebot"]], &[source::APPLE], &["applebot.apple.com"]),
    bot("duckassistbot", "DuckAssistBot", DUCKDUCKGO, AiAssistant, &[&["duckassistbot"]], DUCKDUCKGO_LISTS, NONE),
    bot("duckduckbot", "DuckDuckBot", DUCKDUCKGO, Search, &[&["duckduckbot"], &["duckduckgo-favicons-bot"]], DUCKDUCKGO_LISTS, NONE),
    bot("yandex-images", "YandexImages", YANDEX, Search, &[&["yandeximages"]], NONE, YANDEX_DNS),
    bot("yandexbot", "YandexBot", YANDEX, Search, &[&["yandexbot"], &["yandexmobilebot"]], NONE, YANDEX_DNS),
    bot("yandex-other", "Yandex (other)", YANDEX, Other, &[&["yandex.com/bots"]], NONE, YANDEX_DNS),
    bot("baiduspider", "Baiduspider", "Baidu", Search, &[&["baiduspider"]], NONE, &["baidu.com", "baidu.jp"]),
    bot("petalbot", "PetalBot", "Huawei", Search, &[&["petalbot"]], NONE, &["petalsearch.com"]),
    bot("seznambot", "SeznamBot", "Seznam", Search, &[&["seznambot"]], NONE, &["seznam.cz"]),
    bot("naver-yeti", "Yeti", "Naver", Search, &[&["yeti/", "naver"]], NONE, &["naver.com"]),
    bot("yahoo-slurp", "Yahoo! Slurp", "Yahoo", Search, &[&["yahoo! slurp"]], NONE, &["crawl.yahoo.net"]),
    bot("sogou", "Sogou Spider", "Sogou", Search, &[&["sogou web spider"], &["sogou inst spider"]], NONE, &["sogou.com"]),
    bot("qwant", "Qwantbot", "Qwant", Search, &[&["qwantbot"], &["qwantify"]], NONE, NONE),
    bot("mojeekbot", "MojeekBot", "Mojeek", Search, &[&["mojeekbot"]], NONE, NONE),
    bot("coccocbot", "coccocbot", "Coc Coc", Search, &[&["coccocbot"]], NONE, NONE),
    bot("360spider", "360Spider", "Qihoo 360", Search, &[&["360spider"]], NONE, NONE),
    // SEO tools
    bot("ahrefs-siteaudit", "AhrefsSiteAudit", AHREFS, Seo, &[&["ahrefssiteaudit"]], AHREFS_LISTS, AHREFS_DNS),
    bot("ahrefsbot", "AhrefsBot", AHREFS, Seo, &[&["ahrefsbot"]], AHREFS_LISTS, AHREFS_DNS),
    bot("semrush-siteaudit", "SiteAuditBot", SEMRUSH, Seo, &[&["siteauditbot"]], NONE, NONE),
    bot("semrushbot", "SemrushBot", SEMRUSH, Seo, &[&["semrushbot"]], NONE, NONE),
    bot("mj12bot", "MJ12bot", "Majestic", Seo, &[&["mj12bot"]], NONE, NONE),
    bot("dotbot", "DotBot", MOZ, Seo, &[&["dotbot"]], NONE, NONE),
    bot("rogerbot", "rogerbot", MOZ, Seo, &[&["rogerbot"]], NONE, NONE),
    bot("screaming-frog", "Screaming Frog SEO Spider", "Screaming Frog", Seo, &[&["screaming frog seo spider"]], NONE, NONE),
    bot("sitebulb", "Sitebulb", "Sitebulb", Seo, &[&["sitebulb"]], NONE, NONE),
    bot("dataforseobot", "DataForSeoBot", "DataForSEO", Seo, &[&["dataforseobot"]], NONE, NONE),
    bot("blexbot", "BLEXBot", "WebMeUp", Seo, &[&["blexbot"]], NONE, NONE),
    bot("serpstatbot", "serpstatbot", "Serpstat", Seo, &[&["serpstatbot"]], NONE, NONE),
    bot("barkrowler", "Barkrowler", "Babbar", Seo, &[&["barkrowler"]], NONE, NONE),
    bot("seokicks", "SEOkicks", "SEOkicks", Seo, &[&["seokicks"]], NONE, NONE),
    bot("oncrawl", "OnCrawl", "Oncrawl", Seo, &[&["oncrawl"]], NONE, NONE),
    bot("lumar", "Lumar", "Lumar", Seo, &[&["lumar"], &["deepcrawl"]], NONE, NONE),
    bot("botify", "Botify", "Botify", Seo, &[&["botify"]], NONE, NONE),
    bot("sistrix", "SISTRIX Crawler", "SISTRIX", Seo, &[&["sistrix"]], NONE, NONE),
    // Social and messaging previews
    bot("facebookexternalhit", "facebookexternalhit", META, Social, &[&["facebookexternalhit"], &["facebookcatalog"]], NONE, NONE),
    bot("twitterbot", "Twitterbot", "X", Social, &[&["twitterbot"]], NONE, NONE),
    bot("linkedinbot", "LinkedInBot", "LinkedIn", Social, &[&["linkedinbot"]], NONE, NONE),
    bot("slackbot", "Slackbot", "Slack", Social, &[&["slackbot"], &["slack-imgproxy"]], NONE, NONE),
    bot("discordbot", "Discordbot", "Discord", Social, &[&["discordbot"]], NONE, NONE),
    bot("telegrambot", "TelegramBot", "Telegram", Social, &[&["telegrambot"]], NONE, NONE),
    bot("whatsapp", "WhatsApp", META, Social, &[&["whatsapp/"]], NONE, NONE),
    bot("pinterestbot", "Pinterestbot", "Pinterest", Social, &[&["pinterestbot"], &["pinterest/0."]], NONE, NONE),
    bot("redditbot", "redditbot", "Reddit", Social, &[&["redditbot"]], NONE, NONE),
    bot("skypeuripreview", "SkypeUriPreview", MICROSOFT, Social, &[&["skypeuripreview"]], NONE, NONE),
    bot("embedly", "Embedly", "Embedly", Social, &[&["embedly"]], NONE, NONE),
    bot("iframely", "Iframely", "Iframely", Social, &[&["iframely"]], NONE, NONE),
    // Monitoring
    bot("uptimerobot", "UptimeRobot", "UptimeRobot", Monitoring, &[&["uptimerobot"]], NONE, NONE),
    bot("pingdom", "Pingdom", "SolarWinds", Monitoring, &[&["pingdom"]], NONE, NONE),
    bot("statuscake", "StatusCake", "StatusCake", Monitoring, &[&["statuscake"]], NONE, NONE),
    bot("site24x7", "Site24x7", "Zoho", Monitoring, &[&["site24x7"]], NONE, NONE),
    bot("betterstack", "Better Stack", "Better Stack", Monitoring, &[&["better uptime"], &["betterstack"]], NONE, NONE),
    bot("uptime-kuma", "Uptime Kuma", "Uptime Kuma", Monitoring, &[&["uptime-kuma"]], NONE, NONE),
    bot("chrome-lighthouse", "Lighthouse", GOOGLE, Monitoring, &[&["chrome-lighthouse"]], NONE, NONE),
    bot("gtmetrix", "GTmetrix", "GTmetrix", Monitoring, &[&["gtmetrix"]], NONE, NONE),
    bot("datadog-synthetics", "Datadog Synthetics", "Datadog", Monitoring, &[&["datadogsynthetics"]], NONE, NONE),
    bot("newrelic", "New Relic Synthetics", "New Relic", Monitoring, &[&["newrelicpinger"], &["newrelic synthetics"]], NONE, NONE),
    // Catch-all entries.
    bot("headless-browser", "Headless browser", "", Other, &[&["headlesschrome"], &["phantomjs"], &["puppeteer"], &["playwright"], &["selenium"]], NONE, NONE),
    bot(
        "http-library",
        "HTTP library",
        "",
        Other,
        &[
            &["python-requests"], &["python-urllib"], &["python-httpx"], &["aiohttp"],
            &["go-http-client"], &["curl/"], &["wget/"], &["java/"], &["okhttp"], &["axios/"],
            &["node-fetch"], &["undici"], &["libwww-perl"], &["guzzlehttp"],
            &["apache-httpclient"], &["scrapy"], &["httpclient"], &["http.rb"], &["ruby"],
        ],
        NONE,
        NONE,
    ),
    BotDef {
        id: "unknown-crawler",
        name: "Unidentified crawler",
        operator: "",
        category: Other,
        patterns: &[
            &["bot/"], &["bot;"], &["bot)"], &["bot "], &["-bot"], &["_bot"], &["crawler"],
            &["spider"], &["scraper"], &["+http"], &["http://"], &["https://"],
        ],
        // Phone models whose names end in "bot".
        excludes: &["cubot"],
        ip_lists: NONE,
        dns_suffixes: NONE,
    },
];

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;
    use crate::ranges::SOURCES;

    #[test]
    fn ids_are_unique_and_patterns_are_lowercase() {
        let mut ids = HashSet::new();
        for bot in BOTS {
            assert!(ids.insert(bot.id), "duplicate id {}", bot.id);
            assert!(!bot.patterns.is_empty(), "{}", bot.id);
            for token in bot
                .patterns
                .iter()
                .flat_map(|p| p.iter())
                .chain(bot.excludes)
            {
                assert!(!token.is_empty(), "{}", bot.id);
                assert_eq!(*token, token.to_lowercase(), "{}", bot.id);
            }
        }
    }

    #[test]
    fn verification_sources_exist() {
        for bot in BOTS {
            for list in bot.ip_lists {
                assert!(
                    SOURCES.iter().any(|s| s.id == *list),
                    "{}: unknown list {list}",
                    bot.id
                );
            }
            for suffix in bot.dns_suffixes {
                assert!(!suffix.starts_with('.') && suffix.contains('.'), "{suffix}");
            }
        }
        assert!(find("googlebot-smartphone").is_some_and(BotDef::is_verifiable));
        assert!(find("bytespider").is_some_and(|b| !b.is_verifiable()));
    }
}
