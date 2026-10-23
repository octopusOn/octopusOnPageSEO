//! Desktops syncing with their account's web workspace (ADR 0078): `POST /api/sync` with the
//! account's own token, and audit crawls at `/api/sync/crawls/{audit uid}`.
//!
//! The token is the one every OctopusOn SEO program on the computer shares; the gateway says
//! whose it is (remembered for a few minutes). While the account has not signed in on the
//! web, the server keeps that token for the account's scheduled work, which runs here while a
//! desktop syncs. It is borrowed: a later web sign-in replaces it without signing the desktop
//! out, and signing out on the desktop ends it.

use std::collections::HashMap;
use std::fmt::Write as _;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use octopuson_cloud::{CloudClient, CloudError};
use octopuson_platform_app::sync::{self, CONTENT_TYPE, crawls};
use octopuson_platform_app::{AppError, Workspace};
use sha2::{Digest, Sha256};

use crate::auth::{CLIENT, DEV_USER};
use crate::{AppState, schedule};

/// How long the account of a token is remembered.
const TOKEN_TTL: Duration = Duration::from_secs(300);

/// Requests an account may send in a minute (a desktop sends three or four every half minute;
/// a first sync more).
const PER_MINUTE: u32 = 240;

/// Largest sync request, compressed.
pub const MAX_REQUEST: usize = 16 << 20;

/// The token of the development account (`PLATFORM_DEV_LOGIN`, loopback only).
pub const DEV_TOKEN: &str = "oct_dev";

/// File in an account's folder holding the hash of the token borrowed from a desktop.
const BORROWED: &str = "sync-token.sha256";

/// Who may sync: tokens met recently and how much each account asked this minute.
#[derive(Debug, Default)]
pub struct SyncGate {
    tokens: Mutex<HashMap<String, (String, Instant)>>,
    counts: Mutex<HashMap<String, (Instant, u32)>>,
}

fn hash(token: &str) -> String {
    Sha256::digest(token.as_bytes())
        .iter()
        .fold(String::with_capacity(64), |mut text, byte| {
            let _ = write!(text, "{byte:02x}");
            text
        })
}

/// A refused request: its status and why, in words.
type Refusal = (StatusCode, String);

fn refusal(status: StatusCode, message: &str) -> Refusal {
    (status, message.to_owned())
}

fn refused((status, message): Refusal) -> Response {
    let mut response = (status, message).into_response();
    if status == StatusCode::TOO_MANY_REQUESTS {
        response
            .headers_mut()
            .insert(header::RETRY_AFTER, header::HeaderValue::from_static("30"));
    }
    response
}

fn app_refusal(error: &AppError) -> Refusal {
    let status = match error {
        AppError::Invalid(_) => StatusCode::BAD_REQUEST,
        AppError::Busy(_) => StatusCode::CONFLICT,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    };
    refusal(status, &error.to_string())
}

/// The account of the request's token, and the token.
fn account_of(state: &AppState, headers: &HeaderMap) -> Result<(String, String), Refusal> {
    let token = headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .map(str::trim)
        .filter(|token| token.starts_with("oct_") && token.len() <= 200)
        .ok_or_else(|| {
            refusal(
                StatusCode::UNAUTHORIZED,
                "send the account's token: Authorization: Bearer oct_…",
            )
        })?
        .to_owned();
    let user_id = if state.config.dev_login && token == DEV_TOKEN {
        DEV_USER.to_owned()
    } else {
        let key = hash(&token);
        let known = state
            .sync
            .tokens
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(&key)
            .filter(|(_, at)| at.elapsed() < TOKEN_TTL)
            .map(|(user_id, _)| user_id.clone());
        if let Some(user_id) = known {
            user_id
        } else {
            let client = CloudClient::new(state.gateway.as_ref(), &state.config.gateway, CLIENT);
            let account = client.account(&token).map_err(|error| match error {
                CloudError::SignedOut => refusal(
                    StatusCode::UNAUTHORIZED,
                    "the account's token is not valid any more: sign in again",
                ),
                other => refusal(StatusCode::SERVICE_UNAVAILABLE, &other.to_string()),
            })?;
            let user_id = account.user_id.ok_or_else(|| {
                refusal(
                    StatusCode::SERVICE_UNAVAILABLE,
                    "the account service did not name the account",
                )
            })?;
            let mut tokens = state
                .sync
                .tokens
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            tokens.retain(|_, (_, at)| at.elapsed() < TOKEN_TTL);
            tokens.insert(key, (user_id.clone(), Instant::now()));
            user_id
        }
    };
    let mut counts = state
        .sync
        .counts
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    if counts.len() > 10_000 {
        counts.retain(|_, (start, _)| start.elapsed() < Duration::from_secs(60));
    }
    let count = counts.entry(user_id.clone()).or_insert((Instant::now(), 0));
    if count.0.elapsed() >= Duration::from_secs(60) {
        *count = (Instant::now(), 0);
    }
    count.1 += 1;
    if count.1 > PER_MINUTE {
        return Err(refusal(
            StatusCode::TOO_MANY_REQUESTS,
            "too many sync requests",
        ));
    }
    Ok((user_id, token))
}

/// Keeps a desktop's token for the account's scheduled work while the web has none of its own
/// (or only an earlier borrowed one).
fn borrow_token(workspace: &Workspace, token: &str) {
    let marker = workspace.data_dir().join(BORROWED);
    let current = workspace.account_token();
    let borrowed = std::fs::read_to_string(&marker).ok();
    let replaceable = match &current {
        None => true,
        Some(current) => borrowed.as_deref().map(str::trim) == Some(hash(current).as_str()),
    };
    if replaceable && current.as_deref() != Some(token) {
        if let Err(error) = workspace.set_account_token(Some(token)) {
            tracing::warn!(%error, "a desktop's token was not kept");
            return;
        }
        if let Err(error) = std::fs::write(&marker, hash(token)) {
            tracing::warn!(%error, "a borrowed token was not noted");
        }
    }
}

/// Whether the account's token in `workspace` was borrowed from a desktop (a web sign-in then
/// replaces it without signing the desktop out).
pub fn borrowed(workspace: &Workspace, token: &str) -> bool {
    std::fs::read_to_string(workspace.data_dir().join(BORROWED))
        .is_ok_and(|marker| marker.trim() == hash(token))
}

/// Forgets that the account's token was borrowed (a web sign-in replaced it).
pub fn forget_borrowed(workspace: &Workspace) {
    let _ = std::fs::remove_file(workspace.data_dir().join(BORROWED));
}

/// Answers one sync request of a desktop.
pub(crate) async fn post(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let typed = headers
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        == Some(CONTENT_TYPE);
    if !typed {
        return refused(refusal(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "send the sync's own data",
        ));
    }
    let worker = Arc::clone(&state);
    let answered = tokio::task::spawn_blocking(move || {
        let (user_id, token) = account_of(&worker, &headers)?;
        let request: sync::SyncRequest =
            sync::decode(&body).map_err(|error| app_refusal(&error))?;
        let account = worker
            .accounts
            .get(&user_id)
            .map_err(|message| refusal(StatusCode::FORBIDDEN, &message))?;
        let workspace = account.platform.workspace();
        if user_id != DEV_USER {
            borrow_token(workspace, &token);
        }
        let answer = sync::hub_answer(workspace, &user_id, &request, sync::now_ms())
            .map_err(|error| app_refusal(&error))?;
        for share in &answer.removed_shares {
            if let Err(error) = worker.store.remove_report_share(share) {
                tracing::warn!(%error, "a stopped report link was not forgotten");
            }
        }
        if answer.changed {
            schedule::note(&worker, &user_id, &account.platform);
            account.platform.announce_sync();
            account.platform.look_for_alerts();
        }
        sync::encode(&answer.response).map_err(|error| app_refusal(&error))
    })
    .await;
    match answered {
        Ok(Ok(body)) => {
            (StatusCode::OK, [(header::CONTENT_TYPE, CONTENT_TYPE)], body).into_response()
        }
        Ok(Err(refusal)) => refused(refusal),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

/// The folder of the crawl of a finished audit of the account.
fn crawl_folder(workspace: &Workspace, audit: &str) -> Result<std::path::PathBuf, Refusal> {
    let crawl = workspace
        .store()
        .map_err(|error| app_refusal(&error))?
        .sync_audit_crawl(audit)
        .map_err(|error| refusal(StatusCode::INTERNAL_SERVER_ERROR, &error.to_string()))?
        .ok_or_else(|| refusal(StatusCode::NOT_FOUND, "no such finished audit here"))?;
    crawls::crawl_dir(workspace, &crawl)
        .ok_or_else(|| refusal(StatusCode::NOT_FOUND, "no such crawl"))
}

/// Takes the crawl of an audit a desktop ran.
pub(crate) async fn put_crawl(
    State(state): State<Arc<AppState>>,
    Path(audit): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let worker = Arc::clone(&state);
    let answered = tokio::task::spawn_blocking(move || {
        let (user_id, _) = account_of(&worker, &headers)?;
        let account = worker
            .accounts
            .get(&user_id)
            .map_err(|message| refusal(StatusCode::FORBIDDEN, &message))?;
        let workspace = account.platform.workspace();
        let folder = crawl_folder(workspace, &audit)?;
        if folder.join("crawl.json").is_file() {
            return Ok(StatusCode::OK);
        }
        if body.len() > crawls::MAX_CRAWL {
            return Err(refusal(
                StatusCode::PAYLOAD_TOO_LARGE,
                "the crawl is too large",
            ));
        }
        // Packed crawls take about a quarter of their size.
        let room = crawls::ACCOUNT_CRAWLS.saturating_sub(crawls::crawls_size(workspace));
        if (body.len() as u64).saturating_mul(4) > room {
            return Err(refusal(
                StatusCode::INSUFFICIENT_STORAGE,
                "no room left for this account's crawls",
            ));
        }
        crawls::unpack(&body, &folder).map_err(|error| app_refusal(&error))?;
        account.platform.announce_sync();
        Ok(StatusCode::OK)
    })
    .await;
    match answered {
        Ok(Ok(status)) => status.into_response(),
        Ok(Err(refusal)) => refused(refusal),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

/// Sends the crawl of an audit to a desktop.
pub(crate) async fn get_crawl(
    State(state): State<Arc<AppState>>,
    Path(audit): Path<String>,
    headers: HeaderMap,
) -> Response {
    let worker = Arc::clone(&state);
    let answered = tokio::task::spawn_blocking(move || {
        let (user_id, _) = account_of(&worker, &headers)?;
        let account = worker
            .accounts
            .get(&user_id)
            .map_err(|message| refusal(StatusCode::FORBIDDEN, &message))?;
        let folder = crawl_folder(account.platform.workspace(), &audit)?;
        if !folder.join("crawl.json").is_file() {
            return Err(refusal(StatusCode::NOT_FOUND, "the crawl is not here"));
        }
        crawls::pack(&folder).map_err(|error| match error {
            AppError::Invalid(message) => refusal(StatusCode::NOT_FOUND, &message),
            other => app_refusal(&other),
        })
    })
    .await;
    match answered {
        Ok(Ok(body)) => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, crawls::CRAWL_TYPE)],
            body,
        )
            .into_response(),
        Ok(Err(refusal)) => refused(refusal),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}
