//! End-to-end tests of the IPC layer: requests carry exactly the JSON the frontend sends
//! (`src/api/commands.ts`), so argument names and response shapes stay in sync.

use serde_json::{Value, json};
use tauri::http::HeaderMap;
use tauri::ipc::{CallbackFn, InvokeBody};
use tauri::test::{
    INVOKE_KEY, MockRuntime, get_ipc_response, mock_builder, mock_context, noop_assets,
};
use tauri::webview::InvokeRequest;
use tauri::{WebviewUrl, WebviewWindow, WebviewWindowBuilder};

use seonexus_log_app::Workspace;
use seonexus_log_app::import::{ImportRequest, NoProgress, import_logs};
use seonexus_log_app::{CancelToken, ImportOptions};

use crate::state::AppState;

const LOG: &str = r#"66.249.66.1 - - [07/Oct/2026:18:39:53 +0000] "GET /a HTTP/1.1" 200 535 "-" "Googlebot/2.1"
66.249.66.1 - - [07/Oct/2026:18:40:00 +0000] "GET /missing HTTP/1.1" 404 33 "-" "Googlebot/2.1"
10.0.0.1 - - [08/Oct/2026:09:00:00 +0000] "GET /a?x=1 HTTP/1.1" 200 535 "-" "Mozilla/5.0"
"#;

struct Harness {
    dir: tempfile::TempDir,
    workspace: Workspace,
    webview: WebviewWindow<MockRuntime>,
}

impl Harness {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let workspace = Workspace::at(dir.path().join("data"));
        let app = crate::with_commands(mock_builder(), AppState::with_workspace(workspace.clone()))
            .build(mock_context(noop_assets()))
            .unwrap();
        let webview = WebviewWindowBuilder::new(&app, "main", WebviewUrl::default())
            .build()
            .unwrap();
        Self {
            dir,
            workspace,
            webview,
        }
    }

    fn invoke(&self, command: &str, args: Value) -> Result<Value, Value> {
        let url = if cfg!(windows) {
            "http://tauri.localhost"
        } else {
            "tauri://localhost"
        };
        get_ipc_response(
            &self.webview,
            InvokeRequest {
                cmd: command.into(),
                callback: CallbackFn(0),
                error: CallbackFn(1),
                url: url.parse().unwrap(),
                body: InvokeBody::Json(args),
                headers: HeaderMap::default(),
                invoke_key: INVOKE_KEY.into(),
            },
        )
        .map(|body| body.deserialize::<Value>().unwrap())
    }

    fn ok(&self, command: &str, args: Value) -> Value {
        self.invoke(command, args)
            .unwrap_or_else(|error| panic!("{command} failed: {error}"))
    }
}

/// A harness with client `Acme`, site `acme.com` and [`LOG`] imported (DNS disabled).
fn imported() -> Harness {
    let h = Harness::new();
    h.ok("add_client", json!({ "name": "Acme" }));
    h.ok(
        "add_site",
        json!({ "client": "Acme", "name": "acme.com", "format": { "kind": "builtin", "id": "combined" } }),
    );
    let log = h.dir.path().join("access.log");
    std::fs::write(&log, LOG).unwrap();
    // Import through the use-case: progress channels need a real webview.
    let request = ImportRequest {
        site: "acme.com".into(),
        paths: vec![log],
        max_error_samples: 5,
        options: ImportOptions::default(),
        verify_dns: false,
    };
    import_logs(&h.workspace, &request, &NoProgress, &CancelToken::new()).unwrap();
    h
}

#[test]
fn catalog_commands() {
    let h = Harness::new();
    assert_eq!(
        h.ok("add_client", json!({ "name": "Acme" }))["name"],
        "Acme"
    );
    let site = h.ok(
        "add_site",
        json!({ "client": "Acme", "name": "acme.com", "format": { "kind": "builtin", "id": "combined" } }),
    );
    assert_eq!(site["format"], "builtin:combined");
    assert_eq!(h.ok("list_sites", json!({})).as_array().unwrap().len(), 1);
    assert!(h.ok("list_formats", json!({})).as_array().unwrap().len() >= 4);
    assert!(h.ok("app_info", json!({}))["dataDir"].is_string());
    let duplicate = h
        .invoke("add_client", json!({ "name": "Acme" }))
        .unwrap_err();
    assert_eq!(duplicate["kind"], "duplicate");
    let overview = h.ok("site_overview", json!({ "name": "acme.com" }));
    assert_eq!(overview["hits"], 0);

    h.ok(
        "set_site_format",
        json!({ "name": "acme.com", "format": { "kind": "auto" } }),
    );

    let defaults = h.ok("site_settings", json!({ "site": "acme.com" }));
    assert_eq!(defaults["traffic"], "all");
    let saved = h.ok(
        "set_site_settings",
        json!({ "site": "acme.com", "settings": { "host": "WWW.Acme.com", "traffic": "bots", "exclude_urls": ["^/admin"], "strip_parameters": [" sid "] } }),
    );
    assert_eq!(saved["host"], "acme.com");
    assert_eq!(saved["strip_parameters"], json!(["sid"]));
    let invalid = h
        .invoke(
            "set_site_settings",
            json!({ "site": "acme.com", "settings": { "include_urls": ["("] } }),
        )
        .unwrap_err();
    assert_eq!(invalid["kind"], "invalid-input");
    let missing = h
        .invoke(
            "delete_import",
            json!({ "site": "acme.com", "importId": 7 }),
        )
        .unwrap_err();
    assert_eq!(missing["kind"], "not-found");

    h.ok("remove_site", json!({ "name": "acme.com" }));
    h.ok("remove_client", json!({ "name": "Acme" }));
    assert_eq!(h.ok("list_clients", json!({})), json!([]));
}

#[test]
fn query_commands() {
    let h = imported();
    let checked = h.ok(
        "check_query",
        json!({ "spec": { "kind": "text", "text": "status:404   group:url" } }),
    );
    assert_eq!(checked["query"], "status:404 group:url");
    let syntax = h
        .invoke(
            "check_query",
            json!({ "spec": { "kind": "text", "text": "grop:url" } }),
        )
        .unwrap_err();
    assert_eq!(syntax["kind"], "syntax");
    assert_eq!(syntax["span"], json!([0, 4]));

    let page = h.ok(
        "run_query",
        json!({ "site": "acme.com", "spec": { "kind": "text", "text": "group:url sort:url:asc" }, "offset": 0, "limit": 2 }),
    );
    assert_eq!(page["rows"].as_array().unwrap().len(), 2);
    assert_eq!(page["hasMore"], true);
    assert_eq!(page["outcome"]["columns"][0]["name"], "url");
    let template = h.ok(
        "run_query",
        json!({ "site": "acme.com", "spec": { "kind": "template", "id": "bots" }, "offset": 0, "limit": 100 }),
    );
    assert_eq!(template["rows"][0][0], "googlebot-desktop");
    assert_eq!(template["rows"][0][1], "verified");
    assert!(h.ok("list_templates", json!({})).as_array().unwrap().len() > 20);

    let exported = h.dir.path().join("export.json");
    let report = h.ok(
        "export_query",
        json!({ "site": "acme.com", "spec": { "kind": "text", "text": "group:status" }, "path": exported.display().to_string() }),
    );
    assert_eq!(report["format"], "json");
    let overview = h.ok("site_overview", json!({ "name": "acme.com" }));
    assert_eq!(overview["hits"], 3);

    let suggestions = h.ok(
        "suggest_values",
        json!({ "site": "acme.com", "field": "ua", "search": "googlebot", "filter": "date:2026-10-07", "limit": 20 }),
    );
    assert_eq!(
        suggestions,
        json!([{ "value": "Googlebot/2.1", "hits": 2 }])
    );
}

#[test]
fn analysis_commands() {
    let h = imported();
    let crawl = h.ok(
        "crawl_overview",
        json!({ "site": "acme.com", "period": null }),
    );
    assert_eq!(crawl["total_hits"], 3);
    assert_eq!(crawl["search"]["hits"], 2);

    let list = h.dir.path().join("urls.txt");
    std::fs::write(&list, "https://acme.com/a\nhttps://acme.com/b\n").unwrap();
    let compared = h.ok(
        "compare_urls",
        json!({
            "site": "acme.com",
            "sources": [list.display().to_string()],
            "filter": "",
            "normalize": { "query": "drop-tracking", "ignore_trailing_slash": false, "ignore_case": false },
            "host": null
        }),
    );
    assert_eq!(compared["report"]["summary"]["both"], 1);
    assert_eq!(compared["columns"][1]["name"], "presence");

    let robots = h.dir.path().join("robots.txt");
    std::fs::write(&robots, "User-agent: *\nDisallow: /b\n").unwrap();
    let checked = h.ok(
        "robots_compliance",
        json!({ "site": "acme.com", "robots": robots.display().to_string(), "filter": null }),
    );
    assert!(checked["report"]["bots"].is_array());
    assert_eq!(checked["columns"][6]["name"], "rule");

    let periods = h.ok(
        "compare_periods",
        json!({ "site": "acme.com", "before": "2026-10-07", "after": "2026-10-08", "group": "url", "filter": null }),
    );
    assert_eq!(periods["report"]["summary"]["before_hits"], 2);

    let saved = h.dir.path().join("saved.csv");
    let rows = h.ok(
        "save_table",
        json!({ "path": saved.display().to_string(), "table": { "columns": compared["columns"], "rows": compared["rows"] } }),
    );
    assert_eq!(rows, 4);
    assert!(
        std::fs::read_to_string(&saved)
            .unwrap()
            .starts_with("url,presence,hits")
    );
}

#[test]
fn bot_and_import_commands() {
    let h = imported();
    let check = h.ok(
        "verify_ip",
        json!({ "ip": "66.249.66.1", "bot": "googlebot-smartphone", "dns": false }),
    );
    assert_eq!(check["verdict"], "verified");
    let verified = h.ok(
        "verify_site_bots",
        json!({ "site": "acme.com", "dns": false, "all": true }),
    );
    assert_eq!(verified["verdicts"]["verified"], 1);
    assert!(
        h.ok("list_bots", json!({ "category": null }))
            .as_array()
            .unwrap()
            .len()
            > 80
    );
    assert_eq!(h.ok("bot_range_status", json!({}))[0]["origin"], "bundled");

    let geo = h.ok("geo_status", json!({}));
    assert_eq!(geo["release"], Value::Null);
    assert!(geo["attribution"].as_str().unwrap().contains("DB-IP"));

    assert_eq!(h.ok("list_custom_bots", json!({})), json!([]));
    let saved = h.ok(
        "save_custom_bots",
        json!({ "bots": [{ "id": "Acme-Monitor", "category": "monitoring", "patterns": ["AcmeMonitor"], "ip_ranges": ["203.0.113.0/24"] }] }),
    );
    assert_eq!(saved[0]["id"], "acme-monitor");
    let listed = h.ok("list_bots", json!({ "category": "monitoring" }));
    assert_eq!(listed[0]["id"], "acme-monitor");
    assert_eq!(listed[0]["custom"], true);
    let builtin = h
        .invoke(
            "save_custom_bots",
            json!({ "bots": [{ "id": "googlebot-smartphone", "patterns": ["x"] }] }),
        )
        .unwrap_err();
    assert_eq!(builtin["kind"], "invalid-input");
    let exported = h.dir.path().join("bots.json");
    let exported_path = exported.to_string_lossy();
    assert_eq!(
        h.ok("export_custom_bots", json!({ "path": exported_path })),
        1
    );
    h.ok("save_custom_bots", json!({ "bots": [] }));
    let imported = h.ok("import_custom_bots", json!({ "path": exported_path }));
    assert_eq!(imported.as_array().unwrap().len(), 1);

    let inspect = h.ok(
        "inspect_files",
        json!({ "paths": [h.dir.path().join("access.log").display().to_string()], "format": { "kind": "auto" } }),
    );
    assert_eq!(inspect["parsed_lines"], 3);
    assert_eq!(h.ok("cancel_import", json!({ "importId": "nope" })), false);
}

/// The texts the desktop filter form generates (`src/lib/filterForm.ts`) must parse to the
/// syntax trees the form reads back; the same fixtures drive the TypeScript tests.
#[test]
fn filter_form_fixtures_match_the_parser() {
    let fixtures: Vec<Value> =
        serde_json::from_str(include_str!("../../src/lib/filterForm.fixtures.json")).unwrap();
    let h = Harness::new();
    for fixture in &fixtures {
        let parsed = h.ok("parse_query", json!({ "text": fixture["text"] }));
        assert_eq!(parsed, fixture["query"], "{}", fixture["name"]);
    }
    let schema = h.ok("query_schema", json!({}));
    assert_eq!(
        schema["fields"][0],
        json!({ "name": "date", "kind": "date" })
    );
}

#[test]
fn saved_query_commands() {
    let h = imported();
    let saved = h.ok(
        "save_query",
        json!({ "name": "Googlebot 404", "description": "weekly", "query": "bot:googlebot*   status:404", "overwrite": false }),
    );
    assert_eq!(saved["query"], "bot:googlebot* status:404");
    let duplicate = h
        .invoke(
            "save_query",
            json!({ "name": "Googlebot 404", "description": "", "query": "status:404", "overwrite": false }),
        )
        .unwrap_err();
    assert_eq!(duplicate["kind"], "duplicate");
    let syntax = h
        .invoke(
            "save_query",
            json!({ "name": "broken", "description": "", "query": "grop:url", "overwrite": false }),
        )
        .unwrap_err();
    assert_eq!(syntax["kind"], "syntax");
    assert_eq!(
        h.ok("list_saved_queries", json!({}))
            .as_array()
            .unwrap()
            .len(),
        1
    );

    let page = h.ok(
        "run_query",
        json!({ "site": "acme.com", "spec": { "kind": "saved", "name": "Googlebot 404" }, "offset": 0, "limit": 10 }),
    );
    assert_eq!(page["outcome"]["query"], "bot:googlebot* status:404");

    let file = h.dir.path().join("saved.json");
    assert_eq!(
        h.ok(
            "export_saved_queries",
            json!({ "path": file.display().to_string() })
        ),
        1
    );
    h.ok("remove_saved_query", json!({ "name": "Googlebot 404" }));
    let report = h.ok(
        "import_saved_queries",
        json!({ "path": file.display().to_string(), "overwrite": false }),
    );
    assert_eq!(report["added"], json!(["Googlebot 404"]));
}

#[test]
fn archive_commands() {
    let h = imported();
    let path = h.dir.path().join("acme.snxsite");
    let path_text = path.to_string_lossy();
    let report = h.ok(
        "export_site",
        json!({ "site": "acme.com", "path": path_text }),
    );
    assert!(report["bytes"].as_u64().unwrap() > 0);
    let duplicate = h
        .invoke(
            "import_site",
            json!({ "path": path_text, "name": null, "client": null }),
        )
        .unwrap_err();
    assert_eq!(duplicate["kind"], "duplicate");
    let copy = h.ok(
        "import_site",
        json!({ "path": path_text, "name": "copy.acme.com", "client": null }),
    );
    assert_eq!(copy["name"], "copy.acme.com");
    let invalid = h
        .invoke(
            "import_site",
            json!({ "path": h.dir.path().join("nope.snxsite").to_string_lossy(), "name": null, "client": null }),
        )
        .unwrap_err();
    assert_eq!(invalid["kind"], "error");
}
