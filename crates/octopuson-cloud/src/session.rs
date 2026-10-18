//! The account card of a program: the shared connection, the balance, one-click sign-in and
//! sign-out. Programs without paid sources of their own (the SEO Crawler, the Log Analyser)
//! show it so the OctopusOn SEO account looks and works the same everywhere.

use std::sync::atomic::{AtomicBool, Ordering};

use octopuson_core::http::HttpTransport;
use serde::{Deserialize, Serialize};

use crate::{
    API_VARIABLE, Account, CloudClient, CloudError, DEFAULT_PORTAL, SIGN_IN_TIMEOUT, TokenStore,
    api_base, sign_in_with_browser,
};

/// The account as an account card shows it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AccountStatus {
    /// A program on this computer is connected.
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

/// The shared account of one program.
#[derive(Debug)]
pub struct AccountSession {
    tokens: TokenStore,
    app: String,
    cancel: AtomicBool,
}

impl AccountSession {
    /// The account every OctopusOn SEO program on this computer shares, for the program `app`
    /// (`seo-spider/0.1.0 macos`).
    ///
    /// # Errors
    /// [`CloudError::Invalid`] for an invalid [`API_VARIABLE`] or no data folder.
    pub fn shared(app: impl Into<String>) -> Result<Self, CloudError> {
        let api = api_base(std::env::var(API_VARIABLE).ok().as_deref())?;
        Ok(Self::with_tokens(TokenStore::shared(api)?, app))
    }

    /// An account kept in `tokens` (tests).
    pub fn with_tokens(tokens: TokenStore, app: impl Into<String>) -> Self {
        Self {
            tokens,
            app: app.into(),
            cancel: AtomicBool::new(false),
        }
    }

    fn client<'a>(&self, transport: &'a dyn HttpTransport) -> CloudClient<'a> {
        CloudClient::new(transport, self.tokens.base(), self.app.clone())
    }

    /// The connection and the balance. A token the website revoked is forgotten.
    pub fn status(&self, transport: &dyn HttpTransport) -> AccountStatus {
        let mut status = AccountStatus {
            signed_in: false,
            account: None,
            error: None,
            portal_url: DEFAULT_PORTAL.to_owned(),
            api: self.tokens.base().to_owned(),
        };
        let token = match self.tokens.load() {
            Ok(Some(token)) => token,
            Ok(None) => return status,
            Err(error) => {
                status.error = Some(error.to_string());
                return status;
            }
        };
        status.signed_in = true;
        match self.client(transport).account(&token) {
            Ok(account) => {
                account.portal_url.clone_into(&mut status.portal_url);
                status.account = Some(account);
            }
            Err(CloudError::SignedOut) => {
                let _ = self.tokens.clear();
                status.signed_in = false;
                status.error = Some(CloudError::SignedOut.to_string());
            }
            Err(error) => status.error = Some(error.to_string()),
        }
        status
    }

    /// Signs in with one click through the browser; `false` when [`Self::cancel`] stopped it.
    ///
    /// # Errors
    /// See [`sign_in_with_browser`]; [`CloudError::Secrets`] when the token cannot be saved.
    pub fn connect(
        &self,
        transport: &dyn HttpTransport,
        open: &dyn Fn(&str) -> bool,
    ) -> Result<bool, CloudError> {
        self.cancel.store(false, Ordering::SeqCst);
        let client = self.client(transport);
        let portal = client
            .catalog(None)
            .map_or_else(|_| DEFAULT_PORTAL.to_owned(), |catalog| catalog.portal_url);
        let cancelled = || self.cancel.load(Ordering::SeqCst);
        match sign_in_with_browser(
            &client,
            &portal,
            &self.app,
            open,
            &cancelled,
            SIGN_IN_TIMEOUT,
        )? {
            Some(token) => {
                self.tokens.save(&token)?;
                Ok(true)
            }
            None => Ok(false),
        }
    }

    /// Stops a sign-in that waits for the browser.
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::SeqCst);
    }

    /// Disconnects every OctopusOn SEO program on this computer; the gateway forgets the token
    /// when it can be reached.
    ///
    /// # Errors
    /// [`CloudError::Secrets`] when the account file cannot be written.
    pub fn sign_out(&self, transport: &dyn HttpTransport) -> Result<(), CloudError> {
        if let Some(token) = self.tokens.load()? {
            let _ = self.client(transport).sign_out(&token);
            self.tokens.clear()?;
        }
        Ok(())
    }

    /// An address on the account site: `credits`, `devices`, `usage` or the dashboard.
    pub fn page(&self, transport: &dyn HttpTransport, page: &str) -> String {
        let portal = self
            .client(transport)
            .catalog(None)
            .map_or_else(|_| DEFAULT_PORTAL.to_owned(), |catalog| catalog.portal_url);
        let path = match page {
            "credits" => "/kredi",
            "devices" => "/jetonlar",
            "usage" => "/kullanim",
            _ => "/",
        };
        format!("{}{path}", portal.trim_end_matches('/'))
    }
}
