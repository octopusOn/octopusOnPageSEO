//! Use cases of the OctopusOn GEO-SEO Platform.
//!
//! Every operation the program offers is a command of one [`Router`]: the desktop program calls
//! it through a single Tauri command, the web server through `POST /api/call/{command}`, both
//! with the same JSON arguments and answers. Long work runs as jobs ([`jobs`]) whose progress
//! goes to an [`EventSink`] the host provides: window events on the desktop, a per-account
//! event stream on the web.
//!
//! A [`Workspace`] is one data folder: the program's own on the desktop, one per account on
//! the web. It holds the database, the user's own API keys, the OctopusOn SEO account token
//! and the settings. Paid sources run on the user's own key or on OctopusOn SEO credits
//! through the gateway ([`SourceMode`](octopuson_cloud::SourceMode)).

mod cloud;
pub mod fetch;
mod google;
pub mod jobs;
pub mod mail;
pub mod mcp;
pub mod modules;
mod platform;
mod report;
pub mod router;
pub mod settings;
pub mod sources;

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use octopuson_cloud::{CloudError, TokenStore};
use octopuson_core::Timestamp;
use octopuson_core::http::HttpTransport;
use octopuson_dataforseo::DfsError;
use octopuson_platform::{PlatformStore, PlatformStoreError};
use octopuson_secrets::{SecretError, Secrets, default_backend};
use serde::{Deserialize, Serialize};

pub use cloud::{AccountStatus, app_name};
pub use google::{GoogleStatus, GoogleWeb};
pub use jobs::{EventSink, Job, JobControl, JobManager, JobStatus};
pub use platform::Platform;
pub use router::{CallError, Context, ErrorCode, Router};

/// Environment variable naming another data directory (tests, portable use).
pub const DATA_DIR_VARIABLE: &str = "OCTOPUSON_PLATFORM_DATA_DIR";

/// Program name, also the data folder and credential store service on the desktop.
pub const APPLICATION: &str = "OctopusOn GEO-SEO Platform";

/// Qualifier of the data folder.
const QUALIFIER: &str = "com";

/// Organisation of the data folder.
const ORGANIZATION: &str = "OctopusOn";

/// How long a store held by other work is waited for.
const STORE_WAIT: Duration = Duration::from_secs(5);

/// Where the program runs: what it may do differs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Host {
    /// The desktop program: one person, data on this computer, the browser opens locally.
    Desktop,
    /// The web application: one workspace per account on the server, signed in through the
    /// account site.
    Web,
}

/// Errors of the use cases.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum AppError {
    /// A value the user gave cannot be used.
    #[error("{0}")]
    Invalid(String),
    /// No such record.
    #[error("{0} not found")]
    NotFound(String),
    /// The data is in use by other work for longer than waited.
    #[error("{0}")]
    Busy(String),
    /// A paid source has neither an own key nor credits to run on.
    #[error("{0}")]
    NoSource(String),
    /// The month's budget for a source would be passed.
    #[error("{0}")]
    Budget(String),
    /// A key or the account was refused.
    #[error("{0}")]
    Auth(String),
    /// No balance left with the provider or in credits.
    #[error("{0}")]
    Quota(String),
    /// Too many requests.
    #[error("too many requests, try again later")]
    RateLimited {
        /// Seconds to wait, when known.
        retry_after: Option<u64>,
    },
    /// The service is unavailable.
    #[error("{0}")]
    Unavailable(String),
    /// The provider refused the request or answered something unreadable.
    #[error("{0}")]
    Provider(String),
    /// The command is not offered where the program runs.
    #[error("{0}")]
    Unsupported(String),
    /// The OctopusOn SEO account cannot be used.
    #[error("{0}")]
    Account(String),
    /// Storage or another internal failure.
    #[error("{0}")]
    Internal(String),
}

impl From<PlatformStoreError> for AppError {
    fn from(error: PlatformStoreError) -> Self {
        match error {
            PlatformStoreError::Invalid(message) => Self::Invalid(message),
            PlatformStoreError::NotFound(what) => Self::NotFound(what),
            PlatformStoreError::Busy(_) => Self::Busy(error.to_string()),
            other => Self::Internal(other.to_string()),
        }
    }
}

impl From<DfsError> for AppError {
    fn from(error: DfsError) -> Self {
        let text = format!("DataForSEO: {error}");
        match error {
            DfsError::Config(message) => Self::Invalid(message),
            DfsError::Auth(_) => Self::Auth(text),
            DfsError::Quota(_) => Self::Quota(text),
            DfsError::RateLimited { retry_after } => Self::RateLimited { retry_after },
            DfsError::Unavailable(_) | DfsError::Transport(_) => Self::Unavailable(text),
            DfsError::Rejected(_) | DfsError::Invalid(_) => Self::Provider(text),
        }
    }
}

impl From<CloudError> for AppError {
    fn from(error: CloudError) -> Self {
        Self::Account(error.to_string())
    }
}

impl From<SecretError> for AppError {
    fn from(error: SecretError) -> Self {
        Self::Internal(error.to_string())
    }
}

/// One data folder of the platform.
pub struct Workspace {
    data_dir: PathBuf,
    host: Host,
    secrets: Secrets,
    cloud: cloud::CloudState,
    transport: Option<Arc<dyn HttpTransport>>,
    fetcher: Option<Arc<dyn octopuson_crawl::Fetcher>>,
    google_web: Option<GoogleWeb>,
    google: Mutex<google::GoogleState>,
    mailer: Option<Arc<dyn mail::Mailer>>,
    public_url: Option<String>,
    printer: Option<Arc<Printer>>,
}

/// Prints an HTML page to PDF (tests stand in for the browser).
pub type Printer = dyn Fn(&str) -> Result<Vec<u8>, String> + Send + Sync;

impl std::fmt::Debug for Workspace {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Workspace")
            .field("data_dir", &self.data_dir)
            .field("host", &self.host)
            .field("secrets", &self.secrets)
            .finish_non_exhaustive()
    }
}

impl Workspace {
    /// The desktop workspace in `data_dir`, or in the platform's application data folder. The
    /// account token is the one every OctopusOn SEO program shares.
    ///
    /// # Errors
    /// [`AppError::Internal`] when the platform reports no data folder; [`AppError::Invalid`]
    /// for an unusable gateway address in the environment.
    pub fn resolve(data_dir: Option<PathBuf>) -> Result<Self, AppError> {
        let data_dir = match data_dir {
            Some(dir) => dir,
            None => directories::ProjectDirs::from(QUALIFIER, ORGANIZATION, APPLICATION)
                .ok_or_else(|| AppError::Internal("no application data folder".to_owned()))?
                .data_dir()
                .to_owned(),
        };
        let secrets = Secrets::new(
            default_backend(&data_dir, APPLICATION),
            data_dir.join("secret-bindings.json"),
        );
        let api = gateway()?;
        let tokens = TokenStore::shared(api)?;
        Ok(Self::new(data_dir, Host::Desktop, secrets, tokens))
    }

    /// A workspace with given stores: the web server's per-account folders, and tests.
    pub fn new(data_dir: PathBuf, host: Host, secrets: Secrets, tokens: TokenStore) -> Self {
        Self {
            data_dir,
            host,
            secrets,
            cloud: cloud::CloudState::new(tokens),
            transport: None,
            fetcher: None,
            google_web: None,
            google: Mutex::default(),
            mailer: None,
            public_url: None,
            printer: None,
        }
    }

    /// Sends reports by e-mail through `mailer` (the web server's SMTP).
    #[must_use]
    pub fn with_mailer(mut self, mailer: Arc<dyn mail::Mailer>) -> Self {
        self.mailer = Some(mailer);
        self
    }

    /// The address people open the web version at (`https://app.octopuson.com`), for links in
    /// e-mails.
    #[must_use]
    pub fn with_public_url(mut self, url: &str) -> Self {
        self.public_url = Some(url.trim_end_matches('/').to_owned());
        self
    }

    /// Prints PDFs with `printer` instead of a browser (tests).
    #[must_use]
    pub fn with_printer(mut self, printer: Arc<Printer>) -> Self {
        self.printer = Some(printer);
        self
    }

    /// Signs in to Google with the web server's own client, returning to its address.
    #[must_use]
    pub fn with_google_web(mut self, web: GoogleWeb) -> Self {
        self.google_web = Some(web);
        self
    }

    /// Sends every request through `transport` instead of the network (tests).
    #[must_use]
    pub fn with_transport(mut self, transport: Arc<dyn HttpTransport>) -> Self {
        self.transport = Some(transport);
        self
    }

    /// Crawls through `fetcher` instead of the network (tests).
    #[must_use]
    pub fn with_fetcher(mut self, fetcher: Arc<dyn octopuson_crawl::Fetcher>) -> Self {
        self.fetcher = Some(fetcher);
        self
    }

    /// The fetcher set for tests, if any.
    pub(crate) fn test_fetcher(&self) -> Option<&Arc<dyn octopuson_crawl::Fetcher>> {
        self.fetcher.as_ref()
    }

    /// The data folder.
    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    /// Where the program runs.
    pub const fn host(&self) -> Host {
        self.host
    }

    /// The user's own API keys.
    pub fn secrets(&self) -> &Secrets {
        &self.secrets
    }

    /// Opens the database, waiting briefly while other work holds it.
    ///
    /// # Errors
    /// [`AppError::Busy`] when it stays in use; storage errors.
    pub fn store(&self) -> Result<PlatformStore, AppError> {
        let deadline = Instant::now() + STORE_WAIT;
        loop {
            match PlatformStore::open(&self.data_dir) {
                Err(PlatformStoreError::Busy(_)) if Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(20));
                }
                other => return Ok(other?),
            }
        }
    }
}

/// The gateway address: `OCTOPUSSEO_CLOUD_URL` when set, else the public one.
///
/// # Errors
/// [`AppError::Invalid`] for an address that is not https (or http on this computer).
pub fn gateway() -> Result<String, AppError> {
    octopuson_cloud::api_base(std::env::var(octopuson_cloud::API_VARIABLE).ok().as_deref())
        .map_err(|error| AppError::Invalid(error.to_string()))
}

/// The current time, RFC 3339 in whole seconds.
pub fn now() -> String {
    let micros = Timestamp::now().as_unix_micros();
    Timestamp::from_unix_micros(micros - micros.rem_euclid(1_000_000)).to_string()
}

/// The current month, `YYYY-MM`.
pub fn month() -> String {
    now().chars().take(7).collect()
}

#[cfg(test)]
mod tests;
