//! Google OAuth 2.0 for desktop apps, with the user's own OAuth client ("Desktop app" type).
//!
//! [`begin`] listens on a free port of `127.0.0.1` and builds the consent address with a PKCE
//! challenge (S256) and a random state. The user signs in in their own browser, Google
//! redirects there, [`Authorization::wait`] takes the code (checking the state) and answers
//! with a short page. [`exchange`] trades the code for tokens; [`refresh`] gets new access
//! tokens from the refresh token; [`revoke`] ends the grant. Nothing is logged, and the
//! listener accepts nothing but the redirect.

use std::io::{BufRead, BufReader, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::time::{Duration, Instant};

use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use seonexus_core::http::{HttpRequest, HttpTransport};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use url::Url;

/// Google's consent page.
pub const AUTH_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";
/// Google's token endpoint.
pub const TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
/// Google's revocation endpoint.
pub const REVOKE_URL: &str = "https://oauth2.googleapis.com/revoke";
/// Address tokens are bound to.
pub const TOKEN_ORIGIN: &str = "https://oauth2.googleapis.com";

/// Scopes.
pub mod scopes {
    /// Search Console, read only.
    pub const SEARCH_CONSOLE: &str = "https://www.googleapis.com/auth/webmasters.readonly";
    /// Google Ads.
    pub const ADS: &str = "https://www.googleapis.com/auth/adwords";
}

/// Longest request line or header the listener reads.
const MAX_LINE: usize = 8 * 1024;

/// The user's OAuth client.
#[derive(Clone, PartialEq, Eq)]
pub struct OAuthClient {
    /// Client id (`….apps.googleusercontent.com`).
    pub client_id: String,
    /// Client secret of the desktop client.
    pub client_secret: String,
}

impl std::fmt::Debug for OAuthClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OAuthClient")
            .field("client_id", &self.client_id)
            .finish_non_exhaustive()
    }
}

/// Why signing in failed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum OAuthError {
    /// The local listener failed.
    #[error("local sign-in listener: {0}")]
    Io(String),
    /// No random numbers.
    #[error("no secure random numbers available")]
    Random,
    /// The user declined, or Google refused the request.
    #[error("sign-in was not completed: {0}")]
    Denied(String),
    /// A redirect with another state arrived.
    #[error("the sign-in answer did not belong to this sign-in")]
    StateMismatch,
    /// Nobody signed in in time.
    #[error("sign-in timed out")]
    Timeout,
    /// The user stopped waiting.
    #[error("sign-in cancelled")]
    Cancelled,
    /// The token endpoint refused.
    #[error("Google refused the token request: {0}")]
    Token(String),
    /// The refresh token is no longer valid; sign in again.
    #[error("the Google sign-in expired or was revoked; sign in again")]
    Expired,
    /// No response.
    #[error("network: {0}")]
    Network(String),
    /// Unreadable response.
    #[error("unreadable response: {0}")]
    Parse(String),
}

/// A PKCE verifier and its S256 challenge.
#[derive(Clone, PartialEq, Eq)]
pub struct Pkce {
    /// Sent with the code exchange.
    pub verifier: String,
    /// Sent with the consent request.
    pub challenge: String,
}

impl std::fmt::Debug for Pkce {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Pkce").finish_non_exhaustive()
    }
}

fn random(bytes: usize) -> Result<String, OAuthError> {
    let mut buffer = vec![0_u8; bytes];
    getrandom::fill(&mut buffer).map_err(|_| OAuthError::Random)?;
    Ok(URL_SAFE_NO_PAD.encode(buffer))
}

impl Pkce {
    /// A new random verifier (43 characters).
    ///
    /// # Errors
    /// [`OAuthError::Random`] without a random source.
    pub fn new() -> Result<Self, OAuthError> {
        Ok(Self::from_verifier(random(32)?))
    }

    /// The challenge of a verifier.
    pub fn from_verifier(verifier: String) -> Self {
        let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
        Self {
            verifier,
            challenge,
        }
    }
}

/// A sign-in in progress.
#[derive(Debug)]
pub struct Authorization {
    listener: TcpListener,
    /// The consent page to open in the user's browser.
    pub url: String,
    redirect_uri: String,
    state: String,
    pkce: Pkce,
}

/// The code of a completed consent, ready for [`exchange`].
#[derive(Clone, PartialEq, Eq)]
pub struct AuthorizationCode {
    code: String,
    redirect_uri: String,
    verifier: String,
}

impl AuthorizationCode {
    /// The code to exchange.
    pub fn code(&self) -> &str {
        &self.code
    }

    /// The PKCE verifier the code is exchanged with.
    pub fn verifier(&self) -> &str {
        &self.verifier
    }

    /// The redirect address the code was sent to.
    pub fn redirect_uri(&self) -> &str {
        &self.redirect_uri
    }
}

impl std::fmt::Debug for AuthorizationCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AuthorizationCode")
            .field("redirect_uri", &self.redirect_uri)
            .finish_non_exhaustive()
    }
}

/// Starts a sign-in for `scopes`: listens on `127.0.0.1` and builds the consent address.
///
/// # Errors
/// [`OAuthError::Io`] when no local port can be opened; [`OAuthError::Random`].
pub fn begin(client: &OAuthClient, scopes: &[&str]) -> Result<Authorization, OAuthError> {
    let scope = scopes.join(" ");
    begin_at(
        AUTH_URL,
        &[
            ("client_id", client.client_id.as_str()),
            ("response_type", "code"),
            ("scope", scope.as_str()),
        ],
        &[("access_type", "offline"), ("prompt", "consent")],
    )
}

/// Starts a loopback sign-in against any authorization page: listens on `127.0.0.1` and adds
/// `redirect_uri`, the S256 PKCE challenge and `state` (between `params` and `trailing`).
///
/// # Errors
/// [`OAuthError::Io`] when no local port can be opened or the page address is invalid;
/// [`OAuthError::Random`].
pub fn begin_at(
    authorize_url: &str,
    params: &[(&str, &str)],
    trailing: &[(&str, &str)],
) -> Result<Authorization, OAuthError> {
    let listener = TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0)))
        .map_err(|error| OAuthError::Io(error.to_string()))?;
    listener
        .set_nonblocking(true)
        .map_err(|error| OAuthError::Io(error.to_string()))?;
    let port = listener
        .local_addr()
        .map_err(|error| OAuthError::Io(error.to_string()))?
        .port();
    let redirect_uri = format!("http://127.0.0.1:{port}");
    let state = random(24)?;
    let pkce = Pkce::new()?;
    let mut url = Url::parse(authorize_url).map_err(|error| OAuthError::Io(error.to_string()))?;
    {
        let mut query = url.query_pairs_mut();
        for (name, value) in params.iter().take(1) {
            query.append_pair(name, value);
        }
        query.append_pair("redirect_uri", &redirect_uri);
        for (name, value) in params.iter().skip(1) {
            query.append_pair(name, value);
        }
        query
            .append_pair("code_challenge", &pkce.challenge)
            .append_pair("code_challenge_method", "S256")
            .append_pair("state", &state);
        for (name, value) in trailing {
            query.append_pair(name, value);
        }
    }
    Ok(Authorization {
        listener,
        url: url.into(),
        redirect_uri,
        state,
        pkce,
    })
}

const DONE_PAGE: &str = "<!doctype html><meta charset=utf-8><title>OctopusOn SEO</title>\
<body style=\"font:16px system-ui;margin:3em\"><h1>Signed in</h1>\
<p>You can close this window and return to OctopusOn SEO.</p>";
const FAILED_PAGE: &str = "<!doctype html><meta charset=utf-8><title>OctopusOn SEO</title>\
<body style=\"font:16px system-ui;margin:3em\"><h1>Sign-in not completed</h1>\
<p>Return to OctopusOn SEO to try again.</p>";

fn respond(stream: &mut TcpStream, status: &str, body: &str) {
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\n\
         Cache-Control: no-store\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.write_all(response.as_bytes());
    let _ = stream.flush();
}

/// What a connection to the listener brought.
enum Visit {
    /// Not the redirect (a favicon, a stray request): answered and ignored.
    Other,
    /// The redirect.
    Redirect(Result<String, OAuthError>),
}

impl Authorization {
    /// The redirect address registered with Google for this sign-in.
    pub fn redirect_uri(&self) -> &str {
        &self.redirect_uri
    }

    /// Waits for the redirect, up to `timeout`, checking `cancelled` ten times a second.
    ///
    /// # Errors
    /// [`OAuthError::Denied`], [`OAuthError::StateMismatch`], [`OAuthError::Timeout`],
    /// [`OAuthError::Cancelled`] or [`OAuthError::Io`].
    pub fn wait(
        self,
        timeout: Duration,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<AuthorizationCode, OAuthError> {
        let deadline = Instant::now() + timeout;
        loop {
            if cancelled() {
                return Err(OAuthError::Cancelled);
            }
            if Instant::now() >= deadline {
                return Err(OAuthError::Timeout);
            }
            match self.listener.accept() {
                Ok((stream, address)) => {
                    if !address.ip().is_loopback() {
                        continue;
                    }
                    if let Visit::Redirect(result) = self.visit(stream) {
                        return result.map(|code| AuthorizationCode {
                            code,
                            redirect_uri: self.redirect_uri.clone(),
                            verifier: self.pkce.verifier.clone(),
                        });
                    }
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(100));
                }
                Err(error) => return Err(OAuthError::Io(error.to_string())),
            }
        }
    }

    fn visit(&self, mut stream: TcpStream) -> Visit {
        let _ = stream.set_nonblocking(false);
        let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
        let mut line = String::new();
        let mut reader = BufReader::new(&stream).take_limited();
        if reader.read_line(&mut line).is_err() || line.len() > MAX_LINE {
            return Visit::Other;
        }
        // Headers are not needed; read them so the browser sees a clean answer.
        let mut header = String::new();
        while reader.read_line(&mut header).is_ok_and(|read| read > 2) && header.len() <= MAX_LINE {
            header.clear();
        }
        let target = line.split_whitespace().nth(1).unwrap_or_default();
        let Ok(url) = Url::parse(&format!("http://127.0.0.1{target}")) else {
            respond(&mut stream, "400 Bad Request", FAILED_PAGE);
            return Visit::Other;
        };
        if !line.starts_with("GET ") || url.path() != "/" {
            respond(&mut stream, "404 Not Found", "");
            return Visit::Other;
        }
        let pair = |name: &str| {
            url.query_pairs()
                .find(|(key, _)| key == name)
                .map(|(_, value)| value.into_owned())
        };
        if pair("state").as_deref() != Some(self.state.as_str()) {
            respond(&mut stream, "400 Bad Request", FAILED_PAGE);
            return Visit::Redirect(Err(OAuthError::StateMismatch));
        }
        if let Some(error) = pair("error") {
            respond(&mut stream, "200 OK", FAILED_PAGE);
            return Visit::Redirect(Err(OAuthError::Denied(error)));
        }
        if let Some(code) = pair("code").filter(|code| !code.is_empty()) {
            respond(&mut stream, "200 OK", DONE_PAGE);
            Visit::Redirect(Ok(code))
        } else {
            respond(&mut stream, "400 Bad Request", FAILED_PAGE);
            Visit::Redirect(Err(OAuthError::Denied("no code".to_owned())))
        }
    }
}

/// A reader that stops after [`MAX_LINE`] times eight bytes.
trait TakeLimited: Sized {
    fn take_limited(self) -> std::io::Take<Self>;
}

impl<R: std::io::Read> TakeLimited for BufReader<R> {
    fn take_limited(self) -> std::io::Take<Self> {
        std::io::Read::take(self, (MAX_LINE * 8) as u64)
    }
}

/// Tokens from the token endpoint.
#[derive(Clone, PartialEq, Eq, Deserialize)]
pub struct Tokens {
    /// Access token for API calls.
    pub access_token: String,
    /// Seconds until it expires.
    #[serde(default)]
    pub expires_in: u64,
    /// Refresh token (with the first exchange only).
    #[serde(default)]
    pub refresh_token: Option<String>,
    /// Granted scopes, space separated.
    #[serde(default)]
    pub scope: Option<String>,
}

impl std::fmt::Debug for Tokens {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Tokens")
            .field("expires_in", &self.expires_in)
            .field("scope", &self.scope)
            .finish_non_exhaustive()
    }
}

impl Tokens {
    /// Whether a scope was granted.
    pub fn has_scope(&self, scope: &str) -> bool {
        self.scope
            .as_deref()
            .is_some_and(|scopes| scopes.split_whitespace().any(|granted| granted == scope))
    }
}

fn form(pairs: &[(&str, &str)]) -> Vec<u8> {
    url::form_urlencoded::Serializer::new(String::new())
        .extend_pairs(pairs)
        .finish()
        .into_bytes()
}

fn token_request(
    transport: &dyn HttpTransport,
    pairs: &[(&str, &str)],
) -> Result<Tokens, OAuthError> {
    let request = HttpRequest::post(TOKEN_URL, form(pairs))
        .header("Content-Type", "application/x-www-form-urlencoded");
    let response = transport
        .send(&request)
        .map_err(|error| OAuthError::Network(error.to_string()))?;
    if !response.is_success() {
        let value: serde_json::Value = serde_json::from_slice(&response.body).unwrap_or_default();
        let error = value
            .get("error")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("error");
        if error == "invalid_grant" {
            return Err(OAuthError::Expired);
        }
        let description = value
            .get("error_description")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        return Err(OAuthError::Token(format!(
            "HTTP {}: {error} {description}",
            response.status
        )));
    }
    serde_json::from_slice(&response.body).map_err(|error| OAuthError::Parse(error.to_string()))
}

/// Trades a consent code for tokens.
///
/// # Errors
/// [`OAuthError::Token`], [`OAuthError::Network`] or [`OAuthError::Parse`].
pub fn exchange(
    transport: &dyn HttpTransport,
    client: &OAuthClient,
    code: &AuthorizationCode,
) -> Result<Tokens, OAuthError> {
    token_request(
        transport,
        &[
            ("grant_type", "authorization_code"),
            ("code", &code.code),
            ("code_verifier", &code.verifier),
            ("redirect_uri", &code.redirect_uri),
            ("client_id", &client.client_id),
            ("client_secret", &client.client_secret),
        ],
    )
}

/// A new access token from a refresh token.
///
/// # Errors
/// [`OAuthError::Expired`] when the grant was revoked or expired; other token errors.
pub fn refresh(
    transport: &dyn HttpTransport,
    client: &OAuthClient,
    refresh_token: &str,
) -> Result<Tokens, OAuthError> {
    token_request(
        transport,
        &[
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token),
            ("client_id", &client.client_id),
            ("client_secret", &client.client_secret),
        ],
    )
}

/// Revokes a token (and the grant behind it).
///
/// # Errors
/// [`OAuthError::Network`]; a refusal other than an already invalid token.
pub fn revoke(transport: &dyn HttpTransport, token: &str) -> Result<(), OAuthError> {
    let request = HttpRequest::post(REVOKE_URL, form(&[("token", token)]))
        .header("Content-Type", "application/x-www-form-urlencoded");
    let response = transport
        .send(&request)
        .map_err(|error| OAuthError::Network(error.to_string()))?;
    if response.is_success() || response.status == 400 {
        Ok(())
    } else {
        Err(OAuthError::Token(format!("HTTP {}", response.status)))
    }
}

#[cfg(test)]
mod tests;
