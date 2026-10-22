//! OctopusOn SEO credits for the desktop apps.
//!
//! The paid data sources (DataForSEO, SerpApi, Serper, Yandex) work with the user's own keys.
//! With an OctopusOn SEO account they can also run on credits: the request goes to the OctopusOn SEO
//! gateway (`api.octopuson.com`), which forwards it with OctopusOn SEO's provider accounts and
//! charges the account.
//!
//! - [`sign_in_with_browser`] signs in with one click: the browser comes straight back to the
//!   program when the person is signed in to the account site. [`CloudClient`] also signs in
//!   by device code (for machines without a browser), reads the account and the catalog.
//! - [`TokenStore`] keeps the account token in one file shared by every OctopusOn SEO program on
//!   the computer (readable by the owner only), so connecting once connects them all, without
//!   a credential-store prompt per program.
//! - [`CreditTransport`] wraps the app's transport: requests to a connector's upstream go to the
//!   gateway instead, and the charges it reports are collected.
//! - [`SourceMode`] and [`choose`] decide per source between the user's key and credits.
//!
//! The source clients do not change: in credit mode they get [`PLACEHOLDER_KEY`] as their key,
//! and the gateway drops it and adds OctopusOn SEO's own.

mod client;
mod connect;
mod modes;
mod session;
mod store;
mod transport;

#[cfg(test)]
mod tests;

pub use client::{
    Account, Catalog, CloudClient, Connector, DevicePoll, MeterPrice, MeterReceipt, Metered, Route,
    SignIn,
};
pub use connect::{DEFAULT_PORTAL, SIGN_IN_TIMEOUT, sign_in_with_browser};
pub use modes::{SourceMode, Via, choose, connector_for};
pub use session::{AccountSession, AccountStatus};
pub use store::{TokenStore, shared_dir};
pub use transport::{CreditTransport, matches_pattern};

/// The gateway in production.
pub const DEFAULT_API: &str = "https://api.octopuson.com";

/// Environment variable that points the apps at another gateway (a local one in development).
pub const API_VARIABLE: &str = "OCTOPUSSEO_CLOUD_URL";

/// Environment variable naming the folder of the shared account file (scripts, tests).
pub const ACCOUNT_DIR_VARIABLE: &str = "OCTOPUSSEO_ACCOUNT_DIR";

/// What a source client gets as its key in credit mode; the gateway replaces it.
pub const PLACEHOLDER_KEY: &str = "octopusseo-credit";

/// DataForSEO's placeholder, which has the `login:password` form its client expects.
pub const PLACEHOLDER_LOGIN: &str = "octopusseo:credit";

/// Response header with the US dollars a request was charged.
pub const CHARGE_HEADER: &str = "x-octopus-charge-usd";

/// Response header saying an answer came from the gateway's shared cache (`hit`); sent as
/// `peek` to ask what a kept answer would cost.
pub const CACHE_HEADER: &str = "x-octopus-cache";

/// Response header with what a cached answer would have cost without the discount.
pub const FULL_CHARGE_HEADER: &str = "x-octopus-full-charge-usd";

/// Why the account or the catalog cannot be used.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum CloudError {
    /// No token, or the gateway no longer accepts it.
    #[error("not signed in to an OctopusOn SEO account")]
    SignedOut,
    /// The person denied the sign-in in the browser.
    #[error("the sign-in was denied")]
    Denied,
    /// The sign-in code expired before it was approved.
    #[error("the sign-in code expired; start again")]
    Expired,
    /// The gateway cannot be reached or answered with an error.
    #[error("OctopusOn SEO is not reachable: {0}")]
    Unavailable(String),
    /// The gateway's answer was not understood, or a setting is invalid.
    #[error("{0}")]
    Invalid(String),
    /// The credential store failed.
    #[error("credential store: {0}")]
    Secrets(String),
}

/// The gateway to use: [`API_VARIABLE`] if set (http only for this computer), else
/// [`DEFAULT_API`].
///
/// # Errors
/// [`CloudError::Invalid`] for an address that is not https (or http on localhost).
pub fn api_base(variable: Option<&str>) -> Result<String, CloudError> {
    let Some(value) = variable.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(DEFAULT_API.to_owned());
    };
    let url = url::Url::parse(value)
        .map_err(|error| CloudError::Invalid(format!("{API_VARIABLE}: {error}")))?;
    let local = matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"));
    if url.scheme() != "https" && !(url.scheme() == "http" && local) {
        return Err(CloudError::Invalid(format!(
            "{API_VARIABLE} must be an https address"
        )));
    }
    Ok(value.trim_end_matches('/').to_owned())
}
