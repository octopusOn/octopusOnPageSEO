//! The web server of the OctopusOn GEO-SEO Platform (app.octopuson.com).
//!
//! People sign in with their OctopusOn SEO account on the account site ([`auth`]); the server
//! keeps one workspace per account and offers the platform's commands as
//! `POST /api/call/{command}`, job changes as an event stream ([`api`]), and the web
//! interface — the same one the desktop program shows — as static files. Every account's
//! scheduled work (rank checks, audits, AI runs, Search Console syncs) runs from [`schedule`].

pub mod accounts;
pub mod api;
pub mod auth;
pub mod config;
pub mod mailer;
pub mod mcp;
pub mod schedule;
pub mod sealed;
pub mod shares;
pub mod store;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use axum::Router;
use axum::http::{HeaderName, HeaderValue, header};
use axum::routing::{get, post};
use octopuson_core::http::HttpTransport;
use tower_http::services::{ServeDir, ServeFile};
use tower_http::set_header::SetResponseHeaderLayer;

use crate::accounts::Accounts;
use crate::auth::Pending;
use crate::config::Config;
use crate::store::ServerStore;

/// Everything the handlers share.
pub struct AppState {
    /// Settings.
    pub config: Config,
    /// Accounts and sessions.
    pub store: ServerStore,
    /// Open accounts.
    pub accounts: Accounts,
    /// Sign-ins waiting for the account site to come back, by state.
    pub(crate) pending: Mutex<HashMap<String, Pending>>,
    /// Reaches the gateway for sign-ins.
    pub(crate) gateway: Arc<dyn HttpTransport>,
}

impl std::fmt::Debug for AppState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppState")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

impl AppState {
    /// The server's state: its database opened in the data folder.
    ///
    /// # Errors
    /// A message when the data folder or its database cannot be opened.
    pub fn new(config: Config, gateway: Arc<dyn HttpTransport>) -> Result<Self, String> {
        std::fs::create_dir_all(&config.data_dir)
            .map_err(|error| format!("{}: {error}", config.data_dir.display()))?;
        let store = ServerStore::open(&config.data_dir).map_err(|error| error.to_string())?;
        let mut accounts = Accounts::new(
            config.data_dir.clone(),
            config.gateway.clone(),
            config.master_key,
        );
        accounts = accounts.with_public_url(&config.public_url);
        if let Some((url, from)) = &config.smtp {
            let host = url::Url::parse(&config.public_url)
                .ok()
                .and_then(|address| address.host_str().map(str::to_owned))
                .unwrap_or_else(|| "localhost".to_owned());
            accounts = accounts.with_mailer(Arc::new(mailer::SmtpMailer::new(url, from, &host)?));
        }
        if let Some(client) = &config.google {
            accounts = accounts.with_google(octopuson_platform_app::GoogleWeb {
                client: client.clone(),
                redirect_uri: config.google_callback_url(),
            });
        }
        Ok(Self {
            config,
            store,
            accounts,
            pending: Mutex::new(HashMap::new()),
            gateway,
        })
    }
}

/// Seconds since the Unix epoch.
pub(crate) fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| {
            i64::try_from(elapsed.as_secs()).unwrap_or(i64::MAX)
        })
}

/// `bytes` random bytes in URL-safe base64.
pub(crate) fn random_token(bytes: usize) -> String {
    use base64::Engine as _;
    let mut buffer = vec![0_u8; bytes];
    // Without randomness the server cannot run safely; an empty token fails every check.
    if getrandom::fill(&mut buffer).is_err() {
        return String::new();
    }
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(buffer)
}

const CONTENT_SECURITY: &str = "default-src 'self'; img-src 'self' data:; style-src 'self' 'unsafe-inline'; font-src 'self' data:; connect-src 'self'; frame-ancestors 'none'; base-uri 'none'; object-src 'none'";

/// The whole application.
pub fn app(state: Arc<AppState>) -> Router {
    let ui = state.config.ui_dir.clone();
    let mut router = Router::new()
        .route("/health", get(|| async { "ok" }))
        .route("/auth/login", get(auth::login))
        .route("/auth/callback", get(auth::callback))
        .route("/auth/logout", post(auth::logout))
        .route("/auth/google/callback", get(auth::google_callback))
        .route("/r/{token}", get(shares::shared))
        .route("/mcp", post(mcp::post).get(mcp::other).delete(mcp::other))
        .route("/api/session", get(api::session))
        .route("/api/call/{command}", post(api::call))
        .route("/api/events", get(api::events));
    if state.config.dev_login {
        router = router.route("/auth/dev", get(auth::dev_login));
    }
    let router = match ui {
        Some(dir) => {
            let index = dir.join("index.html");
            router.fallback_service(ServeDir::new(dir).fallback(ServeFile::new(index)))
        }
        None => router.fallback(|| async {
            (
                [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
                "<!doctype html><title>OctopusOn GEO-SEO Platform</title><p>The interface is not built: set PLATFORM_UI_DIR.</p>",
            )
        }),
    };
    let headers: [(HeaderName, &'static str); 5] = [
        (header::CONTENT_SECURITY_POLICY, CONTENT_SECURITY),
        (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
        (header::X_FRAME_OPTIONS, "DENY"),
        (header::REFERRER_POLICY, "same-origin"),
        (
            HeaderName::from_static("permissions-policy"),
            "camera=(), microphone=(), geolocation=()",
        ),
    ];
    headers
        .into_iter()
        .fold(router.with_state(state), |router, (name, value)| {
            router.layer(SetResponseHeaderLayer::if_not_present(
                name,
                HeaderValue::from_static(value),
            ))
        })
}

#[cfg(test)]
mod tests;
