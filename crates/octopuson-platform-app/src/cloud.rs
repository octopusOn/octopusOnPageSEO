//! The OctopusOn SEO account: credits as the second way to pay for paid sources.
//!
//! A source runs on the user's own key or on credits ([`SourceMode`], `auto` by default: the
//! own key when there is one). In credit mode the source client gets a placeholder key and the
//! transport sends its requests through the gateway, which charges the account.

use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use octopuson_cloud::{
    Account, Catalog, CloudClient, CloudError, Connector, CreditTransport, DEFAULT_PORTAL,
    MeterPrice, Metered, PLACEHOLDER_KEY, PLACEHOLDER_LOGIN, SIGN_IN_TIMEOUT, TokenStore, Via,
    choose, connector_for, sign_in_with_browser,
};
use octopuson_core::http::{HttpRequest, HttpResponse, HttpTransport, TransportError};
use octopuson_net::ApiClient;
use serde::{Deserialize, Serialize};

use crate::settings::Settings;
use crate::sources::{PROVIDERS, provider};
use crate::{AppError, Host, Workspace};

/// How long the connector catalog is trusted before it is asked again.
const CATALOG_TTL: Duration = Duration::from_secs(300);

/// How long a token read from the account file is trusted: another OctopusOn SEO program may
/// sign in or out meanwhile, and this one notices within this time.
const TOKEN_TTL: Duration = Duration::from_secs(3);

/// Time limit of one provider or gateway request.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(120);

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

impl CloudState {
    pub(crate) fn new(tokens: TokenStore) -> Self {
        Self {
            tokens,
            token: Mutex::new(CachedToken::Unread),
            catalog: Mutex::new(None),
        }
    }
}

/// The account as the Sources page shows it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AccountStatus {
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
    /// Where the program runs; on the web the account is the one signed in.
    pub host: Host,
}

/// A transport shared by reference.
pub(crate) struct Shared(Arc<dyn HttpTransport>);

impl HttpTransport for Shared {
    fn send(&self, request: &HttpRequest) -> Result<HttpResponse, TransportError> {
        self.0.send(request)
    }

    fn take_charge_usd(&self) -> Option<f64> {
        self.0.take_charge_usd()
    }
}

/// The program as it names itself to the gateway.
pub fn app_name() -> String {
    format!(
        "geo-seo-platform/{} {}",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS
    )
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

impl Workspace {
    /// The saved account token.
    pub(crate) fn cloud_token(&self) -> Option<String> {
        let mut cached = lock(&self.cloud.token);
        let previous = match &*cached {
            CachedToken::Known(token, at) if at.elapsed() < TOKEN_TTL => return token.clone(),
            CachedToken::Known(token, _) => Some(token.clone()),
            CachedToken::Unread => None,
        };
        // Read again: another OctopusOn SEO program may have signed in or out.
        let token = self.cloud.tokens.load().ok().flatten();
        if previous.is_some_and(|previous| previous != token) {
            *lock(&self.cloud.catalog) = None;
        }
        *cached = CachedToken::Known(token.clone(), Instant::now());
        token
    }

    /// The saved account token, if any (the web server checks it is still there).
    pub fn account_token(&self) -> Option<String> {
        self.cloud_token()
    }

    /// Saves (or forgets) the account token: the web server does after a sign-in.
    ///
    /// # Errors
    /// [`AppError::Internal`] when the credential store fails.
    pub fn set_account_token(&self, token: Option<&str>) -> Result<(), AppError> {
        match token {
            Some(token) => self.cloud.tokens.save(token)?,
            None => self.cloud.tokens.clear()?,
        }
        *lock(&self.cloud.token) = CachedToken::Known(token.map(str::to_owned), Instant::now());
        *lock(&self.cloud.catalog) = None;
        Ok(())
    }

    /// The gateway address.
    pub fn cloud_api(&self) -> &str {
        self.cloud.tokens.base()
    }

    fn cloud_client<'a>(&self, transport: &'a dyn HttpTransport) -> CloudClient<'a> {
        CloudClient::new(transport, self.cloud_api(), app_name())
    }

    /// The price of metered server work (`crawl.page`) and the credit unit, from the catalog;
    /// `None` without an account or when the gateway lists no such work.
    ///
    /// # Errors
    /// Settings errors.
    pub(crate) fn meter_price(&self, sku: &str) -> Result<Option<(MeterPrice, i64)>, AppError> {
        let settings = self.settings()?;
        self.warm_catalog(&settings)?;
        Ok(self.cached_catalog().and_then(|catalog| {
            let unit = catalog.credit_unit_micros;
            catalog
                .meters
                .into_iter()
                .find(|meter| meter.sku == sku)
                .map(|meter| (meter, unit))
        }))
    }

    /// Meters `units` of server work on the account under `key` (charged once per key).
    ///
    /// # Errors
    /// [`AppError::Account`] without an account or when the gateway cannot be reached.
    pub(crate) fn meter(&self, sku: &str, units: u32, key: &str) -> Result<Metered, AppError> {
        let token = self.cloud_token().ok_or_else(|| {
            AppError::Account(
                "connect your OctopusOn SEO account to crawl on the server".to_owned(),
            )
        })?;
        let settings = self.settings()?;
        let transport = self.plain_transport(&settings)?;
        Ok(self
            .cloud_client(&transport)
            .meter(&token, sku, units, key)?)
    }

    /// Sends a message to OctopusOn support with the program's name and version, the page it was
    /// sent from and the error shown there.
    ///
    /// # Errors
    /// [`AppError::Account`] without an account, when the gateway cannot be reached or after too
    /// many messages in an hour.
    pub(crate) fn report_problem(
        &self,
        message: &str,
        page: &str,
        error: &str,
    ) -> Result<(), AppError> {
        let token = self.cloud_token().ok_or_else(|| {
            AppError::Account(
                "connect your OctopusOn SEO account to write to support, or write to destek@octopuson.com".to_owned(),
            )
        })?;
        let settings = self.settings()?;
        let transport = self.plain_transport(&settings)?;
        let context = serde_json::json!({
            "program": crate::APPLICATION,
            "version": env!("CARGO_PKG_VERSION"),
            "page": page,
            "error": error,
        });
        Ok(self
            .cloud_client(&transport)
            .support(&token, message, &context)?)
    }

    /// The catalog as last read, however old (no network).
    pub(crate) fn cached_catalog(&self) -> Option<Catalog> {
        lock(&self.cloud.catalog)
            .as_ref()
            .map(|(_, catalog)| catalog.clone())
    }

    /// Reads the catalog again when it is older than a few minutes and an account is signed
    /// in. A gateway that cannot be reached leaves the last catalog in place.
    pub(crate) fn refresh_catalog(&self, transport: &dyn HttpTransport) {
        let Some(token) = self.cloud_token() else {
            return;
        };
        let fresh = lock(&self.cloud.catalog)
            .as_ref()
            .is_some_and(|(at, _)| at.elapsed() < CATALOG_TTL);
        if fresh {
            return;
        }
        if let Ok(catalog) = self.cloud_client(transport).catalog(Some(&token)) {
            *lock(&self.cloud.catalog) = Some((Instant::now(), catalog));
        }
    }

    /// Reads the catalog when an account is signed in and it is old, so the sources know what
    /// runs on credits.
    ///
    /// # Errors
    /// [`AppError::Invalid`] for an unusable proxy setting.
    pub(crate) fn warm_catalog(&self, settings: &Settings) -> Result<(), AppError> {
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

    /// An own key, when saved for the provider's addresses.
    pub(crate) fn own_secret(&self, provider_id: &str, name: &str) -> Option<String> {
        let definition = provider(provider_id).ok()?;
        self.secrets
            .get(definition.secrets_of, name, definition.origins)
            .ok()
            .flatten()
    }

    /// Whether every own key of a source is available.
    pub(crate) fn own_ready(&self, provider_id: &str) -> bool {
        provider(provider_id).is_ok_and(|definition| {
            definition
                .secrets
                .iter()
                .all(|name| self.own_secret(provider_id, name).is_some())
        })
    }

    /// How a source is paid now, if it can be used at all.
    pub(crate) fn via(&self, settings: &Settings, provider_id: &str) -> Option<Via> {
        choose(
            settings.mode(provider_id),
            self.own_ready(provider_id),
            self.credit_connector(provider_id).is_some(),
        )
    }

    /// The key a source client gets: the user's own, or the placeholder on credits.
    pub(crate) fn credential(&self, via: Via, provider_id: &str, name: &str) -> Option<String> {
        match via {
            Via::Own => self.own_secret(provider_id, name),
            Via::Credit if provider_id == octopuson_dataforseo::ID => {
                Some(PLACEHOLDER_LOGIN.to_owned())
            }
            Via::Credit => Some(PLACEHOLDER_KEY.to_owned()),
        }
    }

    /// Connectors of the sources that run on credits now.
    fn credit_connectors(&self, settings: &Settings) -> Vec<Connector> {
        let mut connectors: Vec<Connector> = Vec::new();
        for definition in PROVIDERS {
            if self.via(settings, definition.id) != Some(Via::Credit) {
                continue;
            }
            if let Some(connector) = self.credit_connector(definition.id)
                && !connectors.iter().any(|known| known.id == connector.id)
            {
                connectors.push(connector);
            }
        }
        connectors
    }

    /// The transport for requests outside the gateway (the gateway itself, free sources).
    ///
    /// # Errors
    /// [`AppError::Invalid`] for an unusable proxy setting.
    pub(crate) fn plain_transport(&self, settings: &Settings) -> Result<Shared, AppError> {
        if let Some(transport) = &self.transport {
            return Ok(Shared(Arc::clone(transport)));
        }
        let proxy = match self.host {
            Host::Desktop => settings.proxy.as_deref(),
            Host::Web => None,
        };
        let client = ApiClient::new(&app_name(), REQUEST_TIMEOUT, proxy)
            .map_err(|error| AppError::Invalid(format!("proxy: {error}")))?;
        Ok(Shared(Arc::new(client)))
    }

    /// The transport for free sources (autocomplete), never through the gateway.
    ///
    /// # Errors
    /// [`AppError`] for unreadable settings or an unusable proxy.
    pub(crate) fn free_transport(&self) -> Result<Shared, AppError> {
        self.plain_transport(&self.settings()?)
    }

    /// The transport for provider requests: those of sources on credits go through the gateway
    /// with the account token, the others directly to the provider.
    ///
    /// # Errors
    /// [`AppError::Invalid`] for an unusable proxy setting.
    pub(crate) fn transport(
        &self,
        settings: &Settings,
    ) -> Result<Box<dyn HttpTransport>, AppError> {
        let plain = self.plain_transport(settings)?;
        let connectors = self.credit_connectors(settings);
        match self.cloud_token() {
            Some(token) if !connectors.is_empty() => Ok(Box::new(CreditTransport::new(
                plain,
                self.cloud_api(),
                token,
                app_name(),
                connectors,
            ))),
            _ => Ok(Box::new(plain)),
        }
    }

    /// Credits per US dollar of provider price for a source on credits: the catalog's
    /// multiplier over the credit unit. `None` when the source is not on credits.
    pub(crate) fn credits_per_usd(&self, provider_id: &str) -> Option<f64> {
        let connector = self.credit_connector(provider_id)?;
        let catalog = self.cached_catalog()?;
        #[allow(clippy::cast_precision_loss)]
        let unit_usd = catalog.credit_unit_micros.max(1) as f64 / 1_000_000.0;
        Some(connector.price_multiplier / unit_usd)
    }

    /// The account and its balance.
    ///
    /// # Errors
    /// [`AppError`] for unreadable settings or an unusable proxy.
    pub fn account_status(&self) -> Result<AccountStatus, AppError> {
        let transport = self.plain_transport(&self.settings()?)?;
        let mut status = AccountStatus {
            signed_in: false,
            account: None,
            error: None,
            portal_url: DEFAULT_PORTAL.to_owned(),
            api: self.cloud_api().to_owned(),
            host: self.host,
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
                // The token was revoked on the account site: forget it.
                self.set_account_token(None)?;
                status.signed_in = false;
                status.error = Some(CloudError::SignedOut.to_string());
            }
            Err(error) => status.error = Some(error.to_string()),
        }
        Ok(status)
    }

    /// Signs in with one click on the desktop: the account site opens in the browser and, for
    /// a person signed in there, comes straight back. `false` when `cancelled` stopped it.
    ///
    /// # Errors
    /// [`AppError::Unsupported`] on the web; [`AppError::Account`] when no browser opens,
    /// nobody signs in in time or the gateway cannot be reached.
    pub fn connect(
        &self,
        open: &dyn Fn(&str) -> bool,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<bool, AppError> {
        if self.host == Host::Web {
            return Err(AppError::Unsupported(
                "the web version is signed in with the account it was opened with".to_owned(),
            ));
        }
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
        self.set_account_token(Some(&token))?;
        self.refresh_catalog(&transport);
        Ok(true)
    }

    /// Signs out: the gateway stops accepting the token, and it is forgotten here.
    ///
    /// # Errors
    /// [`AppError::Internal`] when the credential store fails.
    pub fn sign_out(&self) -> Result<(), AppError> {
        // Signing out must work offline too.
        if let Some(token) = self.cloud_token()
            && let Ok(transport) = self.plain_transport(&self.settings()?)
        {
            let _ = self.cloud_client(&transport).sign_out(&token);
        }
        self.set_account_token(None)
    }

    /// An address on the account site: `credits`, `devices`, `usage` or the dashboard (the
    /// site's own Turkish paths, which it redirects per language).
    pub fn account_page(&self, page: &str) -> String {
        let portal = self
            .cached_catalog()
            .map_or_else(|| DEFAULT_PORTAL.to_owned(), |catalog| catalog.portal_url);
        let path = match page {
            "credits" => "/kredi",
            "devices" => "/jetonlar",
            "usage" => "/kullanim",
            _ => "/",
        };
        format!("{}{path}", portal.trim_end_matches('/'))
    }
}
