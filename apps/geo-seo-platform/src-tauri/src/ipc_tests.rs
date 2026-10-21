//! The window's single command answers like the router, errors included.

use serde_json::{Value, json};
use tauri::http::HeaderMap;
use tauri::ipc::{CallbackFn, InvokeBody};
use tauri::test::{INVOKE_KEY, get_ipc_response, mock_builder, mock_context, noop_assets};
use tauri::webview::InvokeRequest;
use tauri::{WebviewUrl, WebviewWindowBuilder};

use octopuson_cloud::TokenStore;
use octopuson_platform_app::{Host, Workspace};
use octopuson_secrets::{FileBackend, Secrets};

fn workspace(dir: &std::path::Path) -> Workspace {
    // Never the keychain, never a real gateway in tests.
    let secrets = Secrets::new(
        Box::new(FileBackend::new(dir.join("secrets.json"))),
        dir.join("secret-bindings.json"),
    )
    .with_environment(|_| None);
    let tokens = TokenStore::new(
        Box::new(FileBackend::new(dir.join("account.json"))),
        "http://127.0.0.1:9",
    );
    Workspace::new(dir.to_owned(), Host::Desktop, secrets, tokens)
}

#[test]
fn the_window_calls_commands_by_name() {
    let dir = tempfile::tempdir().unwrap();
    let state = crate::AppState::new(workspace(dir.path()));
    let app = crate::with_commands(mock_builder(), state)
        .build(mock_context(noop_assets()))
        .unwrap();
    let webview = WebviewWindowBuilder::new(&app, "main", WebviewUrl::default())
        .build()
        .unwrap();
    let invoke = |command: &str, args: Value| {
        get_ipc_response(
            &webview,
            InvokeRequest {
                cmd: "platform_call".into(),
                callback: CallbackFn(0),
                error: CallbackFn(1),
                url: if cfg!(windows) {
                    "http://tauri.localhost"
                } else {
                    "tauri://localhost"
                }
                .parse()
                .unwrap(),
                body: InvokeBody::Json(json!({ "command": command, "args": args })),
                headers: HeaderMap::default(),
                invoke_key: INVOKE_KEY.into(),
            },
        )
        .map(|body| body.deserialize::<Value>().unwrap())
    };

    let info = invoke("app_info", json!({})).unwrap();
    assert_eq!(info["host"], "desktop");
    let created = invoke(
        "create_project",
        json!({ "spec": { "domain": "example.com", "country": "tr", "language": "tr" } }),
    )
    .unwrap();
    assert_eq!(created["domain"], "example.com");
    assert_eq!(
        invoke("list_projects", Value::Null).unwrap()[0]["id"],
        created["id"]
    );

    let error = invoke("no_such_command", json!({})).unwrap_err();
    assert_eq!(error["code"], "not_found");
    let error = invoke("get_project", json!({ "id": 404 })).unwrap_err();
    assert_eq!(error["code"], "not_found");
}
