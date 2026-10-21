//! The MCP server of the web version (`POST /mcp`, Streamable HTTP with JSON answers) and the
//! personal access tokens AI assistants use for it.
//!
//! A token stands for its account: the assistant calls the same tools the desktop's
//! `octopuson-platform mcp` offers, on the account's workspace. The server keeps only each
//! token's hash; the token is shown once, when made. Tokens stop working when revoked, and with
//! the account's OctopusOn SEO token when that is revoked on the account site.

use std::sync::Arc;

use axum::body::Bytes;
use axum::extract::State;
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use octopuson_platform_app::mcp::{McpServer, NAME};
use serde_json::{Value, json};

use crate::auth::DEV_USER;
use crate::{AppState, random_token, unix_now};

/// Tokens an account may have at once.
const MAX_TOKENS: usize = 10;

/// How a token starts (so leaked ones are recognisable).
const PREFIX: &str = "opt_";

/// Answers the token commands of the Settings page; `None` for any other command.
pub(crate) fn command(
    state: &AppState,
    user_id: &str,
    command: &str,
    arguments: &Value,
) -> Option<Result<Value, (StatusCode, String)>> {
    let failed =
        |error: crate::store::StoreError| (StatusCode::INTERNAL_SERVER_ERROR, error.to_string());
    Some(match command {
        "list_access_tokens" => state
            .store
            .access_tokens(user_id)
            .map(|tokens| json!(tokens))
            .map_err(failed),
        "create_access_token" => {
            let name: String = arguments
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ");
            if name.is_empty() || name.chars().count() > 60 {
                return Some(Err((
                    StatusCode::BAD_REQUEST,
                    "name: 1 to 60 characters".to_owned(),
                )));
            }
            match state.store.access_tokens(user_id) {
                Ok(tokens) if tokens.len() >= MAX_TOKENS => {
                    return Some(Err((
                        StatusCode::CONFLICT,
                        format!("at most {MAX_TOKENS} tokens; revoke one first"),
                    )));
                }
                Err(error) => return Some(Err(failed(error))),
                Ok(_) => {}
            }
            let secret = random_token(32);
            if secret.is_empty() {
                return Some(Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "no randomness available".to_owned(),
                )));
            }
            let token = format!("{PREFIX}{secret}");
            state
                .store
                .add_access_token(&token, user_id, &name, unix_now())
                .map(|row| json!({ "token": token, "access_token": row }))
                .map_err(failed)
        }
        "revoke_access_token" => {
            let id = arguments
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or_default();
            match state.store.revoke_access_token(user_id, id) {
                Ok(true) => Ok(Value::Null),
                Ok(false) => Err((StatusCode::NOT_FOUND, format!("token {id} not found"))),
                Err(error) => Err(failed(error)),
            }
        }
        _ => return None,
    })
}

/// Why a request is turned away.
type Refusal = (StatusCode, String);

fn refusal(status: StatusCode, message: &str) -> Refusal {
    (status, message.to_owned())
}

fn refused((status, message): Refusal) -> Response {
    let mut response = (
        status,
        [(header::CONTENT_TYPE, "application/json")],
        json!({ "jsonrpc": "2.0", "id": null, "error": { "code": -32_001, "message": message } })
            .to_string(),
    )
        .into_response();
    if status == StatusCode::UNAUTHORIZED {
        response.headers_mut().insert(
            header::WWW_AUTHENTICATE,
            HeaderValue::from_static("Bearer realm=\"octopuson-platform\""),
        );
    }
    response
}

/// The account of the request's bearer token, while the account can still be used.
fn account_of(state: &AppState, headers: &HeaderMap) -> Result<String, Refusal> {
    let token = headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .map(str::trim)
        .filter(|token| token.starts_with(PREFIX))
        .ok_or_else(|| {
            refusal(
                StatusCode::UNAUTHORIZED,
                "send a personal access token: Authorization: Bearer opt_…",
            )
        })?;
    let user_id = state
        .store
        .access_token_user(token, unix_now())
        .ok()
        .flatten()
        .ok_or_else(|| {
            refusal(
                StatusCode::UNAUTHORIZED,
                "this access token is not valid; make a new one under Settings",
            )
        })?;
    if user_id != DEV_USER || !state.config.dev_login {
        let usable = state
            .accounts
            .get(&user_id)
            .is_ok_and(|account| account.platform.workspace().account_token().is_some());
        if !usable {
            return Err(refusal(
                StatusCode::UNAUTHORIZED,
                "the OctopusOn SEO account of this token is signed out; sign in again",
            ));
        }
    }
    Ok(user_id)
}

/// Answers MCP messages posted with a personal access token.
pub(crate) async fn post(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    // Browsers on other sites may not drive the server (DNS rebinding, cross-site requests).
    if let Some(origin) = headers
        .get(header::ORIGIN)
        .and_then(|value| value.to_str().ok())
        && origin != state.config.public_url
    {
        return refused(refusal(
            StatusCode::FORBIDDEN,
            "requests from other sites are refused",
        ));
    }
    let Ok(text) = String::from_utf8(body.to_vec()) else {
        return refused(refusal(StatusCode::BAD_REQUEST, "the body is not UTF-8"));
    };
    let worker = Arc::clone(&state);
    let answered = tokio::task::spawn_blocking(move || {
        let user_id = account_of(&worker, &headers)?;
        let account = worker
            .accounts
            .get(&user_id)
            .map_err(|message| (StatusCode::FORBIDDEN, message))?;
        let server = McpServer::new(Arc::clone(&account.platform), NAME);
        Ok::<_, Refusal>(server.handle_text(&text))
    })
    .await;
    match answered {
        Ok(Ok(Some(answer))) => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "application/json")],
            answer,
        )
            .into_response(),
        Ok(Ok(None)) => StatusCode::ACCEPTED.into_response(),
        Ok(Err(refusal)) => refused(refusal),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

/// This server sends no event stream and keeps no sessions.
pub(crate) async fn other() -> Response {
    (StatusCode::METHOD_NOT_ALLOWED, [(header::ALLOW, "POST")]).into_response()
}
