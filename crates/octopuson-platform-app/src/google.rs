//! The user's Google account, for Search Console and Google Analytics: the OAuth client, signing
//! in, and access tokens.
//!
//! - **Desktop.** The user's own OAuth client ("Desktop app" type; its secret in the credential
//!   store), else the one the program was built with (`OCTOPUSON_GOOGLE_CLIENT_ID` and
//!   `OCTOPUSON_GOOGLE_CLIENT_SECRET`, at run or build time). Signing in listens on
//!   `127.0.0.1` and opens the consent page in the browser.
//! - **Web.** The server's own web client returns to its address (`/auth/google/callback`):
//!   [`Workspace::begin_google_login`] gives the consent page and remembers its state and PKCE
//!   verifier for ten minutes, [`Workspace::finish_google_login`] takes the code.
//!
//! The refresh token is kept with the user's keys, bound to Google's token address; access
//! tokens stay in memory until shortly before they expire. Signing out revokes the grant.

use std::path::PathBuf;
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

use octopuson_google::GoogleError;
use octopuson_oauth::{OAuthClient, OAuthError, Pkce, Tokens, scopes};
use serde::{Deserialize, Serialize};

use crate::{AppError, Host, Workspace};

/// Owner of the Google keys in the credential store.
const GOOGLE: &str = "google";

/// Name of the user's own client secret.
const CLIENT_SECRET: &str = "client_secret";

/// Name of the refresh token.
const REFRESH_TOKEN: &str = "refresh_token";

/// Addresses the keys are sent to.
const ORIGINS: &[&str] = &[octopuson_oauth::TOKEN_ORIGIN];

/// The connection's other facts, beside the database.
const FILE: &str = "google.json";

/// Variables naming the desktop program's built-in client.
const CLIENT_ID_VARIABLE: &str = "OCTOPUSON_GOOGLE_CLIENT_ID";
/// The built-in client's secret (not confidential for desktop clients).
const CLIENT_SECRET_VARIABLE: &str = "OCTOPUSON_GOOGLE_CLIENT_SECRET";

/// How long a desktop sign-in waits for the browser.
const SIGN_IN_WAIT: Duration = Duration::from_secs(300);

/// How long a web sign-in may take at Google.
const PENDING_TTL: Duration = Duration::from_secs(600);

/// Web sign-ins remembered at most.
const MAX_PENDING: usize = 8;

/// An access token is renewed this long before it expires.
const EXPIRY_MARGIN: Duration = Duration::from_secs(120);

/// The web server's Google client and return address.
#[derive(Clone)]
pub struct GoogleWeb {
    /// The server's OAuth client ("Web application" type).
    pub client: OAuthClient,
    /// `https://app.octopuson.com/auth/google/callback`.
    pub redirect_uri: String,
}

impl std::fmt::Debug for GoogleWeb {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GoogleWeb")
            .field("client", &self.client)
            .field("redirect_uri", &self.redirect_uri)
            .finish()
    }
}

/// A web sign-in waiting for Google.
struct Pending {
    state: String,
    verifier: String,
    scopes: Vec<String>,
    created: Instant,
}

/// What the workspace keeps in memory.
#[derive(Default)]
pub(crate) struct GoogleState {
    access: Option<(String, Instant)>,
    pending: Vec<Pending>,
}

fn lock(state: &Mutex<GoogleState>) -> std::sync::MutexGuard<'_, GoogleState> {
    state.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The connection's facts that are not secret.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
struct Facts {
    /// The desktop user's own client id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    client_id: Option<String>,
    /// Scopes granted.
    #[serde(default)]
    scopes: Vec<String>,
}

/// Where the Google connection stands.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[allow(clippy::struct_excessive_bools)] // facts the page shows
pub struct GoogleStatus {
    /// A client is there to sign in with.
    pub configured: bool,
    /// The desktop user's own client id, when one is saved.
    pub own_client_id: Option<String>,
    /// The program's built-in client is used.
    pub built_in: bool,
    /// Signed in.
    pub connected: bool,
    /// Search Console may be read.
    pub search_console: bool,
    /// Google Analytics may be read.
    pub analytics: bool,
}

/// What a Google refusal means for the user.
pub(crate) fn google_error(error: GoogleError) -> AppError {
    match error {
        GoogleError::Auth(message) => AppError::Auth(format!(
            "Google refused the sign-in ({message}); connect Google again"
        )),
        GoogleError::Forbidden(message) => AppError::Auth(format!(
            "Google refused access: {message}. Check that this Google account can read the property"
        )),
        GoogleError::RateLimited { retry_after } => AppError::RateLimited { retry_after },
        GoogleError::Unavailable(message) => AppError::Unavailable(format!("Google: {message}")),
        GoogleError::Invalid(message) => AppError::Provider(format!("Google: {message}")),
    }
}

fn oauth_error(error: OAuthError) -> AppError {
    match error {
        OAuthError::Expired => AppError::Auth(
            "the Google sign-in expired or was revoked; connect Google again".to_owned(),
        ),
        OAuthError::Network(message) => AppError::Unavailable(format!("Google: {message}")),
        OAuthError::Token(message) => AppError::Auth(format!("Google: {message}")),
        other => AppError::Invalid(other.to_string()),
    }
}

/// The program's built-in desktop client, if it has one.
fn built_in() -> Option<OAuthClient> {
    let read = |name: &str, built: Option<&'static str>| {
        std::env::var(name)
            .ok()
            .or_else(|| built.map(str::to_owned))
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty())
    };
    Some(OAuthClient {
        client_id: read(
            CLIENT_ID_VARIABLE,
            option_env!("OCTOPUSON_GOOGLE_CLIENT_ID"),
        )?,
        client_secret: read(
            CLIENT_SECRET_VARIABLE,
            option_env!("OCTOPUSON_GOOGLE_CLIENT_SECRET"),
        )?,
    })
}

/// Whether a client id looks like Google's.
fn valid_client_id(id: &str) -> bool {
    id.len() > ".apps.googleusercontent.com".len()
        && id.len() <= 256
        && id.ends_with(".apps.googleusercontent.com")
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_'))
}

/// The scopes asked for.
fn wanted(analytics: bool) -> Vec<&'static str> {
    let mut wanted = vec![scopes::SEARCH_CONSOLE];
    if analytics {
        wanted.push(scopes::ANALYTICS);
    }
    wanted
}

impl Workspace {
    fn google_file(&self) -> PathBuf {
        self.data_dir.join(FILE)
    }

    fn google_facts(&self) -> Facts {
        std::fs::read(self.google_file())
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default()
    }

    fn save_google_facts(&self, facts: &Facts) -> Result<(), AppError> {
        let bytes = serde_json::to_vec_pretty(facts)
            .map_err(|error| AppError::Internal(error.to_string()))?;
        octopuson_secrets::write_atomically(&self.google_file(), &bytes)?;
        Ok(())
    }

    /// The desktop user's own client, when its id and secret are saved.
    fn own_google_client(&self) -> Option<OAuthClient> {
        let client_id = self.google_facts().client_id?;
        let client_secret = self
            .secrets
            .get(GOOGLE, CLIENT_SECRET, ORIGINS)
            .ok()
            .flatten()?;
        Some(OAuthClient {
            client_id,
            client_secret,
        })
    }

    /// The client sign-ins and token renewals use.
    fn google_client(&self) -> Option<OAuthClient> {
        match self.host {
            Host::Web => self.google_web.as_ref().map(|web| web.client.clone()),
            Host::Desktop => self.own_google_client().or_else(built_in),
        }
    }

    /// Where the Google connection stands.
    ///
    /// # Errors
    /// Credential store errors.
    pub fn google_status(&self) -> Result<GoogleStatus, AppError> {
        let facts = self.google_facts();
        let own = (self.host == Host::Desktop)
            .then(|| self.own_google_client())
            .flatten();
        let connected = self.secrets.get(GOOGLE, REFRESH_TOKEN, ORIGINS)?.is_some();
        let granted = |scope: &str| connected && facts.scopes.iter().any(|known| known == scope);
        Ok(GoogleStatus {
            configured: self.google_client().is_some(),
            built_in: self.host == Host::Desktop && own.is_none() && built_in().is_some(),
            own_client_id: own.map(|client| client.client_id),
            connected,
            search_console: granted(scopes::SEARCH_CONSOLE),
            analytics: granted(scopes::ANALYTICS),
        })
    }

    /// Saves the desktop user's own OAuth client ("Desktop app" type); an empty id removes it.
    ///
    /// # Errors
    /// [`AppError::Unsupported`] on the web, [`AppError::Invalid`] for an id that is not
    /// Google's or a missing secret; credential store errors.
    pub fn set_google_client(
        &self,
        client_id: &str,
        client_secret: &str,
    ) -> Result<GoogleStatus, AppError> {
        if self.host == Host::Web {
            return Err(AppError::Unsupported(
                "the web version signs in to Google with its own client".to_owned(),
            ));
        }
        let client_id = client_id.trim();
        let mut facts = self.google_facts();
        if client_id.is_empty() {
            facts.client_id = None;
            self.secrets.delete(GOOGLE, CLIENT_SECRET)?;
        } else {
            if !valid_client_id(client_id) {
                return Err(AppError::Invalid(
                    "client_id: the id of a Google OAuth client ends with .apps.googleusercontent.com"
                        .to_owned(),
                ));
            }
            let client_secret = client_secret.trim();
            if client_secret.is_empty() {
                return Err(AppError::Invalid(
                    "client_secret: enter the client's secret".to_owned(),
                ));
            }
            self.secrets
                .set(GOOGLE, CLIENT_SECRET, client_secret, ORIGINS)?;
            facts.client_id = Some(client_id.to_owned());
        }
        self.save_google_facts(&facts)?;
        self.google_status()
    }

    /// Keeps a grant: the refresh token and the scopes Google granted.
    fn keep_grant(&self, tokens: &Tokens, requested: &[String]) -> Result<GoogleStatus, AppError> {
        let refresh = tokens.refresh_token.as_deref().ok_or_else(|| {
            AppError::Auth("Google sent no refresh token; connect Google again".to_owned())
        })?;
        if tokens.scope.is_some() && !tokens.has_scope(scopes::SEARCH_CONSOLE) {
            return Err(AppError::Invalid(
                "Search Console access was not granted; tick it on Google's page".to_owned(),
            ));
        }
        self.secrets.set(GOOGLE, REFRESH_TOKEN, refresh, ORIGINS)?;
        let mut facts = self.google_facts();
        facts.scopes = requested
            .iter()
            .filter(|scope| tokens.scope.is_none() || tokens.has_scope(scope))
            .cloned()
            .collect();
        self.save_google_facts(&facts)?;
        lock(&self.google).access = Some((
            tokens.access_token.clone(),
            Instant::now() + Duration::from_secs(tokens.expires_in),
        ));
        self.google_status()
    }

    fn no_client(&self) -> AppError {
        AppError::NoSource(match self.host {
            Host::Desktop => "enter the client id and secret of your Google OAuth client (Desktop app type) first".to_owned(),
            Host::Web => "Google sign-in is not set up on this server".to_owned(),
        })
    }

    /// Signs in on the desktop: opens Google's consent page with `open` (which says whether it
    /// could), waits for the browser and keeps the grant.
    ///
    /// # Errors
    /// [`AppError::Unsupported`] on the web, [`AppError::NoSource`] without a client; a
    /// declined, timed-out or refused sign-in.
    pub fn connect_google(
        &self,
        analytics: bool,
        open: &dyn Fn(&str) -> bool,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<GoogleStatus, AppError> {
        if self.host == Host::Web {
            return Err(AppError::Unsupported(
                "the web version signs in to Google through its own address".to_owned(),
            ));
        }
        let client = self.google_client().ok_or_else(|| self.no_client())?;
        let requested = wanted(analytics);
        let authorization = octopuson_oauth::begin(&client, &requested).map_err(oauth_error)?;
        open(&authorization.url);
        let code = authorization
            .wait(SIGN_IN_WAIT, cancelled)
            .map_err(oauth_error)?;
        let transport = self.free_transport()?;
        let tokens = octopuson_oauth::exchange(&transport, &client, &code).map_err(oauth_error)?;
        let requested: Vec<String> = requested.iter().map(|&scope| scope.to_owned()).collect();
        self.keep_grant(&tokens, &requested)
    }

    /// Starts a web sign-in: Google's consent page, returning to the server.
    ///
    /// # Errors
    /// [`AppError::Unsupported`] on the desktop, [`AppError::NoSource`] when the server has no
    /// Google client.
    pub fn begin_google_login(&self, analytics: bool) -> Result<String, AppError> {
        if self.host == Host::Desktop {
            return Err(AppError::Unsupported(
                "the desktop program signs in to Google in the browser itself".to_owned(),
            ));
        }
        let web = self.google_web.as_ref().ok_or_else(|| self.no_client())?;
        let pkce = Pkce::new().map_err(oauth_error)?;
        let state = octopuson_oauth::new_state().map_err(oauth_error)?;
        let requested = wanted(analytics);
        let url =
            octopuson_oauth::consent_url(&web.client, &requested, &web.redirect_uri, &state, &pkce)
                .map_err(oauth_error)?;
        let mut memory = lock(&self.google);
        memory
            .pending
            .retain(|pending| pending.created.elapsed() < PENDING_TTL);
        if memory.pending.len() >= MAX_PENDING {
            memory.pending.remove(0);
        }
        memory.pending.push(Pending {
            state,
            verifier: pkce.verifier,
            scopes: requested.iter().map(|&scope| scope.to_owned()).collect(),
            created: Instant::now(),
        });
        Ok(url)
    }

    /// Finishes a web sign-in with the code and state Google returned.
    ///
    /// # Errors
    /// [`AppError::Invalid`] for a state this workspace did not start (or too old); a refused
    /// code.
    pub fn finish_google_login(&self, state: &str, code: &str) -> Result<GoogleStatus, AppError> {
        let web = self.google_web.as_ref().ok_or_else(|| self.no_client())?;
        let pending = {
            let mut memory = lock(&self.google);
            let index = memory
                .pending
                .iter()
                .position(|pending| {
                    pending.state == state && pending.created.elapsed() < PENDING_TTL
                })
                .ok_or_else(|| {
                    AppError::Invalid(
                        "this Google sign-in was not started here or took too long; try again"
                            .to_owned(),
                    )
                })?;
            memory.pending.remove(index)
        };
        let code = octopuson_oauth::AuthorizationCode::new(
            code.to_owned(),
            pending.verifier,
            web.redirect_uri.clone(),
        );
        let transport = self.free_transport()?;
        let tokens =
            octopuson_oauth::exchange(&transport, &web.client, &code).map_err(oauth_error)?;
        self.keep_grant(&tokens, &pending.scopes)
    }

    /// Signs out of Google: revokes the grant (best effort) and forgets it.
    ///
    /// # Errors
    /// Credential store errors.
    pub fn google_logout(&self) -> Result<GoogleStatus, AppError> {
        if let Some(token) = self.secrets.get(GOOGLE, REFRESH_TOKEN, ORIGINS)?
            && let Ok(transport) = self.free_transport()
        {
            let _ = octopuson_oauth::revoke(&transport, &token);
        }
        self.secrets.delete(GOOGLE, REFRESH_TOKEN)?;
        lock(&self.google).access = None;
        let mut facts = self.google_facts();
        facts.scopes.clear();
        self.save_google_facts(&facts)?;
        self.google_status()
    }

    /// A valid access token, renewed when needed; a revoked grant is forgotten.
    ///
    /// # Errors
    /// [`AppError::NoSource`] when not signed in or without a client; [`AppError::Auth`]
    /// when Google refuses the grant.
    pub(crate) fn google_token(&self) -> Result<String, AppError> {
        if let Some((token, expires)) = &lock(&self.google).access
            && Instant::now() + EXPIRY_MARGIN < *expires
        {
            return Ok(token.clone());
        }
        let client = self.google_client().ok_or_else(|| self.no_client())?;
        let refresh = self
            .secrets
            .get(GOOGLE, REFRESH_TOKEN, ORIGINS)?
            .ok_or_else(|| AppError::NoSource("connect Google first".to_owned()))?;
        let transport = self.free_transport()?;
        match octopuson_oauth::refresh(&transport, &client, &refresh) {
            Ok(tokens) => {
                lock(&self.google).access = Some((
                    tokens.access_token.clone(),
                    Instant::now() + Duration::from_secs(tokens.expires_in),
                ));
                Ok(tokens.access_token)
            }
            Err(OAuthError::Expired) => {
                let _ = self.secrets.delete(GOOGLE, REFRESH_TOKEN);
                Err(oauth_error(OAuthError::Expired))
            }
            Err(error) => Err(oauth_error(error)),
        }
    }

    /// Forgets the access token Google just refused.
    pub(crate) fn drop_google_token(&self) {
        lock(&self.google).access = None;
    }
}
