//! Built-in report templates.
//!
//! Each template is plain query text, so users can open it, tweak it and save their own
//! version. Search-engine templates count verified crawler hits only (ADR 0005).

/// A built-in query.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Template {
    /// Stable identifier used by the CLI and saved settings.
    pub id: &'static str,
    /// Short English title (translated by front-ends).
    pub title: &'static str,
    /// What the report answers.
    pub description: &'static str,
    /// Query text.
    pub query: &'static str,
}

/// All built-in templates in menu order.
pub const TEMPLATES: &[Template] = &[
    Template {
        id: "top-urls",
        title: "Most requested URLs",
        description: "The 500 URLs with the most hits.",
        query: "group:url metric:hits,bytes,last_seen sort:hits:desc limit:500",
    },
    Template {
        id: "url-summary",
        title: "URL crawl summary",
        description: "Per URL: hits, last status and crawl, days since the last request, size, speed and hits per major bot.",
        query: "group:url,content_type metric:hits,last_status,last_seen,days_since_last,bytes,avg_bytes,avg_ms,bot_hits,googlebot_hits,bingbot_hits sort:hits:desc limit:1000",
    },
    Template {
        id: "response-codes",
        title: "Response codes per URL",
        description: "Last status, whether responses were inconsistent and hits per status class for every URL.",
        query: "group:url metric:last_status,last_seen,hits,inconsistent,hits_1xx,hits_2xx,hits_3xx,hits_4xx,hits_5xx sort:hits:desc limit:1000",
    },
    Template {
        id: "status-codes",
        title: "Status codes",
        description: "Hits and distinct URLs per HTTP status code.",
        query: "group:status metric:hits,unique_urls sort:hits:desc",
    },
    Template {
        id: "daily-status",
        title: "Daily hits by status class",
        description: "Request volume per day split into 2xx/3xx/4xx/5xx.",
        query: "group:day,status_class metric:hits sort:day:asc,status_class:asc",
    },
    Template {
        id: "hourly-hits",
        title: "Hourly hits",
        description: "Request volume and bytes per hour.",
        query: "group:hour metric:hits,bytes sort:hour:asc",
    },
    Template {
        id: "server-errors",
        title: "Pages returning 5xx",
        description: "URLs with server errors, most frequent first.",
        query: "status:5xx group:url,status metric:hits,last_seen sort:hits:desc limit:500",
    },
    Template {
        id: "client-errors",
        title: "Pages returning 4xx",
        description: "Broken or missing URLs, most frequent first.",
        query: "status:4xx group:url,status metric:hits,last_seen sort:hits:desc limit:500",
    },
    Template {
        id: "redirects",
        title: "Redirected URLs",
        description: "URLs answered with 3xx; candidates for internal link fixes.",
        query: "status:3xx group:url,status metric:hits,last_seen sort:hits:desc limit:500",
    },
    Template {
        id: "bots",
        title: "Bot overview",
        description: "Hits, distinct URLs and last visit per bot and verification verdict.",
        query: "bot:* group:bot,verification metric:hits,unique_urls,last_seen sort:hits:desc",
    },
    Template {
        id: "bot-categories",
        title: "Traffic by bot category",
        description: "Search, AI, SEO-tool and other bot traffic next to browsers (empty category).",
        query: "group:bot_category metric:hits,bytes,unique_urls sort:hits:desc",
    },
    Template {
        id: "googlebot-daily",
        title: "Googlebot crawl volume",
        description: "Daily hits and distinct URLs of verified Googlebot crawlers.",
        query: "bot:googlebot* verification:verified group:day metric:hits,unique_urls sort:day:asc",
    },
    Template {
        id: "googlebot-devices",
        title: "Googlebot smartphone vs desktop",
        description: "Daily verified hits of the mobile and desktop Googlebot (mobile-first indexing).",
        query: "bot:googlebot-smartphone,googlebot-desktop verification:verified group:day,bot \
metric:hits sort:day:asc,bot:asc",
    },
    Template {
        id: "googlebot-urls",
        title: "Googlebot crawl frequency",
        description: "Verified Googlebot hits, first and last crawl per URL.",
        query: "bot:googlebot* verification:verified group:url metric:hits,first_seen,last_seen \
sort:hits:desc limit:1000",
    },
    Template {
        id: "stale-urls",
        title: "URLs not crawled recently",
        description: "Verified search-engine crawling per URL, longest time since the last crawl first.",
        query: "bot_category:search verification:verified group:url metric:days_since_last,last_seen,last_status,hits sort:days_since_last:desc limit:1000",
    },
    Template {
        id: "googlebot-errors",
        title: "Googlebot 4xx/5xx",
        description: "URLs where verified Googlebot received an error.",
        query: "bot:googlebot* verification:verified status:>=400 group:url,status \
metric:hits,last_seen sort:hits:desc limit:500",
    },
    Template {
        id: "bingbot-daily",
        title: "Bingbot crawl volume",
        description: "Daily hits and distinct URLs of verified Bingbot.",
        query: "bot:bingbot verification:verified group:day metric:hits,unique_urls sort:day:asc",
    },
    Template {
        id: "ai-bots",
        title: "AI crawler activity",
        description: "Daily hits per AI crawler (training, AI search and assistant fetchers).",
        query: "bot_category:ai-* group:bot,day metric:hits sort:day:asc,hits:desc",
    },
    Template {
        id: "ai-bot-urls",
        title: "URLs fetched by AI crawlers",
        description: "Which pages AI crawlers and assistants request, by bot.",
        query: "bot_category:ai-* group:url,bot metric:hits,last_seen sort:hits:desc limit:500",
    },
    Template {
        id: "ai-bot-errors",
        title: "AI bot errors",
        description: "Errors served to each AI bot; every failed fetch by an assistant is a missed citation.",
        query: "bot_category:ai-* group:bot metric:hits,errors,hits_4xx,hits_5xx,unique_urls sort:errors:desc",
    },
    Template {
        id: "ai-referrals",
        title: "Visits from AI assistants",
        description: "Human visits referred by ChatGPT, Perplexity, Gemini, Copilot, Claude and others per day.",
        query: "referer_type:ai -bot:* group:day,referer_host metric:hits,unique_urls sort:day:asc,hits:desc",
    },
    Template {
        id: "ai-referral-pages",
        title: "Pages visited from AI assistants",
        description: "Landing pages of visits referred by AI assistants.",
        query: "referer_type:ai -bot:* group:url,referer_host metric:hits,first_seen,last_seen sort:hits:desc limit:500",
    },
    Template {
        id: "traffic-sources",
        title: "Human page views by source",
        description: "Browser page views per day by referer kind: search, AI assistants, social, internal, direct.",
        query: "content_type:html -bot:* group:day,referer_type metric:hits sort:day:asc,referer_type:asc",
    },
    Template {
        id: "bot-bandwidth",
        title: "Bandwidth and CO2 by bot",
        description: "Bytes, average response size, estimated emissions and speed per bot.",
        query: "bot:* group:bot metric:hits,bytes,avg_bytes,co2_mg,avg_ms sort:bytes:desc",
    },
    Template {
        id: "spoofed-bots",
        title: "Spoofed bots",
        description: "IPs claiming to be a verifiable bot but failing verification (raw data).",
        query: "verification:spoofed group:bot,ip metric:hits,unique_urls,last_seen \
sort:hits:desc limit:500",
    },
    Template {
        id: "inconsistent-status",
        title: "Inconsistent response codes",
        description: "URLs answered with more than one status code, most inconsistent first.",
        query: "group:url metric:unique_statuses,status_codes,hits,last_seen \
sort:unique_statuses:desc,hits:desc limit:500",
    },
    Template {
        id: "slow-urls",
        title: "Slowest URLs",
        description: "URLs by 95th percentile response time.",
        query: "group:url metric:hits,avg_ms,p95_ms,max_ms sort:p95_ms:desc limit:500",
    },
    Template {
        id: "parameter-urls",
        title: "Parameterized URLs",
        description: "Paths crawled with query strings; a common source of crawl waste.",
        query: "params:true group:path metric:hits,unique_urls sort:hits:desc limit:500",
    },
    Template {
        id: "directories",
        title: "Directories",
        description: "Hits, distinct URLs and bytes per top-level directory.",
        query: "group:directory metric:hits,unique_urls,bytes,avg_bytes,avg_ms,bot_hits,googlebot_hits,bingbot_hits,hits_3xx,hits_4xx,hits_5xx sort:hits:desc",
    },
    Template {
        id: "content-types",
        title: "Content types",
        description: "Hits, URLs, bytes and speed per content type guessed from the extension.",
        query: "group:content_type metric:hits,unique_urls,bytes,avg_bytes,avg_ms sort:hits:desc",
    },
    Template {
        id: "large-urls",
        title: "Largest responses",
        description: "URLs by average response size, with total bytes and estimated emissions.",
        query: "group:url metric:hits,bytes,avg_bytes,co2_mg sort:avg_bytes:desc limit:500",
    },
    Template {
        id: "static-assets",
        title: "Static asset requests",
        description: "Share of requests spent on scripts, styles, images, fonts and Next.js assets.",
        query: "(path:~\"\\.(js|css|png|jpe?g|gif|svg|webp|avif|ico|woff2?)$\" OR path:/_next/*) \
group:directory metric:hits,bytes sort:hits:desc",
    },
    Template {
        id: "new-urls",
        title: "Recently discovered URLs",
        description: "URLs by the time they were first requested, newest first.",
        query: "group:url metric:first_seen,hits sort:first_seen:desc limit:500",
    },
    Template {
        id: "top-user-agents",
        title: "Top user agents",
        description: "User agents with the most hits and the bot they belong to.",
        query: "group:ua,bot metric:hits,unique_urls,avg_bytes,avg_ms,errors,hits_3xx,hits_4xx,hits_5xx sort:hits:desc limit:200",
    },
    Template {
        id: "referers",
        title: "Referers",
        description: "Referring pages with the URLs they sent requests to and errors (raw data).",
        query: "-referer_type:direct group:referer,referer_type metric:hits,unique_urls,errors sort:hits:desc limit:500",
    },
    Template {
        id: "top-ips",
        title: "Top client IPs",
        description: "Client IPs with the most hits (raw data; slower on large sites).",
        query: "group:ip metric:hits,unique_urls,bytes,avg_ms,errors sort:hits:desc limit:200",
    },
    Template {
        id: "countries",
        title: "Countries",
        description: "Hits, client IPs and URLs per country of the client IP (raw data; needs the IP geolocation data).",
        query: "group:country metric:hits,unique_ips,unique_urls,errors sort:hits:desc",
    },
    Template {
        id: "networks",
        title: "Networks (ASN)",
        description: "Hits per autonomous system: data centres and clouds hosting scrapers stand out (raw data).",
        query: "group:asn,as_org metric:hits,unique_ips,unique_urls,bot_hits sort:hits:desc limit:200",
    },
    Template {
        id: "spoofed-bot-networks",
        title: "Networks of fake bots",
        description: "Where requests posing as verifiable bots really come from, by network and country (raw data).",
        query: "verification:spoofed group:bot,as_org,country metric:hits,unique_ips sort:hits:desc limit:200",
    },
];

/// Looks a template up by id.
pub fn find(id: &str) -> Option<&'static Template> {
    TEMPLATES.iter().find(|t| t.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sql::{CompileContext, compile};

    #[test]
    fn every_template_parses_compiles_and_round_trips() {
        let ctx = CompileContext {
            today: octopuson_core::Date::parse("2026-10-12").unwrap(),
        };
        for template in TEMPLATES {
            let query = crate::parse(template.query)
                .unwrap_or_else(|e| panic!("{}: {e} at {:?}", template.id, e.span()));
            compile(&query, ctx).unwrap_or_else(|e| panic!("{}: {e}", template.id));
            assert_eq!(
                crate::parse(&crate::to_dsl(&query)).unwrap(),
                query,
                "{}",
                template.id
            );
        }
    }

    #[test]
    fn ids_are_unique() {
        for (index, template) in TEMPLATES.iter().enumerate() {
            assert!(
                TEMPLATES[index + 1..].iter().all(|t| t.id != template.id),
                "{}",
                template.id
            );
        }
    }
}
