//! OctopusOn SEO credits for the paid search APIs of a scan.
//!
//! Serper and SerpApi run on the user's own key or on OctopusOn SEO credits ([`SourceMode`],
//! `auto` by default: the own key when there is one, else credits when an account is
//! connected). On credits the scan gives the source [`PLACEHOLDER_KEY`] instead of a key, and a
//! [`CreditTransport`] sends its requests to the OctopusOn SEO gateway, which forwards them with
//! OctopusOn SEO's provider account and charges the connected account. A source on its own key is
//! not among the transport's connectors, so neither its requests nor its key reach the gateway.
//!
//! The search calls used to go out on the scan's own `ureq` agent. Serper and SerpApi now use the
//! `seonexus_core::http::HttpTransport` port (`seonexus_net::ApiClient`) wrapped in
//! [`CreditTransport`], instead of a second rewrite here: they get the routing, header filtering
//! and charge reporting that SERP Checker and SEO Keyword Research use, tested once in `seonexus-cloud`,
//! and tests replace the network with fakes. The free result pages, archives and sitemaps stay on
//! the agent, which sends browser headers.
//!
//! Two sources cannot run on credits, so they keep the agent and the user's key:
//! - Google Custom Search JSON API (`customsearch.googleapis.com`): the gateway has no connector,
//!   and Google's terms very likely forbid reselling its results. A connector would need Google's
//!   written permission, OctopusOn SEO's own key and search engine id (the `key` and `cx` query
//!   parameters stripped and injected) and a fixed price per request.
//! - The Yandex XML endpoint (`yandex.com.tr/search/xml`, `user` and `key` in the query): the
//!   gateway's `yandex-search` connector serves Yandex Search API v2 (`POST
//!   searchapi.api.cloud.yandex.net/v2/web/search`, an `Api-Key` header and a folder id in the
//!   body), which the Finder does not call. Either the Finder moves to Search API v2, as SERP
//!   Finder did, and uses that connector, or a new connector injects `user` and `key` into the
//!   query and prices each XML answer. Yandex has to agree to the resale either way.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

use seonexus_cloud::{
    Account, Catalog, CloudClient, CloudError, Connector, CreditTransport, DEFAULT_PORTAL,
    DevicePoll, PLACEHOLDER_KEY, SIGN_IN_TIMEOUT, SignIn, SourceMode, TokenStore, Via, choose,
    connector_for, sign_in_with_browser,
};
use seonexus_core::http::{HttpRequest, HttpResponse, HttpTransport, TransportError};
use seonexus_net::ApiClient;
use serde::{Deserialize, Serialize};

use crate::model::{ApiConfig, IndexEngine};
use crate::{FinderError, FinderWorkspace};

/// How long the connector catalog is trusted before it is asked again.
const CATALOG_TTL: Duration = Duration::from_secs(300);

/// How long a token read from the shared account file is trusted: another OctopusOn SEO program
/// may sign in or out meanwhile, and this one notices within this time.
const TOKEN_TTL: Duration = Duration::from_secs(3);

/// Timeout of the account requests.
const ACCOUNT_TIMEOUT: Duration = Duration::from_secs(30);

/// The settings file in the data folder.
const SETTINGS_FILE: &str = "settings.json";

/// A paid search source of the Finder.
#[derive(Debug, Clone, Copy)]
struct PaidSource {
    /// Source id in the settings and the gateway's naming (`serper`).
    id: &'static str,
    /// Display name.
    name: &'static str,
    /// The scan engine it feeds.
    engine: IndexEngine,
    /// The only address it calls; on credits the connector must serve it.
    origin: &'static str,
}

/// Sources that can run on credits.
const CREDIT_SOURCES: [PaidSource; 2] = [
    PaidSource {
        id: "serper",
        name: "Serper.dev",
        engine: IndexEngine::SerperApi,
        origin: "https://google.serper.dev",
    },
    PaidSource {
        id: "serpapi",
        name: "SerpApi",
        engine: IndexEngine::SerpApi,
        origin: "https://serpapi.com",
    },
];

/// Sources that only run on the user's key (see the module documentation).
const OWN_KEY_SOURCES: [PaidSource; 2] = [
    PaidSource {
        id: "google-cse",
        name: "Google Custom Search",
        engine: IndexEngine::GoogleApi,
        origin: "https://customsearch.googleapis.com",
    },
    PaidSource {
        id: "yandex-xml",
        name: "Yandex XML",
        engine: IndexEngine::YandexApi,
        origin: "https://yandex.com.tr",
    },
];

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

impl From<CloudError> for FinderError {
    fn from(error: CloudError) -> Self {
        Self::Cloud(error.to_string())
    }
}

/// The Finder's settings (`settings.json` in the data folder).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FinderSettings {
    /// How each paid source is paid (`serper`, `serpapi`); a missing source is `auto`.
    #[serde(default)]
    pub credit: BTreeMap<String, SourceMode>,
}

/// The account as the engines page shows it.
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

/// A paid search source and how the next scan pays for it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CreditSource {
    /// Source id (`serper`, `serpapi`, `google-cse`, `yandex-xml`).
    pub id: String,
    /// Display name.
    pub name: String,
    /// The scan engine it feeds.
    pub engine: IndexEngine,
    /// It can run on OctopusOn SEO credits.
    pub creditable: bool,
    /// What the user chose (always `own` for a source that cannot run on credits).
    pub mode: SourceMode,
    /// The user's key is filled in.
    pub own_key: bool,
    /// How the next scan pays for it; `None` when it does not run.
    pub via: Option<Via>,
    /// The gateway's offer, when an account is connected and the source is on offer.
    pub credit: Option<CreditOffer>,
}

/// How the paid sources of one scan are reached.
#[derive(Debug, Clone)]
pub(crate) struct CreditPlan {
    /// The scan's API settings: the placeholder key for sources on credits, no key for
    /// creditable sources that cannot run now, the user's key otherwise.
    pub(crate) api: ApiConfig,
    /// Engines on credits.
    pub(crate) engines: Vec<IndexEngine>,
    /// Their source ids.
    pub(crate) sources: Vec<String>,
    /// Their gateway connectors.
    pub(crate) connectors: Vec<Connector>,
    /// The account token (empty when nothing runs on credits).
    pub(crate) token: String,
}

/// A borrowed transport, so a scan can wrap the caller's transport in a [`CreditTransport`].
pub(crate) struct Borrowed<'a>(pub(crate) &'a dyn HttpTransport);

impl HttpTransport for Borrowed<'_> {
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
        "index-404-finder/{} {}",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS
    )
}

/// The user's key of a paid source in the scan's API settings.
fn key_of(api: &mut ApiConfig, engine: IndexEngine) -> Option<&mut String> {
    match engine {
        IndexEngine::SerperApi => Some(&mut api.serper_api_key),
        IndexEngine::SerpApi => Some(&mut api.serpapi_key),
        IndexEngine::GoogleApi => Some(&mut api.google_api_key),
        IndexEngine::YandexApi => Some(&mut api.yandex_api_key),
        _ => None,
    }
}

/// Whether the user filled in what a paid source needs.
fn own_ready(api: &ApiConfig, engine: IndexEngine) -> bool {
    match engine {
        IndexEngine::SerperApi => api.has_serper_api(),
        IndexEngine::SerpApi => api.has_serpapi(),
        IndexEngine::GoogleApi => api.has_google_api(),
        IndexEngine::YandexApi => api.has_yandex_api(),
        _ => false,
    }
}

impl FinderWorkspace {
    /// The saved account token, read again from the shared account file every few seconds.
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

    /// The transport of the search APIs and the account, without credit routing.
    ///
    /// # Errors
    /// [`FinderError::InvalidSettings`] when the HTTP client cannot be built.
    pub fn api_transport(&self, timeout: Duration) -> Result<ApiClient, FinderError> {
        let _ = self;
        ApiClient::new(
            concat!("octopusSEO-Index-404-Finder/", env!("CARGO_PKG_VERSION")),
            timeout,
            None,
        )
        .map_err(|error| FinderError::InvalidSettings(error.to_string()))
    }

    fn account_transport(&self) -> Result<ApiClient, FinderError> {
        self.api_transport(ACCOUNT_TIMEOUT)
    }

    fn settings_path(&self) -> PathBuf {
        self.data_dir().join(SETTINGS_FILE)
    }

    /// The saved settings, or the defaults when there is no file yet.
    ///
    /// # Errors
    /// [`FinderError::Io`] when the file cannot be read; [`FinderError::InvalidSettings`] when it
    /// is not valid.
    pub fn settings(&self) -> Result<FinderSettings, FinderError> {
        let path = self.settings_path();
        match std::fs::read(&path) {
            Ok(bytes) => serde_json::from_slice(&bytes).map_err(|error| {
                FinderError::InvalidSettings(format!("{}: {error}", path.display()))
            }),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                Ok(FinderSettings::default())
            }
            Err(source) => Err(FinderError::Io { path, source }),
        }
    }

    fn save_settings(&self, settings: &FinderSettings) -> Result<(), FinderError> {
        let path = self.settings_path();
        let io = |source| FinderError::Io {
            path: path.clone(),
            source,
        };
        std::fs::create_dir_all(self.data_dir()).map_err(io)?;
        let json = serde_json::to_vec_pretty(settings)
            .map_err(|error| FinderError::InvalidSettings(error.to_string()))?;
        // Write next to the file and rename, so a crash never leaves half a file.
        let partial = path.with_extension("json.partial");
        std::fs::write(&partial, json).map_err(io)?;
        std::fs::rename(&partial, &path).map_err(io)
    }

    /// The catalog as last read, however old (no network).
    fn cached_catalog(&self) -> Option<Catalog> {
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

    /// The connector a source runs on with credits: signed in, on offer, and serving the one
    /// address the source calls.
    fn credit_connector(&self, source: &PaidSource) -> Option<Connector> {
        let id = connector_for(source.id)?;
        self.cloud_token()?;
        self.cached_catalog()?
            .connector(id)
            .filter(|connector| connector.upstream.trim_end_matches('/') == source.origin)
            .cloned()
    }

    /// What the user chose for a source.
    fn source_mode(settings: &FinderSettings, source: &str) -> SourceMode {
        settings.credit.get(source).copied().unwrap_or_default()
    }

    /// How a creditable source is paid with `api`, if it can run at all.
    fn via(&self, settings: &FinderSettings, api: &ApiConfig, source: &PaidSource) -> Option<Via> {
        choose(
            Self::source_mode(settings, source.id),
            own_ready(api, source.engine),
            self.credit_connector(source).is_some(),
        )
    }

    /// The paid search sources and how a scan with `api` pays for them. Reads the connector
    /// catalog again when an account is connected and the catalog is old.
    ///
    /// # Errors
    /// Unreadable settings.
    pub fn credit_sources(&self, api: &ApiConfig) -> Result<Vec<CreditSource>, FinderError> {
        if self.cloud_token().is_some() {
            self.refresh_catalog(&self.account_transport()?);
        }
        let settings = self.settings()?;
        let mut sources: Vec<CreditSource> = CREDIT_SOURCES
            .iter()
            .map(|source| {
                let offer = self.credit_connector(source);
                CreditSource {
                    id: source.id.to_owned(),
                    name: source.name.to_owned(),
                    engine: source.engine,
                    creditable: true,
                    mode: Self::source_mode(&settings, source.id),
                    own_key: own_ready(api, source.engine),
                    via: self.via(&settings, api, source),
                    credit: offer.map(|connector| CreditOffer {
                        connector: connector.id,
                        price_multiplier: connector.price_multiplier,
                    }),
                }
            })
            .collect();
        sources.extend(OWN_KEY_SOURCES.iter().map(|source| {
            let own = own_ready(api, source.engine);
            CreditSource {
                id: source.id.to_owned(),
                name: source.name.to_owned(),
                engine: source.engine,
                creditable: false,
                mode: SourceMode::Own,
                own_key: own,
                via: own.then_some(Via::Own),
                credit: None,
            }
        }));
        Ok(sources)
    }

    /// How the paid sources of a scan with `api` are reached; the catalog is read over
    /// `transport` when it is old.
    pub(crate) fn credit_plan(
        &self,
        api: &ApiConfig,
        transport: &dyn HttpTransport,
    ) -> Result<CreditPlan, FinderError> {
        let settings = self.settings()?;
        let token = self.cloud_token();
        if token.is_some() {
            self.refresh_catalog(transport);
        }
        let mut plan = CreditPlan {
            api: api.clone(),
            engines: Vec::new(),
            sources: Vec::new(),
            connectors: Vec::new(),
            token: String::new(),
        };
        for source in &CREDIT_SOURCES {
            let via = self.via(&settings, api, source);
            let connector = self.credit_connector(source);
            let Some(key) = key_of(&mut plan.api, source.engine) else {
                continue;
            };
            match (via, connector) {
                (Some(Via::Own), _) => {}
                (Some(Via::Credit), Some(connector)) => {
                    // Never the user's key: the gateway adds OctopusOn SEO's own.
                    PLACEHOLDER_KEY.clone_into(key);
                    plan.engines.push(source.engine);
                    plan.sources.push(source.id.to_owned());
                    plan.connectors.push(connector);
                }
                _ => key.clear(),
            }
        }
        if !plan.connectors.is_empty() {
            plan.token = token.unwrap_or_default();
        }
        Ok(plan)
    }

    /// Wraps `transport` so the requests of `plan`'s sources on credits go to the gateway.
    pub(crate) fn credit_transport<'a>(
        &self,
        plan: &CreditPlan,
        transport: &'a dyn HttpTransport,
    ) -> CreditTransport<Borrowed<'a>> {
        CreditTransport::new(
            Borrowed(transport),
            self.cloud_api(),
            plan.token.clone(),
            app_name(),
            plan.connectors.clone(),
        )
    }

    /// The account and its balance.
    ///
    /// # Errors
    /// [`FinderError::Cloud`] when the account file cannot be written.
    pub fn cloud_status(&self) -> Result<CloudStatus, FinderError> {
        self.cloud_status_with(&self.account_transport()?)
    }

    pub(crate) fn cloud_status_with(
        &self,
        transport: &dyn HttpTransport,
    ) -> Result<CloudStatus, FinderError> {
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
        match self.cloud_client(transport).account(&token) {
            Ok(account) => {
                account.portal_url.clone_into(&mut status.portal_url);
                status.account = Some(account);
                self.refresh_catalog(transport);
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
    /// is signed in there, comes straight back to this program. `false` when `cancelled`
    /// stopped it.
    ///
    /// # Errors
    /// [`FinderError::Cloud`] when no browser opens, nobody signs in in time or the gateway
    /// cannot be reached.
    pub fn connect(
        &self,
        open: &dyn Fn(&str) -> bool,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<bool, FinderError> {
        self.connect_with(&self.account_transport()?, open, cancelled)
    }

    pub(crate) fn connect_with(
        &self,
        transport: &dyn HttpTransport,
        open: &dyn Fn(&str) -> bool,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<bool, FinderError> {
        let client = self.cloud_client(transport);
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
        self.refresh_catalog(transport);
        Ok(true)
    }

    /// Starts signing in with a code (machines without a browser): the person opens
    /// [`SignIn::verification_uri_complete`] on any device and approves the code.
    ///
    /// # Errors
    /// [`FinderError::Cloud`] when the gateway cannot be reached.
    pub fn start_sign_in(&self) -> Result<SignIn, FinderError> {
        Ok(self
            .cloud_client(&self.account_transport()?)
            .start_sign_in()?)
    }

    /// Waits for the person to approve a sign-in and saves the token; `false` when stopped.
    ///
    /// # Errors
    /// [`FinderError::Cloud`] when the code was denied or expired.
    pub fn finish_sign_in(
        &self,
        sign_in: &SignIn,
        cancelled: &dyn Fn() -> bool,
        sleep: &dyn Fn(Duration),
    ) -> Result<bool, FinderError> {
        self.finish_sign_in_with(&self.account_transport()?, sign_in, cancelled, sleep)
    }

    pub(crate) fn finish_sign_in_with(
        &self,
        transport: &dyn HttpTransport,
        sign_in: &SignIn,
        cancelled: &dyn Fn() -> bool,
        sleep: &dyn Fn(Duration),
    ) -> Result<bool, FinderError> {
        let client = self.cloud_client(transport);
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
                    self.refresh_catalog(transport);
                    return Ok(true);
                }
            }
        }
        Err(CloudError::Expired.into())
    }

    /// Signs out: the gateway stops accepting the token, and it is forgotten on this computer
    /// (in every OctopusOn SEO program, which share it).
    ///
    /// # Errors
    /// [`FinderError::Cloud`] when the account file cannot be written.
    pub fn sign_out(&self) -> Result<(), FinderError> {
        if let Some(token) = self.cloud_token() {
            if let Ok(transport) = self.account_transport() {
                // Signing out here must work offline too.
                let _ = self.cloud_client(&transport).sign_out(&token);
            }
            self.cloud.tokens.clear()?;
        }
        self.set_cloud_token(None);
        Ok(())
    }

    /// Chooses how a paid source (`serper`, `serpapi`) is paid.
    ///
    /// # Errors
    /// [`FinderError::InvalidSettings`] for a source that cannot run on credits; settings
    /// errors.
    pub fn set_source_mode(&self, source: &str, mode: SourceMode) -> Result<(), FinderError> {
        let Some(source) = CREDIT_SOURCES.iter().find(|known| known.id == source) else {
            let names: Vec<&str> = CREDIT_SOURCES.iter().map(|known| known.id).collect();
            return Err(FinderError::InvalidSettings(format!(
                "`{source}` cannot run on OctopusOn SEO credits (choose {})",
                names.join(" or ")
            )));
        };
        let mut settings = self.settings()?;
        if mode == SourceMode::Auto {
            settings.credit.remove(source.id);
        } else {
            settings.credit.insert(source.id.to_owned(), mode);
        }
        self.save_settings(&settings)
    }
}

#[cfg(test)]
#[path = "cloud_tests.rs"]
mod tests;
