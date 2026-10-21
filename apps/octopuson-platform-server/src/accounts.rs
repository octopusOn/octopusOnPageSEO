//! One platform per account: its workspace folder, its jobs and its event stream.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use octopuson_cloud::TokenStore;
use octopuson_core::http::HttpTransport;
use octopuson_platform_app::mail::Mailer;
use octopuson_platform_app::router::{Opener, Router};
use octopuson_platform_app::{EventSink, GoogleWeb, Host, Job, Platform, Workspace};
use octopuson_secrets::Secrets;
use tokio::sync::broadcast;

use crate::sealed::SealedFile;

/// Job changes waiting for a slow event stream before older ones are dropped.
const EVENT_BUFFER: usize = 256;

/// Accounts kept open; idle ones without running jobs are closed beyond this.
const OPEN_ACCOUNTS: usize = 256;

/// How long an account stays open without use.
const IDLE: Duration = Duration::from_secs(30 * 60);

/// Sends an account's job changes to its open event streams.
struct Broadcast(broadcast::Sender<Arc<str>>);

impl EventSink for Broadcast {
    fn job(&self, job: &Job) {
        if let Ok(text) = serde_json::to_string(job) {
            // Nobody listening is fine.
            let _ = self.0.send(text.into());
        }
    }
}

/// An open account.
pub struct Account {
    /// Its platform.
    pub platform: Arc<Platform>,
    events: broadcast::Sender<Arc<str>>,
    used: Mutex<Instant>,
}

impl Account {
    /// A stream of its job changes, as JSON.
    pub fn subscribe(&self) -> broadcast::Receiver<Arc<str>> {
        self.events.subscribe()
    }
}

/// Every open account.
pub struct Accounts {
    root: PathBuf,
    gateway: String,
    master_key: [u8; 32],
    router: Arc<Router>,
    open: Mutex<HashMap<String, Arc<Account>>>,
    google: Option<GoogleWeb>,
    transport: Option<Arc<dyn HttpTransport>>,
    mailer: Option<Arc<dyn Mailer>>,
    public_url: Option<String>,
}

impl std::fmt::Debug for Accounts {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Accounts")
            .field("root", &self.root)
            .finish_non_exhaustive()
    }
}

/// A folder name for an account id: letters, digits, `-` and `_` only.
pub fn folder_name(user_id: &str) -> Option<String> {
    let valid = !user_id.is_empty()
        && user_id.len() <= 128
        && user_id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    valid.then(|| user_id.to_owned())
}

impl Accounts {
    /// Accounts under `root/users`, reaching the gateway at `gateway`.
    pub fn new(root: PathBuf, gateway: String, master_key: [u8; 32]) -> Self {
        Self {
            root,
            gateway,
            master_key,
            router: Arc::new(octopuson_platform_app::modules::router()),
            open: Mutex::new(HashMap::new()),
            google: None,
            transport: None,
            mailer: None,
            public_url: None,
        }
    }

    /// Accounts e-mail their reports through `mailer`.
    #[must_use]
    pub fn with_mailer(mut self, mailer: Arc<dyn Mailer>) -> Self {
        self.mailer = Some(mailer);
        self
    }

    /// Links in e-mails start with `url`.
    #[must_use]
    pub fn with_public_url(mut self, url: &str) -> Self {
        self.public_url = Some(url.to_owned());
        self
    }

    /// Accounts connect Google with the server's own client.
    #[must_use]
    pub fn with_google(mut self, google: GoogleWeb) -> Self {
        self.google = Some(google);
        self
    }

    /// Accounts opened from now on send provider and Google requests through `transport`
    /// (tests).
    pub fn use_transport(&mut self, transport: Arc<dyn HttpTransport>) {
        self.transport = Some(transport);
    }

    fn workspace(&self, folder: &str) -> Workspace {
        let dir = self.root.join("users").join(folder);
        // Keys are the account's own: the server's environment never stands in for them.
        let secrets = Secrets::new(
            Box::new(SealedFile::new(
                dir.join("secrets.sealed"),
                &self.master_key,
            )),
            dir.join("secret-bindings.json"),
        )
        .with_environment(|_| None);
        let tokens = TokenStore::new(
            Box::new(SealedFile::new(
                dir.join("account.sealed"),
                &self.master_key,
            )),
            self.gateway.clone(),
        );
        let mut workspace = Workspace::new(dir, Host::Web, secrets, tokens);
        if let Some(google) = &self.google {
            workspace = workspace.with_google_web(google.clone());
        }
        if let Some(transport) = &self.transport {
            workspace = workspace.with_transport(Arc::clone(transport));
        }
        if let Some(mailer) = &self.mailer {
            workspace = workspace.with_mailer(Arc::clone(mailer));
        }
        if let Some(url) = &self.public_url {
            workspace = workspace.with_public_url(url);
        }
        workspace
    }

    /// The open account of `user_id`, opening it when needed.
    ///
    /// # Errors
    /// A message when the id cannot name a folder.
    pub fn get(&self, user_id: &str) -> Result<Arc<Account>, String> {
        let folder = folder_name(user_id).ok_or_else(|| "invalid account id".to_owned())?;
        let mut open = self.open.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(account) = open.get(&folder) {
            *account.used.lock().unwrap_or_else(PoisonError::into_inner) = Instant::now();
            return Ok(Arc::clone(account));
        }
        if open.len() >= OPEN_ACCOUNTS {
            open.retain(|_, account| {
                let idle = account
                    .used
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .elapsed()
                    > IDLE;
                !idle || account.platform.jobs().running() > 0
            });
        }
        let (sender, _) = broadcast::channel(EVENT_BUFFER);
        // The server opens no browser: pages open account addresses themselves.
        let open_nothing: Opener = Arc::new(|_| false);
        let platform = Platform::with_router(
            self.workspace(&folder),
            Arc::new(Broadcast(sender.clone())),
            open_nothing,
            Arc::clone(&self.router),
        );
        let account = Arc::new(Account {
            platform: Arc::new(platform),
            events: sender,
            used: Mutex::new(Instant::now()),
        });
        open.insert(folder, Arc::clone(&account));
        Ok(account)
    }

    /// Asks every account's running jobs to stop (the server is stopping).
    pub fn stop_all(&self) {
        for account in self
            .open
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .values()
        {
            account.platform.jobs().cancel_all();
        }
    }
}
