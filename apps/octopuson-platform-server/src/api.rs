//! The platform's commands over HTTP, and job changes as an event stream.

use std::convert::Infallible;
use std::sync::Arc;

use axum::Json;
use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use octopuson_platform_app::{CallError, ErrorCode};
use serde_json::{Value, json};
use tokio_stream::StreamExt as _;
use tokio_stream::wrappers::BroadcastStream;

use crate::auth::{DEV_USER, csrf_matches, current};
use crate::store::Session;
use crate::{AppState, mcp, schedule, shares};

/// The HTTP status of a failed command.
pub fn status_of(code: ErrorCode) -> StatusCode {
    match code {
        ErrorCode::Invalid => StatusCode::BAD_REQUEST,
        ErrorCode::NotFound => StatusCode::NOT_FOUND,
        ErrorCode::Busy | ErrorCode::NoSource | ErrorCode::Budget => StatusCode::CONFLICT,
        ErrorCode::Auth | ErrorCode::Account => StatusCode::UNAUTHORIZED,
        ErrorCode::Quota => StatusCode::PAYMENT_REQUIRED,
        ErrorCode::RateLimited => StatusCode::TOO_MANY_REQUESTS,
        ErrorCode::Unavailable => StatusCode::SERVICE_UNAVAILABLE,
        ErrorCode::Provider => StatusCode::BAD_GATEWAY,
        ErrorCode::Unsupported => StatusCode::FORBIDDEN,
        ErrorCode::Internal => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

/// The answer without a session: where to sign in (the development sign-in when offered).
fn signed_out(state: &AppState) -> Response {
    let login_url = if state.config.dev_login {
        "/auth/dev"
    } else {
        "/auth/login"
    };
    (
        StatusCode::UNAUTHORIZED,
        Json(json!({ "signed_in": false, "login_url": login_url })),
    )
        .into_response()
}

/// The session, when its account still has a token (the development account needs none).
/// A token revoked on the account site ends every session of the account.
fn signed_in(state: &AppState, headers: &HeaderMap) -> Option<Session> {
    let (_, session) = current(state, headers)?;
    if session.user_id == DEV_USER && state.config.dev_login {
        return Some(session);
    }
    let account = state.accounts.get(&session.user_id).ok()?;
    if account.platform.workspace().account_token().is_none() {
        let _ = state.store.close_sessions_of(&session.user_id);
        return None;
    }
    Some(session)
}

/// Who is signed in, and the token pages send with every change.
pub(crate) async fn session(State(state): State<Arc<AppState>>, headers: HeaderMap) -> Response {
    let worker = Arc::clone(&state);
    let found = tokio::task::spawn_blocking(move || signed_in(&worker, &headers))
        .await
        .ok()
        .flatten();
    match found {
        Some(session) => Json(json!({
            "signed_in": true,
            "email": session.email,
            "csrf": session.csrf,
            "portal_url": state.config.portal_url,
        }))
        .into_response(),
        None => signed_out(&state),
    }
}

/// Calls a command with the JSON body as its arguments.
pub(crate) async fn call(
    State(state): State<Arc<AppState>>,
    Path(command): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let given = headers
        .get("x-csrf-token")
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    let arguments: Value = if body.is_empty() {
        Value::Null
    } else {
        match serde_json::from_slice(&body) {
            Ok(arguments) => arguments,
            Err(error) => {
                return failure(&CallError {
                    code: ErrorCode::Invalid,
                    message: format!("the arguments are not JSON: {error}"),
                    retry_after: None,
                });
            }
        }
    };
    let worker = Arc::clone(&state);
    let outcome = tokio::task::spawn_blocking(move || {
        let Some(session) = signed_in(&worker, &headers) else {
            return Err(None);
        };
        if !csrf_matches(&session, given.as_deref()) {
            return Err(Some(StatusCode::FORBIDDEN));
        }
        if let Some(handled) = mcp::command(&worker, &session.user_id, &command, &arguments) {
            return Ok(handled.map_err(|(status, message)| CallError {
                code: if status == StatusCode::NOT_FOUND {
                    ErrorCode::NotFound
                } else if status == StatusCode::BAD_REQUEST || status == StatusCode::CONFLICT {
                    ErrorCode::Invalid
                } else {
                    ErrorCode::Internal
                },
                message,
                retry_after: None,
            }));
        }
        let account = worker
            .accounts
            .get(&session.user_id)
            .map_err(|_| Some(StatusCode::FORBIDDEN))?;
        let answer = account.platform.call(&command, arguments);
        if let Ok(value) = &answer {
            schedule::after_call(&worker, &session.user_id, &account.platform, &command);
            shares::after_call(&worker, &session.user_id, &command, value);
        }
        Ok(answer)
    })
    .await;
    match outcome {
        Ok(Ok(Ok(answer))) => Json(answer).into_response(),
        Ok(Ok(Err(error))) => failure(&error),
        Ok(Err(None)) => signed_out(&state),
        Ok(Err(Some(status))) => status.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

fn failure(error: &CallError) -> Response {
    let mut response = (status_of(error.code), Json(error)).into_response();
    if let Some(seconds) = error.retry_after
        && let Ok(value) = seconds.to_string().parse()
    {
        response.headers_mut().insert("retry-after", value);
    }
    response
}

/// Job changes of the signed-in account, as server-sent `job` events.
pub(crate) async fn events(State(state): State<Arc<AppState>>, headers: HeaderMap) -> Response {
    let worker = Arc::clone(&state);
    let account = tokio::task::spawn_blocking(move || {
        let session = signed_in(&worker, &headers)?;
        worker.accounts.get(&session.user_id).ok()
    })
    .await
    .ok()
    .flatten();
    let Some(account) = account else {
        return signed_out(&state);
    };
    let stream = BroadcastStream::new(account.subscribe()).filter_map(|item| {
        item.ok()
            .map(|text| Ok::<_, Infallible>(Event::default().event("job").data(&*text)))
    });
    Sse::new(stream)
        .keep_alive(KeepAlive::default())
        .into_response()
}
