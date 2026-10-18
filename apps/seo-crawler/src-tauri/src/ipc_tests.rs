//! Verify desktop request names and response shapes against the frontend contract.

use std::collections::HashMap;
use std::sync::Mutex;

use serde_json::{Value, json};
use tauri::http::HeaderMap;
use tauri::ipc::{CallbackFn, InvokeBody};
use tauri::test::{INVOKE_KEY, get_ipc_response, mock_builder, mock_context, noop_assets};
use tauri::webview::InvokeRequest;
use tauri::{WebviewUrl, WebviewWindowBuilder};

#[test]
fn empty_workspace_and_invalid_requests() {
    let dir = tempfile::tempdir().unwrap();
    let state = crate::AppState {
        workspace: octopuson_spider_app::SpiderWorkspace::resolve(Some(dir.path().to_owned()))
            .unwrap(),
        jobs: Mutex::new(HashMap::new()),
        opportunities: octopuson_spider_app::opportunities::OpportunityWorkspace::new(
            dir.path().join("opportunities"),
        ),
        opportunity_jobs: octopuson_spider_app::opportunities::OpportunityJobs::default(),
    };
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
    assert_eq!(invoke("list_crawls", json!({})).unwrap(), json!([]));
    assert_eq!(invoke("crawl_storage", json!({})).unwrap(), json!([]));
    assert_eq!(invoke("list_jobs", json!({})).unwrap(), json!([]));
    assert_opportunity_requests(&invoke, dir.path());
    assert_schema_requests(&invoke);
    assert_eq!(invoke("list_crawls", json!({})).unwrap(), json!([]));
    assert_sitemap_requests(&invoke, dir.path());
    assert_comparison_requests(&invoke, dir.path());
    assert!(invoke("start_crawl", json!({"config":{"start_urls":[]}})).is_err());
    assert_invalid_speed_requests(&invoke);
    assert_invalid_viewport_requests(&invoke);
    assert!(invoke("control_crawl", json!({"jobId":"missing","action":"pause"})).is_err());
    assert!(
        invoke(
            "crawl_page",
            json!({"id":"missing","request":{"tab":"internal","limit":250,"offset":0}})
        )
        .is_err()
    );
    assert!(
        invoke(
            "url_detail",
            json!({"id":"missing","url":"https://example.com/"})
        )
        .is_err()
    );
    assert!(invoke("delete_crawl", json!({"id":"../escape"})).is_err());
    assert!(
        invoke(
            "url_duplicate_groups",
            json!({"id":"missing","url":"https://example.com/"})
        )
        .is_err()
    );
    assert!(
        invoke(
            "url_content_match",
            json!({"id":"missing","url":"https://example.com/"})
        )
        .is_err()
    );
    assert!(
        invoke(
            "url_redirects",
            json!({"id":"missing","url":"https://example.com/"})
        )
        .is_err()
    );
    let invalid_direction = invoke("crawl_page", json!({"id":"missing", "request":{"tab":"links", "link_scope":{"url":"https://example.com/", "direction":"invalid"}}})).unwrap_err();
    assert!(invalid_direction.contains("unknown variant"));
    let mismatched = invoke("export_crawl", json!({"id":"missing", "tab":"internal", "path":"unused.json", "selection":{"request":{"tab":"external"},"columns":["address"]}})).unwrap_err();
    assert!(mismatched.contains("does not match"));
    assert!(
        invoke(
            "export_crawl",
            json!({"id":"missing", "tab":"internal", "path":"unused.json"})
        )
        .is_err()
    );
    assert_storage_snapshot(&invoke, dir.path());
}

fn assert_schema_requests(invoke: &impl Fn(&str, Value) -> Result<Value, String>) {
    let result=invoke("schema_test",json!({"request":{"mode":"jsonld","input":"{\"@context\":\"https://schema.org\",\"@type\":\"Product\"}"}})).unwrap();
    assert_eq!(result["report"]["items"][0]["profile"], "Product snippet");
    assert_eq!(result["report"]["items"][0]["status"], "error");
    assert!(result["url"].is_null());
    assert!(result["status"].is_null());
    assert!(
        invoke(
            "schema_test",
            json!({"request":{"mode":"unknown","input":"test"}})
        )
        .is_err()
    );
}

fn assert_storage_snapshot(
    invoke: &impl Fn(&str, Value) -> Result<Value, String>,
    root: &std::path::Path,
) {
    let dir = root.join("crawls/size-test");
    std::fs::create_dir_all(&dir).unwrap();
    let meta = serde_json::to_vec(&json!({
        "id":"size-test", "name":"Size test", "start_urls":[], "mode":"spider",
        "created_at_ms":0, "finished_at_ms":null, "status":"running",
        "progress":octopuson_spider_app::CrawlProgress::default(),
        "limit_reached":false, "error":null, "config":octopuson_spider_app::CrawlConfig::default(),
    }))
    .unwrap();
    std::fs::write(dir.join("crawl.json"), &meta).unwrap();
    std::fs::write(dir.join("crawl.duckdb"), b"invalid database").unwrap();
    std::fs::write(dir.join("crawl.duckdb.wal"), b"wal").unwrap();
    assert_eq!(
        invoke("crawl_storage", json!({})).unwrap(),
        json!([
            {"id":"size-test", "bytes":meta.len() + b"invalid database".len() + b"wal".len()}
        ])
    );
    assert_eq!(std::fs::read(dir.join("crawl.json")).unwrap(), meta);
    assert_eq!(std::fs::read(dir.join("crawl.duckdb.wal")).unwrap(), b"wal");
    assert!(invoke("delete_crawl", json!({"id":"size-test"})).is_err());
    assert!(dir.join("crawl.duckdb").is_file());
}

fn assert_invalid_speed_requests(invoke: &impl Fn(&str, Value) -> Result<Value, String>) {
    for (speed, message) in [
        (json!({"threads":0}), "concurrent requests"),
        (json!({"threads":65}), "concurrent requests"),
        (
            json!({"max_requests_per_second":-1.0}),
            "requests per second",
        ),
    ] {
        let error = invoke(
            "start_crawl",
            json!({"config":{"start_urls":["https://example.com/"],"speed":speed}}),
        )
        .unwrap_err();
        assert!(error.contains(message), "{error}");
        assert_eq!(invoke("list_jobs", json!({})).unwrap(), json!([]));
        assert_eq!(invoke("list_crawls", json!({})).unwrap(), json!([]));
    }
}

fn assert_invalid_viewport_requests(invoke: &impl Fn(&str, Value) -> Result<Value, String>) {
    for (width, height) in [(239, 800), (390, 3841)] {
        let error = invoke("start_crawl", json!({"config":{"start_urls":["https://example.com/"],"rendering":{"mode":"javascript","viewport":{"width":width,"height":height,"mobile":true}}}})).unwrap_err();
        assert!(error.contains("viewport"), "{error}");
        assert_eq!(invoke("list_jobs", json!({})).unwrap(), json!([]));
        assert_eq!(invoke("list_crawls", json!({})).unwrap(), json!([]));
    }
}

fn assert_sitemap_requests(
    invoke: &impl Fn(&str, Value) -> Result<Value, String>,
    path: &std::path::Path,
) {
    for cmd in ["sitemap_plan", "export_sitemap"] {
        let error=invoke(cmd,json!({"id":"missing","selection":{"origin":"https://example.com"},"format":"xml","path":path.join("sitemap.xml")})).unwrap_err();
        assert!(error.contains("no crawl"), "{cmd}: {error}");
    }
    assert!(
        invoke(
            "export_sitemap",
            json!({"id":"missing","selection":{},"format":"html","path":"sitemap.xml"})
        )
        .unwrap_err()
        .contains("unknown variant")
    );
}

fn assert_comparison_requests(
    invoke: &impl Fn(&str, Value) -> Result<Value, String>,
    path: &std::path::Path,
) {
    for cmd in [
        "comparison_summary",
        "comparison_page",
        "comparison_detail",
        "export_comparison",
        "finding_comparison_summary",
        "finding_comparison_page",
        "export_finding_comparison",
    ] {
        let error=invoke(cmd,json!({"baseline":"missing-a","current":"missing-b","internalOnly":true,"url":"https://example.com/","request":{},"path":path.join("comparison.csv")})).unwrap_err();
        assert!(error.contains("no crawl"), "{cmd}: {error}");
    }
    for cmd in ["comparison_summary", "finding_comparison_summary"] {
        let error = invoke(
            cmd,
            json!({"baseline":"missing-a","current":"missing-b","internalOnly":true}),
        )
        .unwrap_err();
        assert!(error.contains("no crawl"), "{cmd}: {error}");
    }
    let invalid_filter = invoke(
        "comparison_page",
        json!({"baseline":"a","current":"b","request":{"filter":"deleted"}}),
    )
    .unwrap_err();
    assert!(invalid_filter.contains("unknown variant"));
    let invalid_finding_change = invoke(
        "finding_comparison_page",
        json!({"baseline":"a","current":"b","request":{"change":"fixed"}}),
    )
    .unwrap_err();
    assert!(invalid_finding_change.contains("unknown variant"));
}

fn assert_opportunity_requests(
    invoke: &impl Fn(&str, Value) -> Result<Value, String>,
    root: &std::path::Path,
) {
    let status = invoke("opportunity_status", json!({})).unwrap();
    assert_eq!(status["projects"], json!([]));
    assert_eq!(status["bytes"], 0);
    assert!(status["google"]["configured"].is_boolean());
    let client_path = root.join("web-client.json");
    std::fs::write(&client_path, br#"{"web":{"client_id":"fixture.apps.googleusercontent.com","client_secret":"do-not-log-value"}}"#).unwrap();
    let client_error = invoke(
        "opportunity_google_client_import",
        json!({"path":client_path}),
    )
    .unwrap_err();
    assert!(client_error.contains("Desktop app OAuth client"));
    assert!(!client_error.contains("do-not-log-value"));
    let project = invoke(
        "opportunity_project_save",
        json!({"project":{"name":"IPC test","site_url":"https://example.com/"}}),
    )
    .unwrap();
    let id = project["id"].as_str().unwrap();
    let path = root.join("import.csv");
    std::fs::write(
        &path,
        "Page,Clicks,Impressions,Position\nhttps://example.com/tr/,10,2000,8\n",
    )
    .unwrap();
    let request = json!({"project_id":id,"path":path,"source":"gsc","grain":"pages","range":{"start":"2026-09-01","end":"2026-09-28"},"locale":"en"});
    let preview = invoke("opportunity_import_preview", json!({"request":request})).unwrap();
    assert_eq!(preview["valid"], 1);
    invoke(
        "opportunity_import",
        json!({"request":request,"digest":preview["digest"]}),
    )
    .unwrap();
    invoke(
        "opportunity_import",
        json!({"request":request,"digest":preview["digest"]}),
    )
    .unwrap();
    assert_eq!(invoke("opportunity_status", json!({})).unwrap()["rows"], 1);
    assert!(
        invoke(
            "opportunity_import",
            json!({"request":request,"digest":"stale"})
        )
        .is_err()
    );
    assert!(
        invoke(
            "opportunity_page",
            json!({"request":{"analysis_id":"missing","limit":100}})
        )
        .is_err()
    );
    invoke("opportunity_project_remove", json!({"id":id})).unwrap();
    assert_eq!(
        invoke("opportunity_status", json!({})).unwrap()["projects"],
        json!([])
    );
}
