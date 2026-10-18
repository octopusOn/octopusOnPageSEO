//! Desktop OAuth: external browser, PKCE S256, one-use loopback callback, OS keychain.
use super::{OpportunityWorkspace, new_id, now_ms, oauth_client::configured_client};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use seonexus_core::http::{HttpRequest, HttpTransport};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{
    Arc, Mutex, OnceLock, PoisonError,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};

const SERVICE: &str = "octopusSEO Spider Google";
const SCOPES: &str = "https://www.googleapis.com/auth/webmasters.readonly https://www.googleapis.com/auth/analytics.readonly";
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoogleConnection {
    pub id: String,
    pub label: String,
}
#[derive(Debug, Clone, Serialize)]
pub struct AuthProgress {
    pub id: String,
    pub status: String,
    pub error: Option<String>,
    pub connection_id: Option<String>,
}
#[derive(Debug, Serialize)]
pub struct GoogleStatus {
    pub configured: bool,
    pub persistent_credentials: bool,
    pub attempts: Vec<AuthProgress>,
}
#[derive(Debug, Serialize)]
pub struct OAuthStart {
    pub id: String,
    pub url: String,
}
pub(super) struct Attempt {
    progress: AuthProgress,
    cancel: Arc<AtomicBool>,
}
type Attempts = Mutex<BTreeMap<String, Attempt>>;
type Tokens = Mutex<BTreeMap<String, (String, i64)>>;
static ATTEMPTS: OnceLock<Attempts> = OnceLock::new();
static ACCESS: OnceLock<Tokens> = OnceLock::new();
#[cfg(not(any(target_os = "macos", windows)))]
static SESSION_REFRESH: OnceLock<Mutex<BTreeMap<String, String>>> = OnceLock::new();
fn attempts() -> &'static Attempts {
    ATTEMPTS.get_or_init(Default::default)
}
fn access() -> &'static Tokens {
    ACCESS.get_or_init(Default::default)
}
pub(super) fn configuration_guard()
-> Result<std::sync::MutexGuard<'static, BTreeMap<String, Attempt>>, String> {
    let running = attempts().lock().unwrap_or_else(PoisonError::into_inner);
    if running.values().any(|a| a.progress.status == "waiting") {
        return Err(
            "Finish or cancel the pending Google sign-in before changing its client".into(),
        );
    }
    Ok(running)
}
pub(super) fn status() -> GoogleStatus {
    GoogleStatus {
        configured: configured_client().ok().flatten().is_some(),
        persistent_credentials: cfg!(any(target_os = "macos", windows)),
        attempts: attempts()
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .values()
            .map(|a| a.progress.clone())
            .collect(),
    }
}
pub(super) fn transport() -> Result<seonexus_net::ApiClient, String> {
    seonexus_net::ApiClient::new("octopusSEO-Spider/0.1", Duration::from_secs(30), None)
        .map_err(|e| e.to_string())
}

impl OpportunityWorkspace {
    /// # Errors
    /// Returns an error when OAuth is unconfigured, another sign-in is pending or the loopback listener cannot start.
    #[allow(clippy::too_many_lines)] // PKCE setup and the one-use callback lifecycle stay adjacent.
    pub fn begin_google(&self) -> Result<OAuthStart, String> {
        let mut running = configuration_guard()?;
        let client = configured_client()?.ok_or(
            "Google OAuth is not configured for this build. CSV/Excel import remains available.",
        )?;
        let listener = TcpListener::bind("127.0.0.1:0").map_err(|e| e.to_string())?;
        listener.set_nonblocking(true).map_err(|e| e.to_string())?;
        let redirect = format!(
            "http://127.0.0.1:{}/oauth/callback",
            listener.local_addr().map_err(|e| e.to_string())?.port()
        );
        let id = new_id()?;
        let state = new_id()?;
        let mut entropy = [0_u8; 32];
        getrandom::fill(&mut entropy).map_err(|e| e.to_string())?;
        let verifier = URL_SAFE_NO_PAD.encode(entropy);
        let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
        let mut url = url::Url::parse("https://accounts.google.com/o/oauth2/v2/auth")
            .map_err(|e| e.to_string())?;
        url.query_pairs_mut().extend_pairs([
            ("client_id", client.client_id.as_str()),
            ("redirect_uri", redirect.as_str()),
            ("response_type", "code"),
            ("scope", SCOPES),
            ("access_type", "offline"),
            ("prompt", "consent select_account"),
            ("state", state.as_str()),
            ("code_challenge", challenge.as_str()),
            ("code_challenge_method", "S256"),
        ]);
        let cancel = Arc::new(AtomicBool::new(false));
        running.retain(|_, a| a.progress.status == "waiting");
        running.insert(
            id.clone(),
            Attempt {
                progress: AuthProgress {
                    id: id.clone(),
                    status: "waiting".into(),
                    error: None,
                    connection_id: None,
                },
                cancel: cancel.clone(),
            },
        );
        drop(running);
        let workspace = self.clone();
        let job = id.clone();
        std::thread::spawn(move || {
            let result = receive_code(&listener, &state, &cancel).and_then(|code| {
                if cancel.load(Ordering::Relaxed) {
                    return Err("Sign-in cancelled".into());
                }
                let token = exchange_with_client(
                    &transport()?,
                    &[
                        ("grant_type", "authorization_code"),
                        ("code", &code),
                        ("redirect_uri", &redirect),
                        ("code_verifier", &verifier),
                    ],
                    &client.client_id,
                    client.client_secret.as_deref(),
                )?;
                if cancel.load(Ordering::Relaxed) {
                    return Err("Sign-in cancelled".into());
                }
                let refresh = token.get("refresh_token").and_then(Value::as_str).ok_or(
                    "Google did not return an offline token; reconnect and approve offline access",
                )?;
                let connection = GoogleConnection {
                    id: new_id()?,
                    label: "Google".into(),
                };
                save_refresh(&connection.id, refresh)?;
                let saved = (|| {
                    cache_token(&connection.id, &token)?;
                    let _guard = workspace
                        .lock
                        .lock()
                        .unwrap_or_else(PoisonError::into_inner);
                    let mut data = workspace.read()?;
                    if cancel.load(Ordering::Relaxed) {
                        return Err("Sign-in cancelled".into());
                    }
                    data.connections.push(connection.clone());
                    workspace.write(&data)
                })();
                if let Err(error) = saved {
                    let _ = remove_refresh(&connection.id);
                    invalidate_access(&connection.id);
                    return Err(error);
                }
                Ok(connection.id)
            });
            if let Some(attempt) = attempts()
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .get_mut(&job)
            {
                match result {
                    Ok(id) => {
                        attempt.progress.status = "connected".into();
                        attempt.progress.connection_id = Some(id);
                    }
                    Err(error) => {
                        attempt.progress.status = if cancel.load(Ordering::Relaxed) {
                            "cancelled"
                        } else {
                            "failed"
                        }
                        .into();
                        attempt.progress.error = Some(error);
                    }
                }
            }
        });
        Ok(OAuthStart {
            id,
            url: url.to_string(),
        })
    }
    pub fn cancel_google(&self, id: &str) {
        if let Some(attempt) = attempts()
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(id)
        {
            attempt.cancel.store(true, Ordering::Relaxed);
        }
    }
    /// # Errors
    /// Returns an error when the OS credential store or module data cannot be updated.
    pub fn disconnect_google(&self, id: &str) -> Result<(), String> {
        remove_refresh(id)?;
        access()
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(id);
        let _guard = self.lock.lock().unwrap_or_else(PoisonError::into_inner);
        let mut data = self.read()?;
        data.connections.retain(|c| c.id != id);
        for project in &mut data.projects {
            if project.google_connection == id {
                project.google_connection.clear();
            }
        }
        self.write(&data)
    }
}

fn receive_code(
    listener: &TcpListener,
    state: &str,
    cancel: &AtomicBool,
) -> Result<String, String> {
    // First-time consent can include account selection, verification and user review.
    receive_code_until(
        listener,
        state,
        cancel,
        Instant::now() + Duration::from_secs(600),
    )
}
fn receive_code_until(
    listener: &TcpListener,
    state: &str,
    cancel: &AtomicBool,
    deadline: Instant,
) -> Result<String, String> {
    while Instant::now() < deadline && !cancel.load(Ordering::Relaxed) {
        match listener.accept() {
            Ok((mut stream, address)) => {
                if !address.ip().is_loopback() {
                    continue;
                }
                stream.set_nonblocking(false).map_err(|e| e.to_string())?;
                stream
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .map_err(|e| e.to_string())?;
                let mut bytes = Vec::new();
                let mut chunk = [0_u8; 1024];
                while bytes.len() < 8192 && !bytes.windows(4).any(|w| w == b"\r\n\r\n") {
                    let Ok(count) = stream.read(&mut chunk) else {
                        break;
                    };
                    if count == 0 {
                        break;
                    }
                    bytes.extend_from_slice(&chunk[..count]);
                }
                let line = std::str::from_utf8(&bytes)
                    .map_err(|_| "Invalid OAuth callback")?
                    .lines()
                    .next()
                    .unwrap_or_default();
                let path = line
                    .strip_prefix("GET ")
                    .and_then(|s| s.split_once(' ').map(|(s, _)| s))
                    .unwrap_or_default();
                let parsed = url::Url::parse(&format!("http://127.0.0.1{path}"))
                    .map_err(|_| "Invalid OAuth callback")?;
                let values: BTreeMap<_, _> = parsed.query_pairs().into_owned().collect();
                let valid = parsed.path() == "/oauth/callback"
                    && values
                        .get("state")
                        .is_some_and(|value| constant_time_eq(value.as_bytes(), state.as_bytes()));
                let message = if valid {
                    "You may close this tab and return to OctopusOn SEO Crawler."
                } else {
                    "Invalid sign-in callback."
                };
                let _ = write!(
                    stream,
                    "HTTP/1.1 {}\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\nCache-Control: no-store\r\n\r\n{}",
                    if valid { "200 OK" } else { "400 Bad Request" },
                    message.len(),
                    message
                );
                if !valid {
                    continue;
                }
                if values.contains_key("error") {
                    return Err("Google sign-in was denied or cancelled".into());
                }
                return values
                    .get("code")
                    .cloned()
                    .filter(|s| !s.is_empty())
                    .ok_or_else(|| "Google did not return an authorization code".into());
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(100));
            }
            Err(error) => return Err(error.to_string()),
        }
    }
    Err(if cancel.load(Ordering::Relaxed) {
        "Sign-in cancelled"
    } else {
        "Google sign-in timed out"
    }
    .into())
}
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0_u8, |acc, (a, b)| acc | a ^ b) == 0
}

fn token_exchange(http: &dyn HttpTransport, fields: &[(&str, &str)]) -> Result<Value, String> {
    let client = configured_client()?.ok_or("Google OAuth is not configured")?;
    exchange_with_client(
        http,
        fields,
        &client.client_id,
        client.client_secret.as_deref(),
    )
}
fn exchange_with_client(
    http: &dyn HttpTransport,
    fields: &[(&str, &str)],
    id: &str,
    secret: Option<&str>,
) -> Result<Value, String> {
    let mut body = url::form_urlencoded::Serializer::new(String::new());
    body.append_pair("client_id", id);
    if let Some(secret) = secret {
        body.append_pair("client_secret", secret);
    }
    for (k, v) in fields {
        body.append_pair(k, v);
    }
    let response = http
        .send(
            &HttpRequest::post(
                "https://oauth2.googleapis.com/token",
                body.finish().into_bytes(),
            )
            .header("content-type", "application/x-www-form-urlencoded"),
        )
        .map_err(|_| "Google authorization endpoint could not be reached")?;
    if response.status != 200 {
        return Err(match response.status {
            400 | 401 => "Google authorization expired or was revoked; reconnect",
            429 => "Google authorization quota reached; try again later",
            _ => "Google authorization failed",
        }
        .into());
    }
    serde_json::from_slice(&response.body)
        .map_err(|_| "Invalid Google authorization response".into())
}
fn cache_token(id: &str, token: &Value) -> Result<String, String> {
    let value = token
        .get("access_token")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .ok_or("Google access token is missing")?
        .to_owned();
    let expires = token
        .get("expires_in")
        .and_then(Value::as_i64)
        .unwrap_or(3600)
        .clamp(1, 86400);
    access()
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .insert(id.into(), (value.clone(), now_ms() + expires * 1000));
    Ok(value)
}
pub(super) fn access_token(id: &str, http: &dyn HttpTransport) -> Result<String, String> {
    if let Some((value, _)) = access()
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .get(id)
        .filter(|(_, expires)| *expires > now_ms() + 60_000)
    {
        return Ok(value.clone());
    }
    let refresh = load_refresh(id)?.ok_or("Google connection is unavailable; reconnect")?;
    let token = token_exchange(
        http,
        &[("grant_type", "refresh_token"), ("refresh_token", &refresh)],
    )?;
    cache_token(id, &token)
}
pub(super) fn invalidate_access(id: &str) {
    access()
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .remove(id);
}

#[cfg(any(target_os = "macos", windows))]
pub(super) fn save_refresh(id: &str, value: &str) -> Result<(), String> {
    keyring::Entry::new(SERVICE, id)
        .and_then(|e| e.set_password(value))
        .map_err(|_| {
            "Cannot save Google connection in the operating-system credential store".into()
        })
}
#[cfg(any(target_os = "macos", windows))]
pub(super) fn load_refresh(id: &str) -> Result<Option<String>, String> {
    match keyring::Entry::new(SERVICE, id).and_then(|e| e.get_password()) {
        Ok(v) => Ok(Some(v)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(_) => Err("Cannot read the operating-system credential store".into()),
    }
}
#[cfg(any(target_os = "macos", windows))]
fn remove_refresh(id: &str) -> Result<(), String> {
    match keyring::Entry::new(SERVICE, id).and_then(|e| e.delete_credential()) {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(_) => Err("Cannot remove the saved Google credential".into()),
    }
}
#[cfg(not(any(target_os = "macos", windows)))]
pub(super) fn save_refresh(id: &str, value: &str) -> Result<(), String> {
    SESSION_REFRESH
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .insert(id.into(), value.into());
    Ok(())
}
#[cfg(not(any(target_os = "macos", windows)))]
pub(super) fn load_refresh(id: &str) -> Result<Option<String>, String> {
    Ok(SESSION_REFRESH
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .get(id)
        .cloned())
}
#[cfg(not(any(target_os = "macos", windows)))]
fn remove_refresh(id: &str) -> Result<(), String> {
    SESSION_REFRESH
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .remove(id);
    Ok(())
}

#[cfg(test)]
#[path = "tests/oauth.rs"]
mod tests;
