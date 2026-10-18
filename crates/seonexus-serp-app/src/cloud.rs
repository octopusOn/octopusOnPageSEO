//! The OctopusOn SEO account: credits as a second way to pay for the paid result-page, volume
//! and backlink sources.
//!
//! A source runs on the user's own key or on credits ([`SourceMode`], `auto` by default: the
//! own key when there is one). In credit mode the source gets a placeholder key and the
//! transport sends its requests through the OctopusOn SEO gateway, which charges the account.
//! Only built-in sources whose requests go to the connector's own address can run on
//! credits; a custom spec never does.

use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

use seonexus_cloud::{
    Account, Catalog, CloudClient, CloudError, Connector, CreditTransport, DEFAULT_PORTAL,
    DevicePoll, PLACEHOLDER_KEY, PLACEHOLDER_LOGIN, SIGN_IN_TIMEOUT, SignIn, SourceMode,
    TokenStore, Via, choose, connector_for, sign_in_with_browser,
};
use seonexus_core::http::HttpTransport;
use seonexus_net::ApiClient;
use serde::{Deserialize, Serialize};

use crate::providers::{Entry, ProviderKind, catalog, load_custom};
use crate::secrets::SecretState;
use crate::settings::Settings;
use crate::{SerpAppError, SerpWorkspace};

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

impl From<CloudError> for SerpAppError {
    fn from(error: CloudError) -> Self {
        Self::Cloud(error.to_string())
    }
}

/// The account as the providers page shows it.
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
        "serp-finder/{} {}",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS
    )
}

/// The placeholder a source client gets as its key on credits.
fn placeholder(provider: &str, name: &str) -> String {
    if provider == "dataforseo" && name == "credentials" {
        PLACEHOLDER_LOGIN
    } else {
        PLACEHOLDER_KEY
    }
    .to_owned()
}

impl SerpWorkspace {
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

    /// The API transport of the user's network settings, without credit routing.
    ///
    /// # Errors
    /// [`SerpAppError::InvalidSettings`] for an unusable proxy.
    pub fn plain_transport(&self, settings: &Settings) -> Result<ApiClient, SerpAppError> {
        ApiClient::new(
            concat!("octopusSEO-SERP-Finder/", env!("CARGO_PKG_VERSION")),
            Duration::from_secs(settings.timeout_secs),
            settings.proxy.as_deref(),
        )
        .map_err(|error| SerpAppError::InvalidSettings(error.to_string()))
    }

    /// The API transport: requests of sources on OctopusOn SEO credits go through the gateway,
    /// everything else directly.
    ///
    /// # Errors
    /// [`SerpAppError::InvalidSettings`] for an unusable proxy; unreadable provider files.
    pub fn transport(
        &self,
        settings: &Settings,
    ) -> Result<CreditTransport<ApiClient>, SerpAppError> {
        let inner = self.plain_transport(settings)?;
        self.refresh_catalog(&inner);
        let connectors = self.credit_connectors(settings)?;
        Ok(CreditTransport::new(
            inner,
            self.cloud_api(),
            self.cloud_token().unwrap_or_default(),
            app_name(),
            connectors,
        ))
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
    pub(crate) fn warm_catalog(&self, settings: &Settings) -> Result<(), SerpAppError> {
        if self.cloud_token().is_some() {
            self.refresh_catalog(&self.plain_transport(settings)?);
        }
        Ok(())
    }

    /// The catalog connector by id, when signed in.
    fn connector(&self, id: &str) -> Option<Connector> {
        self.cloud_token()?;
        self.cached_catalog()?.connector(id).cloned()
    }

    /// The connector a source runs on with credits: signed in, on offer, and the source sends
    /// its requests to the connector's address only.
    pub(crate) fn credit_connector(&self, provider: &str, origins: &[&str]) -> Option<Connector> {
        let connector = self.connector(connector_for(provider)?)?;
        let upstream = connector.upstream.trim_end_matches('/');
        (!origins.is_empty()
            && origins
                .iter()
                .all(|origin| origin.trim_end_matches('/') == upstream))
        .then_some(connector)
    }

    /// How a provider of the catalog is paid now, if it can be used at all.
    pub(crate) fn entry_via(
        &self,
        settings: &Settings,
        entry: &Entry,
    ) -> Result<Option<Via>, SerpAppError> {
        let id = entry.provider.id();
        let origins = entry.origin_refs();
        let mut own = true;
        for secret in &entry.secrets {
            own &= matches!(
                self.secrets().state(id, &secret.name, &origins)?,
                SecretState::Saved | SecretState::Environment
            );
        }
        let credit =
            entry.kind == ProviderKind::Builtin && self.credit_connector(id, &origins).is_some();
        Ok(choose(self.source_mode(settings, id), own, credit))
    }

    /// What the user chose for a source.
    pub(crate) fn source_mode(&self, settings: &Settings, provider: &str) -> SourceMode {
        let _ = self;
        settings.credit.get(provider).copied().unwrap_or_default()
    }

    /// The key a built-in source gets for a direct call (volumes, backlinks): the user's own,
    /// or the placeholder on credits.
    pub(crate) fn source_secret(
        &self,
        settings: &Settings,
        provider: &str,
        name: &str,
        origin: &str,
    ) -> Option<String> {
        let own = self.secrets().get(provider, name, &[origin]).ok().flatten();
        let credit = self.credit_connector(provider, &[origin]).is_some();
        match choose(self.source_mode(settings, provider), own.is_some(), credit)? {
            Via::Own => own,
            Via::Credit => Some(placeholder(provider, name)),
        }
    }

    /// The placeholder key of a source on credits.
    pub(crate) fn credit_secret(provider: &str, name: &str) -> String {
        placeholder(provider, name)
    }

    /// Connectors of the sources that run on credits now. Yandex's result pages and its
    /// Wordstat volumes share the key, so both connectors come with it.
    pub(crate) fn credit_connectors(
        &self,
        settings: &Settings,
    ) -> Result<Vec<Connector>, SerpAppError> {
        if self.cloud_token().is_none() {
            return Ok(Vec::new());
        }
        let (entries, _) = catalog(load_custom(&self.providers_path())?);
        let mut connectors: Vec<Connector> = Vec::new();
        let mut add = |connector: Option<Connector>| {
            if let Some(connector) = connector
                && !connectors.iter().any(|known| known.id == connector.id)
            {
                connectors.push(connector);
            }
        };
        for entry in &entries {
            if self.entry_via(settings, entry)? != Some(Via::Credit) {
                continue;
            }
            let id = entry.provider.id();
            add(self.credit_connector(id, &entry.origin_refs()));
            if id == "yandex-cloud" {
                add(self.connector("wordstat"));
            }
        }
        Ok(connectors)
    }

    /// The credit offer of a source, if it has one.
    pub(crate) fn credit_offer(&self, entry: &Entry) -> Option<CreditOffer> {
        if entry.kind != ProviderKind::Builtin {
            return None;
        }
        self.credit_connector(entry.provider.id(), &entry.origin_refs())
            .map(|connector| CreditOffer {
                connector: connector.id,
                price_multiplier: connector.price_multiplier,
            })
    }

    /// What a source costs relative to its provider price.
    pub(crate) fn price_factor(&self, settings: &Settings, entry: &Entry) -> f64 {
        if self.entry_via(settings, entry).ok().flatten() == Some(Via::Credit) {
            self.credit_offer(entry)
                .map_or(1.0, |offer| offer.price_multiplier)
        } else {
            1.0
        }
    }

    /// The account and its balance.
    ///
    /// # Errors
    /// [`SerpAppError::InvalidSettings`] for an unusable proxy.
    pub fn cloud_status(&self) -> Result<CloudStatus, SerpAppError> {
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
    /// [`SerpAppError::Cloud`] when no browser opens, nobody signs in in time or the gateway
    /// cannot be reached.
    pub fn connect(
        &self,
        open: &dyn Fn(&str) -> bool,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<bool, SerpAppError> {
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
            cancelled,
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
    /// [`SerpAppError::Cloud`] when the gateway cannot be reached.
    pub fn start_sign_in(&self) -> Result<SignIn, SerpAppError> {
        let transport = self.plain_transport(&self.settings()?)?;
        Ok(self.cloud_client(&transport).start_sign_in()?)
    }

    /// Waits for the person to approve a sign-in and saves the token; `false` when stopped.
    ///
    /// # Errors
    /// [`SerpAppError::Cloud`] when the code was denied or expired.
    pub fn finish_sign_in(
        &self,
        sign_in: &SignIn,
        cancelled: &dyn Fn() -> bool,
        sleep: &dyn Fn(Duration),
    ) -> Result<bool, SerpAppError> {
        let transport = self.plain_transport(&self.settings()?)?;
        let client = self.cloud_client(&transport);
        let mut interval = sign_in.interval.max(1);
        let deadline = Instant::now() + Duration::from_secs(sign_in.expires_in);
        while Instant::now() < deadline {
            for _ in 0..interval {
                if cancelled() {
                    return Ok(false);
                }
                sleep(Duration::from_secs(1));
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
    /// [`SerpAppError::Cloud`] when the credential store fails.
    pub fn sign_out(&self) -> Result<(), SerpAppError> {
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
    /// [`SerpAppError::InvalidProvider`] for a source that cannot run on credits; settings
    /// errors.
    pub fn set_source_mode(&self, provider: &str, mode: SourceMode) -> Result<(), SerpAppError> {
        if connector_for(provider).is_none() {
            return Err(SerpAppError::InvalidProvider(format!(
                "`{provider}` cannot run on OctopusOn SEO credits"
            )));
        }
        let mut settings = self.settings()?;
        if mode == SourceMode::Auto {
            settings.credit.remove(provider);
        } else {
            settings.credit.insert(provider.to_owned(), mode);
        }
        self.save_settings(&settings)
    }
}

#[cfg(test)]
#[path = "cloud_tests.rs"]
mod tests;
