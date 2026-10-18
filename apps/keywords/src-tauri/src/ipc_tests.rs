//! Verify desktop request names and response shapes against the frontend contract.

use serde_json::{Value, json};
use tauri::http::HeaderMap;
use tauri::ipc::{CallbackFn, InvokeBody};
use tauri::test::{INVOKE_KEY, get_ipc_response, mock_builder, mock_context, noop_assets};
use tauri::webview::InvokeRequest;
use tauri::{WebviewUrl, WebviewWindowBuilder};

use seonexus_kw_app::KeywordWorkspace;
use seonexus_kw_app::secrets::{FileBackend, Secrets};

fn workspace(dir: &std::path::Path) -> KeywordWorkspace {
    // Never the keychain in tests.
    let secrets = Secrets::new(
        Box::new(FileBackend::new(dir.join("secrets.json"))),
        dir.join("secret-bindings.json"),
    )
    .with_environment(|_| None);
    KeywordWorkspace::with_secrets(dir.to_owned(), secrets)
}

#[test]
#[allow(clippy::too_many_lines)] // one contract, checked in order
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
    for (cmd, args) in [
        ("list_projects", json!({})),
        ("list_jobs", json!({})),
        ("usage", json!({})),
        ("list_runs", json!({"projectId": null})),
    ] {
        assert_eq!(invoke(cmd, args).unwrap(), json!([]), "{cmd}");
    }

    let mut settings = invoke("get_settings", json!({})).unwrap();
    assert_eq!(settings["suggest"]["google"], false);
    assert_eq!(settings["clustering"]["method"], "hybrid");
    assert_eq!(
        settings["browser"],
        json!({"executable": null, "delay_secs": 30, "daily_cap": 100, "proxy": null})
    );
    settings["confirm_above_usd"] = json!(3.5);
    invoke("save_settings", json!({"settings": settings})).unwrap();
    assert_eq!(
        invoke("get_settings", json!({})).unwrap()["confirm_above_usd"],
        3.5
    );
    let mut fast = settings.clone();
    fast["browser"]["delay_secs"] = json!(5);
    assert!(invoke("save_settings", json!({"settings": fast})).is_err());

    // The browser stays out of the chains until the user adds it.
    let browser = invoke("browser_status", json!({})).unwrap();
    assert_eq!(browser["engines"], json!([]));
    assert_eq!(browser["enabled"], true);
    assert_eq!(browser["daily_cap"], 100);
    assert_eq!(browser["used_today"]["google"], 0);
    assert!(
        browser["profile_dir"]
            .as_str()
            .unwrap()
            .ends_with("browser-profile")
    );
    invoke("reset_browser_profile", json!({})).unwrap();

    let project = invoke(
        "create_project",
        json!({"spec": {"name": "Kahve", "country": "tr", "language": "tr",
            "engines": ["google"], "site": "https://www.ornek.com.tr/", "competitors": []}}),
    )
    .unwrap();
    let id = project["id"].as_i64().unwrap();
    assert_eq!(project["site"], "ornek.com.tr");
    assert_eq!(project["keywords"], 0);

    let added = invoke(
        "add_keywords",
        json!({"projectId": id, "keywords": ["kahve makinesi", "kahve makineleri", "çay demliği"]}),
    )
    .unwrap();
    assert_eq!(added["added"], 3);
    let page = invoke(
        "list_keywords",
        json!({"projectId": id, "query": {"sort": "keyword", "descending": false}}),
    )
    .unwrap();
    assert_eq!(page["total"], 3);
    assert_eq!(page["rows"][0]["keyword"], "çay demliği");
    let first = page["rows"][0]["id"].as_i64().unwrap();
    invoke(
        "tag_keywords",
        json!({"projectId": id, "ids": [first], "add": ["çay"], "remove": []}),
    )
    .unwrap();
    let facets = invoke("keyword_facets", json!({"projectId": id})).unwrap();
    assert_eq!(facets["tags"], json!([["çay", 1]]));

    let estimate = invoke(
        "estimate_discovery",
        json!({"projectId": id, "request": {"seeds": ["kahve"], "sources": ["wordstat", "google-suggest"]}}),
    )
    .unwrap();
    assert_eq!(estimate["sources"].as_array().unwrap().len(), 2);
    assert_eq!(estimate["requests"], 0);
    let serps = invoke(
        "estimate_serps",
        json!({"projectId": id, "request": {"device": "desktop"}}),
    )
    .unwrap();
    assert_eq!(serps["keywords"], 3);
    assert!(serps["provider"].is_null());

    let providers = invoke("list_providers", json!({})).unwrap();
    assert!(providers.as_array().unwrap().len() >= 5);
    invoke(
        "set_key",
        json!({"provider": "dataforseo", "name": "credentials", "value": "me@example.com:pw"}),
    )
    .unwrap();
    invoke(
        "set_budget",
        json!({"provider": "dataforseo", "budgetUsd": 5.0}),
    )
    .unwrap();
    let dataforseo = invoke("list_providers", json!({}))
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .find(|provider| provider["id"] == "dataforseo")
        .cloned()
        .unwrap();
    assert_eq!(dataforseo["ready"], true);
    assert_eq!(dataforseo["budget_usd"], 5.0);
    assert_eq!(dataforseo["via"], "own");
    assert_eq!(dataforseo["mode"], "auto");
    assert!(dataforseo["credit"].is_null());
    let google = invoke("google_status", json!({})).unwrap();
    assert_eq!(google["connected"], false);

    // The OctopusOn SEO account: nothing is asked of the network without a token.
    let cloud = invoke("cloud_status", json!({})).unwrap();
    assert_eq!(cloud["signed_in"], false);
    assert!(cloud["account"].is_null());
    invoke(
        "set_source_mode",
        json!({"provider": "dataforseo", "mode": "own"}),
    )
    .unwrap();
    assert!(
        invoke(
            "set_source_mode",
            json!({"provider": "google-suggest", "mode": "credit"})
        )
        .is_err()
    );
    assert!(
        invoke(
            "set_source_mode",
            json!({"provider": "dataforseo", "mode": "free"})
        )
        .is_err()
    );
    invoke("account_logout", json!({})).unwrap();

    let job = invoke(
        "start_cluster",
        json!({"projectId": id, "request": {"method": "lexical", "threshold": 0.5}}),
    )
    .unwrap();
    assert!(job.as_str().unwrap().starts_with("job-"));
    let mut clusterings = json!([]);
    for _ in 0..100 {
        clusterings = invoke("list_clusterings", json!({"projectId": id})).unwrap();
        if clusterings.as_array().is_some_and(|list| !list.is_empty()) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    let clustering = clusterings[0]["id"].as_i64().unwrap();
    let clusters = invoke(
        "list_clusters",
        json!({"clusteringId": clustering, "query": {"sort": "size", "descending": true}}),
    )
    .unwrap();
    assert_eq!(clusters["rows"][0]["size"], 2);
    let members = invoke(
        "cluster_members",
        json!({"clusteringId": clustering, "number": clusters["rows"][0]["number"]}),
    )
    .unwrap();
    assert_eq!(members[0]["pillar"], true);

    for (cmd, args) in [
        (
            "create_project",
            json!({"spec": {"name": "", "country": "tr", "language": "tr", "engines": ["google"], "site": null, "competitors": []}}),
        ),
        (
            "import_file",
            json!({"projectId": id, "path": dir.path().join("x.pdf")}),
        ),
        (
            "set_key",
            json!({"provider": "nobody", "name": "x", "value": "y"}),
        ),
        ("cancel_job", json!({"id": "job-x"})),
    ] {
        assert!(invoke(cmd, args).is_err(), "{cmd}");
    }
}
