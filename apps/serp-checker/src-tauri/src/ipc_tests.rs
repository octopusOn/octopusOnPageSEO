//! Verify desktop request names and response shapes against the frontend contract.

use serde_json::{Value, json};
use tauri::http::HeaderMap;
use tauri::ipc::{CallbackFn, InvokeBody};
use tauri::test::{INVOKE_KEY, get_ipc_response, mock_builder, mock_context, noop_assets};
use tauri::webview::InvokeRequest;
use tauri::{WebviewUrl, WebviewWindowBuilder};

use octopuson_serp_app::SerpWorkspace;
use octopuson_serp_app::secrets::{FileBackend, Secrets};

fn workspace(dir: &std::path::Path) -> SerpWorkspace {
    // Never the keychain in tests.
    let secrets = Secrets::new(
        Box::new(FileBackend::new(dir.join("secrets.json"))),
        dir.join("secret-bindings.json"),
    )
    .with_environment(|_| None);
    SerpWorkspace::with_secrets(dir.to_owned(), secrets)
}

#[test]
fn commands_match_the_frontend_contract() {
    let dir = tempfile::tempdir().unwrap();
    let state = crate::AppState::new(workspace(dir.path()));
    let app = crate::with_commands(mock_builder(), state)
        .build(mock_context(noop_assets()))
        .unwrap();
    let webview = WebviewWindowBuilder::new(&app, "main", WebviewUrl::default())
        .build()
        .unwrap();
    let invoke = |cmd: &str, args: Value| {
        get_ipc_response(
            &webview,
            InvokeRequest {
                cmd: cmd.into(),
                callback: CallbackFn(0),
                error: CallbackFn(1),
                url: if cfg!(windows) {
                    "http://tauri.localhost"
                } else {
                    "tauri://localhost"
                }
                .parse()
                .unwrap(),
                body: InvokeBody::Json(args),
                headers: HeaderMap::default(),
                invoke_key: INVOKE_KEY.into(),
            },
        )
        .map(|body| body.deserialize::<Value>().unwrap())
        .map_err(|error| error.to_string())
    };

    let info = invoke("app_info", json!({})).unwrap();
    assert_eq!(info["secret_store"], "file");
    assert_eq!(info["open_pagerank"], "unset");
    assert!(info["tranco"].is_null());
    assert!(info["tranco_notice"].as_str().unwrap().contains("Tranco"));

    for (cmd, args) in [
        ("list_analyses", json!({})),
        ("list_snapshots", json!({"filter": {}})),
        ("list_projects", json!({})),
        ("list_jobs", json!({})),
        ("usage", json!({})),
        ("custom_specs", json!({})),
    ] {
        assert_eq!(invoke(cmd, args).unwrap(), json!([]), "{cmd}");
    }

    assert_settings(&invoke);
    assert_providers(&invoke);

    // The OctopusOn SEO account: nothing is asked of the network without a token.
    let cloud = invoke("cloud_status", json!({})).unwrap();
    assert_eq!(cloud["signed_in"], false);
    invoke(
        "set_source_mode",
        json!({"provider": "serper", "mode": "credit"}),
    )
    .unwrap();
    assert!(
        invoke(
            "set_source_mode",
            json!({"provider": "open-pagerank", "mode": "credit"})
        )
        .is_err()
    );
    let serper = invoke("list_providers", json!({}))
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .find(|provider| provider["id"] == "serper")
        .cloned()
        .unwrap();
    assert_eq!(serper["mode"], "credit");
    assert!(serper["via"].is_null());
    assert_eq!(serper["ready"], false);
    invoke(
        "set_source_mode",
        json!({"provider": "serper", "mode": "auto"}),
    )
    .unwrap();
    invoke("account_logout", json!({})).unwrap();
    assert_projects(&invoke);
    assert_invalid_requests(&invoke);
    assert_eq!(
        invoke("list_parsers", json!({}))
            .unwrap()
            .as_array()
            .unwrap()
            .len(),
        3
    );
}

type Invoke<'a> = &'a dyn Fn(&str, Value) -> Result<Value, String>;

fn assert_settings(invoke: Invoke<'_>) {
    let mut settings = invoke("get_settings", json!({})).unwrap();
    assert_eq!(settings["browser"]["delay_secs"], 30);
    assert_eq!(settings["chains"]["google"][0], "serper");
    settings["browser"]["delay_secs"] = json!(45);
    invoke("save_settings", json!({ "settings": settings })).unwrap();
    assert_eq!(
        invoke("get_settings", json!({})).unwrap()["browser"]["delay_secs"],
        45
    );
    // The browser waits at least ten seconds between searches.
    settings["browser"]["delay_secs"] = json!(3);
    assert!(invoke("save_settings", json!({ "settings": settings })).is_err());
}

fn assert_providers(invoke: Invoke<'_>) {
    let providers = invoke("list_providers", json!({})).unwrap();
    let ids: Vec<&str> = providers
        .as_array()
        .unwrap()
        .iter()
        .map(|provider| provider["id"].as_str().unwrap())
        .collect();
    for id in ["serper", "serpapi", "dataforseo", "yandex-cloud", "browser"] {
        assert!(ids.contains(&id), "{id} in {ids:?}");
    }
    invoke(
        "set_secret",
        json!({"provider": "serper", "name": "api_key", "value": " test-key "}),
    )
    .unwrap();
    let providers = invoke("list_providers", json!({})).unwrap();
    let serper = providers
        .as_array()
        .unwrap()
        .iter()
        .find(|provider| provider["id"] == "serper")
        .unwrap();
    assert_eq!(serper["secrets"][0]["state"], "saved");
    assert_eq!(serper["ready"], true);
    // Keys never travel back to the window.
    assert!(!providers.to_string().contains("test-key"));
    invoke(
        "remove_secret",
        json!({"provider": "serper", "name": "api_key"}),
    )
    .unwrap();
    assert!(
        invoke(
            "set_secret",
            json!({"provider": "serper", "name": "api_key", "value": "  "})
        )
        .is_err()
    );
    assert!(
        invoke(
            "set_secret",
            json!({"provider": "nobody", "name": "api_key", "value": "x"})
        )
        .is_err()
    );
}

fn assert_projects(invoke: Invoke<'_>) {
    let project = invoke(
        "create_project",
        json!({"spec": {
            "name": "Kahve",
            "target": {"value": "https://www.Örnek.com.tr/", "mode": "domain"},
            "trackers": [{"engine": "google", "country": "tr", "language": "tr", "depth": 20}]
        }}),
    )
    .unwrap();
    assert_eq!(project["target"]["value"], "xn--rnek-4qa.com.tr");
    let id = project["id"].as_i64().unwrap();
    let added = invoke(
        "add_keywords",
        json!({"projectId": id, "keywords": ["türk kahvesi", "Türk Kahvesi", "filtre kahve"], "tags": ["kahve"]}),
    )
    .unwrap();
    assert_eq!(added, 2);
    let keywords = invoke("list_keywords", json!({"projectId": id})).unwrap();
    assert_eq!(keywords.as_array().unwrap().len(), 2);
    invoke(
        "set_competitors",
        json!({"projectId": id, "domains": ["rakip.com"]}),
    )
    .unwrap();
    let report = invoke("rank_report", json!({"projectId": id})).unwrap();
    assert_eq!(report["project"]["competitors"], json!(["rakip.com"]));
    assert_eq!(report["rows"].as_array().unwrap().len(), 2);
    let estimate = invoke("estimate_track", json!({"projectId": id})).unwrap();
    assert_eq!(estimate["searches"], 2);
    let file = tempfile::tempdir().unwrap();
    let path = file.path().join("ranks.xlsx");
    assert_eq!(
        invoke("export_rank_report", json!({"projectId": id, "path": path})).unwrap(),
        2
    );
    assert!(path.exists());
    assert!(
        invoke(
            "export_rank_report",
            json!({"projectId": id, "path": file.path().join("ranks.pdf")})
        )
        .is_err()
    );
    let keyword = keywords[0]["id"].as_i64().unwrap();
    invoke(
        "remove_keyword",
        json!({"projectId": id, "keywordId": keyword}),
    )
    .unwrap();
    invoke("delete_project", json!({"id": id})).unwrap();
    assert_eq!(invoke("list_projects", json!({})).unwrap(), json!([]));
}

fn assert_invalid_requests(invoke: Invoke<'_>) {
    let query = |keyword: &str, country: &str| {
        json!({"request": {"search": {"query": {
            "engine": "google", "keyword": keyword, "country": country, "language": "tr", "depth": 10
        }}}})
    };
    assert!(invoke("start_analysis", query("", "tr")).is_err());
    assert!(invoke("start_analysis", query("kahve", "turkey")).is_err());
    assert!(invoke("get_analysis", json!({"id": 9})).is_err());
    assert!(invoke("get_snapshot", json!({"id": 9})).is_err());
    assert!(invoke("cancel_job", json!({"id": "missing"})).is_err());
    assert!(invoke("start_track", json!({"projectId": 404})).is_err());
    assert!(invoke("update_tranco", json!({"accepted": false})).is_err());
    assert_eq!(
        invoke("stored_pages", json!({"snapshotId": 9})).unwrap(),
        json!([])
    );
    assert!(invoke("test_parser", json!({"snapshotId": 9})).is_err());
    assert!(invoke("reparse_snapshot", json!({"snapshotId": 9})).is_err());
    assert!(invoke("export_analysis", json!({"id": 9, "path": "/tmp/x.csv"})).is_err());
    assert!(
        invoke(
            "create_project",
            json!({"spec": {"name": "x", "target": {"value": "", "mode": "domain"}, "trackers": []}})
        )
        .is_err()
    );
}
