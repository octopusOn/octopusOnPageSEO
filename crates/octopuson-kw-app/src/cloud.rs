//! The OctopusOn SEO account: credits as a second way to pay for the paid sources.
//!
//! A source runs on the user's own key or on credits ([`SourceMode`], `auto` by default: the
//! own key when there is one). In credit mode the source client gets a placeholder key and
//! the transport sends its requests through the OctopusOn SEO gateway, which charges the account.

use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

use octopuson_cloud::{
    Account, Catalog, CloudClient, CloudError, Connector, DEFAULT_PORTAL, DevicePoll,
    PLACEHOLDER_KEY, PLACEHOLDER_LOGIN, SIGN_IN_TIMEOUT, SignIn, SourceMode, TokenStore, Via,
    choose, connector_for, sign_in_with_browser,
};
use octopuson_core::http::HttpTransport;
use octopuson_keyword_sources::dataforseo::DataForSeo;
use serde::{Deserialize, Serialize};

use crate::runner::RunControl;
use crate::settings::Settings;
use crate::sources::{PROVIDERS, provider};
use crate::{KeywordWorkspace, KwAppError};

/// How long the connector catalog is trusted before it is asked again.
const CATALOG_TTL: Duration = Duration::from_secs(300);

/// How long a token read from the shared account file is trusted: another OctopusOn SEO program
/// may sign in or out meanwhile, and this one notices within this time.
const TOKEN_TTL: Duration = Duration::from_secs(3);

/// The account token as known in memory.
#[derive(Debug, Clone)]
enum CachedToken {
    /// The account file was not read yet.
    Unread,
    /// The token, or none, as read at a moment.
    Known(Option<String>, Instant),
}

/// The account token and the catalog, kept in memory.
pub(crate) struct CloudState {
    tokens: TokenStore,
    token: Mutex<CachedToken>,
    catalog: Mutex<Option<(Instant, Catalog)>>,
}

impl std::fmt::Debug for CloudState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CloudState")
            .field("tokens", &self.tokens)
            .finish_non_exhaustive()
    }
}

impl CloudState {
    pub(crate) fn new(tokens: TokenStore) -> Self {
        Self {
            tokens,
            token: Mutex::new(CachedToken::Unread),
            catalog: Mutex::new(None),
        }
    }
}

impl From<CloudError> for KwAppError {
    fn from(error: CloudError) -> Self {
        Self::Cloud(error.to_string())
    }
}

/// The account as the Sources page shows it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CloudStatus {
    /// A token is saved.
    pub signed_in: bool,
    /// The account, when the gateway answered.
    pub account: Option<Account>,
    /// Why the account could not be read.
    pub error: Option<String>,
    /// Where credits are bought.
    pub portal_url: String,
    /// The gateway.
    pub api: String,
}

/// A source on credits: its connector and price.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CreditOffer {
    /// Gateway connector id.
    pub connector: String,
    /// Credit price = provider price × this.
    pub price_multiplier: f64,
}

/// The program as it names itself to the gateway.
pub fn app_name() -> String {
    format!(
        "keyword-miner/{} {}",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS
    )
}

impl KeywordWorkspace {
    /// The saved account token (read from the credential store once).
    pub(crate) fn cloud_token(&self) -> Option<String> {
        let mut cached = self
            .cloud
            .token
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let previous = match &*cached {
            CachedToken::Known(token, at) if at.elapsed() < TOKEN_TTL => return token.clone(),
            CachedToken::Known(token, _) => Some(token.clone()),
            CachedToken::Unread => None,
        };
        // Read again: another OctopusOn SEO program may have signed in or out.
        let token = self.cloud.tokens.load().ok().flatten();
        if previous.is_some_and(|previous| previous != token) {
            *self
                .cloud
                .catalog
                .lock()
                .unwrap_or_else(PoisonError::into_inner) = None;
        }
        *cached = CachedToken::Known(token.clone(), Instant::now());
        token
    }

    /// Lets the next read go to the account file at once (tests).
    #[cfg(test)]
    pub(crate) fn expire_cached_token(&self) {
        let mut cached = self
            .cloud
            .token
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if let CachedToken::Known(_, at) = &mut *cached
            && let Some(past) = Instant::now().checked_sub(TOKEN_TTL)
        {
            *at = past;
        }
    }

    fn set_cloud_token(&self, token: Option<String>) {
        *self
            .cloud
            .token
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = CachedToken::Known(token, Instant::now());
        *self
            .cloud
            .catalog
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = None;
    }

    /// The gateway address.
    pub fn cloud_api(&self) -> &str {
        self.cloud.tokens.base()
    }

    fn cloud_client<'a>(&self, transport: &'a dyn HttpTransport) -> CloudClient<'a> {
        CloudClient::new(transport, self.cloud_api(), app_name())
    }

    /// The catalog as last read, however old (no network).
    pub(crate) fn cached_catalog(&self) -> Option<Catalog> {
        self.cloud
            .catalog
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .as_ref()
            .map(|(_, catalog)| catalog.clone())
    }

    /// Reads the catalog again when it is older than a few minutes and an account is signed
    /// in. A gateway that cannot be reached leaves the last catalog in place.
    pub(crate) fn refresh_catalog(&self, transport: &dyn HttpTransport) {
        let Some(token) = self.cloud_token() else {
            return;
        };
        let fresh = self
            .cloud
            .catalog
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .as_ref()
            .is_some_and(|(at, _)| at.elapsed() < CATALOG_TTL);
        if fresh {
            return;
        }
        if let Ok(catalog) = self.cloud_client(transport).catalog(Some(&token)) {
            *self
                .cloud
                .catalog
                .lock()
                .unwrap_or_else(PoisonError::into_inner) = Some((Instant::now(), catalog));
        }
    }

    /// Refreshes the catalog before an estimate when an account is signed in.
    pub(crate) fn warm_catalog(&self, settings: &Settings) -> Result<(), KwAppError> {
        if self.cloud_token().is_some() {
            self.refresh_catalog(&self.plain_transport(settings)?);
        }
        Ok(())
    }

    /// The connector a source runs on with credits, when signed in and on offer.
    pub(crate) fn credit_connector(&self, provider_id: &str) -> Option<Connector> {
        let id = connector_for(provider_id)?;
        self.cloud_token()?;
        self.cached_catalog()?.connector(id).cloned()
    }

    /// Whether every own key of a source is available.
    pub(crate) fn own_ready(&self, provider_id: &str) -> bool {
        provider(provider_id).is_ok_and(|definition| {
            definition
                .secrets
                .iter()
                .all(|name| self.own_secret(definition.secrets_of, name).is_some())
        })
    }

    /// How a source is paid now, if it can be used at all.
    pub(crate) fn via(&self, settings: &Settings, provider_id: &str) -> Option<Via> {
        let mode = settings
            .credit
            .get(provider_id)
            .copied()
            .unwrap_or_default();
        choose(
            mode,
            self.own_ready(provider_id),
            self.credit_connector(provider_id).is_some(),
        )
    }

    /// The key a source client gets: the placeholder when its sources run on credits only.
    pub(crate) fn secret(&self, owner: &str, name: &str) -> Option<String> {
        let settings = self.settings().unwrap_or_default();
        let vias: Vec<Option<Via>> = PROVIDERS
            .iter()
            .filter(|provider| provider.secrets_of == owner && connector_for(provider.id).is_some())
            .map(|provider| self.via(&settings, provider.id))
            .collect();
        if vias.contains(&Some(Via::Credit)) && !vias.contains(&Some(Via::Own)) {
            let placeholder = if owner == DataForSeo::ID && name == "credentials" {
                PLACEHOLDER_LOGIN
            } else {
                PLACEHOLDER_KEY
            };
            return Some(placeholder.to_owned());
        }
        self.own_secret(owner, name)
    }

    /// Connectors of the sources that run on credits now.
    pub(crate) fn credit_connectors(&self, settings: &Settings) -> Vec<Connector> {
        let mut connectors: Vec<Connector> = Vec::new();
        for provider in PROVIDERS {
            if self.via(settings, provider.id) != Some(Via::Credit) {
                continue;
            }
            if let Some(connector) = self.credit_connector(provider.id)
                && !connectors.iter().any(|known| known.id == connector.id)
            {
                connectors.push(connector);
            }
        }
        connectors
    }

    /// What a source costs relative to its provider price: the catalog's multiplier on
    /// credits, 1 with the own key.
    pub(crate) fn price_factor(&self, settings: &Settings, provider_id: &str) -> f64 {
        if self.via(settings, provider_id) == Some(Via::Credit) {
            self.credit_connector(provider_id)
                .map_or(1.0, |connector| connector.price_multiplier)
        } else {
            1.0
        }
    }

    /// The credit offer of a source, if it has one.
    pub(crate) fn credit_offer(&self, provider_id: &str) -> Option<CreditOffer> {
        self.credit_connector(provider_id)
            .map(|connector| CreditOffer {
                connector: connector.id,
                price_multiplier: connector.price_multiplier,
            })
    }

    /// The account and its balance.
    ///
    /// # Errors
    /// [`KwAppError::InvalidSettings`] for an unusable proxy.
    pub fn cloud_status(&self) -> Result<CloudStatus, KwAppError> {
        let transport = self.plain_transport(&self.settings()?)?;
        let mut status = CloudStatus {
            signed_in: false,
            account: None,
            error: None,
            portal_url: "https://my.octopuson.com".to_owned(),
            api: self.cloud_api().to_owned(),
        };
        let Some(token) = self.cloud_token() else {
            return Ok(status);
        };
        status.signed_in = true;
        match self.cloud_client(&transport).account(&token) {
            Ok(account) => {
                account.portal_url.clone_into(&mut status.portal_url);
                status.account = Some(account);
                self.refresh_catalog(&transport);
            }
            Err(CloudError::SignedOut) => {
                // The token was revoked on the website: forget it.
                self.cloud.tokens.clear()?;
                self.set_cloud_token(None);
                status.signed_in = false;
                status.error = Some(CloudError::SignedOut.to_string());
            }
            Err(error) => status.error = Some(error.to_string()),
        }
        Ok(status)
    }

    /// Signs in with one click: the account site opens in the browser and, for a person who
    /// is signed in there, comes straight back to this program. `false` when stopped.
    ///
    /// # Errors
    /// [`KwAppError::Cloud`] when no browser opens, nobody signs in in time or the gateway
    /// cannot be reached.
    pub fn connect(
        &self,
        open: &dyn Fn(&str) -> bool,
        control: &RunControl<'_>,
    ) -> Result<bool, KwAppError> {
        let transport = self.plain_transport(&self.settings()?)?;
        let client = self.cloud_client(&transport);
        let portal = client
            .catalog(None)
            .map_or_else(|_| DEFAULT_PORTAL.to_owned(), |catalog| catalog.portal_url);
        let Some(token) = sign_in_with_browser(
            &client,
            &portal,
            &app_name(),
            open,
            control.cancelled,
            SIGN_IN_TIMEOUT,
        )?
        else {
            return Ok(false);
        };
        self.cloud.tokens.save(&token)?;
        self.set_cloud_token(Some(token));
        self.refresh_catalog(&transport);
        Ok(true)
    }

    /// Starts signing in with a code (machines without a browser): the person opens
    /// [`SignIn::verification_uri_complete`] elsewhere and approves the code.
    ///
    /// # Errors
    /// [`KwAppError::Cloud`] when the gateway cannot be reached.
    pub fn start_sign_in(&self) -> Result<SignIn, KwAppError> {
        let transport = self.plain_transport(&self.settings()?)?;
        Ok(self.cloud_client(&transport).start_sign_in()?)
    }

    /// Waits for the person to approve a sign-in and saves the token; `false` when stopped.
    ///
    /// # Errors
    /// [`KwAppError::Cloud`] when the code was denied or expired.
    pub fn finish_sign_in(
        &self,
        sign_in: &SignIn,
        control: &RunControl<'_>,
    ) -> Result<bool, KwAppError> {
        let transport = self.plain_transport(&self.settings()?)?;
        let client = self.cloud_client(&transport);
        let mut interval = sign_in.interval.max(1);
        let deadline = Instant::now() + Duration::from_secs(sign_in.expires_in);
        while Instant::now() < deadline {
            for _ in 0..interval {
                if (control.cancelled)() {
                    return Ok(false);
                }
                (control.sleep)(Duration::from_secs(1));
            }
            match client.poll_sign_in(&sign_in.device_code)? {
                DevicePoll::Pending => {}
                DevicePoll::SlowDown => interval += 5,
                DevicePoll::Approved(token) => {
                    self.cloud.tokens.save(&token)?;
                    self.set_cloud_token(Some(token));
                    self.refresh_catalog(&transport);
                    return Ok(true);
                }
            }
        }
        Err(CloudError::Expired.into())
    }

    /// Signs out: the gateway stops accepting the token, and it is forgotten here.
    ///
    /// # Errors
    /// [`KwAppError::Cloud`] when the credential store fails.
    pub fn sign_out(&self) -> Result<(), KwAppError> {
        if let Some(token) = self.cloud_token() {
            if let Ok(transport) = self.plain_transport(&self.settings()?) {
                // Signing out here must work offline too.
                let _ = self.cloud_client(&transport).sign_out(&token);
            }
            self.cloud.tokens.clear()?;
        }
        self.set_cloud_token(None);
        Ok(())
    }

    /// Chooses how a source is paid.
    ///
    /// # Errors
    /// [`KwAppError::Invalid`] for a source that cannot run on credits; settings errors.
    pub fn set_source_mode(&self, provider_id: &str, mode: SourceMode) -> Result<(), KwAppError> {
        let definition = provider(provider_id)?;
        if connector_for(definition.id).is_none() {
            return Err(KwAppError::Invalid(format!(
                "{} cannot run on OctopusOn SEO credits",
                definition.name
            )));
        }
        let mut settings = self.settings()?;
        if mode == SourceMode::Auto {
            settings.credit.remove(definition.id);
        } else {
            settings.credit.insert(definition.id.to_owned(), mode);
        }
        self.save_settings(&settings)
    }
}

#[cfg(test)]
#[path = "cloud_tests.rs"]
mod tests;
