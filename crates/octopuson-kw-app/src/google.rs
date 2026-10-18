//! The user's Google data: signing in with their own OAuth client, Search Console queries and
//! pages, and Google Ads keyword planning.

use std::collections::HashMap;
use std::sync::OnceLock;
use std::time::Duration;

use octopuson_core::Timestamp;
use octopuson_core::http::HttpTransport;
use octopuson_keyword_sources::google_ads::{AdsAccount, GoogleAds};
pub use octopuson_keyword_sources::search_console::Property;
use octopuson_keyword_sources::search_console::{QueryRequest, QueryRow, SearchConsole};
use octopuson_keyword_sources::{Fetched, SourceContext, SourceError};
use octopuson_keywords::normalize;
use octopuson_kw_store::{
    Added, AttemptOutcome, AttemptRecord, ProjectRow, RunKind, RunStatus, SitePageRecord,
};
use octopuson_oauth::{OAuthClient, OAuthError, scopes};
use octopuson_secrets::SecretState;
use octopuson_text::Language;
use serde::{Deserialize, Serialize};

use crate::keywords::{fill_intents, prepare};
use crate::sources::GOOGLE;
use crate::{KeywordWorkspace, KwAppError, now};

/// How long a sign-in waits for the browser.
const SIGN_IN_WAIT: Duration = Duration::from_secs(300);
/// Most Search Console rows read per import (the API serves 50,000 per day and property).
pub const MAX_GSC_ROWS: u32 = 50_000;
/// Source of keywords from Search Console.
pub const GSC_SOURCE: &str = "gsc";

/// What to connect.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct GoogleScopes {
    /// Search Console, read only.
    pub search_console: bool,
    /// Google Ads keyword planning.
    pub ads: bool,
}

impl Default for GoogleScopes {
    fn default() -> Self {
        Self {
            search_console: true,
            ads: false,
        }
    }
}

/// Where the Google connection stands.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GoogleStatus {
    /// Client id of the user's OAuth client.
    pub client_id: Option<String>,
    /// Whether its secret is saved.
    pub client_secret: bool,
    /// Whether a sign-in is saved.
    pub connected: bool,
    /// Scopes granted.
    pub scopes: Vec<String>,
    /// Google Ads customer id.
    pub ads_customer_id: Option<String>,
}

/// What a Search Console import asks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct SearchConsoleRequest {
    /// The property (`sc-domain:example.com` or `https://www.example.com/`).
    pub site_url: String,
    /// Days, ending three days ago (the newest final data).
    pub days: u32,
    /// Only searches from the project's country.
    pub country_only: bool,
    /// Most rows read.
    pub max_rows: u32,
}

impl Default for SearchConsoleRequest {
    fn default() -> Self {
        Self {
            site_url: String::new(),
            days: 90,
            country_only: true,
            max_rows: MAX_GSC_ROWS,
        }
    }
}

/// What a Search Console import did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchConsoleOutcome {
    /// The run.
    pub run_id: i64,
    /// Rows read.
    pub rows: u64,
    /// Queries added and known.
    pub added: Added,
    /// Pages of the site stored for mapping.
    pub pages: u64,
    /// `YYYY-MM-DD..YYYY-MM-DD`.
    pub period: String,
}

/// Query rows read from Search Console.
struct Fetch {
    rows: Vec<QueryRow>,
    status: RunStatus,
    failure: Option<String>,
}

/// `YYYY-MM-DD` of `days` days ago (UTC).
fn day(days: i64) -> String {
    let civil =
        Timestamp::from_unix_micros(Timestamp::now().as_unix_micros() - days * 86_400_000_000)
            .to_civil();
    format!(
        "{:04}-{:02}-{:02}",
        civil.year(),
        civil.month(),
        civil.day()
    )
}

fn oauth_error(error: &OAuthError) -> KwAppError {
    KwAppError::Invalid(error.to_string())
}

impl KeywordWorkspace {
    /// The user's OAuth client.
    ///
    /// # Errors
    /// [`KwAppError::Invalid`] when the client id or secret is missing.
    pub fn google_client(&self) -> Result<OAuthClient, KwAppError> {
        let client_id = self.settings()?.google.client_id.ok_or_else(|| {
            KwAppError::Invalid(
                "set the client id of your Google OAuth client first (Desktop app type)".to_owned(),
            )
        })?;
        let client_secret = self.secret(GOOGLE, "client_secret").ok_or_else(|| {
            KwAppError::Invalid(
                "save the client secret of your Google OAuth client first".to_owned(),
            )
        })?;
        Ok(OAuthClient {
            client_id,
            client_secret,
        })
    }

    /// Saves the client id (in the settings) and secret (in the keychain) of the user's own
    /// OAuth client.
    ///
    /// # Errors
    /// [`KwAppError::InvalidSettings`] for an invalid id; secret store errors.
    pub fn set_google_client(
        &self,
        client_id: &str,
        client_secret: &str,
    ) -> Result<(), KwAppError> {
        let mut settings = self.settings()?;
        settings.google.client_id = Some(client_id.trim().to_owned());
        settings.validate().map_err(KwAppError::InvalidSettings)?;
        self.secrets()
            .set(
                GOOGLE,
                "client_secret",
                client_secret,
                &[octopuson_oauth::TOKEN_ORIGIN],
            )
            .map_err(|error| KwAppError::Secrets(error.to_string()))?;
        self.save_settings(&settings)
    }

    /// Sets the Google Ads customer (and manager) account.
    ///
    /// # Errors
    /// [`KwAppError::InvalidSettings`] for ids that are not ten digits.
    pub fn set_ads_account(
        &self,
        customer_id: Option<&str>,
        login_customer_id: Option<&str>,
    ) -> Result<(), KwAppError> {
        let mut settings = self.settings()?;
        settings.google.ads_customer_id = customer_id.and_then(GoogleAds::customer);
        settings.google.ads_login_customer_id = login_customer_id.and_then(GoogleAds::customer);
        if customer_id.is_some() && settings.google.ads_customer_id.is_none() {
            return Err(KwAppError::InvalidSettings(
                "the Google Ads customer id must have ten digits".to_owned(),
            ));
        }
        self.save_settings(&settings)
    }

    /// Where the Google connection stands.
    ///
    /// # Errors
    /// Settings and secret store errors.
    pub fn google_status(&self) -> Result<GoogleStatus, KwAppError> {
        let settings = self.settings()?;
        let saved = |name: &str| -> Result<bool, KwAppError> {
            Ok(matches!(
                self.secrets()
                    .state(GOOGLE, name, &[octopuson_oauth::TOKEN_ORIGIN])
                    .map_err(|error| KwAppError::Secrets(error.to_string()))?,
                SecretState::Saved | SecretState::Environment
            ))
        };
        Ok(GoogleStatus {
            client_id: settings.google.client_id,
            client_secret: saved("client_secret")?,
            connected: saved("refresh_token")?,
            scopes: settings.google.scopes,
            ads_customer_id: settings.google.ads_customer_id,
        })
    }

    /// Signs in: opens Google's consent page with `open` (which returns `false` when it could
    /// not, so the caller shows the address), waits for the redirect and saves the refresh
    /// token in the keychain.
    ///
    /// # Errors
    /// [`KwAppError::Invalid`] for a missing client, a declined or timed-out sign-in, or
    /// refused tokens.
    pub fn google_login(
        &self,
        wanted: GoogleScopes,
        open: &dyn Fn(&str) -> bool,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<GoogleStatus, KwAppError> {
        let transport = self.transport(&self.settings()?)?;
        self.google_login_with(wanted, &transport, open, cancelled)
    }

    /// [`KeywordWorkspace::google_login`] over a given transport.
    ///
    /// # Errors
    /// See [`KeywordWorkspace::google_login`].
    pub fn google_login_with(
        &self,
        wanted: GoogleScopes,
        transport: &dyn HttpTransport,
        open: &dyn Fn(&str) -> bool,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<GoogleStatus, KwAppError> {
        let client = self.google_client()?;
        let mut requested = Vec::new();
        if wanted.search_console {
            requested.push(scopes::SEARCH_CONSOLE);
        }
        if wanted.ads {
            requested.push(scopes::ADS);
        }
        if requested.is_empty() {
            return Err(KwAppError::Invalid(
                "choose Search Console, Google Ads or both".to_owned(),
            ));
        }
        let authorization =
            octopuson_oauth::begin(&client, &requested).map_err(|error| oauth_error(&error))?;
        open(&authorization.url);
        let code = authorization
            .wait(SIGN_IN_WAIT, cancelled)
            .map_err(|error| oauth_error(&error))?;
        let tokens = octopuson_oauth::exchange(transport, &client, &code)
            .map_err(|error| oauth_error(&error))?;
        let refresh = tokens.refresh_token.clone().ok_or_else(|| {
            KwAppError::Invalid("Google sent no refresh token; sign in again".to_owned())
        })?;
        self.secrets()
            .set(
                GOOGLE,
                "refresh_token",
                &refresh,
                &[octopuson_oauth::TOKEN_ORIGIN],
            )
            .map_err(|error| KwAppError::Secrets(error.to_string()))?;
        let mut settings = self.settings()?;
        settings.google.scopes = requested
            .iter()
            .filter(|scope| tokens.scope.is_none() || tokens.has_scope(scope))
            .map(|scope| (*scope).to_owned())
            .collect();
        self.save_settings(&settings)?;
        self.google_status()
    }

    /// Signs out: revokes the grant (best effort) and removes the refresh token.
    ///
    /// # Errors
    /// Settings and secret store errors.
    pub fn google_logout(&self) -> Result<(), KwAppError> {
        let transport = self.transport(&self.settings()?)?;
        self.google_logout_with(&transport)
    }

    /// [`KeywordWorkspace::google_logout`] over a given transport.
    ///
    /// # Errors
    /// See [`KeywordWorkspace::google_logout`].
    pub fn google_logout_with(&self, transport: &dyn HttpTransport) -> Result<(), KwAppError> {
        if let Some(token) = self.secret(GOOGLE, "refresh_token") {
            let _ = octopuson_oauth::revoke(transport, &token);
        }
        self.secrets()
            .delete(GOOGLE, "refresh_token")
            .map_err(|error| KwAppError::Secrets(error.to_string()))?;
        let mut settings = self.settings()?;
        settings.google.scopes.clear();
        self.save_settings(&settings)
    }

    /// A fresh access token; a revoked grant is forgotten.
    pub(crate) fn google_token(
        &self,
        transport: &dyn HttpTransport,
    ) -> Result<String, SourceError> {
        let client = self
            .google_client()
            .map_err(|error| SourceError::Config(error.to_string()))?;
        let refresh = self
            .secret(GOOGLE, "refresh_token")
            .ok_or_else(|| SourceError::Config("sign in to Google first".to_owned()))?;
        match octopuson_oauth::refresh(transport, &client, &refresh) {
            Ok(tokens) => Ok(tokens.access_token),
            Err(OAuthError::Expired) => {
                let _ = self.secrets().delete(GOOGLE, "refresh_token");
                Err(SourceError::Auth(OAuthError::Expired.to_string()))
            }
            Err(OAuthError::Network(message)) => Err(SourceError::Network(message)),
            Err(error) => Err(SourceError::Auth(error.to_string())),
        }
    }

    /// The Search Console properties the signed-in user can read.
    ///
    /// # Errors
    /// [`KwAppError::Invalid`] when not signed in or refused.
    pub fn search_console_properties(&self) -> Result<Vec<Property>, KwAppError> {
        let transport = self.transport(&self.settings()?)?;
        self.search_console_properties_with(&transport)
    }

    /// [`KeywordWorkspace::search_console_properties`] over a given transport.
    ///
    /// # Errors
    /// See [`KeywordWorkspace::search_console_properties`].
    pub fn search_console_properties_with(
        &self,
        transport: &dyn HttpTransport,
    ) -> Result<Vec<Property>, KwAppError> {
        let token = self
            .google_token(transport)
            .map_err(|error| KwAppError::Invalid(error.to_string()))?;
        self.with_sources(transport, |context| {
            SearchConsole::properties(context, &token)
        })
        .map_err(|error| KwAppError::Invalid(error.to_string()))
    }

    /// Imports a Search Console property's queries into a project, with the pages that were
    /// seen for them (for site mapping).
    ///
    /// # Errors
    /// [`KwAppError::Invalid`] when not signed in or refused; store errors.
    pub fn import_search_console(
        &self,
        project_id: i64,
        request: &SearchConsoleRequest,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<SearchConsoleOutcome, KwAppError> {
        let transport = self.transport(&self.settings()?)?;
        self.import_search_console_with(project_id, request, &transport, cancelled)
    }

    /// [`KeywordWorkspace::import_search_console`] over a given transport.
    ///
    /// # Errors
    /// See [`KeywordWorkspace::import_search_console`].
    pub fn import_search_console_with(
        &self,
        project_id: i64,
        request: &SearchConsoleRequest,
        transport: &dyn HttpTransport,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<SearchConsoleOutcome, KwAppError> {
        if request.site_url.trim().is_empty() {
            return Err(KwAppError::Invalid(
                "choose a Search Console property".to_owned(),
            ));
        }
        let project = self.project(project_id)?;
        let days = i64::from(request.days.clamp(1, 480));
        let (start, end) = (day(days + 2), day(3));
        let token = self
            .google_token(transport)
            .map_err(|error| KwAppError::Invalid(error.to_string()))?;
        let run_id = self
            .store()?
            .begin_run(RunKind::Map, Some(project_id), 0, 0.0, &now())?;
        let query = QueryRequest {
            site_url: request.site_url.trim(),
            start: &start,
            end: &end,
            country: request
                .country_only
                .then_some(project.spec.country.as_str()),
            start_row: 0,
        };
        let fetched = self.fetch_queries(
            run_id,
            &token,
            &query,
            request.max_rows,
            transport,
            cancelled,
        )?;
        let period = format!("{start}..{end}");
        let (added, pages) = if fetched.status == RunStatus::Failed {
            (Added::default(), 0)
        } else {
            self.store_queries(&project, request.site_url.trim(), &fetched.rows, &period)?
        };
        let count = u32::try_from(fetched.rows.len()).unwrap_or(u32::MAX);
        self.store()?.finish_run(
            run_id,
            fetched.status,
            count,
            &now(),
            fetched.failure.as_deref(),
        )?;
        if fetched.status == RunStatus::Failed {
            return Err(KwAppError::Invalid(fetched.failure.unwrap_or_default()));
        }
        Ok(SearchConsoleOutcome {
            run_id,
            rows: u64::from(count),
            added,
            pages,
            period,
        })
    }

    /// Reads query rows page by page, recording each request.
    fn fetch_queries(
        &self,
        run_id: i64,
        token: &str,
        query: &QueryRequest<'_>,
        max_rows: u32,
        transport: &dyn HttpTransport,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Fetch, KwAppError> {
        let mut fetch = Fetch {
            rows: Vec::new(),
            status: RunStatus::Done,
            failure: None,
        };
        while fetch.rows.len() < max_rows.min(MAX_GSC_ROWS) as usize {
            if cancelled() {
                fetch.status = RunStatus::Cancelled;
                break;
            }
            let page_query = QueryRequest {
                start_row: u32::try_from(fetch.rows.len()).unwrap_or(u32::MAX),
                ..query.clone()
            };
            let result = self.with_sources(transport, |context| {
                SearchConsole::query(context, token, &page_query)
            });
            self.store()?.record_attempt(&AttemptRecord {
                run_id: Some(run_id),
                provider: SearchConsole::ID.to_owned(),
                attempted_at: now(),
                outcome: if result.is_ok() {
                    AttemptOutcome::Success
                } else {
                    AttemptOutcome::Failed
                },
                requests: 1,
                items: result
                    .as_ref()
                    .map_or(0, |page| u32::try_from(page.len()).unwrap_or(u32::MAX)),
                cost_usd: 0.0,
                error: result.as_ref().err().map(ToString::to_string),
                engine: None,
                via: None,
            })?;
            match result {
                Ok(page) => {
                    let full = page.len() >= SearchConsole::ROWS as usize;
                    fetch.rows.extend(page);
                    if !full {
                        break;
                    }
                }
                Err(error) => {
                    fetch.status = if fetch.rows.is_empty() {
                        RunStatus::Failed
                    } else {
                        RunStatus::Partial
                    };
                    fetch.failure = Some(error.to_string());
                    break;
                }
            }
        }
        Ok(fetch)
    }

    /// Adds the queries as keywords and stores their pages; returns the addition and the
    /// number of pages.
    fn store_queries(
        &self,
        project: &ProjectRow,
        site_url: &str,
        rows: &[QueryRow],
        period: &str,
    ) -> Result<(Added, u64), KwAppError> {
        let language = Language::from_code(&project.spec.language);
        let mut store = self.store()?;
        let (keywords, _) = prepare(
            rows.iter()
                .map(|row| (row.query.clone(), Some(site_url.to_owned()))),
            language,
            GSC_SOURCE,
        );
        let added = store.add_keywords(project.id, &keywords, &now())?;
        let index = store.key_index(project.id)?;
        // One record per keyword and page: clicks and impressions summed, position weighted.
        let mut pages: HashMap<(i64, String), (f64, f64, f64)> = HashMap::new();
        for row in rows {
            let Some(id) =
                normalize(&row.query, language).and_then(|key| index.get(&key.key).copied())
            else {
                continue;
            };
            let entry = pages.entry((id, row.page.clone())).or_default();
            entry.0 += row.clicks;
            entry.1 += row.impressions;
            entry.2 += row.position * row.impressions.max(1.0);
        }
        let records: Vec<SitePageRecord> = pages
            .into_iter()
            .map(
                |((keyword_id, url), (clicks, impressions, weighted))| SitePageRecord {
                    keyword_id,
                    url,
                    source: GSC_SOURCE.to_owned(),
                    clicks: Some(clicks),
                    impressions: Some(impressions),
                    position: Some(weighted / impressions.max(1.0)),
                    period: period.to_owned(),
                },
            )
            .collect();
        store.put_site_pages(project.id, GSC_SOURCE, &records)?;
        fill_intents(&mut store, project.id, &project.spec.language)?;
        let mut settings = store.project_settings(project.id)?;
        settings["gsc_site"] = serde_json::json!(site_url);
        store.set_project_settings(project.id, &settings)?;
        Ok((added, u64::try_from(records.len()).unwrap_or(u64::MAX)))
    }

    /// A sender of Google Ads requests: the access token is fetched once per run.
    pub(crate) fn ads_sender<'a>(
        &'a self,
        token: &'a OnceLock<Result<String, SourceError>>,
        call: impl Fn(&SourceContext<'_>, &AdsAccount<'_>) -> Result<Fetched, SourceError> + 'a,
    ) -> Result<crate::runner::Sender<'a>, KwAppError> {
        let settings = self.settings()?;
        let customer = settings.google.ads_customer_id.clone();
        let manager = settings.google.ads_login_customer_id.clone();
        Ok(Box::new(move |context: &SourceContext<'_>| {
            let customer = customer.as_deref().ok_or_else(|| {
                SourceError::Config("set your Google Ads customer id first".to_owned())
            })?;
            let token = token
                .get_or_init(|| self.google_token(context.transport))
                .clone()?;
            let developer = self.secret(GOOGLE, "developer_token");
            call(
                context,
                &AdsAccount {
                    token: &token,
                    customer_id: customer,
                    login_customer_id: manager.as_deref(),
                    developer_token: developer.as_deref(),
                },
            )
        }))
    }
}

#[cfg(test)]
#[path = "google_tests.rs"]
mod tests;
