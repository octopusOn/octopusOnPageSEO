use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use octopuson_cloud::TokenStore;
use octopuson_core::http::HttpTransport;
use octopuson_core::http::testing::FakeTransport;
use octopuson_secrets::{FileBackend, Secrets};
use serde_json::{Value, json};

use crate::jobs::NoEvents;
use crate::router::{ErrorCode, Opener};
use crate::{Host, Platform, Workspace};

/// A gateway address nothing listens on: tests never reach a real one.
const LOCAL_GATEWAY: &str = "http://127.0.0.1:9";

const OVERVIEW: &str =
    include_str!("../../octopuson-dataforseo/tests/fixtures/domain_rank_overview.json");

const CATALOG: &str = r#"{"credit_unit_micros":1000,"portal_url":"https://my.octopuson.com","connectors":[{"id":"dataforseo","name":"DataForSEO","description":"","upstream":"https://api.dataforseo.com","routes":[{"method":"POST","path":"/v3/dataforseo_labs/google/*/live"}],"status":"active","price_multiplier":1.35}]}"#;

fn workspace(dir: &Path, host: Host, transport: &Arc<FakeTransport>) -> Workspace {
    let secrets = Secrets::new(
        Box::new(FileBackend::new(dir.join("secrets.json"))),
        dir.join("secret-bindings.json"),
    )
    .with_environment(|_| None);
    let tokens = TokenStore::new(
        Box::new(FileBackend::new(dir.join("account.json"))),
        LOCAL_GATEWAY,
    );
    let shared: Arc<dyn HttpTransport> = transport.clone();
    Workspace::new(dir.to_owned(), host, secrets, tokens).with_transport(shared)
}

fn platform(dir: &Path, transport: &Arc<FakeTransport>) -> Platform {
    let open: Opener = Arc::new(|_| true);
    Platform::new(
        workspace(dir, Host::Desktop, transport),
        Arc::new(NoEvents),
        open,
    )
}

fn call(platform: &Platform, command: &str, arguments: Value) -> Value {
    platform
        .call(command, arguments)
        .unwrap_or_else(|error| panic!("{command}: {error:?}"))
}

fn error(platform: &Platform, command: &str, arguments: Value) -> ErrorCode {
    platform.call(command, arguments).unwrap_err().code
}

fn project(platform: &Platform) -> i64 {
    let created = call(
        platform,
        "create_project",
        json!({ "spec": { "domain": "https://www.example.com/", "country": "tr", "language": "tr", "competitors": ["rival.com"] } }),
    );
    created["id"].as_i64().unwrap()
}

#[test]
fn every_module_registers_its_commands() {
    let names: Vec<&str> = crate::modules::router().names().collect();
    for expected in [
        "app_info",
        "get_settings",
        "save_settings",
        "list_projects",
        "create_project",
        "update_project",
        "delete_project",
        "list_providers",
        "set_key",
        "remove_key",
        "set_source_mode",
        "set_budget",
        "usage",
        "account_status",
        "start_account_login",
        "account_logout",
        "account_page",
        "report_problem",
        "list_jobs",
        "cancel_job",
        "clear_jobs",
        "overview_estimate",
        "get_overview",
        "fetch_overview",
        "open_url",
        "keywords_estimate",
        "get_keywords",
        "fetch_keywords",
        "keyword_autocomplete",
        "keyword_lists",
        "add_to_keyword_list",
        "cluster_keyword_list",
        "backlinks_estimate",
        "get_backlinks",
        "fetch_backlinks",
        "gap_estimate",
        "get_gap",
        "fetch_gap",
        "rank_setup",
        "save_rank_settings",
        "add_rank_tracker",
        "remove_rank_tracker",
        "track_keywords",
        "untrack_keywords",
        "set_keyword_tags",
        "rank_estimate",
        "start_rank_check",
        "rank_runs",
        "rank_report",
        "rank_keyword",
        "rank_volumes_estimate",
        "fetch_rank_volumes",
        "audit_setup",
        "save_audit_settings",
        "start_audit",
        "audit_report",
        "audit_issue",
        "delete_audit",
        "ai_setup",
        "save_ai_settings",
        "add_ai_prompts",
        "remove_ai_prompts",
        "suggest_ai_prompts",
        "ai_estimate",
        "start_ai_run",
        "ai_runs",
        "ai_report",
        "ai_answer",
        "ai_mentions_estimate",
        "get_ai_mentions",
        "fetch_ai_mentions",
        "google_status",
        "set_google_client",
        "start_google_login",
        "google_logout",
        "google_properties",
        "gsc_setup",
        "save_gsc_settings",
        "start_gsc_sync",
        "gsc_report",
        "gsc_query",
        "reports_setup",
        "save_report_branding",
        "save_report",
        "delete_report",
        "report_html",
        "report_pdf",
        "report_xlsx",
        "share_report",
        "unshare_report",
        "report_deliveries",
        "send_report",
    ] {
        assert!(names.contains(&expected), "{expected} missing");
    }
}

#[test]
fn unknown_commands_and_bad_arguments_are_told_apart() {
    let dir = tempfile::tempdir().unwrap();
    let platform = platform(dir.path(), &Arc::new(FakeTransport::new()));
    assert_eq!(
        error(&platform, "no_such_command", Value::Null),
        ErrorCode::NotFound
    );
    assert_eq!(
        error(&platform, "get_project", json!({ "id": "x" })),
        ErrorCode::Invalid
    );
    assert_eq!(
        error(&platform, "list_projects", json!({ "extra": 1 })),
        ErrorCode::Invalid
    );
    assert_eq!(
        error(&platform, "get_project", json!({ "id": 99 })),
        ErrorCode::NotFound
    );
    let info = call(&platform, "app_info", Value::Null);
    assert_eq!(info["host"], "desktop");
    assert_eq!(info["name"], "OctopusOn GEO-SEO Platform");
}

#[test]
fn projects_go_through_the_router() {
    let dir = tempfile::tempdir().unwrap();
    let platform = platform(dir.path(), &Arc::new(FakeTransport::new()));
    let id = project(&platform);
    let listed = call(&platform, "list_projects", Value::Null);
    assert_eq!(listed[0]["domain"], "example.com");
    assert_eq!(listed[0]["competitors"], json!(["rival.com"]));
    let updated = call(
        &platform,
        "update_project",
        json!({ "id": id, "spec": { "name": "Örnek", "domain": "example.com", "country": "tr", "language": "tr" } }),
    );
    assert_eq!(updated["name"], "Örnek");
    assert_eq!(
        error(
            &platform,
            "create_project",
            json!({ "spec": { "domain": "nope", "country": "tr", "language": "tr" } })
        ),
        ErrorCode::Invalid
    );
    call(&platform, "delete_project", json!({ "id": id }));
    assert_eq!(call(&platform, "list_projects", Value::Null), json!([]));
}

#[test]
fn a_paid_question_without_key_or_credits_is_blocked_before_any_request() {
    let dir = tempfile::tempdir().unwrap();
    let transport = Arc::new(FakeTransport::new());
    let platform = platform(dir.path(), &transport);
    let id = project(&platform);
    let estimate = call(&platform, "overview_estimate", json!({ "project_id": id }));
    assert_eq!(estimate["via"], Value::Null);
    assert_eq!(estimate["blocked"]["code"], "no_source");
    assert!(
        estimate["blocked"]["message"]
            .as_str()
            .unwrap()
            .contains("DataForSEO")
    );
    assert_eq!(
        call(&platform, "get_overview", json!({ "project_id": id })),
        Value::Null
    );
    assert_eq!(
        error(&platform, "fetch_overview", json!({ "project_id": id })),
        ErrorCode::NoSource
    );
    assert_eq!(transport.requests().len(), 0);
}

#[test]
fn own_keys_fetch_keep_and_reuse_answers() {
    let dir = tempfile::tempdir().unwrap();
    let transport = Arc::new(
        FakeTransport::new()
            .reply(200, OVERVIEW)
            .reply(200, OVERVIEW),
    );
    let platform = platform(dir.path(), &transport);
    let id = project(&platform);
    assert_eq!(
        error(
            &platform,
            "set_key",
            json!({ "provider": "dataforseo", "name": "credentials", "value": "no-colon" })
        ),
        ErrorCode::Invalid
    );
    call(
        &platform,
        "set_key",
        json!({ "provider": "dataforseo", "name": "credentials", "value": "me@example.com:secret" }),
    );
    let providers = call(&platform, "list_providers", Value::Null);
    assert_eq!(providers[0]["via"], "own");
    assert_eq!(providers[0]["keys"][0]["state"], "saved");

    let estimate = call(&platform, "overview_estimate", json!({ "project_id": id }));
    assert_eq!(estimate["via"], "own");
    assert_eq!(estimate["blocked"], Value::Null);
    assert_eq!(estimate["credits"], Value::Null);

    let fetched = call(&platform, "fetch_overview", json!({ "project_id": id }));
    assert_eq!(fetched["cached"], false);
    assert_eq!(fetched["via"], "own");
    assert_eq!(fetched["data"]["organic"]["keywords"], 3052);
    assert_eq!(transport.requests().len(), 1);

    // Fresh: the kept answer, nothing paid.
    let again = call(&platform, "fetch_overview", json!({ "project_id": id }));
    assert_eq!(again["cached"], true);
    assert_eq!(again["spent_usd"], 0.0);
    assert_eq!(transport.requests().len(), 1);
    let kept = call(&platform, "get_overview", json!({ "project_id": id }));
    assert_eq!(kept["data"]["target"], "example.com");
    assert_eq!(kept["fresh"], true);

    // Forced: paid again.
    let forced = call(
        &platform,
        "fetch_overview",
        json!({ "project_id": id, "force": true }),
    );
    assert_eq!(forced["cached"], false);
    assert_eq!(transport.requests().len(), 2);

    let usage = call(&platform, "usage", Value::Null);
    assert_eq!(usage[0]["provider"], "dataforseo");
    assert_eq!(usage[0]["via"], "own");
    assert_eq!(usage[0]["requests"], 2);
}

#[test]
fn a_competitor_domain_is_asked_in_the_projects_market() {
    let dir = tempfile::tempdir().unwrap();
    let transport = Arc::new(FakeTransport::new().reply(200, OVERVIEW));
    let platform = platform(dir.path(), &transport);
    let id = project(&platform);
    call(
        &platform,
        "set_key",
        json!({ "provider": "dataforseo", "name": "credentials", "value": "a:b" }),
    );
    call(
        &platform,
        "fetch_overview",
        json!({ "project_id": id, "domain": "https://www.rival.com/" }),
    );
    let body: Value =
        serde_json::from_slice(transport.requests()[0].body.as_deref().unwrap()).unwrap();
    assert_eq!(body[0]["target"], "rival.com");
    assert_eq!(body[0]["location_code"], 2792);
}

#[test]
fn budgets_stop_paid_questions() {
    let dir = tempfile::tempdir().unwrap();
    let transport = Arc::new(FakeTransport::new());
    let platform = platform(dir.path(), &transport);
    let id = project(&platform);
    call(
        &platform,
        "set_key",
        json!({ "provider": "dataforseo", "name": "credentials", "value": "a:b" }),
    );
    call(
        &platform,
        "set_budget",
        json!({ "provider": "dataforseo", "usd": 0.001 }),
    );
    let estimate = call(&platform, "overview_estimate", json!({ "project_id": id }));
    assert_eq!(estimate["blocked"]["code"], "budget");
    assert!(
        estimate["blocked"]["message"]
            .as_str()
            .unwrap()
            .contains("budget")
    );
    assert_eq!(
        error(&platform, "fetch_overview", json!({ "project_id": id })),
        ErrorCode::Budget
    );
    assert_eq!(
        error(
            &platform,
            "set_budget",
            json!({ "provider": "dataforseo", "usd": -1 })
        ),
        ErrorCode::Invalid
    );
    call(
        &platform,
        "set_budget",
        json!({ "provider": "dataforseo", "usd": null }),
    );
    let estimate = call(&platform, "overview_estimate", json!({ "project_id": id }));
    assert_eq!(estimate["blocked"], Value::Null);
    assert_eq!(transport.requests().len(), 0);
}

#[test]
fn credits_go_through_the_gateway_and_record_the_charge() {
    let dir = tempfile::tempdir().unwrap();
    let kept = [
        ("x-octopus-charge-usd", "0.0041"),
        ("x-octopus-full-charge-usd", "0.0136"),
        ("x-octopus-cache", "hit"),
    ];
    let transport = Arc::new(
        FakeTransport::new()
            .reply(200, CATALOG)
            // The gateway keeps an answer to the same question for every account (ADR 0079).
            .reply_with_headers(
                200,
                vec![("x-octopus-cache", "hit")],
                r#"{"cached":true,"cacheable":true,"age_s":60,"charge_usd":0.0041,"full_charge_usd":0.0136}"#,
            )
            .reply_with_headers(200, kept.to_vec(), OVERVIEW)
            .reply_with_headers(200, vec![("x-octopus-charge-usd", "0.0136")], OVERVIEW),
    );
    let platform = platform(dir.path(), &transport);
    platform
        .workspace()
        .set_account_token(Some("oct_test"))
        .unwrap();
    let id = project(&platform);

    let estimate = call(&platform, "overview_estimate", json!({ "project_id": id }));
    assert_eq!(estimate["via"], "credit");
    let credits = estimate["credits"].as_f64().unwrap();
    let expected = octopuson_dataforseo::labs::domain_overview_estimate() * 1.35 / 0.001;
    assert!((credits - expected).abs() < 1e-9, "{credits}");
    assert!((estimate["shared"]["credits"].as_f64().unwrap() - 4.1).abs() < 1e-9);
    assert!((estimate["shared"]["full_credits"].as_f64().unwrap() - 13.6).abs() < 1e-9);

    let fetched = call(&platform, "fetch_overview", json!({ "project_id": id }));
    assert_eq!(fetched["via"], "credit");
    assert!((fetched["spent_usd"].as_f64().unwrap() - 0.0041).abs() < 1e-12);
    assert!((fetched["saved_usd"].as_f64().unwrap() - 0.0095).abs() < 1e-12);
    // Asked again while fresh here: the person wants it anew, past the gateway's kept answer.
    call(
        &platform,
        "fetch_overview",
        json!({ "project_id": id, "force": true }),
    );

    let requests = transport.requests();
    let header = |request: &octopuson_core::http::HttpRequest, name: &str| {
        request
            .headers
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.clone())
    };
    assert_eq!(requests[0].url, format!("{LOCAL_GATEWAY}/v1/catalog"));
    assert_eq!(
        header(&requests[1], "x-octopus-cache").as_deref(),
        Some("peek")
    );
    let proxied = &requests[2];
    assert_eq!(
        proxied.url,
        format!(
            "{LOCAL_GATEWAY}/v1/p/dataforseo/v3/dataforseo_labs/google/domain_rank_overview/live"
        )
    );
    assert_eq!(
        header(proxied, "authorization").as_deref(),
        Some("Bearer oct_test")
    );
    assert_eq!(header(proxied, "cache-control"), None);
    assert_eq!(
        header(&requests[3], "cache-control").as_deref(),
        Some("no-cache")
    );
    let usage = call(&platform, "usage", Value::Null);
    assert_eq!(usage[0]["via"], "credit");

    // Kept fresh here: no price is asked of the gateway.
    let again = call(&platform, "overview_estimate", json!({ "project_id": id }));
    assert_eq!(again["shared"], Value::Null);
    assert_eq!(transport.requests().len(), 4);
}

#[test]
fn the_web_cannot_sign_in_out_or_use_a_proxy() {
    let dir = tempfile::tempdir().unwrap();
    let transport = Arc::new(FakeTransport::new());
    let open: Opener = Arc::new(|_| panic!("the web never opens a browser on the server"));
    let platform = Platform::new(
        workspace(dir.path(), Host::Web, &transport),
        Arc::new(NoEvents),
        open,
    );
    assert_eq!(
        error(&platform, "start_account_login", Value::Null),
        ErrorCode::Unsupported
    );
    assert_eq!(
        error(&platform, "account_logout", Value::Null),
        ErrorCode::Unsupported
    );
    assert_eq!(
        error(
            &platform,
            "save_settings",
            json!({ "settings": { "proxy": "http://10.0.0.1:3128" } })
        ),
        ErrorCode::Invalid
    );
    assert_eq!(
        error(
            &platform,
            "open_url",
            json!({ "url": "file:///etc/passwd" })
        ),
        ErrorCode::Invalid
    );
    let opened = call(
        &platform,
        "open_url",
        json!({ "url": "https://app.dataforseo.com/register" }),
    );
    assert_eq!(opened["opened"], false);
    let page = call(&platform, "account_page", json!({ "page": "credits" }));
    assert_eq!(page["url"], "https://my.octopuson.com/kredi");
    assert_eq!(page["opened"], false);
    let info = call(&platform, "app_info", Value::Null);
    assert_eq!(info["host"], "web");
    assert_eq!(info.get("data_dir"), None);
}

#[test]
fn jobs_report_and_can_be_cancelled() {
    let dir = tempfile::tempdir().unwrap();
    let platform = platform(dir.path(), &Arc::new(FakeTransport::new()));
    let jobs = platform.jobs();
    let id = jobs
        .start("wait", "test", None, |control| {
            control.progress("waiting", 1, 3);
            while control.sleep(Duration::from_millis(20)) {}
            Ok(crate::jobs::JobDone::with(true))
        })
        .unwrap();
    // The same kind for the same project is not started twice.
    let again = jobs
        .start("wait", "test", None, |_| {
            Ok(crate::jobs::JobDone::with(false))
        })
        .unwrap();
    assert_eq!(again, id);
    assert!(
        call(&platform, "cancel_job", json!({ "id": id }))
            .as_bool()
            .unwrap()
    );
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let job = jobs.get(&id).unwrap();
        if job.status != crate::JobStatus::Running {
            assert_eq!(job.status, crate::JobStatus::Cancelled);
            assert_eq!(job.step, "waiting");
            break;
        }
        assert!(Instant::now() < deadline, "the job did not stop");
        std::thread::sleep(Duration::from_millis(10));
    }
    call(&platform, "clear_jobs", Value::Null);
    assert_eq!(call(&platform, "list_jobs", Value::Null), json!([]));
}

const RANKED: &str = include_str!("../../octopuson-dataforseo/tests/fixtures/ranked_keywords.json");
const BULK: &str =
    include_str!("../../octopuson-dataforseo/tests/fixtures/bulk_traffic_estimation.json");

#[test]
fn site_explorer_reports_are_kept_per_report_and_row_count() {
    let dir = tempfile::tempdir().unwrap();
    let transport = Arc::new(
        FakeTransport::new()
            .reply(200, RANKED)
            .reply(200, RANKED)
            .reply(200, BULK),
    );
    let platform = platform(dir.path(), &transport);
    let id = project(&platform);
    call(
        &platform,
        "set_key",
        json!({ "provider": "dataforseo", "name": "credentials", "value": "a:b" }),
    );

    let small = call(
        &platform,
        "overview_estimate",
        json!({ "project_id": id, "report": "keywords", "limit": 100 }),
    );
    let large = call(
        &platform,
        "overview_estimate",
        json!({ "project_id": id, "report": "keywords", "limit": 1000 }),
    );
    assert!(large["usd"].as_f64().unwrap() > small["usd"].as_f64().unwrap());

    let keywords = call(
        &platform,
        "fetch_overview",
        json!({ "project_id": id, "report": "keywords", "limit": 100 }),
    );
    assert_eq!(keywords["data"]["total"], 3052);
    assert_eq!(keywords["data"]["keywords"][0]["keyword"], "örnek ayakkabı");
    let body: Value =
        serde_json::from_slice(transport.requests()[0].body.as_deref().unwrap()).unwrap();
    assert_eq!(body[0]["limit"], 100);
    assert!(
        transport.requests()[0]
            .url
            .ends_with("/ranked_keywords/live")
    );

    // Another row count is another question; the summary has nothing kept.
    assert_eq!(
        call(
            &platform,
            "get_overview",
            json!({ "project_id": id, "report": "keywords", "limit": 500 })
        ),
        Value::Null
    );
    assert_eq!(
        call(&platform, "get_overview", json!({ "project_id": id })),
        Value::Null
    );
    call(
        &platform,
        "fetch_overview",
        json!({ "project_id": id, "report": "keywords", "limit": 500 }),
    );
    assert_eq!(transport.requests().len(), 2);

    // The comparison takes the project's domain and competitors.
    let compared = call(
        &platform,
        "fetch_overview",
        json!({ "project_id": id, "report": "compare" }),
    );
    assert_eq!(compared["data"][1]["target"], "rival.com");
    let body: Value =
        serde_json::from_slice(transport.requests()[2].body.as_deref().unwrap()).unwrap();
    assert_eq!(body[0]["targets"], json!(["example.com", "rival.com"]));
}

#[test]
fn comparing_needs_competitors_and_reports_must_be_known() {
    let dir = tempfile::tempdir().unwrap();
    let platform = platform(dir.path(), &Arc::new(FakeTransport::new()));
    let created = call(
        &platform,
        "create_project",
        json!({ "spec": { "domain": "alone.com", "country": "tr", "language": "tr" } }),
    );
    let id = created["id"].as_i64().unwrap();
    assert_eq!(
        error(
            &platform,
            "overview_estimate",
            json!({ "project_id": id, "report": "compare" })
        ),
        ErrorCode::Invalid
    );
    assert_eq!(
        error(
            &platform,
            "overview_estimate",
            json!({ "project_id": id, "report": "everything" })
        ),
        ErrorCode::Invalid
    );
}

const KEYWORD_OVERVIEW: &str =
    include_str!("../../octopuson-dataforseo/tests/fixtures/keyword_overview.json");
const SUGGESTIONS: &str =
    include_str!("../../octopuson-dataforseo/tests/fixtures/keyword_suggestions.json");
const SERP_PAGE: &str = include_str!("../../octopuson-dataforseo/tests/fixtures/serp_organic.json");

#[test]
fn keyword_reports_measure_suggest_and_show_the_result_page() {
    let dir = tempfile::tempdir().unwrap();
    let transport = Arc::new(
        FakeTransport::new()
            .reply(200, KEYWORD_OVERVIEW)
            .reply(200, SUGGESTIONS)
            .reply(200, SERP_PAGE),
    );
    let platform = platform(dir.path(), &transport);
    let id = project(&platform);
    call(
        &platform,
        "set_key",
        json!({ "provider": "dataforseo", "name": "credentials", "value": "a:b" }),
    );

    let estimate = call(
        &platform,
        "keywords_estimate",
        json!({ "project_id": id, "report": "overview", "keywords": ["spor ayakkabı", "koşu ayakkabısı"] }),
    );
    assert_eq!(estimate["via"], "own");
    let measured = call(
        &platform,
        "fetch_keywords",
        json!({ "project_id": id, "report": "overview", "keywords": ["Spor Ayakkabı", "koşu ayakkabısı"] }),
    );
    assert_eq!(measured["data"]["keywords"][0]["volume"], 2900);
    let body: Value =
        serde_json::from_slice(transport.requests()[0].body.as_deref().unwrap()).unwrap();
    assert_eq!(
        body[0]["keywords"],
        json!(["spor ayakkabı", "koşu ayakkabısı"])
    );

    let suggestions = call(
        &platform,
        "fetch_keywords",
        json!({ "project_id": id, "report": "suggestions", "seed": "ayakkabı", "limit": 100 }),
    );
    assert_eq!(suggestions["data"]["seed"]["volume"], 90_500);
    assert!(
        transport.requests()[1]
            .url
            .ends_with("/keyword_suggestions/live")
    );

    let page = call(
        &platform,
        "fetch_keywords",
        json!({ "project_id": id, "report": "serp", "seed": "spor ayakkabı" }),
    );
    assert_eq!(page["data"]["organic"][0]["domain"], "www.example.com");
    assert!(
        transport.requests()[2]
            .url
            .ends_with("/serp/google/organic/live/advanced")
    );

    // Kept: the overview of the same keywords costs nothing again.
    let again = call(
        &platform,
        "get_keywords",
        json!({ "project_id": id, "report": "overview", "keywords": ["spor ayakkabı", "koşu ayakkabısı"] }),
    );
    assert_eq!(again["cached"], true);
    assert_eq!(
        error(
            &platform,
            "keywords_estimate",
            json!({ "project_id": id, "report": "related" })
        ),
        ErrorCode::Invalid
    );
}

#[test]
fn wordstat_runs_on_the_yandex_cloud_keys() {
    let dir = tempfile::tempdir().unwrap();
    let transport = Arc::new(
        FakeTransport::new().reply(
            200,
            json!({
                "totalCount": "52000",
                "results": [{ "phrase": "türk kahvesi", "count": "30000" }],
                "associations": [{ "phrase": "dibek kahvesi", "count": "4100" }]
            })
            .to_string(),
        ),
    );
    let platform = platform(dir.path(), &transport);
    let id = project(&platform);
    let providers = call(&platform, "list_providers", Value::Null);
    let wordstat = providers
        .as_array()
        .unwrap()
        .iter()
        .find(|provider| provider["id"] == "wordstat")
        .unwrap();
    assert_eq!(wordstat["keys"].as_array().unwrap().len(), 2);
    assert_eq!(wordstat["via"], Value::Null);
    for (name, value) in [("api_key", "yc-secret"), ("folder_id", "b1gfolder")] {
        call(
            &platform,
            "set_key",
            json!({ "provider": "wordstat", "name": name, "value": value }),
        );
    }
    let found = call(
        &platform,
        "fetch_keywords",
        json!({ "project_id": id, "report": "wordstat", "seed": "türk kahvesi" }),
    );
    let keywords: Vec<&str> = found["data"]["keywords"]
        .as_array()
        .unwrap()
        .iter()
        .map(|keyword| keyword["keyword"].as_str().unwrap())
        .collect();
    assert_eq!(keywords, ["türk kahvesi", "dibek kahvesi"]);
    assert_eq!(found["data"]["keywords"][0]["volume"], 30_000);
    let request = &transport.requests()[0];
    assert!(request.url.ends_with("/v2/wordstat/topRequests"));
    let body: Value = serde_json::from_slice(request.body.as_deref().unwrap()).unwrap();
    assert_eq!(body["folderId"], "b1gfolder");
    let usage = call(&platform, "usage", Value::Null);
    assert_eq!(usage[0]["provider"], "wordstat");
}

#[test]
fn autocomplete_is_free_and_lists_group_into_topics() {
    let dir = tempfile::tempdir().unwrap();
    let transport =
        Arc::new(FakeTransport::new().reply(200, r#"["kahve",["kahve falı","kahve makinesi"]]"#));
    let platform = platform(dir.path(), &transport);
    let id = project(&platform);
    let suggested = call(
        &platform,
        "keyword_autocomplete",
        json!({ "project_id": id, "seed": "kahve", "engine": "google" }),
    );
    assert_eq!(suggested, json!(["kahve falı", "kahve makinesi"]));
    assert!(transport.requests()[0].url.contains("hl=tr&gl=tr"));
    assert_eq!(call(&platform, "usage", Value::Null), json!([]));

    let list = call(
        &platform,
        "create_keyword_list",
        json!({ "project_id": id, "name": "Kahve" }),
    );
    let list_id = list["id"].as_i64().unwrap();
    let held = call(
        &platform,
        "add_to_keyword_list",
        json!({ "list_id": list_id, "keywords": [
            { "keyword": "türk kahvesi", "volume": 30000 },
            { "keyword": "türk kahvesi nasıl yapılır", "volume": 9000 },
            { "keyword": "kahve makinesi", "volume": 22000 },
            { "keyword": "kahve makineleri", "volume": 4000 },
            { "volume": 1 }
        ] }),
    );
    assert_eq!(held, 4);
    let topics = call(
        &platform,
        "cluster_keyword_list",
        json!({ "list_id": list_id }),
    );
    let pillars: Vec<&str> = topics
        .as_array()
        .unwrap()
        .iter()
        .map(|topic| topic["pillar"].as_str().unwrap())
        .collect();
    // The how-to question is a page of its own; the two machines are one.
    assert_eq!(
        pillars,
        [
            "türk kahvesi",
            "kahve makinesi",
            "türk kahvesi nasıl yapılır"
        ]
    );
    assert_eq!(topics[1]["volume"], 26_000);
    assert_eq!(topics[1]["keywords"].as_array().unwrap().len(), 2);
    assert_eq!(
        error(
            &platform,
            "add_to_keyword_list",
            json!({ "list_id": list_id, "keywords": [{}] })
        ),
        ErrorCode::Invalid
    );
    call(
        &platform,
        "delete_keyword_list",
        json!({ "list_id": list_id }),
    );
    assert_eq!(
        call(&platform, "keyword_lists", json!({ "project_id": id })),
        json!([])
    );
}

const LINK_SUMMARY: &str =
    include_str!("../../octopuson-dataforseo/tests/fixtures/backlinks_summary.json");
const LINKS: &str =
    include_str!("../../octopuson-dataforseo/tests/fixtures/backlinks_backlinks.json");
const RANKS: &str =
    include_str!("../../octopuson-dataforseo/tests/fixtures/backlinks_bulk_ranks.json");

#[test]
fn backlink_reports_profile_list_and_rank_sites() {
    let dir = tempfile::tempdir().unwrap();
    let transport = Arc::new(
        FakeTransport::new()
            .reply(200, LINK_SUMMARY)
            .reply(200, LINKS)
            .reply(200, RANKS),
    );
    let platform = platform(dir.path(), &transport);
    let id = project(&platform);
    call(
        &platform,
        "set_key",
        json!({ "provider": "dataforseo", "name": "credentials", "value": "a:b" }),
    );
    let summary = call(&platform, "fetch_backlinks", json!({ "project_id": id }));
    assert_eq!(summary["data"]["rank"], 48);
    assert!(
        transport.requests()[0]
            .url
            .ends_with("/backlinks/summary/live")
    );

    let lost = call(
        &platform,
        "fetch_backlinks",
        json!({ "project_id": id, "report": "backlinks", "status": "broken", "dofollow_only": true }),
    );
    assert_eq!(lost["data"][1]["is_broken"], true);
    let body: Value =
        serde_json::from_slice(transport.requests()[1].body.as_deref().unwrap()).unwrap();
    assert_eq!(
        body[0]["filters"],
        json!([["is_broken", "=", true], "and", ["dofollow", "=", true]])
    );

    // Another state is another question.
    assert_eq!(
        call(
            &platform,
            "get_backlinks",
            json!({ "project_id": id, "report": "backlinks", "status": "new" })
        ),
        Value::Null
    );

    let ranks = call(
        &platform,
        "fetch_backlinks",
        json!({ "project_id": id, "report": "bulk" }),
    );
    assert_eq!(ranks["data"][1]["rank"], 61);
    let body: Value =
        serde_json::from_slice(transport.requests()[2].body.as_deref().unwrap()).unwrap();
    assert_eq!(body[0]["targets"], json!(["example.com", "rival.com"]));

    let estimate = call(
        &platform,
        "backlinks_estimate",
        json!({ "project_id": id, "report": "referring_domains", "limit": 1000 }),
    );
    let expected = octopuson_dataforseo::prices::backlinks(1000);
    assert!((estimate["usd"].as_f64().unwrap() - expected).abs() < 1e-12);
}

const MISSING_RIVAL: &str =
    include_str!("../../octopuson-dataforseo/tests/fixtures/intersection_missing_rival.json");
const MISSING_OTHER: &str =
    include_str!("../../octopuson-dataforseo/tests/fixtures/intersection_missing_other.json");
const BULK_TRAFFIC: &str =
    include_str!("../../octopuson-dataforseo/tests/fixtures/bulk_traffic_estimation.json");

#[test]
fn gaps_ask_once_per_competitor_and_pay_together() {
    let dir = tempfile::tempdir().unwrap();
    let transport = Arc::new(
        FakeTransport::new()
            .reply(200, MISSING_RIVAL)
            .reply(200, MISSING_OTHER)
            .reply(200, BULK_TRAFFIC)
            .reply(200, RANKS),
    );
    let platform = platform(dir.path(), &transport);
    let created = call(
        &platform,
        "create_project",
        json!({ "spec": { "domain": "example.com", "country": "tr", "language": "tr", "competitors": ["rival.com", "other.com.tr"] } }),
    );
    let id = created["id"].as_i64().unwrap();
    call(
        &platform,
        "set_key",
        json!({ "provider": "dataforseo", "name": "credentials", "value": "a:b" }),
    );
    let estimate = call(&platform, "gap_estimate", json!({ "project_id": id }));
    let one = octopuson_dataforseo::site::estimate(100);
    assert!((estimate["usd"].as_f64().unwrap() - 2.0 * one).abs() < 1e-12);
    assert_eq!(estimate["kept"], Value::Null);
    assert_eq!(
        call(&platform, "get_gap", json!({ "project_id": id })),
        Value::Null
    );

    let gap = call(&platform, "fetch_gap", json!({ "project_id": id }));
    assert_eq!(gap["data"]["keywords"][0]["keyword"], "koşu ayakkabısı");
    assert_eq!(
        gap["data"]["keywords"][0]["competitors"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(transport.requests().len(), 2);
    let first: Value =
        serde_json::from_slice(transport.requests()[0].body.as_deref().unwrap()).unwrap();
    assert_eq!(first[0]["target1"], "rival.com");
    assert_eq!(first[0]["target2"], "example.com");

    // Both parts are kept now: the estimate says so and reading costs nothing.
    let estimate = call(&platform, "gap_estimate", json!({ "project_id": id }));
    assert_eq!(estimate["kept"]["fresh"], true);
    let kept = call(&platform, "get_gap", json!({ "project_id": id }));
    assert_eq!(kept["cached"], true);
    let usage = call(&platform, "usage", Value::Null);
    assert_eq!(usage[0]["requests"], 2);

    // Side by side: traffic and authority of the project and its competitors.
    let bulk = call(
        &platform,
        "fetch_gap",
        json!({ "project_id": id, "report": "bulk" }),
    );
    assert_eq!(bulk["data"][0]["target"], "example.com");
    assert_eq!(bulk["data"][0]["organic_keywords"], 3052);
    assert_eq!(bulk["data"][0]["rank"], 48);
    assert_eq!(bulk["data"][1]["target"], "rival.com");
    assert_eq!(bulk["data"][1]["rank"], 61);
    // A site without data reads as zero traffic and no rank.
    assert_eq!(bulk["data"][2]["target"], "other.com.tr");
    assert_eq!(bulk["data"][2]["organic_traffic"], 0.0);
    assert_eq!(bulk["data"][2]["rank"], Value::Null);
    assert!(
        transport.requests()[3]
            .url
            .ends_with("/backlinks/bulk_ranks/live")
    );

    assert_eq!(
        error(
            &platform,
            "gap_estimate",
            json!({ "project_id": id, "competitors": ["example.com"] })
        ),
        ErrorCode::Invalid
    );
}

const RANK_PAGE: &str = include_str!("../../octopuson-dataforseo/tests/fixtures/serp_rank.json");
const KEYWORD_NUMBERS: &str =
    include_str!("../../octopuson-dataforseo/tests/fixtures/keyword_overview.json");

/// Waits for a job to end.
fn finished(platform: &Platform, id: &str) -> crate::Job {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let job = platform.jobs().get(id).unwrap();
        if job.status != crate::JobStatus::Running {
            return job;
        }
        assert!(Instant::now() < deadline, "the job did not end");
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
#[allow(clippy::too_many_lines)] // one tracker from its first keyword to its schedule
fn rank_checks_run_as_jobs_keep_readings_and_follow_the_schedule() {
    let dir = tempfile::tempdir().unwrap();
    let transport = Arc::new(
        FakeTransport::new()
            .reply(200, RANK_PAGE)
            .reply(500, "{}")
            .reply(200, RANK_PAGE)
            .reply(200, RANK_PAGE)
            .reply(200, KEYWORD_NUMBERS)
            .reply(200, RANK_PAGE)
            .reply(200, RANK_PAGE)
            .reply(200, RANK_PAGE)
            .reply(200, RANK_PAGE),
    );
    let platform = platform(dir.path(), &transport);
    let id = project(&platform);

    let setup = call(&platform, "rank_setup", json!({ "project_id": id }));
    assert_eq!(setup["settings"]["schedule"], "manual");
    assert_eq!(setup["trackers"], json!([]));
    assert_eq!(setup["next_check_at"], Value::Null);
    assert_eq!(
        error(&platform, "start_rank_check", json!({ "project_id": id })),
        ErrorCode::Invalid
    );

    // The first keywords bring the first place: Google in the project's market.
    let tracked = call(
        &platform,
        "track_keywords",
        json!({ "project_id": id, "tags": ["ayakkabı"], "keywords": [
            { "keyword": "spor ayakkabı", "volume": 2900 },
            { "keyword": "koşu ayakkabısı" }
        ] }),
    );
    assert_eq!(tracked, json!({ "added": 2, "total": 2 }));
    let setup = call(
        &platform,
        "add_rank_tracker",
        json!({ "project_id": id, "engine": "google", "device": "mobile", "location": "Istanbul, Istanbul, Turkey" }),
    );
    assert_eq!(setup["trackers"][0]["device"], "desktop");
    assert_eq!(setup["trackers"][1]["location"], "Istanbul,Istanbul,Turkey");
    assert_eq!(setup["missing_volumes"], 1);
    assert_eq!(
        error(
            &platform,
            "add_rank_tracker",
            json!({ "project_id": id, "engine": "google", "location": "Berlin,Germany" })
        ),
        ErrorCode::Invalid
    );

    // Four checks at 20 results with the late AI Overview: 4 × $0.006, blocked without a key.
    let estimate = call(&platform, "rank_estimate", json!({ "project_id": id }));
    assert_eq!(estimate["checks"], 4);
    assert!((estimate["run"]["usd"].as_f64().unwrap() - 0.024).abs() < 1e-9);
    assert_eq!(estimate["run"]["blocked"]["code"], "no_source");
    assert_eq!(estimate["runs_per_month"], 0);
    assert_eq!(
        error(&platform, "start_rank_check", json!({ "project_id": id })),
        ErrorCode::NoSource
    );
    assert_eq!(transport.requests().len(), 0);

    call(
        &platform,
        "set_key",
        json!({ "provider": "dataforseo", "name": "credentials", "value": "me@example.com:secret" }),
    );
    let setup = call(
        &platform,
        "save_rank_settings",
        json!({ "project_id": id, "schedule": "weekly", "depth": 15, "ai_overview": true }),
    );
    assert_eq!(setup["settings"]["depth"], 20);
    assert!(setup["next_check_at"].is_string(), "never checked: due now");
    let estimate = call(&platform, "rank_estimate", json!({ "project_id": id }));
    assert_eq!(estimate["runs_per_month"], 4);
    assert!((estimate["month_usd"].as_f64().unwrap() - 0.096).abs() < 1e-9);

    // A run: the second keyword's first check fails and the run goes on.
    let job = call(&platform, "start_rank_check", json!({ "project_id": id }));
    let job = finished(&platform, job.as_str().unwrap());
    assert_eq!(job.status, crate::JobStatus::Partial);
    assert_eq!(job.result.as_ref().unwrap()["checked"], 3);
    assert_eq!(job.result.as_ref().unwrap()["failed"], 1);
    let requests = transport.requests();
    assert_eq!(requests.len(), 4);
    assert!(
        requests[0]
            .url
            .ends_with("/v3/serp/google/organic/live/advanced")
    );
    let city: Value = serde_json::from_slice(requests[2].body.as_deref().unwrap()).unwrap();
    assert_eq!(city[0]["location_name"], "Istanbul,Istanbul,Turkey");
    assert_eq!(city[0]["device"], "mobile");
    assert_eq!(city[0]["depth"], 20);
    assert_eq!(city[0]["load_async_ai_overview"], true);
    let runs = call(&platform, "rank_runs", json!({ "project_id": id }));
    assert_eq!(runs[0]["status"], "partial");
    assert_eq!(runs[0]["scheduled"], false);
    let usage = call(&platform, "usage", Value::Null);
    assert_eq!(usage[0]["requests"], 3);

    // The report of the first place.
    let report = call(&platform, "rank_report", json!({ "project_id": id }));
    assert_eq!(report["tracker"]["device"], "desktop");
    let spor = &report["rows"][0];
    assert_eq!(spor["keyword"], "spor ayakkabı");
    assert_eq!(spor["position"], 2);
    assert_eq!(spor["url"], "https://www.example.com/spor-ayakkabi");
    assert_eq!(spor["cannibalised"], true);
    assert_eq!(spor["ai_cited"], true);
    assert_eq!(spor["snippet"], true);
    assert_eq!(spor["tags"], json!(["ayakkabı"]));
    let failed = &report["rows"][1];
    assert_eq!(failed["position"], Value::Null);
    assert!(failed["error"].as_str().unwrap().contains("500"));
    assert_eq!(report["summary"]["ranked"], 1);
    assert_eq!(report["summary"]["checked"], 2);
    assert_eq!(report["summary"]["ai_overviews"], 1);
    assert_eq!(report["summary"]["cannibalised"], 1);
    assert_eq!(report["visibility"].as_array().unwrap().len(), 1);
    assert_eq!(report["competitors"][0]["domain"], "example.com");
    assert_eq!(report["competitors"][0]["own"], true);
    assert_eq!(report["competitors"][1]["domain"], "rival.com");
    assert_eq!(report["competitors"][1]["average_position"], 1.0);
    let shares = &report["share_history"][0]["shares"];
    assert!(shares["rival.com"].as_f64().unwrap() > shares["example.com"].as_f64().unwrap());

    let keyword_id = spor["keyword_id"].as_i64().unwrap();
    let tracker_id = report["tracker"]["id"].as_i64().unwrap();
    let detail = call(
        &platform,
        "rank_keyword",
        json!({ "project_id": id, "keyword_id": keyword_id, "tracker_id": tracker_id }),
    );
    assert_eq!(detail["checks"].as_array().unwrap().len(), 1);
    assert_eq!(detail["results"][0]["domain"], "rival.com");
    assert_eq!(detail["checks"][0]["pages"].as_array().unwrap().len(), 2);

    // The missing volume, from Labs keyword numbers.
    let volumes = call(
        &platform,
        "rank_volumes_estimate",
        json!({ "project_id": id }),
    );
    assert_eq!(volumes["blocked"], Value::Null);
    let fetched = call(&platform, "fetch_rank_volumes", json!({ "project_id": id }));
    assert_eq!(fetched["updated"], 1);
    let setup = call(&platform, "rank_setup", json!({ "project_id": id }));
    assert_eq!(setup["missing_volumes"], 0);
    assert_eq!(
        error(&platform, "fetch_rank_volumes", json!({ "project_id": id })),
        ErrorCode::Invalid
    );

    // The schedule: the run just made moves it a week on; an old last run makes it due.
    let next = platform.next_due().unwrap().unwrap();
    assert!(next > octopuson_platform::unix_now() + 6 * 86_400);
    assert_eq!(platform.run_due().unwrap(), Vec::<String>::new());
    platform
        .workspace()
        .store()
        .unwrap()
        .mark_rank_run(id, "2026-01-01T00:00:00Z")
        .unwrap();
    let started = platform.run_due().unwrap();
    assert_eq!(started.len(), 1);
    assert_eq!(
        finished(&platform, &started[0]).status,
        crate::JobStatus::Done
    );
    let runs = call(&platform, "rank_runs", json!({ "project_id": id }));
    assert_eq!(runs[0]["scheduled"], true);
    assert_eq!(runs[0]["status"], "done");
    assert_eq!(transport.requests().len(), 9);
    assert!(crate::modules::rank::changes_schedule("track_keywords"));
    assert!(!crate::modules::rank::changes_schedule("rank_report"));

    // A place cannot go while it is the last one; keywords and the project can.
    let only = call(
        &platform,
        "remove_rank_tracker",
        json!({ "project_id": id, "tracker_id": setup["trackers"][1]["id"] }),
    );
    assert_eq!(
        error(
            &platform,
            "remove_rank_tracker",
            json!({ "project_id": id, "tracker_id": only["trackers"][0]["id"] })
        ),
        ErrorCode::Invalid
    );
    assert_eq!(
        call(
            &platform,
            "untrack_keywords",
            json!({ "project_id": id, "keyword_ids": [keyword_id] })
        ),
        1
    );
    call(&platform, "delete_project", json!({ "id": id }));
    assert_eq!(platform.next_due().unwrap(), None);
}

/// A site served from memory: pages by address, everything else 404.
struct FakeSite(std::collections::HashMap<String, String>);

impl octopuson_crawl::Fetcher for FakeSite {
    fn fetch(
        &self,
        request: &octopuson_crawl::FetchRequest<'_>,
    ) -> Result<octopuson_crawl::FetchResponse, octopuson_crawl::FetchError> {
        Ok(match self.0.get(request.url) {
            Some(body) => octopuson_crawl::FetchResponse::new(
                200,
                &[("Content-Type", "text/html")],
                body.clone(),
            ),
            None => octopuson_crawl::FetchResponse::new(404, &[("Content-Type", "text/html")], ""),
        })
    }
}

fn small_site() -> FakeSite {
    FakeSite(std::collections::HashMap::from([
        (
            "https://example.com/".to_owned(),
            r#"<html lang="tr"><head><title>Örnek</title>
            <script type="application/ld+json">{"@type":"Organization","name":"Örnek"}</script></head>
            <body><h1>Örnek</h1><h2>Kahve nasıl demlenir?</h2><a href="/a">A</a><a href="/b">B</a></body></html>"#
                .to_owned(),
        ),
        (
            "https://example.com/a".to_owned(),
            "<html><body><h1>A</h1></body></html>".to_owned(),
        ),
        (
            "https://example.com/b".to_owned(),
            "<html><head><title>B</title></head><body><h1>B</h1></body></html>".to_owned(),
        ),
    ]))
}

/// A home page linking to `pages` pages.
fn big_site(pages: usize) -> FakeSite {
    let mut links = String::new();
    for n in 0..pages {
        use std::fmt::Write as _;
        let _ = write!(links, "<a href=\"/p{n}\">{n}</a>");
    }
    let mut site = std::collections::HashMap::from([(
        "https://example.com/".to_owned(),
        format!("<html><head><title>Home</title></head><body><h1>Home</h1>{links}</body></html>"),
    )]);
    for n in 0..pages {
        site.insert(
            format!("https://example.com/p{n}"),
            format!("<html><head><title>P{n}</title></head><body><h1>P{n}</h1></body></html>"),
        );
    }
    FakeSite(site)
}

fn crawling_platform(
    dir: &Path,
    host: Host,
    transport: &Arc<FakeTransport>,
    site: FakeSite,
) -> Platform {
    let open: Opener = Arc::new(|_| true);
    Platform::new(
        workspace(dir, host, transport).with_fetcher(Arc::new(site)),
        Arc::new(NoEvents),
        open,
    )
}

#[test]
#[allow(clippy::too_many_lines)] // two audits from settings to deletion
fn audits_crawl_the_site_compare_and_follow_the_schedule() {
    let dir = tempfile::tempdir().unwrap();
    let transport = Arc::new(FakeTransport::new());
    let platform = crawling_platform(dir.path(), Host::Desktop, &transport, small_site());
    let id = project(&platform);

    let setup = call(&platform, "audit_setup", json!({ "project_id": id }));
    assert_eq!(setup["settings"]["schedule"], "manual");
    assert_eq!(setup["meter"], Value::Null);
    assert_eq!(setup["next_audit_at"], Value::Null);
    assert_eq!(
        call(&platform, "audit_report", json!({ "project_id": id })),
        Value::Null
    );
    let setup = call(
        &platform,
        "save_audit_settings",
        json!({ "project_id": id, "schedule": "weekly", "max_pages": 5 }),
    );
    assert_eq!(setup["settings"]["max_pages"], 10, "at least ten");

    let job = call(&platform, "start_audit", json!({ "project_id": id }));
    assert_eq!(
        finished(&platform, job.as_str().unwrap()).status,
        crate::JobStatus::Done
    );
    let report = call(
        &platform,
        "audit_report",
        json!({ "project_id": id, "lang": "tr" }),
    );
    let first = report["audit"]["id"].as_i64().unwrap();
    assert_eq!(report["audit"]["status"], "done");
    assert_eq!(report["previous"], Value::Null);
    assert!(report["overview"]["html"].as_u64().unwrap() >= 3);
    assert!(report["overview"]["health"].is_number());
    assert!(report["geo"]["score"].is_number());
    let issue = |report: &Value, id: &str| -> Value {
        report["categories"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|category| category["issues"].as_array().unwrap().clone())
            .find(|issue| issue["id"] == id)
            .unwrap_or(Value::Null)
    };
    let missing = issue(&report, "title-missing");
    assert_eq!(missing["count"], 1);
    assert_eq!(missing["title"], "Başlık (title) yok");
    assert_eq!(missing["previous"], Value::Null);
    assert_eq!(
        issue(&report, "llms-txt-missing")["count"],
        1,
        "the AI readiness checks run too"
    );

    let detail = call(
        &platform,
        "audit_issue",
        json!({ "project_id": id, "audit_id": first, "issue_id": "title-missing" }),
    );
    assert_eq!(detail["total"], 1);
    assert!(
        detail["rows"][0]
            .as_array()
            .unwrap()
            .iter()
            .any(|cell| cell == "https://example.com/a")
    );
    assert_ne!(detail["fix"].as_str().unwrap(), "");
    assert_eq!(
        error(
            &platform,
            "audit_issue",
            json!({ "project_id": id, "audit_id": first, "issue_id": "no-such-check" })
        ),
        ErrorCode::NotFound
    );

    // A second audit is compared with the first.
    let job = call(&platform, "start_audit", json!({ "project_id": id }));
    finished(&platform, job.as_str().unwrap());
    let report = call(&platform, "audit_report", json!({ "project_id": id }));
    assert_eq!(report["previous"]["id"], first);
    assert_eq!(issue(&report, "title-missing")["previous"], 1);
    assert_eq!(issue(&report, "title-missing")["title"], "Missing title");

    // Weekly: not due again for days.
    let next = platform.next_due().unwrap().unwrap();
    assert!(next > octopuson_platform::unix_now() + 6 * 86_400);
    assert_eq!(platform.run_due().unwrap(), Vec::<String>::new());

    let first_crawl = call(
        &platform,
        "audit_report",
        json!({ "project_id": id, "audit_id": first }),
    )["audit"]["crawl_id"]
        .as_str()
        .unwrap()
        .to_owned();
    assert!(dir.path().join("crawls").join(&first_crawl).exists());
    call(
        &platform,
        "delete_audit",
        json!({ "project_id": id, "audit_id": first }),
    );
    assert!(!dir.path().join("crawls").join(&first_crawl).exists());
    call(&platform, "delete_project", json!({ "id": id }));
    assert_eq!(
        std::fs::read_dir(dir.path().join("crawls"))
            .unwrap()
            .count(),
        0,
        "the project's crawls go with it"
    );
    assert!(crate::modules::changes_schedule("save_audit_settings"));
    assert!(crate::modules::changes_schedule("track_keywords"));
}

const METER_CATALOG: &str = r#"{"credit_unit_micros":1000,"portal_url":"https://my.octopuson.com","connectors":[],"meters":[{"sku":"crawl.page","micros_per_unit":200,"free_per_month":1000}]}"#;

fn receipt(units: u64) -> String {
    format!(
        r#"{{"sku":"crawl.page","units":{units},"free_units":{units},"charged_micros":0,"free_left":500,"available_micros":5000000,"duplicate":false}}"#
    )
}

#[test]
fn web_audits_meter_every_page_and_stop_without_credits() {
    let dir = tempfile::tempdir().unwrap();
    let transport = Arc::new(
        FakeTransport::new()
            .reply(200, METER_CATALOG)
            .reply(200, receipt(100))
            .reply(200, receipt(100)),
    );
    let platform = crawling_platform(dir.path(), Host::Web, &transport, big_site(130));
    let id = project(&platform);
    assert_eq!(
        error(&platform, "start_audit", json!({ "project_id": id })),
        ErrorCode::Account,
        "the server crawls only for a signed-in account"
    );
    platform
        .workspace()
        .set_account_token(Some("oct_test"))
        .unwrap();
    let setup = call(&platform, "audit_setup", json!({ "project_id": id }));
    assert_eq!(setup["max_pages_limit"], 5000);
    assert_eq!(setup["meter"]["free_per_month"], 1000);
    assert_eq!(setup["meter"]["micros_per_unit"], 200);

    let job = call(&platform, "start_audit", json!({ "project_id": id }));
    assert_eq!(
        finished(&platform, job.as_str().unwrap()).status,
        crate::JobStatus::Done
    );
    let audit = &call(&platform, "audit_report", json!({ "project_id": id }))["audit"];
    let pages = audit["pages"].as_u64().unwrap();
    assert!(pages > 100);
    assert_eq!(audit["metered_pages"], pages);
    let meters: Vec<Value> = transport
        .requests()
        .iter()
        .filter(|request| request.url.ends_with("/v1/meter"))
        .map(|request| serde_json::from_slice(request.body.as_deref().unwrap()).unwrap())
        .collect();
    assert_eq!(meters.len(), 2);
    assert_eq!(meters[0]["units"], 100);
    assert_eq!(meters[1]["units"], pages - 100);
    let crawl = audit["crawl_id"].as_str().unwrap();
    assert_eq!(meters[0]["idempotency_key"], format!("{crawl}:0"));
    assert_eq!(meters[1]["idempotency_key"], format!("{crawl}:1"));

    // Without credits the crawl stops and says why.
    let dir = tempfile::tempdir().unwrap();
    let transport = Arc::new(FakeTransport::new().reply(
        402,
        r#"{"error":"insufficient_credits","message":"x","available_micros":0}"#,
    ));
    let platform = crawling_platform(dir.path(), Host::Web, &transport, big_site(130));
    platform
        .workspace()
        .set_account_token(Some("oct_test"))
        .unwrap();
    let id = project(&platform);
    let job = call(&platform, "start_audit", json!({ "project_id": id }));
    assert_eq!(
        finished(&platform, job.as_str().unwrap()).status,
        crate::JobStatus::Partial
    );
    let audit = &call(&platform, "audit_report", json!({ "project_id": id }))["audit"];
    assert_eq!(audit["status"], "stopped");
    assert_eq!(audit["metered_pages"], 0);
    assert!(audit["error"].as_str().unwrap().contains("credits"));
}

/// A DataForSEO answer with one result.
fn dataforseo(cost: f64, result: &Value) -> String {
    json!({
        "status_code": 20000,
        "status_message": "Ok.",
        "cost": cost,
        "tasks_count": 1,
        "tasks_error": 0,
        "tasks": [{ "id": "t", "status_code": 20000, "status_message": "Ok.", "cost": cost, "result_count": 1, "result": [result] }]
    })
    .to_string()
}

/// A Claude answer with web search citations.
fn claude(text: &str, citations: &Value) -> String {
    json!({
        "model": "claude-opus-5-5",
        "stop_reason": "end_turn",
        "content": [{ "type": "text", "text": text, "citations": citations }],
        "usage": { "input_tokens": 1000, "output_tokens": 200, "server_tool_use": { "web_search_requests": 1 } }
    })
    .to_string()
}

fn mention_metrics(mentions: u64) -> Value {
    json!({
        "total_count": 0,
        "items": [],
        "aggregated_metrics": {
            "total": { "mentions": mentions, "ai_search_volume": 18500 },
            "platform": [{ "key": "google", "mentions": mentions, "ai_search_volume": 18500 }],
            "sources_domain": [],
            "brand_entities_title": []
        }
    })
}

#[test]
#[allow(clippy::too_many_lines)] // questions, three ways of asking, the report and the schedule
fn ai_visibility_asks_assistants_reads_brands_and_follows_the_schedule() {
    let dir = tempfile::tempdir().unwrap();
    let ai_mode = || {
        dataforseo(
            0.004,
            &json!({ "type": "ai_mode", "items": [{ "type": "ai_overview", "markdown": "Örnek Mağaza en iyi seçim.",
                "references": [{ "url": "https://ornekmagaza.com/a", "title": "Örnek", "domain": "ornekmagaza.com" }] }] }),
        )
    };
    let transport = Arc::new(
        FakeTransport::new()
            .reply(
                200,
                claude(
                    "Rakip Mağaza ve Örnek Mağaza iyi seçenekler. Örnek Mağaza harika.",
                    &json!([
                        { "type": "web_search_result_location", "url": "https://www.ornekmagaza.com/spor", "title": "Spor", "cited_text": "..." },
                        { "type": "web_search_result_location", "url": "https://rakip-magaza.com/x", "title": "Rakip", "cited_text": "..." }
                    ]),
                ),
            )
            .reply(500, "{}")
            .reply(
                200,
                dataforseo(
                    0.03,
                    &json!({ "model_name": "gpt-5", "items": [{ "type": "message", "sections": [
                        { "type": "text", "text": "Rakip Mağaza öne çıkıyor.", "annotations": [{ "title": "Rakip", "url": "https://rakip-magaza.com/y" }] }
                    ]}]}),
                ),
            )
            .reply(200, ai_mode())
            .reply(200, dataforseo(0.13, &mention_metrics(42)))
            .reply(200, dataforseo(0.13, &mention_metrics(17)))
            .reply(200, ai_mode()),
    );
    let platform = platform(dir.path(), &transport);
    let id = call(
        &platform,
        "create_project",
        json!({ "spec": { "domain": "ornekmagaza.com", "country": "tr", "language": "tr",
            "competitors": ["rakip-magaza.com"], "brands": ["Örnek Mağaza"] } }),
    )["id"]
        .as_i64()
        .unwrap();

    // Nothing to ask yet, and nothing to ask with.
    let setup = call(&platform, "ai_setup", json!({ "project_id": id }));
    assert_eq!(setup["settings"]["schedule"], "manual");
    assert_eq!(setup["engines"].as_array().unwrap().len(), 5);
    assert_eq!(setup["engines"][1]["engine"], "claude");
    assert_eq!(setup["engines"][1]["provider"], "anthropic");
    assert_eq!(setup["engines"][1]["via"], Value::Null);
    assert_eq!(setup["brands"][1]["key"], "rakip-magaza.com");
    assert_eq!(
        error(&platform, "start_ai_run", json!({ "project_id": id })),
        ErrorCode::Invalid
    );
    let added = call(
        &platform,
        "add_ai_prompts",
        json!({ "project_id": id, "tags": ["ayakkabı"], "prompts": ["En iyi spor ayakkabı markaları?", "Spor ayakkabı nereden alınır?"] }),
    );
    assert_eq!(added, json!({ "added": 2, "total": 2 }));
    let again = call(
        &platform,
        "add_ai_prompts",
        json!({ "project_id": id, "prompts": ["En iyi spor ayakkabı markaları?"] }),
    );
    assert_eq!(again["added"], 0);
    call(
        &platform,
        "track_keywords",
        json!({ "project_id": id, "keywords": [{ "keyword": "iç giyim", "volume": 900 }] }),
    );
    let suggested = call(&platform, "suggest_ai_prompts", json!({ "project_id": id }));
    let suggested = suggested.as_array().unwrap();
    assert!(suggested.contains(&json!("Örnek Mağaza güvenilir mi?")));
    assert!(suggested.contains(&json!("İç giyim için en iyi markalar hangileri?")));
    let estimate = call(&platform, "ai_estimate", json!({ "project_id": id }));
    // A new project asks ChatGPT, Gemini and Google AI Mode; none has a source yet.
    assert_eq!(estimate["unavailable"].as_array().unwrap().len(), 3);
    assert_eq!(
        error(&platform, "start_ai_run", json!({ "project_id": id })),
        ErrorCode::NoSource
    );
    assert_eq!(transport.requests().len(), 0);

    // Claude on the user's own Anthropic key: the second answer fails and the run goes on.
    call(
        &platform,
        "set_key",
        json!({ "provider": "anthropic", "name": "api_key", "value": "sk-ant-test" }),
    );
    let setup = call(
        &platform,
        "save_ai_settings",
        json!({ "project_id": id, "schedule": "weekly", "engines": ["claude"],
            "rivals": [{ "domain": "https://rakip-magaza.com/", "names": ["Rakip Mağaza"] }] }),
    );
    assert_eq!(setup["engines"][1]["via"], "own");
    assert_eq!(setup["engines"][1]["model"], "claude-opus-5-5");
    assert_eq!(setup["engines"][0]["enabled"], false);
    assert!(setup["next_run_at"].is_string(), "never run: due now");
    let estimate = call(&platform, "ai_estimate", json!({ "project_id": id }));
    assert_eq!(estimate["answers"], 2);
    assert_eq!(estimate["runs_per_month"], 4);
    assert_eq!(estimate["engines"][0]["via"], "own");
    let job = call(&platform, "start_ai_run", json!({ "project_id": id }));
    let job = finished(&platform, job.as_str().unwrap());
    assert_eq!(job.status, crate::JobStatus::Partial);
    assert_eq!(job.result.as_ref().unwrap()["checked"], 1);
    assert_eq!(job.result.as_ref().unwrap()["failed"], 1);
    let requests = transport.requests();
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0].url, "https://api.anthropic.com/v1/messages");
    let sent: Value = serde_json::from_slice(requests[0].body.as_deref().unwrap()).unwrap();
    assert_eq!(
        sent["messages"][0]["content"],
        "En iyi spor ayakkabı markaları?"
    );

    let report = call(&platform, "ai_report", json!({ "project_id": id }));
    assert_eq!(report["summary"]["answers"], 1);
    assert_eq!(report["summary"]["mention_rate"], 100.0);
    assert_eq!(report["summary"]["citation_rate"], 100.0);
    assert_eq!(report["summary"]["average_position"], 2.0);
    assert_eq!(report["summary"]["share_of_voice"], 50.0);
    assert!(report["summary"]["sentiment"].as_f64().unwrap() > 0.0);
    assert_eq!(report["engines"].as_array().unwrap().len(), 1);
    assert_eq!(report["brands"][0]["key"], "ornekmagaza.com");
    assert_eq!(report["brands"][0]["own"], true);
    assert_eq!(report["brands"][1]["key"], "rakip-magaza.com");
    assert_eq!(report["brands"][1]["citations"], 1);
    assert_eq!(report["prompts"][0]["results"]["claude"]["position"], 2);
    assert!(
        report["prompts"][1]["results"]["claude"]["error"]
            .as_str()
            .unwrap()
            .contains("500")
    );
    assert_eq!(
        report["own_pages"][0]["key"],
        "https://www.ornekmagaza.com/spor"
    );
    assert_eq!(report["top_domains"].as_array().unwrap().len(), 2);
    assert_eq!(report["last_run"]["status"], "partial");
    let prompt_id = report["prompts"][0]["prompt_id"].as_i64().unwrap();
    let detail = call(
        &platform,
        "ai_answer",
        json!({ "project_id": id, "prompt_id": prompt_id, "engine": "claude" }),
    );
    assert_eq!(detail["answers"].as_array().unwrap().len(), 1);
    assert!(
        detail["answers"][0]["text"]
            .as_str()
            .unwrap()
            .starts_with("Rakip Mağaza")
    );

    // ChatGPT through DataForSEO without an OpenAI key, then Google AI Mode.
    let failed_id = report["prompts"][1]["prompt_id"].as_i64().unwrap();
    assert_eq!(
        call(
            &platform,
            "remove_ai_prompts",
            json!({ "project_id": id, "prompt_ids": [failed_id] })
        ),
        1
    );
    call(
        &platform,
        "set_key",
        json!({ "provider": "dataforseo", "name": "credentials", "value": "me@example.com:secret" }),
    );
    for engine in ["chatgpt", "google_ai_mode"] {
        let setup = call(
            &platform,
            "save_ai_settings",
            json!({ "project_id": id, "schedule": "weekly", "engines": [engine] }),
        );
        let info = setup["engines"]
            .as_array()
            .unwrap()
            .iter()
            .find(|info| info["engine"] == engine)
            .unwrap()
            .clone();
        assert_eq!(info["via"], "dataforseo");
        let job = call(&platform, "start_ai_run", json!({ "project_id": id }));
        assert_eq!(
            finished(&platform, job.as_str().unwrap()).status,
            crate::JobStatus::Done
        );
    }
    let requests = transport.requests();
    assert!(
        requests[2]
            .url
            .ends_with("/v3/ai_optimization/chat_gpt/llm_responses/live")
    );
    let task: Value = serde_json::from_slice(requests[2].body.as_deref().unwrap()).unwrap();
    assert_eq!(task[0]["model_name"], "gpt-5");
    assert_eq!(task[0]["web_search_country_iso_code"], "TR");
    assert!(
        requests[3]
            .url
            .ends_with("/v3/serp/google/ai_mode/live/advanced")
    );
    let report = call(&platform, "ai_report", json!({ "project_id": id }));
    assert_eq!(report["summary"]["answers"], 3);
    assert_eq!(report["summary"]["mention_rate"], 66.7);
    assert_eq!(report["engines"].as_array().unwrap().len(), 3);
    assert_eq!(report["history"].as_array().unwrap().len(), 3);
    assert_eq!(report["prompts"].as_array().unwrap().len(), 1);
    let usage = call(&platform, "usage", Value::Null);
    let requests_of = |provider: &str| {
        usage
            .as_array()
            .unwrap()
            .iter()
            .filter(|row| row["provider"] == provider)
            .map(|row| row["requests"].as_u64().unwrap())
            .sum::<u64>()
    };
    assert_eq!(requests_of("anthropic"), 1);
    assert_eq!(requests_of("dataforseo"), 2);

    // LLM Mentions: the site's metrics, kept; then beside its rival's, asking only for those.
    let estimate = call(
        &platform,
        "ai_mentions_estimate",
        json!({ "project_id": id, "report": "summary" }),
    );
    assert!((estimate["usd"].as_f64().unwrap() - 0.13).abs() < 1e-9);
    assert_eq!(
        call(
            &platform,
            "get_ai_mentions",
            json!({ "project_id": id, "report": "summary" })
        ),
        Value::Null
    );
    let summary = call(
        &platform,
        "fetch_ai_mentions",
        json!({ "project_id": id, "report": "summary" }),
    );
    assert_eq!(summary["data"]["mentions"], 42);
    assert_eq!(summary["data"]["target"], "ornekmagaza.com");
    let kept = call(
        &platform,
        "get_ai_mentions",
        json!({ "project_id": id, "report": "summary" }),
    );
    assert_eq!(kept["cached"], true);
    let compared = call(
        &platform,
        "fetch_ai_mentions",
        json!({ "project_id": id, "report": "compare" }),
    );
    assert_eq!(compared["data"][1]["target"], "rakip-magaza.com");
    assert_eq!(compared["data"][0]["mentions"], 42);
    assert_eq!(compared["data"][1]["mentions"], 17);
    assert_eq!(transport.requests().len(), 6);

    // The schedule: the run just made moves it a week on; an old last run makes it due.
    let next = platform.next_due().unwrap().unwrap();
    assert!(next > octopuson_platform::unix_now() + 6 * 86_400);
    assert_eq!(platform.run_due().unwrap(), Vec::<String>::new());
    platform
        .workspace()
        .store()
        .unwrap()
        .begin_ai_run(id, false, 0, "2026-01-01T00:00:00Z")
        .unwrap();
    let started = platform.run_due().unwrap();
    assert_eq!(started.len(), 1);
    assert_eq!(
        finished(&platform, &started[0]).status,
        crate::JobStatus::Done
    );
    let runs = call(&platform, "ai_runs", json!({ "project_id": id }));
    assert_eq!(runs[0]["scheduled"], true);
    // Runs are listed by when they started; the one begun by hand above was interrupted.
    assert!(
        runs.as_array()
            .unwrap()
            .iter()
            .any(|run| run["status"] == "interrupted")
    );
    assert_eq!(transport.requests().len(), 7);
    assert!(crate::modules::ai::changes_schedule("add_ai_prompts"));
    assert!(!crate::modules::ai::changes_schedule("ai_report"));
}

/// A day `back` days before today (UTC), as Search Console names it.
fn ago(back: i32) -> String {
    let today = octopuson_core::Date::of(octopuson_core::Timestamp::now()).days_since_epoch();
    octopuson_core::Date::from_days_since_epoch(today - back)
        .unwrap()
        .to_string()
}

/// Search Console rows of one dimension: `(key, clicks, impressions, position)`.
fn gsc_rows(rows: &[(&str, f64, f64, f64)]) -> String {
    json!({ "rows": rows.iter().map(|(key, clicks, impressions, position)| json!({
        "keys": [key], "clicks": clicks, "impressions": impressions,
        "ctr": clicks / impressions, "position": position
    })).collect::<Vec<_>>() })
    .to_string()
}

/// GA4 organic rows: `(key, sessions, key events)`.
fn ga4_rows(rows: &[(&str, f64, f64)]) -> String {
    json!({ "rowCount": rows.len(), "metadata": { "currencyCode": "TRY" }, "rows": rows.iter().map(|(key, sessions, events)| json!({
        "dimensionValues": [{ "value": key }],
        "metricValues": [{ "value": sessions.to_string() }, { "value": (sessions / 2.0).to_string() }, { "value": events.to_string() }, { "value": "0" }]
    })).collect::<Vec<_>>() })
    .to_string()
}

const GRANT: &str = r#"{"access_token":"ya29.first","expires_in":3599,"refresh_token":"1//refresh","scope":"https://www.googleapis.com/auth/webmasters.readonly https://www.googleapis.com/auth/analytics.readonly"}"#;

#[test]
fn google_sign_in_works_on_the_desktop_and_the_web() {
    // The desktop: the user's own client, the browser comes back to the local listener.
    let dir = tempfile::tempdir().unwrap();
    let transport = Arc::new(FakeTransport::new().reply(200, GRANT));
    let desktop = workspace(dir.path(), Host::Desktop, &transport);
    let status = desktop.google_status().unwrap();
    assert!(!status.connected);
    assert!(matches!(
        desktop.set_google_client("not-a-client", "s"),
        Err(crate::AppError::Invalid(_))
    ));
    let status = desktop
        .set_google_client("123-abc.apps.googleusercontent.com", " secret ")
        .unwrap();
    assert!(status.configured);
    assert_eq!(
        status.own_client_id.as_deref(),
        Some("123-abc.apps.googleusercontent.com")
    );
    let open = |url: &str| {
        let parsed = url::Url::parse(url).unwrap();
        let pair = |name: &str| {
            parsed
                .query_pairs()
                .find(|(key, _)| key == name)
                .map(|(_, value)| value.into_owned())
                .unwrap()
        };
        let address = pair("redirect_uri")
            .trim_start_matches("http://")
            .to_owned();
        let state = pair("state");
        assert!(pair("scope").contains("analytics.readonly"));
        std::thread::spawn(move || {
            use std::io::{Read, Write};
            let mut stream = std::net::TcpStream::connect(address).unwrap();
            write!(
                stream,
                "GET /?state={state}&code=4%2Fcode HTTP/1.1\r\nHost: x\r\n\r\n"
            )
            .unwrap();
            let mut answer = String::new();
            stream.read_to_string(&mut answer).unwrap();
        });
        true
    };
    let status = desktop.connect_google(true, &open, &|| false).unwrap();
    assert!(status.connected && status.search_console && status.analytics);
    let exchange = String::from_utf8(transport.requests()[0].body.clone().unwrap()).unwrap();
    assert!(exchange.contains("client_secret=secret"));
    assert!(exchange.contains("code=4%2Fcode"));
    let removed = desktop.set_google_client("", "").unwrap();
    assert_eq!(removed.own_client_id, None);

    // The web: the server's client, Google comes back to the server's address.
    let dir = tempfile::tempdir().unwrap();
    let transport = Arc::new(FakeTransport::new().reply(200, GRANT).reply(200, "{}"));
    let web = workspace(dir.path(), Host::Web, &transport).with_google_web(crate::GoogleWeb {
        client: octopuson_oauth::OAuthClient {
            client_id: "web.apps.googleusercontent.com".to_owned(),
            client_secret: "web-secret".to_owned(),
        },
        redirect_uri: "https://app.octopuson.com/auth/google/callback".to_owned(),
    });
    assert!(matches!(
        web.set_google_client("123-abc.apps.googleusercontent.com", "s"),
        Err(crate::AppError::Unsupported(_))
    ));
    let url = web.begin_google_login(false).unwrap();
    let state = url::Url::parse(&url)
        .unwrap()
        .query_pairs()
        .find(|(key, _)| key == "state")
        .map(|(_, value)| value.into_owned())
        .unwrap();
    assert!(
        url.contains("redirect_uri=https%3A%2F%2Fapp.octopuson.com%2Fauth%2Fgoogle%2Fcallback")
    );
    assert!(matches!(
        web.finish_google_login("other-state", "code"),
        Err(crate::AppError::Invalid(_))
    ));
    let status = web.finish_google_login(&state, "4/web").unwrap();
    assert!(status.connected && status.search_console);
    assert!(!status.analytics, "only asked for Search Console");
    assert!(
        web.finish_google_login(&state, "4/web").is_err(),
        "a state is used once"
    );
    let status = web.google_logout().unwrap();
    assert!(!status.connected);
    assert!(transport.requests()[1].url.ends_with("/revoke"));
}

#[test]
#[allow(clippy::too_many_lines)] // a sync from the properties to the report and the schedule
fn search_console_syncs_join_analytics_and_the_rank_tracker() {
    let dir = tempfile::tempdir().unwrap();
    let (now_day, before_day) = (ago(5), ago(40));
    let transport = Arc::new(
        FakeTransport::new()
            .reply(200, GRANT)
            // Properties: the token is refused once and renewed.
            .reply(401, r#"{"error":{"code":401,"message":"Invalid Credentials"}}"#)
            .reply(200, r#"{"access_token":"ya29.second","expires_in":3599}"#)
            .reply(200, json!({ "siteEntry": [
                { "siteUrl": "sc-domain:example.com", "permissionLevel": "siteOwner" },
                { "siteUrl": "https://other.com/", "permissionLevel": "siteOwner" }
            ]}).to_string())
            .reply(200, json!({ "accountSummaries": [{ "displayName": "Example", "propertySummaries": [
                { "property": "properties/42", "displayName": "Example site" }
            ]}]}).to_string())
            // The sync: days, queries, pages, devices, countries, queries on pages.
            .reply(200, gsc_rows(&[(&now_day, 30.0, 600.0, 6.0), (&before_day, 10.0, 400.0, 9.0)]))
            .reply(200, gsc_rows(&[("sneaker", 20.0, 300.0, 4.2), ("spor ayakkabı", 10.0, 2_000.0, 8.5)]))
            .reply(200, gsc_rows(&[("sneaker", 5.0, 200.0, 7.0)]))
            .reply(200, gsc_rows(&[("https://example.com/sneaker", 25.0, 500.0, 4.0)]))
            .reply(200, gsc_rows(&[("https://example.com/sneaker", 8.0, 300.0, 6.0)]))
            .reply(200, gsc_rows(&[("MOBILE", 22.0, 400.0, 5.0), ("DESKTOP", 8.0, 200.0, 7.0)]))
            .reply(200, gsc_rows(&[("tur", 28.0, 550.0, 5.5)]))
            .reply(200, json!({ "rows": [
                { "keys": ["sneaker", "https://example.com/sneaker"], "clicks": 18.0, "impressions": 250.0, "ctr": 0.07, "position": 4.0 },
                { "keys": ["sneaker", "https://example.com/blog/sneaker"], "clicks": 2.0, "impressions": 50.0, "ctr": 0.04, "position": 9.0 }
            ]}).to_string())
            // Analytics: days, landing pages now and before.
            .reply(200, ga4_rows(&[(&now_day.replace('-', ""), 40.0, 3.0)]))
            .reply(200, ga4_rows(&[("/sneaker", 35.0, 2.0)]))
            .reply(200, ga4_rows(&[("/sneaker", 20.0, 1.0)])),
    );
    let platform = platform(dir.path(), &transport);
    let id = project(&platform);
    assert_eq!(
        error(&platform, "start_gsc_sync", json!({ "project_id": id })),
        ErrorCode::Invalid,
        "no property yet"
    );
    assert_eq!(
        error(&platform, "google_properties", json!({ "project_id": id })),
        ErrorCode::NoSource,
        "not signed in"
    );
    let setup = call(&platform, "gsc_setup", json!({ "project_id": id }));
    assert_eq!(setup["settings"]["days"], 28);
    assert_eq!(setup["google"]["connected"], false);

    // Signed in (the desktop flow is covered above): the grant is kept like a sign-in would.
    platform
        .workspace()
        .set_google_client("123-abc.apps.googleusercontent.com", "secret")
        .unwrap();
    let open = |url: &str| {
        let parsed = url::Url::parse(url).unwrap();
        let pair = |name: &str| {
            parsed
                .query_pairs()
                .find(|(key, _)| key == name)
                .map(|(_, value)| value.into_owned())
                .unwrap()
        };
        let address = pair("redirect_uri")
            .trim_start_matches("http://")
            .to_owned();
        let state = pair("state");
        std::thread::spawn(move || {
            use std::io::{Read, Write};
            let mut stream = std::net::TcpStream::connect(address).unwrap();
            write!(
                stream,
                "GET /?state={state}&code=c HTTP/1.1\r\nHost: x\r\n\r\n"
            )
            .unwrap();
            let mut answer = String::new();
            stream.read_to_string(&mut answer).unwrap();
        });
        true
    };
    platform
        .workspace()
        .connect_google(true, &open, &|| false)
        .unwrap();
    // Google refuses the kept token once: it is renewed and the request asked again.
    let properties = call(&platform, "google_properties", json!({ "project_id": id }));
    assert_eq!(properties["suggested"], "sc-domain:example.com");
    assert_eq!(properties["analytics"][0]["id"], "properties/42");
    let renewal = String::from_utf8(transport.requests()[2].body.clone().unwrap()).unwrap();
    assert!(renewal.contains("grant_type=refresh_token"));

    let setup = call(
        &platform,
        "save_gsc_settings",
        json!({ "project_id": id, "property": "sc-domain:example.com", "ga4_property": "properties/42",
            "ga4_name": "Example site", "days": 28, "schedule": "weekly" }),
    );
    assert!(setup["next_sync_at"].is_string(), "never synced: due now");
    call(
        &platform,
        "track_keywords",
        json!({ "project_id": id, "keywords": [{ "keyword": "Sneaker" }] }),
    );
    let job = call(&platform, "start_gsc_sync", json!({ "project_id": id }));
    let job = finished(&platform, job.as_str().unwrap());
    assert_eq!(job.status, crate::JobStatus::Done, "{:?}", job.error);
    let requests = transport.requests();
    assert_eq!(requests.len(), 16);
    let days: Value = serde_json::from_slice(requests[5].body.as_deref().unwrap()).unwrap();
    assert_eq!(days["dimensions"], json!(["date"]));
    assert_eq!(days["endDate"], ago(3));
    assert_eq!(days["startDate"], ago(58));
    let queries: Value = serde_json::from_slice(requests[6].body.as_deref().unwrap()).unwrap();
    assert_eq!(
        (queries["startDate"].as_str(), queries["endDate"].as_str()),
        (Some(ago(30).as_str()), Some(ago(3).as_str()))
    );
    let before: Value = serde_json::from_slice(requests[7].body.as_deref().unwrap()).unwrap();
    assert_eq!(before["endDate"], ago(31));
    assert!(requests[13].url.ends_with("/properties/42:runReport"));
    assert_eq!(
        requests
            .iter()
            .filter(|request| request.url.contains("oauth2"))
            .count(),
        2,
        "a sign-in, a renewal, nothing more"
    );

    let report = call(&platform, "gsc_report", json!({ "project_id": id }));
    assert_eq!(report["now"]["clicks"], 30.0);
    assert_eq!(report["before"]["clicks"], 10.0);
    assert_eq!(report["now"]["ctr"], 5.0);
    assert_eq!(report["period"][1], ago(3));
    assert_eq!(report["previous"][1], ago(31));
    assert_eq!(report["traffic_now"]["sessions"], 40.0);
    assert_eq!(report["currency"], "TRY");
    let sneaker = &report["queries"][0];
    assert_eq!(sneaker["query"], "sneaker");
    assert_eq!(sneaker["clicks_change"], 15.0);
    assert_eq!(sneaker["position_change"], 2.8);
    assert_eq!(sneaker["tracked"], true);
    assert_eq!(sneaker["pages"], 2);
    let spor = &report["queries"][1];
    assert_eq!(spor["near_top"], true, "position 8.5 with many impressions");
    assert_eq!(spor["clicks_change"], Value::Null);
    assert_eq!(report["pages"][0]["sessions"], 35.0);
    assert_eq!(report["pages"][0]["clicks_change"], 17.0);
    assert_eq!(report["cannibalised"][0]["query"], "sneaker");
    assert_eq!(report["devices"][0]["key"], "MOBILE");
    assert_eq!(report["tracked"], json!([1, 1]));
    assert_eq!(report["audit"], Value::Null);
    let pages = call(
        &platform,
        "gsc_query",
        json!({ "project_id": id, "query": "sneaker" }),
    );
    assert_eq!(pages.as_array().unwrap().len(), 2);

    // The schedule moves a week on.
    let next = platform.next_due().unwrap().unwrap();
    assert!(next > octopuson_platform::unix_now() + 6 * 86_400);
    assert!(crate::modules::gsc::changes_schedule("save_gsc_settings"));
    assert!(!crate::modules::gsc::changes_schedule("gsc_report"));
}

/// Keeps the messages it is asked to send.
#[derive(Default)]
struct KeptMail(std::sync::Mutex<Vec<crate::mail::Mail>>);

impl crate::mail::Mailer for KeptMail {
    fn send(&self, mail: &crate::mail::Mail) -> Result<(), String> {
        self.0.lock().unwrap().push(mail.clone());
        Ok(())
    }
}

fn printer() -> Arc<crate::Printer> {
    Arc::new(|html: &str| Ok(format!("%PDF-1.7 {}", html.len()).into_bytes()))
}

#[test]
#[allow(clippy::too_many_lines)] // a report from its definition to its link and e-mail
fn reports_are_drawn_printed_shared_and_emailed() {
    use base64::Engine as _;
    let dir = tempfile::tempdir().unwrap();
    let transport = Arc::new(FakeTransport::new());
    let desktop = Platform::new(
        workspace(dir.path(), Host::Desktop, &transport).with_printer(printer()),
        Arc::new(NoEvents),
        Arc::new(|_| true),
    );
    let id = project(&desktop);
    let setup = call(&desktop, "reports_setup", json!({ "project_id": id }));
    assert_eq!(setup["can_print"], true);
    assert_eq!(setup["can_share"], false);
    assert_eq!(setup["can_email"], false);
    assert_eq!(setup["widgets"][0], "summary");
    let branding = call(
        &desktop,
        "save_report_branding",
        json!({ "branding": { "company": "Ajans", "accent": "#123abc" } }),
    );
    assert_eq!(branding["company"], "Ajans");
    let spec = json!({ "project_id": id, "name": "Aylık rapor", "language": "tr", "days": 28,
        "widgets": ["summary", "rankings", "site_audit"], "schedule": "manual", "recipients": [] });
    let report = call(&desktop, "save_report", spec.clone());
    let report_id = report["id"].as_i64().unwrap();
    let mut weekly = spec.clone();
    weekly["schedule"] = json!("weekly");
    assert_eq!(
        error(&desktop, "save_report", weekly.clone()),
        ErrorCode::Unsupported
    );

    let page = call(
        &desktop,
        "report_html",
        json!({ "project_id": id, "report_id": report_id }),
    );
    let html = page["html"].as_str().unwrap();
    assert!(html.contains("<h1>Aylık rapor</h1>"));
    assert!(html.contains("--accent:#123abc"));
    assert!(
        html.contains("Bu bölüm için henüz veri yok."),
        "nothing tracked or audited yet"
    );
    assert!(page["name"].as_str().unwrap().starts_with("aylik-rapor-"));
    let pdf = call(
        &desktop,
        "report_pdf",
        json!({ "project_id": id, "report_id": report_id }),
    );
    assert_eq!(pdf["content_type"], "application/pdf");
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(pdf["data"].as_str().unwrap())
        .unwrap();
    assert!(bytes.starts_with(b"%PDF-"));
    let xlsx = call(
        &desktop,
        "report_xlsx",
        json!({ "project_id": id, "report_id": report_id }),
    );
    assert!(
        std::path::Path::new(xlsx["name"].as_str().unwrap())
            .extension()
            .is_some_and(|extension| extension == "xlsx")
    );
    assert_eq!(
        error(
            &desktop,
            "share_report",
            json!({ "project_id": id, "report_id": report_id })
        ),
        ErrorCode::Unsupported
    );
    assert_eq!(
        error(
            &desktop,
            "send_report",
            json!({ "project_id": id, "report_id": report_id })
        ),
        ErrorCode::Unsupported
    );
    assert_eq!(transport.requests().len(), 0, "a report pays no source");

    // The web: links, and e-mails on a schedule.
    let dir = tempfile::tempdir().unwrap();
    let mail = Arc::new(KeptMail::default());
    let web = Platform::new(
        workspace(dir.path(), Host::Web, &transport)
            .with_printer(printer())
            .with_mailer(mail.clone())
            .with_public_url("https://app.octopuson.com/"),
        Arc::new(NoEvents),
        Arc::new(|_| false),
    );
    let id = project(&web);
    weekly["project_id"] = json!(id);
    weekly["recipients"] = json!(["musteri@example.com", "Patron@Example.com"]);
    let report = call(&web, "save_report", weekly);
    let report_id = report["id"].as_i64().unwrap();
    assert_eq!(
        report["recipients"],
        json!(["musteri@example.com", "patron@example.com"])
    );
    let shared = call(
        &web,
        "share_report",
        json!({ "project_id": id, "report_id": report_id }),
    );
    let token = shared["share_token"].as_str().unwrap().to_owned();
    assert_eq!(token.len(), 43);
    let page = crate::modules::reports::shared_report_html(web.workspace(), &token)
        .unwrap()
        .unwrap();
    assert!(page.contains("Aylık rapor"));
    assert_eq!(
        crate::modules::reports::shared_report_html(web.workspace(), "wrong").unwrap(),
        None
    );

    let started = web.run_due().unwrap();
    assert_eq!(started.len(), 1, "never sent: due now");
    let job = finished(&web, &started[0]);
    assert_eq!(job.status, crate::JobStatus::Done, "{:?}", job.error);
    {
        let sent = mail.0.lock().unwrap();
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0].to, ["musteri@example.com", "patron@example.com"]);
        assert!(sent[0].subject.starts_with("Aylık rapor · example.com"));
        assert!(sent[0].text.contains("raporu ektedir"));
        assert!(
            sent[0]
                .text
                .contains(&format!("https://app.octopuson.com/r/{token}"))
        );
        let attachment = sent[0].attachment.as_ref().unwrap();
        assert_eq!(attachment.content_type, "application/pdf");
        assert!(
            std::path::Path::new(&attachment.name)
                .extension()
                .is_some_and(|extension| extension == "pdf")
        );
    }
    let deliveries = call(
        &web,
        "report_deliveries",
        json!({ "project_id": id, "report_id": report_id }),
    );
    assert_eq!(deliveries[0]["status"], "sent");
    assert_eq!(deliveries[0]["scheduled"], true);
    assert!(web.next_due().unwrap().unwrap() > octopuson_platform::unix_now() + 6 * 86_400);
    assert_eq!(web.run_due().unwrap(), Vec::<String>::new());

    let unshared = call(
        &web,
        "unshare_report",
        json!({ "project_id": id, "report_id": report_id }),
    );
    assert_eq!(unshared["share_token"], Value::Null);
    assert_eq!(
        crate::modules::reports::shared_report_html(web.workspace(), &token).unwrap(),
        None
    );
    call(
        &web,
        "delete_report",
        json!({ "project_id": id, "report_id": report_id }),
    );
    assert_eq!(
        call(&web, "reports_setup", json!({ "project_id": id }))["reports"],
        json!([])
    );
    assert!(crate::modules::reports::changes_schedule("save_report"));
}

fn mcp(server: &crate::mcp::McpServer, method: &str, params: Value) -> Value {
    let mut request = json!({ "jsonrpc": "2.0", "id": 1, "method": method });
    request["params"] = params;
    server.handle(&request).unwrap()
}

fn tool(server: &crate::mcp::McpServer, name: &str, arguments: Value) -> Value {
    let mut params = json!({ "name": name });
    params["arguments"] = arguments;
    mcp(server, "tools/call", params)["result"].clone()
}

#[test]
#[allow(clippy::too_many_lines)] // the protocol, then every kind of tool
fn mcp_tools_read_free_data_and_buy_only_what_is_allowed() {
    let dir = tempfile::tempdir().unwrap();
    let transport = Arc::new(FakeTransport::new().reply(200, OVERVIEW));
    let platform = Arc::new(platform(dir.path(), &transport));
    let id = project(&platform);
    let server = crate::mcp::McpServer::new(Arc::clone(&platform), "octopuson-platform");

    let hello = mcp(
        &server,
        "initialize",
        json!({ "protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": { "name": "test", "version": "1" } }),
    );
    assert_eq!(hello["result"]["protocolVersion"], "2025-06-18");
    assert_eq!(hello["result"]["serverInfo"]["name"], "octopuson-platform");
    assert!(hello["result"]["capabilities"]["tools"].is_object());
    let newest = mcp(
        &server,
        "initialize",
        json!({ "protocolVersion": "1999-01-01" }),
    );
    assert_eq!(
        newest["result"]["protocolVersion"],
        crate::mcp::PROTOCOL_VERSIONS[0]
    );
    assert_eq!(
        server.handle(&json!({ "jsonrpc": "2.0", "method": "notifications/initialized" })),
        None
    );
    assert_eq!(mcp(&server, "ping", Value::Null)["result"], json!({}));
    let tools = mcp(&server, "tools/list", json!({}));
    let names: Vec<&str> = tools["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|tool| tool["name"].as_str().unwrap())
        .collect();
    for expected in [
        "list_projects",
        "domain_overview",
        "keyword_research",
        "serp_overview",
        "backlinks",
        "keyword_gap",
        "ai_mentions",
        "rank_report",
        "site_audit_issues",
        "ai_visibility",
        "search_console",
        "estimate_cost",
        "spending",
    ] {
        assert!(names.contains(&expected), "{expected} missing");
    }
    assert_eq!(
        mcp(&server, "resources/list", json!({}))["error"]["code"],
        -32_601
    );
    assert_eq!(
        mcp(
            &server,
            "tools/call",
            json!({ "name": "weather", "arguments": {} })
        )["error"]["code"],
        -32_602
    );
    let broken: Value = serde_json::from_str(&server.handle_text("{not json").unwrap()).unwrap();
    assert_eq!(broken["error"]["code"], -32_700);
    assert_eq!(broken["id"], Value::Null);
    let batch: Value = serde_json::from_str(
        &server
            .handle_text(r#"[{"jsonrpc":"2.0","id":7,"method":"ping"},{"jsonrpc":"2.0","method":"notifications/cancelled"}]"#)
            .unwrap(),
    )
    .unwrap();
    assert_eq!(batch.as_array().unwrap().len(), 1);
    assert_eq!(batch[0]["id"], 7);

    // Free tools read what is stored.
    let projects = tool(&server, "list_projects", json!({}));
    assert_eq!(projects["isError"], false);
    assert_eq!(projects["structuredContent"]["projects"][0]["id"], id);
    let audit = tool(&server, "site_audit_issues", json!({ "project_id": id }));
    assert!(
        audit["structuredContent"]["message"]
            .as_str()
            .unwrap()
            .contains("no site audit")
    );
    let missing = tool(&server, "rank_report", json!({}));
    assert_eq!(missing["isError"], true);
    assert!(
        missing["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("list_projects")
    );
    let spending = tool(&server, "spending", json!({}));
    assert_eq!(
        spending["structuredContent"]["sources"][0]["id"],
        "dataforseo"
    );
    assert_eq!(
        tool(&server, "serp_overview", json!({ "project_id": id }))["isError"],
        true,
        "a keyword is needed"
    );

    // Paid tools: no source, then the price, then data only with the allowance.
    let blocked = tool(&server, "domain_overview", json!({ "project_id": id }));
    assert_eq!(blocked["isError"], true);
    assert!(
        blocked["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("Sources")
    );
    call(
        &platform,
        "set_key",
        json!({ "provider": "dataforseo", "name": "credentials", "value": "me@example.com:secret" }),
    );
    let priced = tool(&server, "domain_overview", json!({ "project_id": id }));
    assert_eq!(priced["structuredContent"]["status"], "needs_approval");
    let usd = priced["structuredContent"]["price"]["usd"]
        .as_f64()
        .unwrap();
    assert!(usd > 0.0);
    let estimate = tool(
        &server,
        "estimate_cost",
        json!({ "tool": "domain_overview", "arguments": { "project_id": id } }),
    );
    assert_eq!(estimate["structuredContent"]["usd"].as_f64().unwrap(), usd);
    let flat = tool(
        &server,
        "estimate_cost",
        json!({ "tool": "domain_overview", "project_id": id }),
    );
    assert_eq!(flat["structuredContent"]["usd"].as_f64().unwrap(), usd);
    let too_little = tool(
        &server,
        "domain_overview",
        json!({ "project_id": id, "allow_spend_usd": usd / 2.0 }),
    );
    assert_eq!(too_little["structuredContent"]["status"], "needs_approval");
    assert_eq!(transport.requests().len(), 0, "nothing bought yet");
    let bought = tool(
        &server,
        "domain_overview",
        json!({ "project_id": id, "allow_spend_usd": usd }),
    );
    assert_eq!(bought["structuredContent"]["status"], "fetched", "{bought}");
    assert!(bought["structuredContent"]["data"]["organic"].is_object());
    assert_eq!(transport.requests().len(), 1);
    let kept = tool(&server, "domain_overview", json!({ "project_id": id }));
    assert_eq!(kept["structuredContent"]["status"], "kept");
    assert_eq!(kept["structuredContent"]["spent_usd"], 0);
    assert_eq!(transport.requests().len(), 1, "fresh kept data is free");
}

#[test]
fn the_agent_skills_name_only_tools_and_arguments_that_exist() {
    let dir = tempfile::tempdir().unwrap();
    let platform = Arc::new(platform(dir.path(), &Arc::new(FakeTransport::new())));
    let server = crate::mcp::McpServer::new(platform, "octopuson-platform");
    let tools = mcp(&server, "tools/list", json!({}))["result"]["tools"].clone();
    let mut known: Vec<String> = vec!["needs_approval".to_owned()];
    for tool in tools.as_array().unwrap() {
        known.push(tool["name"].as_str().unwrap().to_owned());
        for (property, schema) in tool["inputSchema"]["properties"].as_object().unwrap() {
            known.push(property.clone());
            for value in schema["enum"].as_array().into_iter().flatten() {
                known.push(value.as_str().unwrap().to_owned());
            }
        }
    }
    let skills =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../distribution/geo-seo-platform/skills");
    let mut found = 0;
    for entry in std::fs::read_dir(&skills).unwrap() {
        let folder = entry.unwrap().path();
        if !folder.is_dir() {
            continue;
        }
        found += 1;
        let name = folder.file_name().unwrap().to_str().unwrap().to_owned();
        let text = std::fs::read_to_string(folder.join("SKILL.md")).unwrap();
        let front = text
            .strip_prefix("---\n")
            .and_then(|rest| rest.split_once("\n---\n"))
            .map_or_else(|| panic!("{name}: no frontmatter"), |(front, _)| front);
        assert!(front.contains(&format!("name: {name}\n")), "{name}: name");
        let description = front
            .lines()
            .find_map(|line| line.strip_prefix("description: "))
            .unwrap_or_else(|| panic!("{name}: no description"));
        assert!(
            (40..=1024).contains(&description.len()),
            "{name}: description"
        );
        assert!(
            text.contains("allow_spend_usd"),
            "{name}: says how spending is allowed"
        );
        // Every snake_case word in code spans is a tool, an argument or a value of one.
        for (index, span) in text.split('`').enumerate() {
            if index % 2 == 1
                && span.contains('_')
                && span.chars().all(|c| c.is_ascii_lowercase() || c == '_')
            {
                assert!(
                    known.iter().any(|word| word == span),
                    "{name}: unknown `{span}`"
                );
            }
        }
    }
    assert_eq!(found, 5);
}

#[test]
fn the_stdio_transport_answers_one_line_per_request_and_nothing_for_notifications() {
    let dir = tempfile::tempdir().unwrap();
    let platform = Arc::new(platform(dir.path(), &Arc::new(FakeTransport::new())));
    let server = crate::mcp::McpServer::new(platform, crate::mcp::NAME);
    let input = concat!(
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18"}}"#,
        "\n",
        r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
        "\n\n",
        r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"list_projects","arguments":{}}}"#,
        "\n",
    );
    let mut output = Vec::new();
    crate::mcp::serve(&server, input.as_bytes(), &mut output).unwrap();
    let lines: Vec<Value> = String::from_utf8(output)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(lines.len(), 2);
    assert_eq!(
        lines[0]["result"]["serverInfo"]["name"],
        "octopuson-platform"
    );
    assert_eq!(lines[1]["id"], 2);
    assert_eq!(
        lines[1]["result"]["structuredContent"]["projects"],
        Value::Array(Vec::new())
    );
}

#[test]
fn a_problem_report_needs_words_and_an_account() {
    let dir = tempfile::tempdir().unwrap();
    let platform = platform(dir.path(), &Arc::new(FakeTransport::new()));
    let empty = platform
        .call("report_problem", json!({ "message": "  " }))
        .unwrap_err();
    assert_eq!(empty.code, ErrorCode::Invalid);
    let signed_out = platform
        .call(
            "report_problem",
            json!({ "message": "Rapor açılmıyor", "page": "backlinks", "error": "unavailable" }),
        )
        .unwrap_err();
    assert_eq!(signed_out.code, ErrorCode::Account);
    assert!(signed_out.message.contains("destek@octopuson.com"));
}
