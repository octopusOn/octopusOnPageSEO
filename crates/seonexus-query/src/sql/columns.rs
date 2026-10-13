//! SQL expressions for fields, dimensions, metrics and listing columns per [`Source`].
//!
//! The base table is aliased `t`; `u` is `urls`, `a` is `user_agents` and `v` is `bot_ips`,
//! joined only when an expression needs them. Every string here is a constant, so no user
//! input reaches SQL text. Long expressions are assembled with `concat!` from macros that
//! expand to string literals, so they stay compile-time constants.

use seonexus_core::table::ColumnKind;

use super::source::Source;
use crate::field::Field;
use crate::measure::{Dimension, Metric};

/// Dimension tables an expression reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "independent flags, one per joinable table"
)]
pub(super) struct Joins {
    pub(super) urls: bool,
    pub(super) user_agents: bool,
    /// Verdicts of raw hits; implies `user_agents` (joined on `ip` and `a.bot`).
    pub(super) bot_ips: bool,
    /// Country and autonomous system of raw hits' IPs (`ip_geo g`).
    pub(super) geo: bool,
}

impl Joins {
    pub(super) fn union(self, other: Self) -> Self {
        Self {
            urls: self.urls || other.urls,
            user_agents: self.user_agents || other.user_agents,
            bot_ips: self.bot_ips || other.bot_ips,
            geo: self.geo || other.geo,
        }
    }
}

/// A constant SQL expression plus the joins it requires.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct SqlExpr {
    pub(super) sql: &'static str,
    pub(super) joins: Joins,
}

const NO_JOINS: Joins = Joins {
    urls: false,
    user_agents: false,
    bot_ips: false,
    geo: false,
};

const fn on_geo(sql: &'static str) -> SqlExpr {
    SqlExpr {
        sql,
        joins: Joins {
            geo: true,
            ..NO_JOINS
        },
    }
}

const COUNTRY: SqlExpr = on_geo("g.country");
const ASN: SqlExpr = on_geo("g.asn");
const AS_ORG: SqlExpr = on_geo("g.as_org");

const fn plain(sql: &'static str) -> SqlExpr {
    SqlExpr {
        sql,
        joins: NO_JOINS,
    }
}

const fn on_url(sql: &'static str) -> SqlExpr {
    SqlExpr {
        sql,
        joins: Joins {
            urls: true,
            ..NO_JOINS
        },
    }
}

const fn on_ua(sql: &'static str) -> SqlExpr {
    SqlExpr {
        sql,
        joins: Joins {
            user_agents: true,
            ..NO_JOINS
        },
    }
}

const URL: SqlExpr = on_url("u.raw_url");
const PATH: SqlExpr = on_url("split_part(u.raw_url, '?', 1)");
const DIRECTORY: SqlExpr =
    on_url("coalesce(nullif(regexp_extract(u.raw_url, '^/[^/?#]*/'), ''), '/')");
const PARAMS: SqlExpr = on_url("(strpos(u.raw_url, '?') > 0)");
const UA: SqlExpr = on_ua("a.raw");
const BOT: SqlExpr = on_ua("a.bot");
const BOT_CATEGORY: SqlExpr = on_ua("a.bot_category");
/// Raw hits look verdicts up per `(ip, bot)`; rollups store them as a key.
const HIT_VERIFICATION: SqlExpr = SqlExpr {
    sql: "v.verdict",
    joins: Joins {
        user_agents: true,
        bot_ips: true,
        urls: false,
        geo: false,
    },
};
const ROLLUP_VERIFICATION: SqlExpr = plain("t.verification");
const STATUS: SqlExpr = plain("t.status");
const STATUS_CLASS: SqlExpr = plain("CAST(t.status // 100 AS VARCHAR) || 'xx'");

/// Lower-cased extension of the path; empty without one (or for `/v1.2`-like numbers).
macro_rules! extension {
    () => {
        "regexp_extract(lower(split_part(u.raw_url, '?', 1)), '\\.([a-z][a-z0-9]{0,7})$', 1)"
    };
}

/// Content type guessed from the extension, as logs carry no `Content-Type`; pages without
/// an extension (or with a script extension) count as `html`.
const CONTENT_TYPE: SqlExpr = on_url(concat!(
    "CASE WHEN ",
    extension!(),
    " IN ('', 'html', 'htm', 'xhtml', 'shtml', 'php', 'asp', 'aspx', 'jsp', 'cfm') THEN 'html'",
    " WHEN ",
    extension!(),
    " = 'css' THEN 'css'",
    " WHEN ",
    extension!(),
    " IN ('js', 'mjs', 'cjs') THEN 'js'",
    " WHEN ",
    extension!(),
    " IN ('png', 'jpg', 'jpeg', 'gif', 'webp', 'avif', 'svg', 'ico', 'bmp', 'tif', 'tiff', 'heic') THEN 'image'",
    " WHEN ",
    extension!(),
    " IN ('woff', 'woff2', 'ttf', 'otf', 'eot') THEN 'font'",
    " WHEN ",
    extension!(),
    " = 'json' THEN 'json'",
    " WHEN ",
    extension!(),
    " IN ('xml', 'rss', 'atom', 'xsl') THEN 'xml'",
    " WHEN ",
    extension!(),
    " = 'pdf' THEN 'pdf'",
    " WHEN ",
    extension!(),
    " = 'txt' THEN 'txt'",
    " WHEN ",
    extension!(),
    " IN ('mp4', 'webm', 'mov', 'avi', 'mkv', 'm3u8', 'mp3', 'wav', 'ogg', 'm4a', 'aac', 'flac') THEN 'media'",
    " WHEN ",
    extension!(),
    " IN ('doc', 'docx', 'xls', 'xlsx', 'ppt', 'pptx', 'odt', 'ods', 'csv', 'rtf') THEN 'document'",
    " WHEN ",
    extension!(),
    " IN ('zip', 'gz', 'tgz', 'rar', '7z', 'tar', 'bz2', 'xz') THEN 'archive'",
    " ELSE 'other' END",
));

/// Host of the referer, lower-cased and without `www.`; NULL when it has none.
macro_rules! referer_host {
    () => {
        "nullif(regexp_replace(lower(regexp_extract(t.referer, '^[A-Za-z][A-Za-z0-9+.-]*://([^/:?#@]+)', 1)), '^www\\.', ''), '')"
    };
}

const REFERER_HOST: SqlExpr = plain(referer_host!());

/// Referer kind. `internal` compares with the request host (when logged) and the site's
/// configured host (`site_meta.site_host`), subdomains included; AI assistants are tested
/// before search so `gemini.google.com` is not counted as Google search, and webmail before
/// both.
const REFERER_TYPE: SqlExpr = plain(concat!(
    "CASE WHEN t.referer IS NULL OR t.referer IN ('', '-') THEN 'direct'",
    " WHEN ",
    referer_host!(),
    " IS NULL THEN 'other'",
    " WHEN ",
    referer_host!(),
    " = regexp_replace(lower(t.host), '^www\\.|:[0-9]+$', '', 'g')",
    " OR ",
    referer_host!(),
    " LIKE ('%.' || regexp_replace(lower(t.host), '^www\\.|:[0-9]+$', '', 'g'))",
    " OR ",
    referer_host!(),
    " = (SELECT lower(value) FROM site_meta WHERE key = 'site_host')",
    " OR ",
    referer_host!(),
    " LIKE ('%.' || (SELECT lower(value) FROM site_meta WHERE key = 'site_host')) THEN 'internal'",
    " WHEN regexp_matches(",
    referer_host!(),
    ", '(^|\\.)(chatgpt\\.com|chat\\.openai\\.com|openai\\.com|com\\.openai\\.chatgpt|perplexity\\.ai|perplexity\\.com|gemini\\.google\\.com|bard\\.google\\.com|copilot\\.microsoft\\.com|copilot\\.cloud\\.microsoft|claude\\.ai|you\\.com|phind\\.com|meta\\.ai|deepseek\\.com|grok\\.com|x\\.ai|mistral\\.ai|poe\\.com|duck\\.ai|kimi\\.com|chat\\.qwen\\.ai)$') THEN 'ai'",
    " WHEN regexp_matches(",
    referer_host!(),
    ", '^(mail|webmail|outlook|e\\.mail)\\.|^com\\.google\\.android\\.gm$') THEN 'email'",
    " WHEN regexp_matches(",
    referer_host!(),
    ", '(^|\\.)(google\\.[a-z.]{2,6}|bing\\.com|yandex\\.[a-z.]{2,6}|ya\\.ru|duckduckgo\\.com|baidu\\.com|yahoo\\.[a-z.]{2,6}|ecosia\\.org|search\\.brave\\.com|startpage\\.com|naver\\.com|seznam\\.cz|qwant\\.com|ask\\.com|aol\\.com|kagi\\.com|sogou\\.com|so\\.com|mojeek\\.com|petalsearch\\.com)$|^com\\.google\\.android\\.googlequicksearchbox$') THEN 'search'",
    " WHEN regexp_matches(",
    referer_host!(),
    ", '(^|\\.)(facebook\\.com|fb\\.com|fb\\.me|instagram\\.com|t\\.co|twitter\\.com|x\\.com|linkedin\\.com|lnkd\\.in|youtube\\.com|youtu\\.be|reddit\\.com|pinterest\\.[a-z.]{2,6}|pin\\.it|tiktok\\.com|threads\\.net|quora\\.com|vk\\.com|ok\\.ru|t\\.me|telegram\\.org|whatsapp\\.com|wa\\.me|bsky\\.app|tumblr\\.com|medium\\.com|snapchat\\.com|discord\\.com|eksisozluk\\.com)$') THEN 'social'",
    " ELSE 'other' END",
));

/// Milligrams `CO2e` per transferred byte: 0.81 kWh/GB × 442 g/kWh (Sustainable Web Design v3).
macro_rules! co2_mg_per_byte {
    () => {
        "0.00035802"
    };
}

/// Day of the site's latest request, the reference for `days_since_last`.
macro_rules! site_last_day {
    () => {
        "(SELECT CAST(max(last_ts) AS DATE) FROM r_day_url)"
    };
}

/// Fields whose column may be NULL; negating a test on them must keep NULL rows.
pub(super) const fn is_nullable(field: Field) -> bool {
    matches!(
        field,
        Field::Bytes
            | Field::ResponseMs
            | Field::Ua
            | Field::Referer
            | Field::Host
            | Field::Bot
            | Field::BotCategory
            | Field::Verification
            | Field::RefererHost
            | Field::Country
            | Field::Asn
            | Field::AsOrg
    )
}

/// Filter expression of `field`, or `None` if `source` lacks it.
pub(super) fn field_column(field: Field, source: Source) -> Option<SqlExpr> {
    use Source::{DayUrlRollup, Hits, HourRollup};
    Some(match (field, source) {
        (Field::Date, Hits) => plain("CAST(t.ts AS DATE)"),
        (Field::Date, DayUrlRollup) => plain("t.day"),
        (Field::Date, HourRollup) => plain("CAST(t.hour AS DATE)"),
        (Field::Status, _) => STATUS,
        (Field::Url, Hits | DayUrlRollup) => URL,
        (Field::Path, Hits | DayUrlRollup) => PATH,
        (Field::Directory, Hits | DayUrlRollup) => DIRECTORY,
        (Field::Params, Hits | DayUrlRollup) => PARAMS,
        (Field::Ua, Hits | DayUrlRollup) => UA,
        (Field::Bot, Hits | DayUrlRollup) => BOT,
        (Field::BotCategory, Hits | DayUrlRollup) => BOT_CATEGORY,
        (Field::Verification, Hits) => HIT_VERIFICATION,
        (Field::Verification, DayUrlRollup) => ROLLUP_VERIFICATION,
        (Field::Ip, Hits) => plain("t.ip"),
        (Field::Method, Hits) => plain("t.method"),
        (Field::Bytes, Hits) => plain("t.bytes"),
        (Field::ResponseMs, Hits) => plain("(t.duration_us / 1000.0)"),
        (Field::Referer, Hits) => plain("t.referer"),
        (Field::Host, Hits) => plain("t.host"),
        (Field::Protocol, Hits) => plain("t.protocol"),
        (Field::ContentType, Hits | DayUrlRollup) => CONTENT_TYPE,
        (Field::RefererHost, Hits) => REFERER_HOST,
        (Field::RefererType, Hits) => REFERER_TYPE,
        (Field::Country, Hits) => COUNTRY,
        (Field::Asn, Hits) => ASN,
        (Field::AsOrg, Hits) => AS_ORG,
        _ => return None,
    })
}

/// Grouping expression of `dimension`, or `None` if `source` lacks it.
pub(super) fn dimension_column(dimension: Dimension, source: Source) -> Option<SqlExpr> {
    use Source::{DayUrlRollup, Hits, HourRollup};
    Some(match (dimension, source) {
        (Dimension::Status, _) => STATUS,
        (Dimension::StatusClass, _) => STATUS_CLASS,
        (Dimension::Url, Hits | DayUrlRollup) => URL,
        (Dimension::Path, Hits | DayUrlRollup) => PATH,
        (Dimension::Directory, Hits | DayUrlRollup) => DIRECTORY,
        (Dimension::Ua, Hits | DayUrlRollup) => UA,
        (Dimension::Bot, Hits | DayUrlRollup) => BOT,
        (Dimension::BotCategory, Hits | DayUrlRollup) => BOT_CATEGORY,
        (Dimension::Verification, Hits) => HIT_VERIFICATION,
        (Dimension::Verification, DayUrlRollup) => ROLLUP_VERIFICATION,
        (Dimension::Method, Hits) => plain("t.method"),
        (Dimension::Ip, Hits) => plain("t.ip"),
        (Dimension::Referer, Hits) => plain("t.referer"),
        (Dimension::Host, Hits) => plain("t.host"),
        (Dimension::Protocol, Hits) => plain("t.protocol"),
        (Dimension::Day, Hits) => plain("CAST(t.ts AS DATE)"),
        (Dimension::Day, DayUrlRollup) => plain("t.day"),
        (Dimension::Day, HourRollup) => plain("CAST(t.hour AS DATE)"),
        (Dimension::Hour, Hits) => plain("date_trunc('hour', t.ts)"),
        (Dimension::Hour, HourRollup) => plain("t.hour"),
        (Dimension::Month, Hits) => plain("strftime(t.ts, '%Y-%m')"),
        (Dimension::Month, DayUrlRollup) => plain("strftime(t.day, '%Y-%m')"),
        (Dimension::Month, HourRollup) => plain("strftime(t.hour, '%Y-%m')"),
        (Dimension::ContentType, Hits | DayUrlRollup) => CONTENT_TYPE,
        (Dimension::RefererHost, Hits) => REFERER_HOST,
        (Dimension::RefererType, Hits) => REFERER_TYPE,
        (Dimension::Country, Hits) => COUNTRY,
        (Dimension::Asn, Hits) => ASN,
        (Dimension::AsOrg, Hits) => AS_ORG,
        _ => return None,
    })
}

/// Aggregate expression of `metric`, or `None` if `source` lacks it. Rollups store partial
/// aggregates, so they are combined with `sum`/`min`/`max` rather than counted.
pub(super) fn metric_column(metric: Metric, source: Source) -> Option<SqlExpr> {
    use Source::{DayUrlRollup, Hits, HourRollup};
    Some(match (metric, source) {
        (Metric::Hits, Hits) => plain("count(*)"),
        (Metric::Hits, DayUrlRollup | HourRollup) => plain("sum(t.hits)"),
        (Metric::Bytes, _) => plain("sum(t.bytes)"),
        (Metric::AvgMs, Hits) => plain("avg(t.duration_us) / 1000.0"),
        (Metric::AvgMs, DayUrlRollup) => {
            plain("sum(t.duration_sum_us) / nullif(sum(t.duration_count), 0) / 1000.0")
        }
        (Metric::MaxMs, Hits) => plain("max(t.duration_us) / 1000.0"),
        (Metric::MaxMs, DayUrlRollup) => plain("max(t.duration_max_us) / 1000.0"),
        (Metric::P95Ms, Hits) => plain("quantile_cont(t.duration_us, 0.95) / 1000.0"),
        (Metric::UniqueUrls, Hits | DayUrlRollup) => plain("count(DISTINCT t.url_id)"),
        (Metric::UniqueIps, Hits) => plain("count(DISTINCT t.ip)"),
        (Metric::FirstSeen, Hits) => plain("min(t.ts)"),
        (Metric::FirstSeen, DayUrlRollup) => plain("min(t.first_ts)"),
        (Metric::LastSeen, Hits) => plain("max(t.ts)"),
        (Metric::LastSeen, DayUrlRollup) => plain("max(t.last_ts)"),
        (Metric::StatusCodes, _) => {
            plain("array_to_string(list_sort(list_distinct(list(t.status))), ',')")
        }
        (Metric::UniqueStatuses, _) => plain("count(DISTINCT t.status)"),
        (Metric::Inconsistent, _) => plain("count(DISTINCT t.status) > 1"),
        (Metric::AvgBytes, Hits) => plain("avg(t.bytes)"),
        (Metric::AvgBytes, DayUrlRollup | HourRollup) => {
            plain("sum(t.bytes) / nullif(sum(t.hits) FILTER (WHERE t.bytes IS NOT NULL), 0)")
        }
        (Metric::Co2Mg, _) => plain(concat!("sum(t.bytes) * ", co2_mg_per_byte!())),
        (Metric::Errors, Hits) => plain("count(*) FILTER (WHERE t.status >= 400)"),
        (Metric::Errors, DayUrlRollup | HourRollup) => {
            plain("coalesce(sum(t.hits) FILTER (WHERE t.status >= 400), 0)")
        }
        (Metric::Hits1xx, _) => class_hits(source, 1),
        (Metric::Hits2xx, _) => class_hits(source, 2),
        (Metric::Hits3xx, _) => class_hits(source, 3),
        (Metric::Hits4xx, _) => class_hits(source, 4),
        (Metric::Hits5xx, _) => class_hits(source, 5),
        (Metric::LastStatus, Hits) => plain("arg_max(t.status, t.ts)"),
        (Metric::LastStatus, DayUrlRollup) => plain("arg_max(t.status, t.last_ts)"),
        (Metric::DaysSinceLast, Hits) => plain(concat!(
            "date_diff('day', CAST(max(t.ts) AS DATE), ",
            site_last_day!(),
            ")"
        )),
        (Metric::DaysSinceLast, DayUrlRollup) => plain(concat!(
            "date_diff('day', CAST(max(t.last_ts) AS DATE), ",
            site_last_day!(),
            ")"
        )),
        (Metric::BotHits, Hits | DayUrlRollup) => bot_hits(source, BotSet::Any),
        (Metric::SearchHits, Hits | DayUrlRollup) => bot_hits(source, BotSet::Search),
        (Metric::AiHits, Hits | DayUrlRollup) => bot_hits(source, BotSet::Ai),
        (Metric::GooglebotHits, Hits | DayUrlRollup) => bot_hits(source, BotSet::Googlebot),
        (Metric::GooglebotSmartphoneHits, Hits | DayUrlRollup) => {
            bot_hits(source, BotSet::GooglebotSmartphone)
        }
        (Metric::GooglebotDesktopHits, Hits | DayUrlRollup) => {
            bot_hits(source, BotSet::GooglebotDesktop)
        }
        (Metric::BingbotHits, Hits | DayUrlRollup) => bot_hits(source, BotSet::Bingbot),
        (Metric::YandexHits, Hits | DayUrlRollup) => bot_hits(source, BotSet::Yandex),
        _ => return None,
    })
}

/// Hits of one status class (`class` 1–5).
const fn class_hits(source: Source, class: u8) -> SqlExpr {
    plain(match (source, class) {
        (Source::Hits, 1) => "count(*) FILTER (WHERE t.status // 100 = 1)",
        (Source::Hits, 2) => "count(*) FILTER (WHERE t.status // 100 = 2)",
        (Source::Hits, 3) => "count(*) FILTER (WHERE t.status // 100 = 3)",
        (Source::Hits, 4) => "count(*) FILTER (WHERE t.status // 100 = 4)",
        (Source::Hits, _) => "count(*) FILTER (WHERE t.status // 100 = 5)",
        (_, 1) => "coalesce(sum(t.hits) FILTER (WHERE t.status // 100 = 1), 0)",
        (_, 2) => "coalesce(sum(t.hits) FILTER (WHERE t.status // 100 = 2), 0)",
        (_, 3) => "coalesce(sum(t.hits) FILTER (WHERE t.status // 100 = 3), 0)",
        (_, 4) => "coalesce(sum(t.hits) FILTER (WHERE t.status // 100 = 4), 0)",
        (_, _) => "coalesce(sum(t.hits) FILTER (WHERE t.status // 100 = 5), 0)",
    })
}

/// Bots counted by a `*_hits` metric (by user agent; filter `verification:` for genuine ones).
#[derive(Clone, Copy)]
enum BotSet {
    Any,
    Search,
    Ai,
    Googlebot,
    GooglebotSmartphone,
    GooglebotDesktop,
    Bingbot,
    Yandex,
}

/// Generates the raw-hit and rollup variants of a bot-filtered hit count.
macro_rules! bot_count {
    ($source:expr, $condition:literal) => {
        on_ua(match $source {
            Source::Hits => concat!("count(*) FILTER (WHERE ", $condition, ")"),
            _ => concat!("coalesce(sum(t.hits) FILTER (WHERE ", $condition, "), 0)"),
        })
    };
}

const fn bot_hits(source: Source, set: BotSet) -> SqlExpr {
    match set {
        BotSet::Any => bot_count!(source, "a.bot IS NOT NULL"),
        BotSet::Search => bot_count!(source, "a.bot_category = 'search'"),
        BotSet::Ai => bot_count!(source, "a.bot_category LIKE 'ai-%'"),
        BotSet::Googlebot => bot_count!(source, "a.bot LIKE 'googlebot-%'"),
        BotSet::GooglebotSmartphone => bot_count!(source, "a.bot = 'googlebot-smartphone'"),
        BotSet::GooglebotDesktop => bot_count!(source, "a.bot = 'googlebot-desktop'"),
        BotSet::Bingbot => bot_count!(source, "a.bot = 'bingbot'"),
        BotSet::Yandex => bot_count!(source, "a.bot LIKE 'yandex%'"),
    }
}

/// Expression of a hit-listing column (see [`ROW_COLUMNS`](crate::ROW_COLUMNS)).
pub(super) fn row_column(name: &str) -> Option<SqlExpr> {
    Some(match name {
        "ts" => plain("t.ts"),
        "ip" => plain("t.ip"),
        "method" => plain("t.method"),
        "url" => URL,
        "status" => STATUS,
        "bytes" => plain("t.bytes"),
        "response_ms" => plain("t.duration_us / 1000.0"),
        "ua" => UA,
        "bot" => BOT,
        "verification" => HIT_VERIFICATION,
        "referer" => plain("t.referer"),
        "host" => plain("t.host"),
        _ => return None,
    })
}

/// Casts an output expression so the executor sees exactly the declared column type.
pub(super) fn cast_to(kind: ColumnKind, sql: &str) -> String {
    match kind {
        ColumnKind::Integer => format!("CAST({sql} AS BIGINT)"),
        ColumnKind::Float => format!("CAST({sql} AS DOUBLE)"),
        ColumnKind::Text => format!("CAST({sql} AS VARCHAR)"),
        ColumnKind::Boolean | ColumnKind::Date | ColumnKind::Timestamp => sql.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ROW_COLUMNS;

    #[test]
    fn raw_hits_support_everything() {
        for field in Field::ALL {
            assert!(field_column(field, Source::Hits).is_some(), "{field:?}");
        }
        for dimension in Dimension::ALL {
            assert!(
                dimension_column(dimension, Source::Hits).is_some(),
                "{dimension:?}"
            );
        }
        for metric in Metric::ALL {
            assert!(metric_column(metric, Source::Hits).is_some(), "{metric:?}");
        }
        for (name, _) in ROW_COLUMNS {
            assert!(row_column(name).is_some(), "{name}");
        }
    }
}
