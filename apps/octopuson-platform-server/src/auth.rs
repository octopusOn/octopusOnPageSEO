//! Signing in with the OctopusOn SEO account.
//!
//! `/auth/login` sends the browser to the account site's `/baglan` with a PKCE challenge; a
//! person signed in there comes straight back to `/auth/callback` with a one-time code, which
//! the server swaps at the gateway for the account's token (PKCE verifier kept here). The token
//! is sealed in the account's folder: scheduled work (rank checks, AI answers) keeps running on
//! it while nobody is signed in, until it is revoked on the account site. A session cookie,
//! stored as a hash, then ties the browser to the account.

use std::sync::{Arc, PoisonError};
use std::time::{Duration, Instant};

use axum::extract::{Query, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use octopuson_cloud::{CloudClient, CloudError};
use serde::Deserialize;

use crate::store::Session;
use crate::{AppState, random_token, unix_now};

/// How the web version names itself to the account site and in the account's tokens.
pub const CLIENT: &str = "geo-seo-platform-web";

/// Cookie tying a sign-in in progress to the browser that started it.
const LOGIN_COOKIE: &str = "octopuson_login";

/// How long a session lasts.
const SESSION_SECONDS: i64 = 30 * 24 * 3600;

/// How long a sign-in may take on the account site.
const PENDING_TTL: Duration = Duration::from_secs(600);

/// Sign-ins in progress at most (older ones are dropped first).
const MAX_PENDING: usize = 10_000;

/// Account id of the development sign-in.
pub const DEV_USER: &str = "dev-local";

/// A sign-in waiting for the account site.
pub(crate) struct Pending {
    verifier: String,
    next: String,
    created: Instant,
}

/// The session cookie's name: `__Host-` bound to the exact origin over https.
pub fn session_cookie(secure: bool) -> &'static str {
    if secure {
        "__Host-octopuson"
    } else {
        "octopuson_session"
    }
}

/// A cookie's value from the request.
pub(crate) fn cookie(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(';'))
        .filter_map(|pair| pair.trim().split_once('='))
        .find(|(key, _)| *key == name)
        .map(|(_, value)| value.to_owned())
        .filter(|value| !value.is_empty())
}

fn set_cookie(name: &str, value: &str, max_age: i64, path: &str, secure: bool) -> HeaderValue {
    let secure = if secure { "; Secure" } else { "" };
    HeaderValue::from_str(&format!(
        "{name}={value}; Path={path}; Max-Age={max_age}; HttpOnly; SameSite=Lax{secure}"
    ))
    .unwrap_or_else(|_| HeaderValue::from_static(""))
}

/// The session of the request, with its cookie value.
pub(crate) fn current(state: &AppState, headers: &HeaderMap) -> Option<(String, Session)> {
    let value = cookie(headers, session_cookie(state.config.secure()))?;
    let session = state.store.session(&value, unix_now()).ok().flatten()?;
    Some((value, session))
}

/// Where to go after signing in: a path on this site only.
fn safe_next(next: Option<&str>) -> String {
    match next {
        Some(next)
            if next.starts_with('/')
                && !next.starts_with("//")
                && !next.contains('\\')
                && !next.contains(['\r', '\n']) =>
        {
            next.to_owned()
        }
        _ => "/".to_owned(),
    }
}

fn redirect(location: &str) -> Response {
    let mut response = StatusCode::SEE_OTHER.into_response();
    if let Ok(value) = HeaderValue::from_str(location) {
        response.headers_mut().insert(header::LOCATION, value);
    }
    response
}

fn encode(value: &str) -> String {
    url::form_urlencoded::byte_serialize(value.as_bytes()).collect()
}

/// A page explaining a failed sign-in, with a way to try again.
fn failed(status: StatusCode, message: &str) -> Response {
    let escaped = message
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;");
    (
        status,
        [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
        format!(
            "<!doctype html><meta charset=utf-8><title>OctopusOn GEO-SEO Platform</title>\
             <p>{escaped}</p><p><a href=\"/auth/login\">Sign in again</a></p>"
        ),
    )
        .into_response()
}

#[derive(Debug, Deserialize)]
pub(crate) struct LoginQuery {
    next: Option<String>,
}

/// Starts a sign-in on the account site.
pub(crate) async fn login(
    State(state): State<Arc<AppState>>,
    Query(query): Query<LoginQuery>,
) -> Response {
    let pkce = match octopuson_oauth::Pkce::new() {
        Ok(pkce) => pkce,
        Err(error) => return failed(StatusCode::INTERNAL_SERVER_ERROR, &error.to_string()),
    };
    let login_state = random_token(24);
    if login_state.is_empty() {
        return failed(StatusCode::INTERNAL_SERVER_ERROR, "no randomness available");
    }
    {
        let mut pending = state.pending.lock().unwrap_or_else(PoisonError::into_inner);
        pending.retain(|_, waiting| waiting.created.elapsed() < PENDING_TTL);
        if pending.len() >= MAX_PENDING
            && let Some(oldest) = pending
                .iter()
                .min_by_key(|(_, waiting)| waiting.created)
                .map(|(key, _)| key.clone())
        {
            pending.remove(&oldest);
        }
        pending.insert(
            login_state.clone(),
            Pending {
                verifier: pkce.verifier.clone(),
                next: safe_next(query.next.as_deref()),
                created: Instant::now(),
            },
        );
    }
    let config = &state.config;
    let location = format!(
        "{}/baglan?client={}&redirect_uri={}&code_challenge={}&code_challenge_method=S256&state={}",
        config.portal_url,
        encode(CLIENT),
        encode(&config.callback_url()),
        pkce.challenge,
        login_state
    );
    let mut response = redirect(&location);
    response.headers_mut().append(
        header::SET_COOKIE,
        set_cookie(LOGIN_COOKIE, &login_state, 600, "/auth", config.secure()),
    );
    response
}

#[derive(Debug, Deserialize)]
pub(crate) struct CallbackQuery {
    code: Option<String>,
    state: Option<String>,
}

/// Finishes a sign-in: the code becomes the account's token and a session begins.
pub(crate) async fn callback(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(query): Query<CallbackQuery>,
) -> Response {
    let (Some(code), Some(returned)) = (query.code, query.state) else {
        return failed(StatusCode::BAD_REQUEST, "The sign-in did not finish.");
    };
    if cookie(&headers, LOGIN_COOKIE).as_deref() != Some(returned.as_str()) {
        return failed(
            StatusCode::BAD_REQUEST,
            "This sign-in was started in another browser or expired.",
        );
    }
    let pending = state
        .pending
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .remove(&returned);
    let Some(pending) = pending.filter(|waiting| waiting.created.elapsed() < PENDING_TTL) else {
        return failed(StatusCode::BAD_REQUEST, "The sign-in expired.");
    };
    let worker = Arc::clone(&state);
    let verifier = pending.verifier;
    let signed_in = tokio::task::spawn_blocking(move || sign_in(&worker, &code, &verifier)).await;
    let (user_id, email) = match signed_in {
        Ok(Ok(account)) => account,
        Ok(Err(message)) => return failed(StatusCode::BAD_GATEWAY, &message),
        Err(error) => return failed(StatusCode::INTERNAL_SERVER_ERROR, &error.to_string()),
    };
    open_session(&state, &user_id, email.as_deref(), &pending.next)
}

/// Swaps the code for a token, reads the account and keeps the token in its folder.
fn sign_in(
    state: &AppState,
    code: &str,
    verifier: &str,
) -> Result<(String, Option<String>), String> {
    let client = CloudClient::new(state.gateway.as_ref(), &state.config.gateway, CLIENT);
    let token = client
        .swap_connect_code(code, verifier)
        .map_err(|error| match error {
            CloudError::Expired => "The sign-in code was already used or expired.".to_owned(),
            other => other.to_string(),
        })?;
    let account = client.account(&token).map_err(|error| error.to_string())?;
    let user_id = account
        .user_id
        .clone()
        .ok_or_else(|| "The account service did not name the account.".to_owned())?;
    let workspace = state.accounts.get(&user_id)?.platform.workspace().clone();
    if let Some(previous) = workspace.account_token()
        && previous != token
        && !crate::sync::borrowed(&workspace, &previous)
    {
        // A token of an earlier sign-in is not needed any more; one a desktop lent for
        // syncing stays that desktop's.
        let _ = client.sign_out(&previous);
    }
    workspace
        .set_account_token(Some(&token))
        .map_err(|error| error.to_string())?;
    crate::sync::forget_borrowed(&workspace);
    Ok((user_id, account.email))
}

fn open_session(state: &AppState, user_id: &str, email: Option<&str>, next: &str) -> Response {
    let now = unix_now();
    let value = random_token(32);
    let csrf = random_token(24);
    if value.is_empty() || csrf.is_empty() {
        return failed(StatusCode::INTERNAL_SERVER_ERROR, "no randomness available");
    }
    let saved = state.store.signed_in(user_id, email, now).and_then(|()| {
        state
            .store
            .open_session(&value, user_id, &csrf, now, now + SESSION_SECONDS)
    });
    if let Err(error) = saved {
        return failed(StatusCode::INTERNAL_SERVER_ERROR, &error.to_string());
    }
    let secure = state.config.secure();
    let mut response = redirect(next);
    let headers = response.headers_mut();
    headers.append(
        header::SET_COOKIE,
        set_cookie(session_cookie(secure), &value, SESSION_SECONDS, "/", secure),
    );
    headers.append(
        header::SET_COOKIE,
        set_cookie(LOGIN_COOKIE, "", 0, "/auth", secure),
    );
    response
}

/// Whether `given` is the session's CSRF token, compared in constant time.
pub(crate) fn csrf_matches(session: &Session, given: Option<&str>) -> bool {
    let Some(given) = given else {
        return false;
    };
    let (expected, given) = (session.csrf.as_bytes(), given.as_bytes());
    !expected.is_empty()
        && expected.len() == given.len()
        && expected
            .iter()
            .zip(given)
            .fold(0_u8, |difference, (left, right)| {
                difference | (left ^ right)
            })
            == 0
}

/// Ends the browser's session. The account's token stays for scheduled work; it is revoked
/// on the account site.
pub(crate) async fn logout(State(state): State<Arc<AppState>>, headers: HeaderMap) -> Response {
    let Some((value, session)) = current(&state, &headers) else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    let given = headers
        .get("x-csrf-token")
        .and_then(|value| value.to_str().ok());
    if !csrf_matches(&session, given) {
        return StatusCode::FORBIDDEN.into_response();
    }
    if state.store.close_session(&value).is_err() {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }
    let secure = state.config.secure();
    let mut response = StatusCode::NO_CONTENT.into_response();
    response.headers_mut().append(
        header::SET_COOKIE,
        set_cookie(session_cookie(secure), "", 0, "/", secure),
    );
    response
}

/// Signs in as a local development account without the account site (loopback only).
pub(crate) async fn dev_login(
    State(state): State<Arc<AppState>>,
    Query(query): Query<LoginQuery>,
) -> Response {
    if !state.config.dev_login || !state.config.bind.ip().is_loopback() {
        return StatusCode::NOT_FOUND.into_response();
    }
    open_session(
        &state,
        DEV_USER,
        Some("dev@localhost"),
        &safe_next(query.next.as_deref()),
    )
}

#[derive(Debug, Deserialize)]
pub(crate) struct GoogleQuery {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
}

/// A page explaining a Google sign-in that did not finish, with the way back.
fn google_failed(status: StatusCode, message: &str) -> Response {
    let escaped = message
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;");
    (
        status,
        [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
        format!(
            "<!doctype html><meta charset=utf-8><title>OctopusOn GEO-SEO Platform</title>\
             <p>{escaped}</p><p><a href=\"/#/gsc\">Back to Search Console</a></p>"
        ),
    )
        .into_response()
}

/// Finishes connecting Google: the signed-in account's workspace swaps the code for its grant
/// (it checks the state it started), then the browser returns to the Search Console page.
pub(crate) async fn google_callback(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(query): Query<GoogleQuery>,
) -> Response {
    let Some((_, session)) = current(&state, &headers) else {
        return failed(
            StatusCode::UNAUTHORIZED,
            "Sign in first, then connect Google again.",
        );
    };
    if let Some(error) = query.error {
        return google_failed(
            StatusCode::BAD_REQUEST,
            &format!("Google sign-in was not completed ({error})."),
        );
    }
    let (Some(code), Some(returned)) = (query.code, query.state) else {
        return google_failed(StatusCode::BAD_REQUEST, "Google sign-in did not finish.");
    };
    let account = match state.accounts.get(&session.user_id) {
        Ok(account) => account,
        Err(message) => return google_failed(StatusCode::INTERNAL_SERVER_ERROR, &message),
    };
    let workspace = Arc::clone(account.platform.workspace());
    let finished =
        tokio::task::spawn_blocking(move || workspace.finish_google_login(&returned, &code)).await;
    match finished {
        Ok(Ok(_)) => redirect("/#/gsc"),
        Ok(Err(error)) => google_failed(StatusCode::BAD_REQUEST, &error.to_string()),
        Err(error) => google_failed(StatusCode::INTERNAL_SERVER_ERROR, &error.to_string()),
    }
}
