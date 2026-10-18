//! Choosing between the user's own key and OctopusOn SEO credits for a source.

use serde::{Deserialize, Serialize};

/// What the user chose for a source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SourceMode {
    /// The own key when there is one, else credits when available.
    #[default]
    Auto,
    /// Only the own key.
    Own,
    /// Only credits.
    Credit,
}

/// How a request is paid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Via {
    /// With the user's own key, directly to the provider.
    Own,
    /// With OctopusOn SEO credits, through the gateway.
    Credit,
}

/// How a source is used: `own_ready` when every own key is available, `credit_ready` when the
/// account is signed in and the source's connector is in the catalog. `None` when it cannot be
/// used.
pub const fn choose(mode: SourceMode, own_ready: bool, credit_ready: bool) -> Option<Via> {
    match mode {
        SourceMode::Own | SourceMode::Auto if own_ready => Some(Via::Own),
        SourceMode::Credit | SourceMode::Auto if credit_ready => Some(Via::Credit),
        _ => None,
    }
}

/// The gateway connector serving an app's source id.
pub fn connector_for(source: &str) -> Option<&'static str> {
    Some(match source {
        "dataforseo" | "dataforseo-ads" => "dataforseo",
        "serpapi" => "serpapi",
        "serper" => "serper",
        "wordstat" => "wordstat",
        "yandex-cloud" | "yandex-search" => "yandex-search",
        _ => return None,
    })
}
