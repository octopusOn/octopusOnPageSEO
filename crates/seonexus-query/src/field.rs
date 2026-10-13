//! Filterable hit attributes.

use serde::{Deserialize, Serialize};

/// Value type of a [`Field`]; decides which predicates it accepts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FieldKind {
    /// Exact match, lists, `*` wildcards and regular expressions.
    Text,
    /// Lists, comparisons and ranges.
    Integer,
    /// Lists, comparisons, ranges and `lastNd`.
    Date,
    /// `true` or `false`.
    Boolean,
}

/// A hit attribute usable in filters (`field:value`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Field {
    /// UTC day of the request.
    Date,
    /// Client IP.
    Ip,
    /// HTTP method.
    Method,
    /// Raw request target including the query string.
    Url,
    /// Request target without the query string.
    Path,
    /// First path segment, e.g. `/blog/` (`/` for top-level pages).
    Directory,
    /// Whether the URL has a query string.
    Params,
    /// HTTP status code.
    Status,
    /// Response size in bytes.
    Bytes,
    /// Response time in milliseconds.
    ResponseMs,
    /// User agent.
    Ua,
    /// Referer.
    Referer,
    /// Virtual host / router.
    Host,
    /// HTTP protocol, e.g. `HTTP/2.0`.
    Protocol,
    /// Identified bot, e.g. `googlebot-smartphone` (empty for browsers).
    Bot,
    /// Bot category, e.g. `search` or `ai-training`.
    BotCategory,
    /// Bot verification verdict: `verified`, `spoofed` or `unverified`.
    Verification,
    /// Content type guessed from the path's extension: `html`, `image`, `js`, …
    ContentType,
    /// Host of the referer without `www.`, e.g. `chatgpt.com`.
    RefererHost,
    /// Kind of referer: `direct`, `internal`, `search`, `ai`, `social`, `email` or `other`.
    RefererType,
    /// Country of the client IP (ISO 3166-1 alpha-2, e.g. `TR`).
    Country,
    /// Autonomous system number of the client IP.
    Asn,
    /// Organization operating the client IP's network, e.g. `Hetzner Online GmbH`.
    AsOrg,
}

impl Field {
    /// All fields in documentation order.
    pub const ALL: [Self; 23] = [
        Self::Date,
        Self::Ip,
        Self::Method,
        Self::Url,
        Self::Path,
        Self::Directory,
        Self::Params,
        Self::Status,
        Self::Bytes,
        Self::ResponseMs,
        Self::Ua,
        Self::Referer,
        Self::Host,
        Self::Protocol,
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

    /// Name used in the query language.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Date => "date",
            Self::Ip => "ip",
            Self::Method => "method",
            Self::Url => "url",
            Self::Path => "path",
            Self::Directory => "directory",
            Self::Params => "params",
            Self::Status => "status",
            Self::Bytes => "bytes",
            Self::ResponseMs => "response_ms",
            Self::Ua => "ua",
            Self::Referer => "referer",
            Self::Host => "host",
            Self::Protocol => "protocol",
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

    /// Looks a field up by its query-language name.
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|field| field.name() == name)
    }

    /// Value type.
    pub const fn kind(self) -> FieldKind {
        match self {
            Self::Date => FieldKind::Date,
            Self::Status | Self::Bytes | Self::ResponseMs | Self::Asn => FieldKind::Integer,
            Self::Params => FieldKind::Boolean,
            Self::Ip
            | Self::Method
            | Self::Url
            | Self::Path
            | Self::Directory
            | Self::Ua
            | Self::Referer
            | Self::Host
            | Self::Protocol
            | Self::Bot
            | Self::BotCategory
            | Self::Verification
            | Self::ContentType
            | Self::RefererHost
            | Self::RefererType
            | Self::Country
            | Self::AsOrg => FieldKind::Text,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_round_trip() {
        for field in Field::ALL {
            assert_eq!(Field::from_name(field.name()), Some(field));
        }
        assert_eq!(Field::from_name("nope"), None);
    }
}
