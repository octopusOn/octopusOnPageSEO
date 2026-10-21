//! Read-only report links: `/r/{token}` shows a report to anyone with the link, drawn from the
//! account's current data, without signing in. The server keeps only a hash of each token and
//! whose report it is; the account's workspace decides whether the token still opens it.

use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde_json::Value;

use crate::{AppState, unix_now};

/// Length of a token (32 random bytes in URL-safe base64).
const TOKEN_LENGTH: usize = 43;

/// Notes the owner of a link made by `share_report`.
pub(crate) fn after_call(state: &AppState, user_id: &str, command: &str, answer: &Value) {
    if command != "share_report" {
        return;
    }
    if let Some(token) = answer.get("share_token").and_then(Value::as_str)
        && let Err(error) = state.store.add_report_share(token, user_id, unix_now())
    {
        tracing::warn!(%error, "a report link was not noted");
    }
}

fn not_found() -> Response {
    (
        StatusCode::NOT_FOUND,
        [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
        "<!doctype html><meta charset=utf-8><title>OctopusOn GEO-SEO Platform</title><p>This report link does not exist or was taken away.</p>",
    )
        .into_response()
}

/// Shows the report a link opens.
pub(crate) async fn shared(
    State(state): State<Arc<AppState>>,
    Path(token): Path<String>,
) -> Response {
    if token.len() != TOKEN_LENGTH
        || !token
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    {
        return not_found();
    }
    let Ok(Some(user_id)) = state.store.report_share_owner(&token) else {
        return not_found();
    };
    let Ok(account) = state.accounts.get(&user_id) else {
        return not_found();
    };
    let workspace = Arc::clone(account.platform.workspace());
    let page = tokio::task::spawn_blocking(move || {
        octopuson_platform_app::modules::reports::shared_report_html(&workspace, &token)
    })
    .await;
    match page {
        Ok(Ok(Some(html))) => (
            StatusCode::OK,
            [
                (header::CONTENT_TYPE, "text/html; charset=utf-8"),
                (header::CACHE_CONTROL, "no-store"),
                (
                    header::HeaderName::from_static("x-robots-tag"),
                    "noindex, nofollow",
                ),
            ],
            html,
        )
            .into_response(),
        Ok(Ok(None)) => not_found(),
        Ok(Err(error)) => {
            tracing::warn!(%error, "a shared report could not be drawn");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}
