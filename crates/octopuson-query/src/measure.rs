//! Grouping keys (`group:`) and aggregates (`metric:`).

use octopuson_core::table::ColumnKind;
use serde::{Deserialize, Serialize};

/// A grouping key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Dimension {
    /// Raw URL.
    Url,
    /// URL without query string.
    Path,
    /// First path segment.
    Directory,
    /// Status code.
    Status,
    /// Status class (`2xx`, `3xx`, …).
    StatusClass,
    /// HTTP method.
    Method,
    /// Client IP.
    Ip,
    /// User agent.
    Ua,
    /// Referer.
    Referer,
    /// Virtual host / router.
    Host,
    /// HTTP protocol.
    Protocol,
    /// UTC day.
    Day,
    /// UTC hour.
    Hour,
    /// UTC month (`YYYY-MM`).
    Month,
    /// Identified bot.
    Bot,
    /// Bot category.
    BotCategory,
    /// Bot verification verdict.
    Verification,
    /// Content type guessed from the extension.
    ContentType,
    /// Referer host.
    RefererHost,
    /// Kind of referer (search, AI assistant, social, …).
    RefererType,
    /// Country of the client IP.
    Country,
    /// Autonomous system number of the client IP.
    Asn,
    /// Organization of the client IP's network.
    AsOrg,
}

impl Dimension {
    /// All dimensions.
    pub const ALL: [Self; 23] = [
        Self::Url,
        Self::Path,
        Self::Directory,
        Self::Status,
        Self::StatusClass,
        Self::Method,
        Self::Ip,
        Self::Ua,
        Self::Referer,
        Self::Host,
        Self::Protocol,
        Self::Day,
        Self::Hour,
        Self::Month,
        Self::Bot,
        Self::BotCategory,
        Self::Verification,
        Self::ContentType,
        Self::RefererHost,
        Self::RefererType,
        Self::Country,
        Self::Asn,
        Self::AsOrg,
    ];

    /// Name used in the query language and as result column name.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Url => "url",
            Self::Path => "path",
            Self::Directory => "directory",
            Self::Status => "status",
            Self::StatusClass => "status_class",
            Self::Method => "method",
            Self::Ip => "ip",
            Self::Ua => "ua",
            Self::Referer => "referer",
            Self::Host => "host",
            Self::Protocol => "protocol",
            Self::Day => "day",
            Self::Hour => "hour",
            Self::Month => "month",
            Self::Bot => "bot",
            Self::BotCategory => "bot_category",
            Self::Verification => "verification",
            Self::ContentType => "content_type",
            Self::RefererHost => "referer_host",
            Self::RefererType => "referer_type",
            Self::Country => "country",
            Self::Asn => "asn",
            Self::AsOrg => "as_org",
        }
    }

    /// Looks a dimension up by name.
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|d| d.name() == name)
    }

    /// Result column type.
    pub const fn column_kind(self) -> ColumnKind {
        match self {
            Self::Status | Self::Asn => ColumnKind::Integer,
            Self::Day => ColumnKind::Date,
            Self::Hour => ColumnKind::Timestamp,
            _ => ColumnKind::Text,
        }
    }
}

/// An aggregate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Metric {
    /// Number of requests.
    Hits,
    /// Bytes sent.
    Bytes,
    /// Mean response time (ms).
    AvgMs,
    /// Maximum response time (ms).
    MaxMs,
    /// 95th percentile response time (ms).
    P95Ms,
    /// Distinct URLs.
    UniqueUrls,
    /// Distinct client IPs.
    UniqueIps,
    /// First request time.
    FirstSeen,
    /// Last request time.
    LastSeen,
    /// Distinct status codes, e.g. `200,301`.
    StatusCodes,
    /// Number of distinct status codes (more than one means inconsistent responses).
    UniqueStatuses,
    /// Mean response size in bytes.
    AvgBytes,
    /// Requests answered with 4xx or 5xx.
    Errors,
    /// Requests answered with 1xx.
    Hits1xx,
    /// Requests answered with 2xx.
    Hits2xx,
    /// Requests answered with 3xx.
    Hits3xx,
    /// Requests answered with 4xx.
    Hits4xx,
    /// Requests answered with 5xx.
    Hits5xx,
    /// Status code of the most recent request.
    LastStatus,
    /// Days between the most recent request and the site's latest request.
    DaysSinceLast,
    /// Estimated emissions of the transferred bytes in milligrams `CO2e` (Sustainable Web
    /// Design model v3: 0.81 kWh/GB at the 442 g/kWh global average grid intensity).
    Co2Mg,
    /// Whether more than one status code was returned.
    Inconsistent,
    /// Requests from any identified bot.
    BotHits,
    /// Requests from search-engine bots.
    SearchHits,
    /// Requests from AI bots (training, search and assistants).
    AiHits,
    /// Requests from Googlebot (smartphone, desktop, image, video, news).
    GooglebotHits,
    /// Requests from Googlebot Smartphone.
    GooglebotSmartphoneHits,
    /// Requests from Googlebot Desktop.
    GooglebotDesktopHits,
    /// Requests from Bingbot.
    BingbotHits,
    /// Requests from Yandex bots.
    YandexHits,
}

impl Metric {
    /// All metrics.
    pub const ALL: [Self; 30] = [
        Self::Hits,
        Self::Bytes,
        Self::AvgMs,
        Self::MaxMs,
        Self::P95Ms,
        Self::UniqueUrls,
        Self::UniqueIps,
        Self::FirstSeen,
        Self::LastSeen,
        Self::StatusCodes,
        Self::UniqueStatuses,
        Self::AvgBytes,
        Self::Errors,
        Self::Hits1xx,
        Self::Hits2xx,
        Self::Hits3xx,
        Self::Hits4xx,
        Self::Hits5xx,
        Self::LastStatus,
        Self::DaysSinceLast,
        Self::Co2Mg,
        Self::Inconsistent,
        Self::BotHits,
        Self::SearchHits,
        Self::AiHits,
        Self::GooglebotHits,
        Self::GooglebotSmartphoneHits,
        Self::GooglebotDesktopHits,
        Self::BingbotHits,
        Self::YandexHits,
    ];

    /// Name used in the query language and as result column name.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Hits => "hits",
            Self::Bytes => "bytes",
            Self::AvgMs => "avg_ms",
            Self::MaxMs => "max_ms",
            Self::P95Ms => "p95_ms",
            Self::UniqueUrls => "unique_urls",
            Self::UniqueIps => "unique_ips",
            Self::FirstSeen => "first_seen",
            Self::LastSeen => "last_seen",
            Self::StatusCodes => "status_codes",
            Self::UniqueStatuses => "unique_statuses",
            Self::AvgBytes => "avg_bytes",
            Self::Errors => "errors",
            Self::Hits1xx => "hits_1xx",
            Self::Hits2xx => "hits_2xx",
            Self::Hits3xx => "hits_3xx",
            Self::Hits4xx => "hits_4xx",
            Self::Hits5xx => "hits_5xx",
            Self::LastStatus => "last_status",
            Self::DaysSinceLast => "days_since_last",
            Self::Co2Mg => "co2_mg",
            Self::Inconsistent => "inconsistent",
            Self::BotHits => "bot_hits",
            Self::SearchHits => "search_hits",
            Self::AiHits => "ai_hits",
            Self::GooglebotHits => "googlebot_hits",
            Self::GooglebotSmartphoneHits => "googlebot_smartphone_hits",
            Self::GooglebotDesktopHits => "googlebot_desktop_hits",
            Self::BingbotHits => "bingbot_hits",
            Self::YandexHits => "yandex_hits",
        }
    }

    /// Looks a metric up by name.
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|m| m.name() == name)
    }

    /// Result column type.
    pub const fn column_kind(self) -> ColumnKind {
        match self {
            Self::Hits
            | Self::Bytes
            | Self::UniqueUrls
            | Self::UniqueIps
            | Self::UniqueStatuses
            | Self::Errors
            | Self::Hits1xx
            | Self::Hits2xx
            | Self::Hits3xx
            | Self::Hits4xx
            | Self::Hits5xx
            | Self::LastStatus
            | Self::DaysSinceLast
            | Self::BotHits
            | Self::SearchHits
            | Self::AiHits
            | Self::GooglebotHits
            | Self::GooglebotSmartphoneHits
            | Self::GooglebotDesktopHits
            | Self::BingbotHits
            | Self::YandexHits => ColumnKind::Integer,
            Self::StatusCodes => ColumnKind::Text,
            Self::Inconsistent => ColumnKind::Boolean,
            Self::AvgMs | Self::MaxMs | Self::P95Ms | Self::AvgBytes | Self::Co2Mg => {
                ColumnKind::Float
            }
            Self::FirstSeen | Self::LastSeen => ColumnKind::Timestamp,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_round_trip() {
        for dimension in Dimension::ALL {
            assert_eq!(Dimension::from_name(dimension.name()), Some(dimension));
        }
        for metric in Metric::ALL {
            assert_eq!(Metric::from_name(metric.name()), Some(metric));
        }
    }

    #[test]
    fn dimension_and_metric_names_differ() {
        // Both become result column names, so they must never collide.
        for dimension in Dimension::ALL {
            assert!(
                Metric::from_name(dimension.name()).is_none(),
                "{dimension:?}"
            );
        }
    }
}
