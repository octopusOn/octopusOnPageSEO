//! One-click sign-in (RFC 8252 loopback redirect with PKCE).
//!
//! The program listens on `127.0.0.1` and opens `{portal}/baglan` in the browser. A person
//! already signed in to the account site comes straight back with a one-time code; otherwise
//! the site asks them to sign in first. The code is swapped for a token with the PKCE verifier
//! only this program holds, so there is no code to read out or compare.

use std::time::Duration;

use octopuson_oauth::OAuthError;

use crate::{CloudClient, CloudError};

/// The account site when the gateway does not say (production).
pub const DEFAULT_PORTAL: &str = "https://my.octopuson.com";

/// How long the browser is waited for.
pub const SIGN_IN_TIMEOUT: Duration = Duration::from_secs(600);

/// Signs in through the browser and returns the account token; `None` when `cancelled` stopped
/// it. `open` opens an address in the default browser and says whether it could.
///
/// # Errors
/// [`CloudError::Invalid`] when no local port can be opened or no browser started;
/// [`CloudError::Expired`] when nobody signed in in time or the code was refused;
/// [`CloudError::Unavailable`] when the gateway cannot be reached.
pub fn sign_in_with_browser(
    client: &CloudClient<'_>,
    portal_url: &str,
    app: &str,
    open: &dyn Fn(&str) -> bool,
    cancelled: &dyn Fn() -> bool,
    timeout: Duration,
) -> Result<Option<String>, CloudError> {
    let page = format!("{}/baglan", portal_url.trim_end_matches('/'));
    let authorization = octopuson_oauth::begin_at(&page, &[("client", app)], &[])
        .map_err(|error| CloudError::Invalid(error.to_string()))?;
    if !open(&authorization.url) {
        return Err(CloudError::Invalid(
            "no browser could be opened; sign in with a code instead (account login --code)"
                .to_owned(),
        ));
    }
    match authorization.wait(timeout, cancelled) {
        Ok(code) => client
            .swap_connect_code(code.code(), code.verifier())
            .map(Some),
        Err(OAuthError::Cancelled) => Ok(None),
        Err(OAuthError::Timeout) => Err(CloudError::Expired),
        Err(error) => Err(CloudError::Unavailable(error.to_string())),
    }
}
