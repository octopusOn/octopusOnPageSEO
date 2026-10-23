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
#[allow(clippy::too_many_lines)] // the command list
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
        "metric_history",
        "list_ads",
        "research_estimate",
        "get_research",
        "fetch_research",
        "serp_cluster_estimate",
        "start_serp_clustering",
        "serp_clusters",
        "link_decisions",
        "decide_links",
        "disavow_file",
        "link_prospects",
        "save_link_prospect",
        "remove_link_prospect",
        "audit_outbound",
        "get_category_names",
        "fetch_category_names",
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
        "rank_places",
        "rank_keyword",
        "rank_volumes_estimate",
        "fetch_rank_volumes",
        "alert_setup",
        "save_alert_settings",
        "list_alerts",
        "mark_alerts_read",
        "check_alerts",
        "start_link_watch",
        "opportunities",
        "set_opportunity",
        "project_dashboard",
        "vitals_setup",
        "start_vitals",
        "vitals_report",
        "audit_links",
        "audit_hreflang",
        "audit_pages",
        "audit_page",
        "audit_findings_csv",
        "gsc_export",
        "content_brief_estimate",
        "start_content_brief",
        "content_briefs",
        "content_brief",
        "delete_content_brief",
        "score_content",
        "content_pages",
        "content_writing_estimate",
        "start_content_writing",
        "reply_estimate",
        "draft_review_reply",
        "local_setup",
        "save_local_settings",
        "choose_business",
        "local_estimate",
        "get_local",
        "fetch_local",
        "local_grid_estimate",
        "start_local_grid",
        "local_runs",
        "local_grid",
        "local_reviews_estimate",
        "fetch_local_reviews",
        "local_reviews",
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
fn filtered_lists_and_other_markets_are_questions_of_their_own() {
    let dir = tempfile::tempdir().unwrap();
    let transport = Arc::new(
        FakeTransport::new()
            .reply(200, RANKED)
            .reply(200, RANKED)
            .reply(200, RANKED),
    );
    let platform = platform(dir.path(), &transport);
    let id = project(&platform);
    call(
        &platform,
        "set_key",
        json!({ "provider": "dataforseo", "name": "credentials", "value": "a:b" }),
    );
    let plain = call(
        &platform,
        "fetch_overview",
        json!({ "project_id": id, "report": "keywords" }),
    );
    assert_eq!(plain["total"], 3052);
    let body: Value =
        serde_json::from_slice(transport.requests()[0].body.as_deref().unwrap()).unwrap();
    assert_eq!(body[0].get("offset"), None);

    // An empty query is the same question: the kept answer, nothing asked.
    let empty = json!({ "filters": [], "order": [], "offset": 0 });
    let kept = call(
        &platform,
        "get_overview",
        json!({ "project_id": id, "report": "keywords", "query": empty }),
    );
    assert_eq!(kept["data"]["total"], 3052);

    // Filters, order and a page are asked of DataForSEO, in a fixed order.
    let query = json!({
        "filters": [
            { "field": "volume", "op": "ge", "value": 100 },
            { "field": "keyword", "op": "contains", "value": "ayakkabı" }
        ],
        "order": [{ "field": "volume", "direction": "desc" }],
        "offset": 100
    });
    let ask = json!({ "project_id": id, "report": "keywords", "query": query });
    assert_eq!(call(&platform, "get_overview", ask.clone()), Value::Null);
    call(&platform, "fetch_overview", ask.clone());
    let body: Value =
        serde_json::from_slice(transport.requests()[1].body.as_deref().unwrap()).unwrap();
    assert_eq!(
        body[0]["filters"],
        json!([
            ["keyword_data.keyword", "ilike", "%ayakkabı%"],
            "and",
            ["keyword_data.keyword_info.search_volume", ">=", 100]
        ])
    );
    assert_eq!(
        body[0]["order_by"],
        json!(["keyword_data.keyword_info.search_volume,desc"])
    );
    assert_eq!(body[0]["offset"], 100);
    assert_eq!(call(&platform, "get_overview", ask)["cached"], true);

    // Unknown fields and reports that are not filtered are refused before anything is asked.
    for (report, query) in [
        (
            "keywords",
            json!({ "filters": [{ "field": "colour", "op": "eq", "value": "red" }] }),
        ),
        (
            "keywords",
            json!({ "filters": [{ "field": "volume", "op": "contains", "value": "1" }] }),
        ),
        ("summary", json!({ "offset": 10 })),
    ] {
        assert_eq!(
            error(
                &platform,
                "overview_estimate",
                json!({ "project_id": id, "report": report, "query": query })
            ),
            ErrorCode::Invalid
        );
    }

    // Another market is another question.
    call(
        &platform,
        "fetch_overview",
        json!({ "project_id": id, "report": "keywords", "market": "de-de" }),
    );
    let body: Value =
        serde_json::from_slice(transport.requests()[2].body.as_deref().unwrap()).unwrap();
    assert_eq!(body[0]["location_code"], 2276);
    assert_eq!(body[0]["language_code"], "de");
    for market in ["de", "xx-de", "de-german"] {
        assert_eq!(
            error(
                &platform,
                "keywords_estimate",
                json!({ "project_id": id, "report": "ideas", "seed": "spor", "market": market })
            ),
            ErrorCode::Invalid
        );
    }
    assert_eq!(transport.requests().len(), 3);
}

#[test]
fn fetched_summaries_leave_a_point_of_history_a_day() {
    let dir = tempfile::tempdir().unwrap();
    let transport = Arc::new(
        FakeTransport::new()
            .reply(200, OVERVIEW)
            .reply(200, OVERVIEW),
    );
    let platform = platform(dir.path(), &transport);
    let id = project(&platform);
    call(
        &platform,
        "set_key",
        json!({ "provider": "dataforseo", "name": "credentials", "value": "a:b" }),
    );
    let history = |arguments: Value| call(&platform, "metric_history", arguments);
    assert_eq!(
        history(json!({ "project_id": id, "kind": "domain" })),
        json!([])
    );
    call(&platform, "fetch_overview", json!({ "project_id": id }));
    // Taken again from what is kept: still one point that day.
    call(&platform, "fetch_overview", json!({ "project_id": id }));
    let points = history(json!({ "project_id": id, "kind": "domain" }));
    assert_eq!(points.as_array().unwrap().len(), 1);
    assert_eq!(points[0]["target"], "example.com");
    assert_eq!(points[0]["market"], "tr-tr");
    assert_eq!(points[0]["metrics"]["organic"]["keywords"], 3052);
    // Another market and a competitor have histories of their own.
    assert_eq!(
        history(json!({ "project_id": id, "kind": "domain", "market": "de-de" })),
        json!([])
    );
    call(
        &platform,
        "fetch_overview",
        json!({ "project_id": id, "domain": "https://www.rival.com/" }),
    );
    let rival = history(json!({ "project_id": id, "kind": "domain", "target": "rival.com" }));
    assert_eq!(rival.as_array().unwrap().len(), 1);
    // Lists leave none.
    assert_eq!(
        error(
            &platform,
            "metric_history",
            json!({ "project_id": id, "kind": "keywords" })
        ),
        ErrorCode::Invalid
    );
    assert_eq!(transport.requests().len(), 2);
}

#[test]
fn site_explorer_asks_lost_and_paid_keywords_countries_and_longer_histories() {
    let dir = tempfile::tempdir().unwrap();
    let paid = include_str!("../../octopuson-dataforseo/tests/fixtures/ranked_keywords_paid.json");
    let countries = include_str!(
        "../../octopuson-dataforseo/tests/fixtures/domain_rank_overview_countries.json"
    );
    let history =
        include_str!("../../octopuson-dataforseo/tests/fixtures/historical_rank_overview.json");
    let links = include_str!("../../octopuson-dataforseo/tests/fixtures/backlinks_history.json");
    let transport = Arc::new(
        FakeTransport::new()
            .reply(200, RANKED)
            .reply(200, paid)
            .reply(200, countries)
            .reply(200, history)
            .reply(200, BULK)
            .reply(200, links),
    );
    let platform = platform(dir.path(), &transport);
    let id = project(&platform);
    call(
        &platform,
        "set_key",
        json!({ "provider": "dataforseo", "name": "credentials", "value": "a:b" }),
    );
    let body = |index: usize| -> Value {
        serde_json::from_slice(transport.requests()[index].body.as_deref().unwrap()).unwrap()
    };

    call(
        &platform,
        "fetch_overview",
        json!({ "project_id": id, "report": "keywords", "set": "lost" }),
    );
    assert_eq!(body(0)[0]["historical_serp_mode"], "lost");

    // Paid keywords keep the ads they show.
    assert_eq!(
        call(&platform, "list_ads", json!({ "project_id": id })),
        json!([])
    );
    let fetched = call(
        &platform,
        "fetch_overview",
        json!({ "project_id": id, "report": "keywords", "set": "paid" }),
    );
    assert_eq!(body(1)[0]["item_types"], json!(["paid"]));
    assert_eq!(
        fetched["data"]["keywords"][0]["ad"]["title"],
        "Spor Ayakkabıda %40 İndirim"
    );
    let ads = call(&platform, "list_ads", json!({ "project_id": id }));
    assert_eq!(ads.as_array().unwrap().len(), 1);
    assert_eq!(
        ads[0]["keywords"],
        json!(["spor ayakkabı indirim", "koşu ayakkabısı"])
    );

    let rows = call(
        &platform,
        "fetch_overview",
        json!({ "project_id": id, "report": "countries", "market": "de-de" }),
    );
    assert_eq!(body(2)[0], json!({ "target": "example.com", "limit": 100 }));
    assert_eq!(rows["data"][0]["country"], "tr");

    // More months reach back to October 2020; the price follows the months.
    let short = call(
        &platform,
        "overview_estimate",
        json!({ "project_id": id, "report": "history" }),
    );
    let long = call(
        &platform,
        "overview_estimate",
        json!({ "project_id": id, "report": "history", "months": 999 }),
    );
    assert!(long["usd"].as_f64().unwrap() > short["usd"].as_f64().unwrap());
    call(
        &platform,
        "fetch_overview",
        json!({ "project_id": id, "report": "history", "months": 999 }),
    );
    assert_eq!(body(3)[0]["date_from"], "2020-10-01");

    // Domains typed in join the comparison beside the project's competitors.
    call(
        &platform,
        "fetch_overview",
        json!({ "project_id": id, "report": "compare", "targets": ["https://www.third.com/"] }),
    );
    assert_eq!(
        body(4)[0]["targets"],
        json!(["example.com", "rival.com", "third.com"])
    );

    let points = call(
        &platform,
        "fetch_backlinks",
        json!({ "project_id": id, "report": "history" }),
    );
    assert_eq!(body(5)[0]["date_from"], "2019-01-01");
    assert_eq!(points["data"][2]["rank"], 45);
    assert_eq!(transport.requests().len(), 6);
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
    assert!(
        (estimate["usd"].as_f64().unwrap() - 2.0 * one).abs() < 1e-12,
        "all kept: the price is a refresh's"
    );
    // One part kept and one new: only the new one is priced.
    let partly = call(
        &platform,
        "gap_estimate",
        json!({ "project_id": id, "competitors": ["rival.com", "third.com"] }),
    );
    assert!(
        (partly["usd"].as_f64().unwrap() - one).abs() < 1e-12,
        "{partly}"
    );
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
#[allow(clippy::too_many_lines)] // the gap and the market, report by report
fn gaps_group_every_keyword_and_the_market_is_read() {
    let dir = tempfile::tempdir().unwrap();
    let fixture = |name: &str| {
        std::fs::read_to_string(format!(
            "{}/../octopuson-dataforseo/tests/fixtures/{name}.json",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap()
    };
    let transport = Arc::new(
        FakeTransport::new()
            .reply(200, RANKED)
            .reply(200, RANKED)
            .reply(200, fixture("page_intersection"))
            .reply(200, fixture("historical_bulk_traffic_estimation"))
            .reply(200, fixture("categories_for_domain"))
            .reply(200, fixture("domain_metrics_by_categories"))
            .reply(200, fixture("dataforseo_labs_categories")),
    );
    let platform = platform(dir.path(), &transport);
    let id = project(&platform);
    call(
        &platform,
        "set_key",
        json!({ "provider": "dataforseo", "name": "credentials", "value": "a:b" }),
    );
    let body = |index: usize| -> Value {
        serde_json::from_slice(transport.requests()[index].body.as_deref().unwrap()).unwrap()
    };

    let matrix = call(
        &platform,
        "fetch_gap",
        json!({ "project_id": id, "report": "keywords", "limit": 100 }),
    );
    assert_eq!(body(0)[0]["target"], "example.com");
    assert_eq!(body(1)[0]["target"], "rival.com");
    assert_eq!(matrix["data"]["competitors"], json!(["rival.com"]));
    assert_eq!(matrix["data"]["rows_per_domain"], 100);
    // The same keywords on both sides: all shared.
    assert_eq!(matrix["data"]["counts"]["shared"], 3);
    // The Site Explorer's list of the site is the same question: kept already.
    let kept = call(
        &platform,
        "get_overview",
        json!({ "project_id": id, "report": "keywords", "limit": 100 }),
    );
    assert_eq!(kept["cached"], true);

    assert_eq!(
        error(
            &platform,
            "gap_estimate",
            json!({ "project_id": id, "report": "pages" })
        ),
        ErrorCode::Invalid
    );
    let pages = call(
        &platform,
        "fetch_gap",
        json!({ "project_id": id, "report": "pages", "own_page": "https://www.example.com/kosu", "competitor_page": "https://www.rival.com/kosu" }),
    );
    assert_eq!(
        body(2)[0]["exclude_pages"],
        json!(["https://www.example.com/kosu"])
    );
    assert_eq!(pages["data"][0]["position"], 3);

    let share = call(
        &platform,
        "fetch_gap",
        json!({ "project_id": id, "report": "share" }),
    );
    assert_eq!(body(3)[0]["targets"], json!(["example.com", "rival.com"]));
    assert_eq!(share["data"][1]["target"], "rival.com");

    let categories = call(
        &platform,
        "fetch_gap",
        json!({ "project_id": id, "report": "categories" }),
    );
    assert_eq!(categories["data"][0]["code"], 10885);
    let players = call(
        &platform,
        "fetch_gap",
        json!({ "project_id": id, "report": "players", "categories": [10885] }),
    );
    assert_eq!(body(5)[0]["category_codes"], json!([10885]));
    assert_eq!(players["data"][0]["domain"], "rival.com");

    assert_eq!(
        call(&platform, "get_category_names", json!({ "project_id": id })),
        Value::Null
    );
    let names = call(
        &platform,
        "fetch_category_names",
        json!({ "project_id": id }),
    );
    assert_eq!(names[2]["name"], "Athletic Shoes");
    assert_eq!(
        transport.requests()[6].method,
        octopuson_core::http::HttpMethod::Get
    );
    assert!(
        transport.requests()[6]
            .url
            .ends_with("/v3/dataforseo_labs/categories")
    );
    assert_eq!(
        call(&platform, "get_category_names", json!({ "project_id": id }))[0]["code"],
        10021
    );
    assert_eq!(transport.requests().len(), 7);
}

#[test]
fn links_are_audited_decided_disavowed_and_prospected() {
    let dir = tempfile::tempdir().unwrap();
    let links = include_str!("../../octopuson-dataforseo/tests/fixtures/backlinks_backlinks.json");
    let transport = Arc::new(FakeTransport::new().reply(200, links));
    let platform = platform(dir.path(), &transport);
    let id = project(&platform);
    call(
        &platform,
        "set_key",
        json!({ "provider": "dataforseo", "name": "credentials", "value": "a:b" }),
    );
    let audit = call(
        &platform,
        "fetch_backlinks",
        json!({ "project_id": id, "report": "audit" }),
    );
    let body: Value =
        serde_json::from_slice(transport.requests()[0].body.as_deref().unwrap()).unwrap();
    assert_eq!(body[0]["mode"], "one_per_domain");
    assert_eq!(body[0]["order_by"], json!(["backlink_spam_score,desc"]));
    assert_eq!(body[0]["limit"], 1000);
    let first = &audit["data"][0];
    assert!(first["domain"].is_string());
    assert!(["low", "medium", "high"].contains(&first["risk"].as_str().unwrap()));

    // Decisions, then the file Google takes.
    let decided = call(
        &platform,
        "decide_links",
        json!({ "project_id": id, "domains": ["spam.xyz", "farm.net"], "decision": "disavow" }),
    );
    assert_eq!(decided.as_array().unwrap().len(), 2);
    call(
        &platform,
        "decide_links",
        json!({ "project_id": id, "domains": ["farm.net"], "decision": "watch", "note": "bakılacak" }),
    );
    assert_eq!(
        error(
            &platform,
            "decide_links",
            json!({ "project_id": id, "domains": [], "decision": "keep" })
        ),
        ErrorCode::Invalid
    );
    let file = call(&platform, "disavow_file", json!({ "project_id": id }));
    assert_eq!(file["file_name"], "disavow-example.com.txt");
    assert_eq!(file["domains"], 1);
    assert!(
        file["text"]
            .as_str()
            .unwrap()
            .ends_with("domain:spam.xyz\n")
    );

    call(
        &platform,
        "save_link_prospect",
        json!({ "project_id": id, "prospect": { "domain": "blog.example.org", "source": "gap", "status": "prospect" } }),
    );
    call(
        &platform,
        "save_link_prospect",
        json!({ "project_id": id, "prospect": { "domain": "blog.example.org", "source": "gap", "status": "won", "note": "yazı yayında" } }),
    );
    let prospects = call(&platform, "link_prospects", json!({ "project_id": id }));
    assert_eq!(prospects[0]["status"], "won");
    call(
        &platform,
        "remove_link_prospect",
        json!({ "project_id": id, "domain": "blog.example.org" }),
    );
    assert_eq!(
        call(&platform, "link_prospects", json!({ "project_id": id })),
        json!([])
    );
    // No audit yet: no outbound links.
    assert_eq!(
        call(&platform, "audit_outbound", json!({ "project_id": id })),
        Value::Null
    );
    assert_eq!(transport.requests().len(), 1);
}

const KEYWORD_HISTORY: &str =
    include_str!("../../octopuson-dataforseo/tests/fixtures/historical_keyword_data.json");
const SERP_HISTORY: &str =
    include_str!("../../octopuson-dataforseo/tests/fixtures/historical_serps.json");

#[test]
#[allow(clippy::too_many_lines)] // one keyword through every research report
fn keyword_research_reads_history_weights_parent_and_countries() {
    let dir = tempfile::tempdir().unwrap();
    let fixture = |name: &str| {
        std::fs::read_to_string(format!(
            "{}/../octopuson-dataforseo/tests/fixtures/{name}.json",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap()
    };
    let transport = Arc::new(
        FakeTransport::new()
            .reply(200, KEYWORD_HISTORY)
            .reply(200, SERP_PAGE)
            .reply(200, fixture("backlinks_bulk_ranks"))
            .reply(200, fixture("backlinks_bulk_referring_domains"))
            .reply(200, BULK)
            .reply(200, RANKED)
            .reply(200, fixture("keyword_overview"))
            .reply(200, fixture("keyword_overview")),
    );
    let platform = platform(dir.path(), &transport);
    let id = project(&platform);
    call(
        &platform,
        "set_key",
        json!({ "provider": "dataforseo", "name": "credentials", "value": "a:b" }),
    );
    let body = |index: usize| -> Value {
        serde_json::from_slice(transport.requests()[index].body.as_deref().unwrap()).unwrap()
    };
    let ask = |report: &str| json!({ "project_id": id, "report": report, "seed": "spor ayakkabı" });

    let history = call(&platform, "fetch_research", ask("history"));
    assert_eq!(history["data"][0]["year"], 2021);
    assert!(
        transport.requests()[0]
            .url
            .ends_with("/historical_keyword_data/live")
    );

    // The result page's weights and the parent topic read the kept result page.
    assert_eq!(
        error(&platform, "research_estimate", ask("serp_metrics")),
        ErrorCode::Invalid
    );
    call(
        &platform,
        "fetch_keywords",
        json!({ "project_id": id, "report": "serp", "seed": "spor ayakkabı" }),
    );
    let weights = call(&platform, "fetch_research", ask("serp_metrics"));
    assert_eq!(weights["data"][0]["rank"], 1);
    assert_eq!(
        weights["data"][0]["url"],
        "https://www.example.com/spor-ayakkabi"
    );
    assert_eq!(
        body(2)[0]["targets"],
        json!([
            "https://www.example.com/spor-ayakkabi",
            "https://www.rival.com/spor"
        ])
    );
    assert!(
        transport.requests()[3]
            .url
            .ends_with("/bulk_referring_domains/live")
    );

    let parent = call(&platform, "fetch_research", ask("parent"));
    assert_eq!(
        body(5)[0]["target"],
        "https://www.example.com/spor-ayakkabi"
    );
    assert_eq!(
        parent["data"]["url"],
        "https://www.example.com/spor-ayakkabi"
    );
    assert_eq!(parent["data"]["reach"]["keywords"], 3052);
    assert!(parent["data"]["parent"]["keyword"].is_string());

    let countries = call(
        &platform,
        "fetch_research",
        json!({ "project_id": id, "report": "countries", "seed": "spor ayakkabı", "markets": ["de-de", "us-en"] }),
    );
    assert_eq!(body(6)[0]["location_code"], 2276);
    assert_eq!(body(7)[0]["location_code"], 2840);
    assert_eq!(countries["data"][1]["market"], "us-en");
    assert_eq!(transport.requests().len(), 8);
}

#[test]
fn lists_group_by_the_result_pages_their_keywords_share() {
    let dir = tempfile::tempdir().unwrap();
    // Two keywords with the same first results, one with others.
    let elsewhere = SERP_HISTORY.replace("https://www.", "https://www.baska-");
    let transport = Arc::new(
        FakeTransport::new()
            .reply(200, SERP_HISTORY)
            .reply(200, SERP_HISTORY)
            .reply(200, elsewhere),
    );
    let platform = platform(dir.path(), &transport);
    let id = project(&platform);
    call(
        &platform,
        "set_key",
        json!({ "provider": "dataforseo", "name": "credentials", "value": "a:b" }),
    );
    let list = call(
        &platform,
        "create_keyword_list",
        json!({ "project_id": id, "name": "Ayakkabı" }),
    );
    let list_id = list["id"].as_i64().unwrap();
    call(
        &platform,
        "add_to_keyword_list",
        json!({ "list_id": list_id, "keywords": [
            { "keyword": "spor ayakkabı", "volume": 40000, "difficulty": 60, "intent": "commercial" },
            { "keyword": "spor ayakkabı modelleri", "volume": 9000, "difficulty": 40, "intent": "commercial" },
            { "keyword": "ayakkabı bakımı", "volume": 4000, "difficulty": 20, "intent": "informational" }
        ] }),
    );
    let estimate = call(
        &platform,
        "serp_cluster_estimate",
        json!({ "list_id": list_id }),
    );
    assert_eq!(estimate["keywords"], 3);
    assert_eq!(estimate["kept"], 0);
    assert!(estimate["estimate"]["usd"].as_f64().unwrap() > 0.0);
    // Nothing kept: every keyword waits for its page.
    let empty = call(&platform, "serp_clusters", json!({ "list_id": list_id }));
    assert_eq!(empty["without_page"].as_array().unwrap().len(), 3);

    let job = call(
        &platform,
        "start_serp_clustering",
        json!({ "list_id": list_id }),
    );
    let job = finished(&platform, job.as_str().unwrap());
    assert_eq!(job.status, crate::JobStatus::Done);
    assert_eq!(transport.requests().len(), 3);
    let body: Value =
        serde_json::from_slice(transport.requests()[0].body.as_deref().unwrap()).unwrap();
    assert!(
        transport.requests()[0]
            .url
            .ends_with("/historical_serps/live")
    );
    assert!(body[0]["date_from"].as_str().unwrap().ends_with("-01"));

    let groups = call(&platform, "serp_clusters", json!({ "list_id": list_id }));
    let sizes: Vec<usize> = groups["groups"]
        .as_array()
        .unwrap()
        .iter()
        .map(|group| group["keywords"].as_array().unwrap().len())
        .collect();
    let mut sorted = sizes.clone();
    sorted.sort_unstable();
    assert_eq!(sorted, [1, 2]);
    assert_eq!(groups["without_page"], json!([]));
    let big = groups["groups"]
        .as_array()
        .unwrap()
        .iter()
        .find(|group| group["keywords"].as_array().unwrap().len() == 2)
        .unwrap();
    // The first two keywords of the list got the same page.
    let listed = call(&platform, "list_keywords", json!({ "list_id": list_id }));
    let volume = |index: usize| listed[index]["data"]["volume"].as_u64().unwrap();
    assert_eq!(big["volume"], volume(0) + volume(1));
    // Kept now: grouping again asks nothing.
    let again = call(
        &platform,
        "serp_cluster_estimate",
        json!({ "list_id": list_id }),
    );
    assert_eq!(again["kept"], 3);
    assert_eq!(again["estimate"], Value::Null);
}

/// What the standard queue answers to a request of two tasks (keyword and tracker ids in
/// their tags); a task without an id is refused.
fn posted(tasks: &[(&str, Option<&str>)], cost: f64) -> String {
    json!({
        "status_code": 20_000,
        "cost": cost,
        "tasks": tasks.iter().map(|(tag, id)| match id {
            Some(id) => json!({ "id": id, "status_code": 20_100, "status_message": "Task Created.", "cost": cost / 2.0, "data": { "tag": tag } }),
            None => json!({ "id": "refused", "status_code": 40_501, "status_message": "Invalid Field.", "cost": 0, "data": { "tag": tag } }),
        }).collect::<Vec<_>>()
    })
    .to_string()
}

/// A queued task still waiting.
const IN_QUEUE: &str = r#"{"status_code": 20000, "tasks": [{"status_code": 40602, "status_message": "Task In Queue.", "cost": 0, "result": null}]}"#;

/// Makes every queued check due now.
fn queue_due(platform: &Platform) {
    let store = platform.workspace().store().unwrap();
    for task in store.due_rank_tasks(i64::MAX, 100).unwrap() {
        store.delay_rank_task(task.id, 0).unwrap();
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
            // The scheduled run: a request of two tasks per place, one refused.
            .reply(
                200,
                posted(&[("1|1", Some("q-1")), ("2|1", Some("q-2"))], 0.0024),
            )
            .reply(200, posted(&[("1|2", Some("q-3")), ("2|2", None)], 0.0012))
            // Their answers: one still in the queue at first.
            .reply(200, RANK_PAGE)
            .reply(200, IN_QUEUE)
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
    // Scheduled runs on the own key go through the standard queue: 4 × $0.0018 a run.
    assert_eq!(estimate["queued"], true);
    assert!((estimate["scheduled_usd"].as_f64().unwrap() - 0.0072).abs() < 1e-9);
    assert!((estimate["month_usd"].as_f64().unwrap() - 0.0288).abs() < 1e-9);

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
    let job = finished(&platform, &started[0]);
    assert_eq!(job.status, crate::JobStatus::Partial);
    assert_eq!(job.result.as_ref().unwrap()["queued"], 3);
    assert_eq!(job.result.as_ref().unwrap()["failed"], 1);
    let requests = transport.requests();
    assert!(
        requests[5]
            .url
            .ends_with("/v3/serp/google/organic/task_post")
    );
    let batch: Value = serde_json::from_slice(requests[5].body.as_deref().unwrap()).unwrap();
    assert_eq!(batch.as_array().unwrap().len(), 2);
    assert_eq!(batch[0]["priority"], 1);
    assert_eq!(batch[1]["tag"], "2|1");
    // The run waits for its answers; the schedule does not start another meanwhile.
    let runs = call(&platform, "rank_runs", json!({ "project_id": id }));
    assert_eq!(runs[0]["scheduled"], true);
    assert_eq!(runs[0]["status"], "running");
    assert_eq!(runs[0]["waiting"], 3);
    assert!((runs[0]["cost_usd"].as_f64().unwrap() - 0.0036).abs() < 1e-9);
    assert!(platform.next_collect().unwrap().is_some());
    platform
        .workspace()
        .store()
        .unwrap()
        .mark_rank_run(id, "2026-01-01T00:00:00Z")
        .unwrap();
    assert_eq!(platform.run_due().unwrap(), Vec::<String>::new());

    // Nothing is asked before the first wait; then one answer is not ready yet.
    assert_eq!(platform.collect_queued().unwrap().read, 0);
    queue_due(&platform);
    let round = platform.collect_queued().unwrap();
    assert_eq!((round.read, round.waiting, round.finished), (2, 1, 0));
    assert!(
        transport.requests()[7]
            .url
            .ends_with("/v3/serp/google/organic/task_get/advanced/q-1")
    );
    queue_due(&platform);
    let round = platform.collect_queued().unwrap();
    assert_eq!((round.read, round.waiting, round.finished), (1, 0, 1));
    let runs = call(&platform, "rank_runs", json!({ "project_id": id }));
    assert_eq!(runs[0]["status"], "partial");
    assert_eq!(runs[0]["checked"], 3);
    assert_eq!(runs[0]["failed"], 1);
    assert_eq!(runs[0]["waiting"], 0);
    assert_eq!(platform.next_collect().unwrap(), None);
    let today = crate::now()[..10].to_owned();
    let report = call(
        &platform,
        "rank_report",
        json!({ "project_id": id, "compare": today }),
    );
    let spor = &report["rows"][0];
    assert_eq!(spor["position"], 2);
    assert_eq!(spor["owned"], json!(["ai_overview", "featured_snippet"]));
    // 2,900 searches at second place: 15% of them.
    assert_eq!(spor["traffic"], 435.0);
    assert_eq!(spor["rivals"]["rival.com"], 1);
    assert_eq!(spor["compared"]["position"], 2);
    assert_eq!(report["compare"], json!(today));
    assert_eq!(
        report["landing_pages"][0]["url"],
        "https://www.example.com/spor-ayakkabi"
    );
    assert!(report["traffic"]["rival.com"].as_f64().unwrap() >= 812.0);
    let overview = report["summary"]["features"]
        .as_array()
        .unwrap()
        .iter()
        .find(|feature| feature["feature"] == "ai_overview")
        .unwrap();
    assert_eq!(
        (overview["shown"].as_u64(), overview["owned"].as_u64()),
        (Some(2), Some(2))
    );
    // A day before any check compares with nothing.
    let before = call(
        &platform,
        "rank_report",
        json!({ "project_id": id, "compare": "2020-01-01" }),
    );
    assert_eq!(before["rows"][0].get("compared"), None);

    // Every place side by side.
    let places = call(&platform, "rank_places", json!({ "project_id": id }));
    assert_eq!(places["trackers"].as_array().unwrap().len(), 2);
    assert_eq!(places["rows"][0]["places"].as_array().unwrap().len(), 2);
    assert_eq!(places["rows"][0]["places"][1]["position"], 2);
    // The refused check keeps the keyword's last position on the city place.
    assert_eq!(places["summaries"][1]["ranked"], 2);
    assert_eq!(transport.requests().len(), 11);
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

/// Makes every queued local task due now.
fn local_due(platform: &Platform) {
    let store = platform.workspace().store().unwrap();
    for task in store.due_local_tasks(i64::MAX, 100).unwrap() {
        store.delay_local_task(task.id, 0).unwrap();
    }
}

#[test]
#[allow(clippy::too_many_lines)] // a business from its search to its scheduled grid
fn local_seo_finds_the_business_checks_its_grid_and_reads_reviews() {
    let dir = tempfile::tempdir().unwrap();
    let maps = fixture("maps_grid");
    let grid_tasks: Vec<String> = (0..9).map(|n| format!("grid|{n}")).collect();
    let posted_grid: Vec<(&str, Option<&str>)> = grid_tasks
        .iter()
        .enumerate()
        .map(|(n, tag)| {
            (
                tag.as_str(),
                Some(
                    [
                        "m-0", "m-1", "m-2", "m-3", "m-4", "m-5", "m-6", "m-7", "m-8",
                    ][n],
                ),
            )
        })
        .collect();
    let mut transport = FakeTransport::new()
        .reply(200, fixture("business_info"))
        .reply(200, fixture("business_listings"));
    // A live grid of nine points, read several at once.
    for _ in 0..9 {
        transport = transport.reply(200, maps.clone());
    }
    // The scheduled grid: nine points in one request, then the newest reviews.
    transport = transport
        .reply(200, posted(&posted_grid, 0.0054))
        .reply(200, posted(&[("reviews", Some("rv-1"))], 0.003));
    // Their answers: one point still in the queue at first.
    transport = transport.reply(200, IN_QUEUE);
    for _ in 0..8 {
        transport = transport.reply(200, maps.clone());
    }
    transport = transport
        .reply(200, fixture("google_reviews"))
        .reply(200, maps.clone());
    let transport = Arc::new(transport);
    let platform = platform(dir.path(), &transport);
    let id = project(&platform);
    call(
        &platform,
        "set_key",
        json!({ "provider": "dataforseo", "name": "credentials", "value": "me@example.com:secret" }),
    );

    let setup = call(&platform, "local_setup", json!({ "project_id": id }));
    assert_eq!(setup["business"], Value::Null);
    assert_eq!(setup["points"], json!([]));
    assert_eq!(
        error(&platform, "start_local_grid", json!({ "project_id": id })),
        ErrorCode::Invalid
    );
    assert_eq!(
        error(
            &platform,
            "local_estimate",
            json!({ "project_id": id, "report": "profile" })
        ),
        ErrorCode::Invalid
    );

    // Found by name, then chosen.
    let find = json!({ "project_id": id, "report": "find", "query": "Örnek Kahve Kadıköy" });
    let estimate = call(&platform, "local_estimate", find.clone());
    assert!((estimate["usd"].as_f64().unwrap() - 0.0054).abs() < 1e-9);
    assert_eq!(estimate["blocked"], Value::Null);
    let found = call(&platform, "fetch_local", find);
    let business = found["data"]["business"].clone();
    assert_eq!(business["cid"], "222222222222222222");
    assert_eq!(found["data"]["hours"][0]["hours"][0], "08:00–22:30");
    let request: Value =
        serde_json::from_slice(transport.requests()[0].body.as_deref().unwrap()).unwrap();
    assert_eq!(request[0]["keyword"], "Örnek Kahve Kadıköy");
    let setup = call(
        &platform,
        "choose_business",
        json!({ "project_id": id, "business": business }),
    );
    assert_eq!(setup["business"]["title"], "Örnek Kahve Kadıköy");
    assert_eq!(setup["points"].as_array().unwrap().len(), 25);
    let setup = call(
        &platform,
        "save_local_settings",
        json!({ "project_id": id, "keywords": ["kahve"], "grid": 3, "radius_km": 1.0, "schedule": "weekly" }),
    );
    assert_eq!(setup["points"].as_array().unwrap().len(), 9);
    assert_eq!(setup["zoom"], 16);
    assert!(setup["next_run_at"].is_string(), "never run: due now");

    // Nine live pages by hand; scheduled runs through the queue with twenty reviews.
    let estimate = call(
        &platform,
        "local_grid_estimate",
        json!({ "project_id": id }),
    );
    assert_eq!(
        (estimate["points"].as_u64(), estimate["checks"].as_u64()),
        (Some(9), Some(9))
    );
    assert!((estimate["run"]["usd"].as_f64().unwrap() - 0.018).abs() < 1e-9);
    assert_eq!(estimate["queued"], true);
    assert!((estimate["scheduled_usd"].as_f64().unwrap() - 0.0069).abs() < 1e-9);
    assert!((estimate["month_usd"].as_f64().unwrap() - 0.0276).abs() < 1e-9);

    // The businesses around it, the project's own marked.
    let around = call(
        &platform,
        "fetch_local",
        json!({ "project_id": id, "report": "competitors" }),
    );
    assert_eq!(around["data"]["total"], 214);
    let own: Vec<bool> = around["data"]["businesses"]
        .as_array()
        .unwrap()
        .iter()
        .map(|business| business["own"].as_bool().unwrap())
        .collect();
    assert_eq!(own, [false, false, true]);
    assert_eq!(around["data"]["businesses"][2]["distance_km"], 0.0);

    let job = call(&platform, "start_local_grid", json!({ "project_id": id }));
    let job = finished(&platform, job.as_str().unwrap());
    assert_eq!(job.status, crate::JobStatus::Done);
    assert_eq!(job.result.as_ref().unwrap()["checked"], 9);
    let maps_request = transport
        .requests()
        .into_iter()
        .find(|request| request.url.ends_with("/v3/serp/google/maps/live/advanced"))
        .unwrap();
    let body: Value = serde_json::from_slice(maps_request.body.as_deref().unwrap()).unwrap();
    assert_eq!(body[0]["keyword"], "kahve");
    assert!(
        body[0]["location_coordinate"]
            .as_str()
            .unwrap()
            .ends_with(",16z")
    );
    let grid = call(&platform, "local_grid", json!({ "project_id": id }));
    assert_eq!(grid["run"]["status"], "done");
    assert_eq!(grid["keywords"][0]["keyword"], "kahve");
    assert_eq!(grid["keywords"][0]["cells"].as_array().unwrap().len(), 9);
    assert_eq!(grid["summary"]["arp"], 2.0);
    assert_eq!(grid["summary"]["solv"], 100.0);
    assert_eq!(grid["rivals"][0]["title"], "Rakip Kahve");
    assert_eq!(grid["rivals"][0]["top3"], 9);
    assert_eq!(
        grid["rivals"].as_array().unwrap().len(),
        2,
        "the project's own is not a rival"
    );

    // The schedule: nine points posted to the queue, and the newest reviews.
    platform
        .workspace()
        .store()
        .unwrap()
        .mark_local_run(id, "2026-01-01T00:00:00Z")
        .unwrap();
    let started = platform.run_due().unwrap();
    assert_eq!(started.len(), 1);
    let job = finished(&platform, &started[0]);
    assert_eq!(job.result.as_ref().unwrap()["queued"], 9);
    assert_eq!(job.result.as_ref().unwrap()["reviews_queued"], 1);
    let runs = call(&platform, "local_runs", json!({ "project_id": id }));
    assert_eq!(
        (runs[0]["status"].as_str(), runs[0]["waiting"].as_u64()),
        (Some("running"), Some(9))
    );
    let setup = call(&platform, "local_setup", json!({ "project_id": id }));
    assert_eq!(setup["waiting_reviews"], 1);
    assert!(platform.next_collect().unwrap().is_some());
    local_due(&platform);
    let round = platform.collect_queued().unwrap();
    assert_eq!((round.read, round.waiting, round.finished), (9, 1, 0));
    local_due(&platform);
    let round = platform.collect_queued().unwrap();
    assert_eq!((round.read, round.waiting, round.finished), (1, 0, 1));
    let grid = call(&platform, "local_grid", json!({ "project_id": id }));
    assert_eq!(grid["run"]["scheduled"], true);
    assert_eq!(grid["run"]["checked"], 9);
    assert_eq!(grid["trend"].as_array().unwrap().len(), 2);

    let reviews = call(&platform, "local_reviews", json!({ "project_id": id }));
    assert_eq!(reviews["reviews"].as_array().unwrap().len(), 3);
    assert_eq!(reviews["summary"]["unanswered"], 2);
    assert_eq!(reviews["summary"]["unanswered_negative"], 0);
    assert_eq!(reviews["summary"]["stars"], json!([0, 1, 0, 1, 1]));
    assert_eq!(reviews["ratings"].as_array().unwrap().len(), 1);
    assert_eq!(reviews["ratings"][0]["votes"], 640);
    assert_eq!(reviews["waiting"], 0);
    assert_eq!(platform.next_collect().unwrap(), None);
    call(&platform, "delete_project", json!({ "id": id }));
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
    // The crawl's links to other sites: listed, not checked unless asked.
    let outbound = call(&platform, "audit_outbound", json!({ "project_id": id }));
    assert_eq!(outbound["audit_id"], first);
    assert_eq!(outbound["checked"], false);
    assert!(
        outbound["columns"]
            .as_array()
            .unwrap()
            .iter()
            .any(|column| column == "inlinks")
    );
    assert_eq!(setup["settings"]["check_outbound"], false);
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
    assert_eq!(report["top_pages"].as_array().unwrap().len(), 2);
    assert_eq!(report["top_pages"][0]["answers"], 1);
    // The run's share of voice, brand by brand: the own one and the rival it names.
    assert_eq!(report["history"][0]["brands"]["ornekmagaza.com"], 50.0);
    assert_eq!(report["history"][0]["brands"]["rakip-magaza.com"], 50.0);
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
            // The sync: days (now, then before), queries, pages, devices, countries, queries on
            // pages.
            .reply(200, gsc_rows(&[(&now_day, 30.0, 600.0, 6.0)]))
            .reply(200, gsc_rows(&[(&before_day, 10.0, 400.0, 9.0)]))
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
            // Analytics: days (now, then before), landing pages now and before.
            .reply(200, ga4_rows(&[(&now_day.replace('-', ""), 40.0, 3.0)]))
            .reply(200, ga4_rows(&[]))
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
    assert_eq!(requests.len(), 18);
    let days: Value = serde_json::from_slice(requests[5].body.as_deref().unwrap()).unwrap();
    assert_eq!(days["dimensions"], json!(["date"]));
    assert_eq!(
        (days["startDate"].as_str(), days["endDate"].as_str()),
        (Some(ago(30).as_str()), Some(ago(3).as_str()))
    );
    let days_before: Value = serde_json::from_slice(requests[6].body.as_deref().unwrap()).unwrap();
    assert_eq!(
        (
            days_before["startDate"].as_str(),
            days_before["endDate"].as_str()
        ),
        (Some(ago(58).as_str()), Some(ago(31).as_str()))
    );
    let queries: Value = serde_json::from_slice(requests[7].body.as_deref().unwrap()).unwrap();
    assert_eq!(
        (queries["startDate"].as_str(), queries["endDate"].as_str()),
        (Some(ago(30).as_str()), Some(ago(3).as_str()))
    );
    let before: Value = serde_json::from_slice(requests[8].body.as_deref().unwrap()).unwrap();
    assert_eq!(before["endDate"], ago(31));
    assert!(requests[14].url.ends_with("/properties/42:runReport"));
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
    assert_eq!(report["compare"], "previous");
    assert_eq!(report["brand"]["terms"], json!(["example"]));
    assert_eq!(report["brand"]["other"]["clicks"], 30.0);
    assert_eq!(report["brand"]["brand"]["clicks"], 0.0);
    assert_eq!(report["queries"][0]["brand"], false);
    let exported = call(
        &platform,
        "gsc_export",
        json!({ "project_id": id, "kind": "queries" }),
    );
    assert_eq!(exported["columns"][0], "query");
    assert_eq!(exported["rows"].as_array().unwrap().len(), 2);
    assert_eq!(exported["rows"][0][5], 5.0);
    let pairs = call(
        &platform,
        "gsc_export",
        json!({ "project_id": id, "kind": "query_pages" }),
    );
    assert_eq!(pairs["rows"][1][1], "https://example.com/blog/sneaker");
    assert_eq!(
        error(
            &platform,
            "gsc_export",
            json!({ "project_id": id, "kind": "days" })
        ),
        ErrorCode::Invalid
    );
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
    // The edit dialog's preview draws settings not saved yet, and stores nothing.
    let mut edited = spec.clone();
    edited["report_id"] = json!(report_id);
    edited["design"] = json!("accent");
    edited["colors"] = json!({ "cover": "#14213d", "heading": "#7a1f3d" });
    let preview = call(&desktop, "report_preview", edited);
    let preview = preview["html"].as_str().unwrap();
    assert!(preview.contains("class=\"report design-accent custom-cover cover-dark\""));
    assert!(preview.contains("--heading:#7a1f3d"));
    let kept = call(&desktop, "reports_setup", json!({ "project_id": id }));
    assert_eq!(kept["reports"][0]["design"], "clean");
    assert_eq!(
        kept["designs"],
        json!(["clean", "ocean", "accent", "elegant"])
    );
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
        "local_seo",
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

fn fixture(name: &str) -> String {
    std::fs::read_to_string(format!(
        "{}/../octopuson-dataforseo/tests/fixtures/{name}.json",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap()
}

#[test]
#[allow(clippy::too_many_lines)] // taking, keeping, comparing, the tools, and no source
fn snapshots_keep_the_whole_project_and_compare_two_moments() {
    let dir = tempfile::tempdir().unwrap();
    let mut transport = FakeTransport::new();
    for name in [
        "domain_rank_overview",
        "ranked_keywords",
        "relevant_pages",
        "backlinks_summary",
        "backlinks_referring_domains",
        "domain_rank_overview",
        "backlinks_summary",
    ] {
        transport = transport.reply(200, fixture(name));
    }
    let transport = Arc::new(transport);
    let platform = Arc::new(platform(dir.path(), &transport));
    let server = crate::mcp::McpServer::new(Arc::clone(&platform), "octopuson-platform");
    let id = project(&platform);
    call(
        &platform,
        "set_key",
        json!({ "provider": "dataforseo", "name": "credentials", "value": "me@example.com:secret" }),
    );
    let priced = tool(&server, "take_snapshot", json!({ "project_id": id }));
    let priced = &priced["structuredContent"];
    assert_eq!(priced["status"], "needs_approval", "{priced}");
    assert!(priced["price"]["usd"].as_f64().unwrap() > 0.2);
    assert_eq!(
        transport.requests().len(),
        0,
        "nothing is bought without an allowance"
    );
    let setup = call(&platform, "snapshot_setup", json!({ "project_id": id }));
    assert_eq!(setup["competitors"], json!(["rival.com"]));
    assert_eq!(setup["baseline_id"], Value::Null);
    call(
        &platform,
        "save_snapshot_settings",
        json!({ "project_id": id, "settings": { "schedule": "monthly", "competitors": ["rival.com", "unknown.com"] } }),
    );
    let estimate = call(&platform, "snapshot_estimate", json!({ "project_id": id }));
    assert_eq!(estimate["paid"], 7, "{estimate}");
    assert_eq!(estimate["via"], "own");
    assert!(estimate["usd"].as_f64().unwrap() > 0.2);

    let job = call(
        &platform,
        "start_snapshot",
        json!({ "project_id": id, "label": "Başlangıç" }),
    );
    let done = finished(&platform, job.as_str().unwrap());
    assert_eq!(done.status, crate::JobStatus::Done, "{:?}", done.error);
    assert_eq!(transport.requests().len(), 7);
    // What the snapshot paid for is kept for the modules' pages too: a second one pays nothing.
    let again = call(&platform, "snapshot_estimate", json!({ "project_id": id }));
    assert_eq!(
        (again["paid"].as_u64(), again["kept"].as_u64()),
        (Some(0), Some(7))
    );
    let second = call(&platform, "start_snapshot", json!({ "project_id": id }));
    finished(&platform, second.as_str().unwrap());
    assert_eq!(transport.requests().len(), 7);

    let list = call(&platform, "list_snapshots", json!({ "project_id": id }));
    assert_eq!(list.as_array().unwrap().len(), 2);
    let first_id = list[1]["id"].as_i64().unwrap();
    let first = call(
        &platform,
        "get_snapshot",
        json!({ "project_id": id, "snapshot_id": first_id }),
    );
    assert_eq!(first["label"], "Başlangıç");
    assert_eq!(first["data"]["domain"], "example.com");
    assert!(first["data"]["overview"]["traffic"].as_f64().unwrap() > 0.0);
    assert_ne!(first["data"]["keywords"], json!([]));
    assert_ne!(first["data"]["domains"], json!([]));
    assert_eq!(first["data"]["competitors"][0]["domain"], "rival.com");
    assert!(first["cost_usd"].as_f64().unwrap() > 0.0);

    let compared = call(
        &platform,
        "compare_snapshots",
        json!({ "project_id": id, "lang": "en" }),
    );
    assert_eq!(compared["from"]["id"], first_id);
    assert_eq!(compared["keywords"]["gained"], 0);
    assert_eq!(compared["summary"][0]["tone"], "note");
    assert_eq!(compared["sites"][1]["domain"], "rival.com");
    let setup = call(&platform, "snapshot_setup", json!({ "project_id": id }));
    assert_eq!(setup["baseline_id"], first_id);
    assert!(setup["next_at"].as_i64().unwrap() > octopuson_platform::unix_now());
    assert!(crate::modules::snapshots::changes_schedule(
        "save_snapshot_settings"
    ));
    let listed = tool(&server, "list_snapshots", json!({ "project_id": id }));
    let listed = &listed["structuredContent"];
    assert_eq!(listed["baseline_id"], first_id);
    assert_eq!(listed["schedule"], "monthly");
    assert_eq!(listed["snapshots"].as_array().unwrap().len(), 2);
    assert!(listed["snapshots"][0].get("data").is_none());
    let sheet = call(
        &platform,
        "snapshot_xlsx",
        json!({ "project_id": id, "lang": "en" }),
    );
    assert!(
        sheet["name"]
            .as_str()
            .unwrap()
            .starts_with("snapshots-example.com-")
    );
    assert_ne!(sheet["data"].as_str().unwrap(), "");
    assert!(
        list[0]["headline"]["traffic"].as_f64().unwrap() > 0.0,
        "{}",
        list[0]
    );
    let progress = call(&platform, "snapshot_progress", Value::Null);
    assert_eq!(progress[0]["project_id"], id);
    assert_eq!(progress[0]["kpis"][0]["key"], "traffic");
    let told = tool(
        &server,
        "compare_snapshots",
        json!({ "project_id": id, "previous": true }),
    );
    assert_eq!(
        told["structuredContent"]["summary"][0]["tone"], "note",
        "{told}"
    );

    // Kept answers are reused even without a source; a project with nothing kept and no source
    // has nothing to keep, and the snapshot says why.
    call(
        &platform,
        "remove_key",
        json!({ "provider": "dataforseo", "name": "credentials" }),
    );
    let other = call(
        &platform,
        "create_project",
        json!({ "spec": { "domain": "baska.com", "country": "tr", "language": "tr" } }),
    )["id"]
        .as_i64()
        .unwrap();
    let blocked = call(
        &platform,
        "snapshot_estimate",
        json!({ "project_id": other }),
    );
    assert_eq!(blocked["blocked"]["code"], "no_source");
    let refused = tool(&server, "take_snapshot", json!({ "project_id": other }));
    assert_eq!(
        refused["structuredContent"]["status"], "blocked",
        "{refused}"
    );
    let none = tool(&server, "compare_snapshots", json!({ "project_id": other }));
    assert!(
        none["structuredContent"]["message"]
            .as_str()
            .unwrap()
            .contains("fewer than two")
    );
    let empty = call(&platform, "start_snapshot", json!({ "project_id": other }));
    let failed = finished(&platform, empty.as_str().unwrap());
    assert_eq!(failed.status, crate::JobStatus::Partial);
    let rows = call(&platform, "list_snapshots", json!({ "project_id": other }));
    assert_eq!(rows[0]["status"], "failed");
    assert!(
        rows[0]["error"].as_str().unwrap().contains("DataForSEO"),
        "{}",
        rows[0]
    );
    assert_eq!(transport.requests().len(), 7);
}

#[test]
fn reports_go_out_after_each_new_snapshot() {
    use crate::snapshot::tests::{after, before};
    use octopuson_core::Timestamp;
    use octopuson_platform::SnapshotEnd;

    let dir = tempfile::tempdir().unwrap();
    let transport = Arc::new(FakeTransport::new());
    let mail = Arc::new(KeptMail::default());
    let web = Platform::new(
        workspace(dir.path(), Host::Web, &transport)
            .with_printer(printer())
            .with_mailer(mail.clone()),
        Arc::new(NoEvents),
        Arc::new(|_| false),
    );
    let id = project(&web);
    let at = |back: i64| {
        Timestamp::from_unix_micros((octopuson_platform::unix_now() - back) * 1_000_000).to_string()
    };
    let keep = |data: &crate::snapshot::SnapshotData, when: &str| {
        let store = web.workspace().store().unwrap();
        let snapshot = store.begin_snapshot(id, true, None, when).unwrap();
        store
            .finish_snapshot(
                snapshot,
                &SnapshotEnd {
                    status: "done",
                    cost_usd: 0.3,
                    error: None,
                    data: &serde_json::to_value(data).unwrap(),
                },
                when,
            )
            .unwrap();
    };
    keep(&before(), &at(7_200));
    let report = call(
        &web,
        "save_report",
        json!({ "project_id": id, "name": "Önce / sonra", "language": "tr", "days": 28,
            "widgets": ["comparison", "comparison_previous"], "schedule": "snapshot",
            "recipients": ["musteri@example.com"] }),
    );
    let report_id = report["id"].as_i64().unwrap();
    assert_eq!(
        web.next_due().unwrap(),
        None,
        "no new snapshot since the report was saved"
    );
    assert_eq!(web.run_due().unwrap(), Vec::<String>::new());

    // A snapshot being taken: looked at again a little later.
    std::thread::sleep(std::time::Duration::from_millis(1_100));
    let taking = web
        .workspace()
        .store()
        .unwrap()
        .begin_snapshot(id, true, None, &at(0))
        .unwrap();
    let next = web.next_due().unwrap().unwrap();
    assert!(next > octopuson_platform::unix_now() + 500, "{next}");
    web.workspace()
        .store()
        .unwrap()
        .delete_snapshot(id, taking)
        .unwrap();
    keep(&after(), &at(0));
    assert!(web.next_due().unwrap().unwrap() <= octopuson_platform::unix_now());
    let started = web.run_due().unwrap();
    assert_eq!(started.len(), 1, "sent once the snapshot is kept");
    let job = finished(&web, &started[0]);
    assert_eq!(job.status, crate::JobStatus::Done, "{:?}", job.error);
    assert_eq!(mail.0.lock().unwrap().len(), 1);
    assert_eq!(
        web.next_due().unwrap(),
        None,
        "sent: waits for the next snapshot"
    );
    assert_eq!(web.run_due().unwrap(), Vec::<String>::new());

    let page = call(
        &web,
        "report_html",
        json!({ "project_id": id, "report_id": report_id }),
    );
    let html = page["html"].as_str().unwrap();
    assert!(html.contains("Organik trafik %23 arttı (1.240 → 1.525)."));
    assert!(html.contains("Önce / sonra: son anlık görüntüden bu yana"));
    assert_eq!(transport.requests().len(), 0, "a report pays no source");
}

/// Two ranking pages about running shoes and the project's own, thinner page.
fn ranking_pages() -> FakeSite {
    let page = |title: &str, extra: &str| {
        format!(
            r#"<html lang="tr"><head><title>{title}</title><meta name="description" content="Spor ayakkabı seçimi">
            <script type="application/ld+json">{{"@context":"https://schema.org","@type":"Article"}}</script></head>
            <body><main><h1>{title}</h1>
            <h2>Spor ayakkabı nasıl seçilir</h2><p>Koşu için hafif taban, doğru numara ve nefes alan kumaş önemlidir. Spor ayakkabı seçerken ayak tipinize bakın.</p>
            <h2>Taban ve yastıklama</h2><p>Yastıklama dizleri korur. Taban esnek olmalı; koşu ayakkabısı ile yürüyüş ayakkabısı farklıdır.</p>
            <h2>Fiyatlar</h2><p>Fiyat aralığı geniştir; kampanya dönemlerinde indirim olur. {extra}</p></main></body></html>"#
        )
    };
    FakeSite(std::collections::HashMap::from([
        ("https://www.rival.com/spor".to_owned(), page("Spor Ayakkabı Rehberi", "Numara tablosu ile doğru beden.")),
        ("https://www.example.com/spor-ayakkabi".to_owned(), page("Spor Ayakkabı Modelleri", "Numara seçimi kolaydır.")),
        (
            "https://www.example.com/kosu".to_owned(),
            "<html><head><title>Koşu</title></head><body><h1>Koşu</h1><p>Kısa bir sayfa.</p></body></html>".to_owned(),
        ),
    ]))
}

#[test]
#[allow(clippy::too_many_lines)] // a brief from the result page to writing
fn briefs_read_the_ranking_pages_score_pages_and_write_with_the_users_key() {
    let dir = tempfile::tempdir().unwrap();
    let claude = json!({
        "model": "claude-opus-5-5",
        "stop_reason": "end_turn",
        "content": [{ "type": "text", "text": "# Brief\nHedef: 900 kelime" }],
        "usage": { "input_tokens": 2_000, "output_tokens": 1_000 }
    })
    .to_string();
    let transport = Arc::new(
        FakeTransport::new()
            .reply(200, SERP_PAGE)
            .reply(200, claude),
    );
    let platform = crawling_platform(dir.path(), Host::Desktop, &transport, ranking_pages());
    let id = project(&platform);
    let ask = json!({ "project_id": id, "keyword": "Spor  ayakkabı", "page": "https://www.example.com/kosu" });
    assert_eq!(
        error(&platform, "start_content_brief", ask.clone()),
        ErrorCode::NoSource
    );
    call(
        &platform,
        "set_key",
        json!({ "provider": "dataforseo", "name": "credentials", "value": "a:b" }),
    );
    let estimate = call(&platform, "content_brief_estimate", ask.clone());
    assert_eq!(estimate["via"], "own");
    assert!((estimate["usd"].as_f64().unwrap() - 0.002).abs() < 1e-9);

    let started = call(&platform, "start_content_brief", ask);
    let job = finished(&platform, started["job_id"].as_str().unwrap());
    assert_eq!(job.status, crate::JobStatus::Done, "{:?}", job.error);
    let brief_id = started["brief_id"].as_i64().unwrap();
    assert_eq!(job.result.as_ref().unwrap()["read"], 2);

    let briefs = call(&platform, "content_briefs", json!({ "project_id": id }));
    assert_eq!(briefs[0]["status"], "done");
    assert_eq!(briefs[0]["keyword"], "Spor ayakkabı");
    assert_eq!(briefs[0]["pages"], 2);
    let view = call(&platform, "content_brief", json!({ "brief_id": brief_id }));
    let brief = &view["brief"];
    assert_eq!(brief["competitors"].as_array().unwrap().len(), 2);
    assert!(brief["competitors"][0]["score"].as_f64().unwrap() > 0.0);
    let terms: Vec<&str> = brief["terms"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|term| term["term"].as_str())
        .collect();
    assert!(terms.iter().any(|term| term.contains("taban")), "{terms:?}");
    assert_eq!(brief["schema"], json!(["Article"]));
    // The fixture's "People also ask" block lists no questions; the shared headings make the
    // outline.
    assert_eq!(brief["questions"], json!([]));
    assert!(!brief["outline"].as_array().unwrap().is_empty(), "{brief}");
    // The project's thin page was scored against the brief, missing most terms.
    let scores = view["scores"].as_array().unwrap();
    assert_eq!(scores.len(), 1);
    assert_eq!(scores[0]["url"], "https://www.example.com/kosu");
    assert_ne!(scores[0]["result"]["missing"].as_array().unwrap().len(), 0);
    let thin = scores[0]["score"].as_f64().unwrap();

    let draft = json!({
        "brief_id": brief_id,
        "title": "Spor Ayakkabı Rehberi 2026",
        "description": "Spor ayakkabı nasıl seçilir, taban ve numara",
        "body": "# Spor ayakkabı rehberi\n\n## Spor ayakkabı nasıl seçilir\nKoşu için hafif taban, doğru numara ve nefes alan kumaş önemlidir.\n\n## Taban ve yastıklama\nYastıklama dizleri korur, taban esnek olmalı.\n\n## Fiyatlar\n- Kampanya dönemlerinde indirim\n- Numara tablosu ile doğru beden\n",
    });
    let preview = call(&platform, "score_content", draft.clone());
    assert_eq!(preview["id"], Value::Null);
    assert!(preview["breakdown"]["score"].as_f64().unwrap() > thin);
    assert!(preview["keyword_in_title"].as_bool().unwrap());
    let mut kept = draft.clone();
    kept["save"] = json!(true);
    let saved = call(&platform, "score_content", kept);
    let draft_id = saved["id"].as_i64().unwrap();
    let view = call(&platform, "content_brief", json!({ "brief_id": brief_id }));
    assert_eq!(view["scores"][0]["kind"], "draft");
    assert!(
        view["scores"][0]["body"]
            .as_str()
            .unwrap()
            .starts_with("# Spor")
    );
    assert_eq!(
        error(
            &platform,
            "score_content",
            json!({ "brief_id": brief_id, "body": " " })
        ),
        ErrorCode::Invalid
    );
    assert_eq!(
        call(&platform, "content_pages", json!({ "project_id": id })),
        json!([])
    );

    let writing = json!({ "brief_id": brief_id, "task": "brief" });
    let writers = call(&platform, "content_writing_estimate", writing.clone());
    assert_eq!(writers["writers"][0]["engine"], "claude");
    assert_eq!(writers["writers"][0]["ready"], false);
    assert_eq!(
        error(&platform, "start_content_writing", writing.clone()),
        ErrorCode::NoSource
    );
    call(
        &platform,
        "set_key",
        json!({ "provider": "anthropic", "name": "api_key", "value": "sk-test" }),
    );
    let job = call(&platform, "start_content_writing", writing);
    let job = finished(&platform, job.as_str().unwrap());
    assert_eq!(job.status, crate::JobStatus::Done, "{:?}", job.error);
    let view = call(&platform, "content_brief", json!({ "brief_id": brief_id }));
    assert_eq!(view["ai_text"], "# Brief\nHedef: 900 kelime");
    assert_eq!(view["ai_model"], "claude:claude-opus-5-5");
    let requests = transport.requests();
    let sent = requests.last().unwrap();
    assert_eq!(sent.url, "https://api.anthropic.com/v1/messages");
    let body: Value = serde_json::from_slice(sent.body.as_deref().unwrap()).unwrap();
    assert!(body["system"].as_str().unwrap().contains("Turkish"));
    assert!(
        body["messages"][0]["content"]
            .as_str()
            .unwrap()
            .contains("Spor ayakkabı")
    );
    assert!(view["cost_usd"].as_f64().unwrap() > 0.002);
    assert_eq!(
        error(
            &platform,
            "start_content_writing",
            json!({ "brief_id": brief_id, "task": "improve" })
        ),
        ErrorCode::Invalid
    );
    let _ = draft_id;

    call(
        &platform,
        "delete_content_brief",
        json!({ "brief_id": brief_id }),
    );
    assert_eq!(
        call(&platform, "content_briefs", json!({ "project_id": id })),
        json!([])
    );
}

#[test]
fn a_reply_to_a_review_is_drafted_with_the_users_key() {
    let dir = tempfile::tempdir().unwrap();
    let claude = json!({
        "model": "claude-opus-5-5",
        "stop_reason": "end_turn",
        "content": [{ "type": "text", "text": " Teşekkürler Ayşe Hanım! " }],
        "usage": { "input_tokens": 400, "output_tokens": 120 }
    })
    .to_string();
    let transport = Arc::new(FakeTransport::new().reply(200, claude));
    let platform = platform(dir.path(), &transport);
    let id = project(&platform);
    {
        let store = platform.workspace().store().unwrap();
        store
            .save_local_settings(
                id,
                &octopuson_platform::LocalSettings {
                    business: Some(json!({ "title": "Örnek Kahve", "category": "Kafe", "lat": 40.9, "lng": 29.0 })),
                    ..octopuson_platform::LocalSettings::default()
                },
            )
            .unwrap();
        store
            .save_local_reviews(
                id,
                &[octopuson_platform::LocalReviewRow {
                    review_id: "r-1".to_owned(),
                    rating: Some(2),
                    text: Some("Kahve soğuktu.".to_owned()),
                    author: Some("Ayşe".to_owned()),
                    author_url: None,
                    url: None,
                    published_at: None,
                    owner_answer: None,
                    answered_at: None,
                    photos: 0,
                    local_guide: false,
                    first_seen: "2026-10-23T09:00:00Z".to_owned(),
                }],
                "2026-10-23T09:00:00Z",
            )
            .unwrap();
    }
    let ask = json!({ "project_id": id, "review_id": "r-1" });
    assert_eq!(
        error(&platform, "draft_review_reply", ask.clone()),
        ErrorCode::NoSource
    );
    call(
        &platform,
        "set_key",
        json!({ "provider": "anthropic", "name": "api_key", "value": "sk-test" }),
    );
    assert_eq!(
        call(&platform, "reply_estimate", json!({ "project_id": id }))[0]["ready"],
        true
    );
    let drafted = call(&platform, "draft_review_reply", ask);
    assert_eq!(drafted["text"], "Teşekkürler Ayşe Hanım!");
    let body: Value =
        serde_json::from_slice(transport.requests()[0].body.as_deref().unwrap()).unwrap();
    let material = body["messages"][0]["content"].as_str().unwrap();
    assert!(material.contains("Örnek Kahve") && material.contains("2 of 5 stars"));
    assert_eq!(
        error(
            &platform,
            "draft_review_reply",
            json!({ "project_id": id, "review_id": "nope" })
        ),
        ErrorCode::NotFound
    );
}

#[test]
fn an_audits_crawl_shows_its_links_pages_hreflang_and_findings() {
    let dir = tempfile::tempdir().unwrap();
    let transport = Arc::new(FakeTransport::new());
    let platform = crawling_platform(dir.path(), Host::Desktop, &transport, small_site());
    let id = project(&platform);
    assert_eq!(
        call(&platform, "audit_links", json!({ "project_id": id })),
        Value::Null
    );
    let job = call(&platform, "start_audit", json!({ "project_id": id }));
    assert_eq!(
        finished(&platform, job.as_str().unwrap()).status,
        crate::JobStatus::Done
    );

    let links = call(&platform, "audit_links", json!({ "project_id": id }));
    assert_eq!(links["pages"], 3);
    assert_eq!(links["depth"], json!([[0, 1], [1, 2]]));
    // The home page passes its rank to the two pages it links to, which link nowhere.
    assert_eq!(links["top"][0]["url"], "https://example.com/a");
    assert_eq!(links["top"][0]["rank"], 100.0);
    assert_eq!(links["top"][2]["url"], "https://example.com/");
    assert_eq!(links["top"][2]["rank"], 0.0);
    assert_eq!(links["sitemaps"], false);
    // Kept in memory: asked again, the same.
    assert_eq!(
        call(&platform, "audit_links", json!({ "project_id": id })),
        links
    );

    let pages = call(
        &platform,
        "audit_pages",
        json!({ "project_id": id, "sort": "address", "limit": 2 }),
    );
    assert_eq!(pages["total"], 3);
    assert_eq!(pages["rows"].as_array().unwrap().len(), 2);
    let address = pages["columns"]
        .as_array()
        .unwrap()
        .iter()
        .position(|column| column == "address")
        .unwrap();
    assert_eq!(pages["rows"][0][address], "https://example.com/");
    let searched = call(
        &platform,
        "audit_pages",
        json!({ "project_id": id, "search": "/b" }),
    );
    assert_eq!(searched["total"], 1);

    let audit_id = pages["audit_id"].as_i64().unwrap();
    let page = call(
        &platform,
        "audit_page",
        json!({ "project_id": id, "audit_id": audit_id, "url": "https://example.com/a", "lang": "tr" }),
    );
    assert_eq!(page["inlinks_total"], 1);
    assert_eq!(page["inlinks"][0]["url"], "https://example.com/");
    assert_eq!(page["inlinks"][0]["anchor"], "A");
    assert!(
        page["findings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|finding| finding["id"] == "title-missing")
    );
    assert!(
        page["facts"]
            .as_array()
            .unwrap()
            .iter()
            .any(|fact| fact[0] == "h1" && fact[1] == "A")
    );
    assert_eq!(
        error(
            &platform,
            "audit_page",
            json!({ "project_id": id, "audit_id": audit_id, "url": "https://example.com/zzz" })
        ),
        ErrorCode::NotFound
    );

    let hreflang = call(&platform, "audit_hreflang", json!({ "project_id": id }));
    assert_eq!(hreflang["pages"], 0);
    assert!(
        hreflang["issues"]
            .as_array()
            .unwrap()
            .iter()
            .any(|issue| issue["id"] == "hreflang-missing-return")
    );

    let file = call(
        &platform,
        "audit_findings_csv",
        json!({ "project_id": id, "lang": "en" }),
    );
    let csv = file["csv"].as_str().unwrap();
    assert!(csv.starts_with("check,title,priority,url\n"));
    assert!(csv.contains("title-missing,"));
    assert!(csv.contains("https://example.com/a"));
    assert_eq!(file["cut"], false);
}

#[test]
fn core_web_vitals_come_from_google_with_the_users_key() {
    let dir = tempfile::tempdir().unwrap();
    let record = json!({ "record": {
        "metrics": {
            "largest_contentful_paint": { "histogram": [{ "density": 0.8 }, { "density": 0.15 }, { "density": 0.05 }], "percentiles": { "p75": 2100 } },
            "cumulative_layout_shift": { "histogram": [], "percentiles": { "p75": "0.03" } },
            "interaction_to_next_paint": { "histogram": [], "percentiles": { "p75": 150 } }
        },
        "collectionPeriod": { "firstDate": { "year": 2026, "month": 9, "day": 22 }, "lastDate": { "year": 2026, "month": 10, "day": 19 } }
    }});
    let history = json!({ "record": {
        "metrics": { "largest_contentful_paint": { "percentilesTimeseries": { "p75s": [2300, 2100] } } },
        "collectionPeriods": [
            { "lastDate": { "year": 2026, "month": 10, "day": 12 } },
            { "lastDate": { "year": 2026, "month": 10, "day": 19 } }
        ]
    }});
    let missing = json!({ "error": { "code": 404, "message": "chrome ux report data not found" } })
        .to_string();
    let lab = json!({ "lighthouseResult": {
        "finalDisplayedUrl": "https://example.com/",
        "categories": { "performance": { "score": 0.83 } },
        "audits": { "largest-contentful-paint": { "numericValue": 2400 }, "unused-css-rules": { "title": "Unused CSS", "details": { "type": "opportunity", "overallSavingsMs": 300 } } }
    }});
    let transport = Arc::new(
        FakeTransport::new()
            .reply(200, record.to_string())
            .reply(200, history.to_string())
            .reply(404, missing.clone())
            .reply(404, missing)
            .reply(200, lab.to_string()),
    );
    let platform = platform(dir.path(), &transport);
    let id = project(&platform);
    let setup = call(&platform, "vitals_setup", json!({ "project_id": id }));
    assert_eq!(setup["key_ready"], false);
    assert_eq!(setup["pages"][0], "https://example.com/");
    assert_eq!(
        error(&platform, "start_vitals", json!({ "project_id": id })),
        ErrorCode::NoSource
    );
    call(
        &platform,
        "set_key",
        json!({ "provider": "google-api", "name": "api_key", "value": "AIza-test" }),
    );
    assert_eq!(
        error(
            &platform,
            "start_vitals",
            json!({ "project_id": id, "pages": ["ftp://x"] })
        ),
        ErrorCode::Invalid
    );
    let job = call(
        &platform,
        "start_vitals",
        json!({ "project_id": id, "pages": ["https://example.com/"] }),
    );
    let job = finished(&platform, job.as_str().unwrap());
    assert_eq!(job.status, crate::JobStatus::Done, "{:?}", job.error);
    assert_eq!(job.result.as_ref().unwrap()["origins"], 1);
    let requests = transport.requests();
    assert_eq!(requests.len(), 5);
    assert!(requests.iter().all(|request| !request.url.contains("AIza")));
    let report = call(&platform, "vitals_report", json!({ "project_id": id }));
    let origin = &report["origins"][0];
    assert_eq!(
        (origin["origin"].as_str(), origin["device"].as_str()),
        (Some("https://example.com"), Some("phone"))
    );
    assert_eq!(origin["passes"], true);
    assert_eq!(origin["field"]["lcp"]["p75"], 2100.0);
    assert_eq!(origin["history"]["lcp"][0], 2300.0);
    let page = &report["pages"][0];
    assert_eq!(page["lab"]["score"], 83.0);
    assert_eq!(page["lab"]["opportunities"][0]["id"], "unused-css-rules");
    assert_eq!(report["scores"][0][3], 83.0);
    assert!(
        call(&platform, "vitals_setup", json!({ "project_id": id }))["last_checked"].is_string()
    );
}

#[test]
#[allow(clippy::too_many_lines)] // a project's data, its opportunities, decisions and dashboard
fn opportunities_come_from_the_modules_data_and_decisions_hide_them() {
    use octopuson_platform::{
        CheckResult, GscData, GscSyncEnd, LinkProspect, Period, QueryPage, RankCheckRow,
        SearchKind, SearchRow, TrackerSpec,
    };
    let dir = tempfile::tempdir().unwrap();
    let transport = Arc::new(FakeTransport::new());
    let platform = platform(dir.path(), &transport);
    let id = project(&platform);
    let empty = call(&platform, "opportunities", json!({ "project_id": id }));
    assert_eq!(empty["open"], 0);
    assert_eq!(empty["sources"]["rank"], false);
    {
        let store = platform.workspace().store().unwrap();
        let now = crate::now();
        let tracker = store
            .add_rank_tracker(
                id,
                &TrackerSpec {
                    engine: "google".to_owned(),
                    country: "tr".to_owned(),
                    language: "tr".to_owned(),
                    device: "desktop".to_owned(),
                    location: None,
                },
                &now,
            )
            .unwrap();
        store
            .track_keywords(id, &[("spor ayakkabı".to_owned(), Some(9_900))], &[], &now)
            .unwrap();
        let keyword = store.tracked_keywords(id).unwrap()[0].id;
        let run = store.begin_rank_run(id, false, 1, &now).unwrap();
        store
            .add_rank_check(&RankCheckRow {
                run_id: run,
                project_id: id,
                keyword_id: keyword,
                tracker_id: tracker.id,
                checked_at: now.clone(),
                position: Some(6),
                url: Some("https://example.com/spor".to_owned()),
                pages: vec![(6, "https://example.com/spor".to_owned())],
                features: Vec::new(),
                ai_overview: false,
                ai_cited: false,
                snippet: false,
                owned: Vec::new(),
                results: vec![CheckResult {
                    position: 1,
                    domain: "rival.com".to_owned(),
                    url: "https://rival.com/".to_owned(),
                    title: None,
                }],
                error: None,
            })
            .unwrap();
        let row = |key: &str, clicks: f64, impressions: f64, position: f64| SearchRow {
            key: key.to_owned(),
            clicks,
            impressions,
            position,
        };
        store
            .replace_gsc_data(
                id,
                &GscData {
                    search: vec![
                        (
                            SearchKind::Date,
                            Period::Days,
                            row(&ago(10), 40.0, 9_000.0, 7.0),
                        ),
                        (
                            SearchKind::Date,
                            Period::Days,
                            row(&ago(40), 60.0, 9_000.0, 7.0),
                        ),
                        (
                            SearchKind::Query,
                            Period::Now,
                            row("beyaz sneaker", 20.0, 8_000.0, 8.0),
                        ),
                        (
                            SearchKind::Query,
                            Period::Now,
                            row("example", 30.0, 300.0, 1.0),
                        ),
                        (
                            SearchKind::Page,
                            Period::Now,
                            row("https://example.com/eski", 10.0, 900.0, 9.0),
                        ),
                        (
                            SearchKind::Page,
                            Period::Before,
                            row("https://example.com/eski", 60.0, 1_200.0, 6.0),
                        ),
                    ],
                    query_pages: vec![QueryPage {
                        query: "beyaz sneaker".to_owned(),
                        page: "https://example.com/beyaz".to_owned(),
                        clicks: 20.0,
                        impressions: 8_000.0,
                        position: 8.0,
                    }],
                    traffic: Vec::new(),
                },
            )
            .unwrap();
        let sync = store.begin_gsc_sync(id, false, &now).unwrap();
        let (start, end) = (ago(30), ago(3));
        store
            .finish_gsc_sync(
                sync,
                &GscSyncEnd {
                    status: "done",
                    period: Some((&start, &end)),
                    rows: 7,
                    currency: None,
                    limited: false,
                    error: None,
                },
                &now,
            )
            .unwrap();
        store
            .save_prospect(
                id,
                &LinkProspect {
                    domain: "blog.com".to_owned(),
                    url: None,
                    source: "lost".to_owned(),
                    status: "prospect".to_owned(),
                    note: None,
                    added_at: now.clone(),
                    updated_at: now.clone(),
                },
                &now,
            )
            .unwrap();
    }
    let found = call(
        &platform,
        "opportunities",
        json!({ "project_id": id, "lang": "en" }),
    );
    let keys: Vec<&str> = found["items"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|item| item["key"].as_str())
        .collect();
    for key in [
        "near_top|spor ayakkabı",
        "near_top|beyaz sneaker",
        "low_ctr|beyaz sneaker",
        "losing_page|https://example.com/eski",
        "lost_links|lost",
    ] {
        assert!(keys.contains(&key), "{key} in {keys:?}");
    }
    // The brand query is no opportunity.
    assert!(!keys.iter().any(|key| key.ends_with("|example")));
    assert_eq!(found["sources"]["rank"], true);
    assert_eq!(found["sources"]["gsc"], true);
    // A low-effort fix comes first.
    assert_eq!(found["items"][0]["key"], "low_ctr|beyaz sneaker");
    assert_eq!(found["items"][0]["effort"], "low");
    let near = found["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["key"] == "near_top|spor ayakkabı")
        .unwrap();
    assert_eq!(near["page"], "rank");
    assert!(near["gain"].as_f64().unwrap() > 100.0);
    let open = found["open"].as_u64().unwrap();

    call(
        &platform,
        "set_opportunity",
        json!({ "project_id": id, "key": "near_top|spor ayakkabı", "state": "done" }),
    );
    call(
        &platform,
        "set_opportunity",
        json!({ "project_id": id, "key": "lost_links|lost", "state": "snoozed", "days": 14 }),
    );
    assert_eq!(
        error(
            &platform,
            "set_opportunity",
            json!({ "project_id": id, "key": "x", "state": "later" })
        ),
        ErrorCode::Invalid
    );
    let after = call(&platform, "opportunities", json!({ "project_id": id }));
    assert_eq!(after["open"].as_u64().unwrap(), open - 2);
    assert_eq!(after["hidden"], json!({ "done": 1, "snoozed": 1 }));
    let every = call(
        &platform,
        "opportunities",
        json!({ "project_id": id, "all": true }),
    );
    let done = every["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["key"] == "near_top|spor ayakkabı")
        .unwrap();
    assert_eq!(done["state"], "done");
    call(
        &platform,
        "set_opportunity",
        json!({ "project_id": id, "key": "near_top|spor ayakkabı", "state": "open" }),
    );

    let dashboard = call(
        &platform,
        "project_dashboard",
        json!({ "project_id": id, "lang": "en" }),
    );
    assert_eq!(dashboard["project"]["id"], id);
    assert_eq!(dashboard["rank"]["checked"], 1.0);
    assert_eq!(dashboard["rank"]["top10"], 1.0);
    assert_eq!(dashboard["search"]["clicks"], 40.0);
    assert_eq!(dashboard["search"]["clicks_before"], 60.0);
    assert_eq!(dashboard["audit"], Value::Null);
    assert_eq!(
        dashboard["opportunities"][0]["key"],
        "low_ctr|beyaz sneaker"
    );
    assert_eq!(dashboard["opportunities_open"].as_u64().unwrap(), open - 1);
    assert_eq!(dashboard["spend_total"], 0.0);
    assert_eq!(dashboard["content"]["briefs"], 0);
}

/// Keeps the notifications it is asked to show.
#[derive(Default)]
struct KeptNotes(std::sync::Mutex<Vec<(String, String)>>);

impl crate::mail::Notifier for KeptNotes {
    fn notify(&self, title: &str, body: &str) {
        self.0
            .lock()
            .unwrap()
            .push((title.to_owned(), body.to_owned()));
    }
}

/// A finished Search Console sync whose clicks fell from 100 to 40.
fn seed_click_drop(platform: &Platform, id: i64) {
    use octopuson_platform::{GscData, GscSyncEnd, Period, SearchKind, SearchRow};
    let store = platform.workspace().store().unwrap();
    let row = |key: String, clicks: f64| SearchRow {
        key,
        clicks,
        impressions: clicks * 30.0,
        position: 6.0,
    };
    store
        .replace_gsc_data(
            id,
            &GscData {
                search: vec![
                    (SearchKind::Date, Period::Days, row(ago(10), 40.0)),
                    (SearchKind::Date, Period::Days, row(ago(40), 100.0)),
                ],
                query_pages: Vec::new(),
                traffic: Vec::new(),
            },
        )
        .unwrap();
    let now = crate::now();
    let sync = store.begin_gsc_sync(id, false, &now).unwrap();
    let (start, end) = (ago(30), ago(3));
    store
        .finish_gsc_sync(
            sync,
            &GscSyncEnd {
                status: "done",
                period: Some((&start, &end)),
                rows: 2,
                currency: None,
                limited: false,
                error: None,
            },
            &now,
        )
        .unwrap();
}

#[test]
fn alerts_are_found_once_mailed_from_the_web_and_shown_on_the_desktop() {
    let dir = tempfile::tempdir().unwrap();
    let transport = Arc::new(FakeTransport::new());
    let mail = Arc::new(KeptMail::default());
    let web = Platform::new(
        workspace(dir.path(), Host::Web, &transport)
            .with_mailer(mail.clone())
            .with_public_url("https://app.octopuson.com"),
        Arc::new(NoEvents),
        Arc::new(|_| false),
    );
    let id = project(&web);
    seed_click_drop(&web, id);
    let setup = call(&web, "alert_setup", json!({ "project_id": id }));
    assert_eq!(
        (setup["can_email"].as_bool(), setup["can_notify"].as_bool()),
        (Some(true), Some(false))
    );
    let mut settings = setup["settings"].clone();
    settings["email"] = json!("instant");
    settings["lang"] = json!("en");
    assert_eq!(
        error(
            &web,
            "save_alert_settings",
            json!({ "project_id": id, "settings": settings })
        ),
        ErrorCode::Invalid,
        "no recipients"
    );
    settings["recipients"] = json!(["ajans@ornek.com"]);
    call(
        &web,
        "save_alert_settings",
        json!({ "project_id": id, "settings": settings }),
    );

    assert_eq!(call(&web, "check_alerts", json!({ "project_id": id })), 1);
    {
        let sent = mail.0.lock().unwrap();
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0].to, ["ajans@ornek.com"]);
        assert!(
            sent[0].subject.ends_with(": 1 new alerts"),
            "{}",
            sent[0].subject
        );
        assert!(
            sent[0]
                .text
                .contains("Search Console clicks fell 60%: 100 → 40"),
            "{}",
            sent[0].text
        );
        assert!(sent[0].text.contains(&format!(
            "https://app.octopuson.com/#/project?project={id}&tab=alerts"
        )));
    }
    assert_eq!(
        call(&web, "check_alerts", json!({ "project_id": id })),
        0,
        "kept once"
    );
    assert_eq!(mail.0.lock().unwrap().len(), 1);
    let listed = call(&web, "list_alerts", json!({}));
    assert_eq!(listed["unread"], 1);
    assert_eq!(listed["alerts"][0]["kind"], "gsc_drop");
    assert_eq!(listed["alerts"][0]["detail"]["drop"], 60.0);
    assert_eq!(listed["by_project"][id.to_string()], 1);
    assert_eq!(
        call(&web, "mark_alerts_read", json!({ "project_id": id })),
        1
    );
    assert_eq!(
        call(&web, "list_alerts", json!({ "unread": true }))["unread"],
        0
    );

    let dir = tempfile::tempdir().unwrap();
    let notes = Arc::new(KeptNotes::default());
    let desktop = Platform::new(
        workspace(dir.path(), Host::Desktop, &transport).with_notifier(notes.clone()),
        Arc::new(NoEvents),
        Arc::new(|_| false),
    );
    let id = project(&desktop);
    seed_click_drop(&desktop, id);
    assert_eq!(
        call(&desktop, "alert_setup", json!({ "project_id": id }))["can_notify"],
        true
    );
    assert_eq!(
        call(&desktop, "check_alerts", json!({ "project_id": id })),
        1
    );
    let shown = notes.0.lock().unwrap().clone();
    assert_eq!(shown.len(), 1);
    assert!(shown[0].1.contains("60"), "{shown:?}");
    // Already shown: not again.
    crate::modules::alerts::announce(desktop.workspace()).unwrap();
    assert_eq!(notes.0.lock().unwrap().len(), 1);
}

#[test]
fn a_finished_job_reaches_the_hook() {
    let jobs = crate::JobManager::new(Arc::new(NoEvents));
    let seen = Arc::new(std::sync::Mutex::new(Vec::new()));
    let kept = seen.clone();
    jobs.on_finish(move |job| kept.lock().unwrap().push((job.kind.clone(), job.status)));
    jobs.start("site-audit", "x", Some(1), |_| {
        Ok(crate::jobs::JobDone::with(json!({})))
    })
    .unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    while seen.lock().unwrap().is_empty() {
        assert!(Instant::now() < deadline, "the hook was not called");
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(
        seen.lock().unwrap()[0],
        ("site-audit".to_owned(), crate::JobStatus::Done)
    );
}

#[test]
fn the_link_watch_reads_won_and_lost_sites_and_keeps_the_lost_ones_to_win_back() {
    let dir = tempfile::tempdir().unwrap();
    let recent = format!("{} 08:30:00 +00:00", ago(2));
    let old = format!("{} 08:30:00 +00:00", ago(40));
    let answer = |items: Value| {
        json!({ "status_code": 20000, "cost": 0.02, "tasks": [{ "status_code": 20000, "status_message": "Ok.", "cost": 0.02,
            "result": [{ "target": "example.com", "total_count": 2, "items_count": 2, "items": items }] }] })
        .to_string()
    };
    let transport = Arc::new(
        FakeTransport::new()
            .reply(200, answer(json!([
                { "domain_from": "yeni.com", "url_from": "https://yeni.com/a", "first_seen": recent, "last_seen": recent, "is_new": true },
                { "domain_from": "eskiyeni.com", "url_from": "https://eskiyeni.com/a", "first_seen": old, "last_seen": recent, "is_new": true }
            ])))
            .reply(200, answer(json!([
                { "domain_from": "kayip.com", "url_from": "https://kayip.com/a", "first_seen": old, "last_seen": recent, "is_lost": true },
                { "domain_from": "eski.com", "url_from": "https://eski.com/a", "first_seen": old, "last_seen": old, "is_lost": true }
            ]))),
    );
    let platform = platform(dir.path(), &transport);
    let id = project(&platform);
    call(
        &platform,
        "set_key",
        json!({ "provider": "dataforseo", "name": "credentials", "value": "a:b" }),
    );
    let job = call(&platform, "start_link_watch", json!({ "project_id": id }));
    let job = finished(&platform, job.as_str().unwrap());
    assert_eq!(job.status, crate::JobStatus::Done, "{:?}", job.error);
    let result = job.result.unwrap();
    assert_eq!(
        (result["new"].clone(), result["lost"].clone()),
        (json!(["yeni.com"]), json!(["kayip.com"]))
    );
    let requests = transport.requests();
    let first: Value = serde_json::from_slice(requests[0].body.as_deref().unwrap()).unwrap();
    assert_eq!(first[0]["filters"], json!(["is_new", "=", true]));
    let second: Value = serde_json::from_slice(requests[1].body.as_deref().unwrap()).unwrap();
    assert_eq!(second[0]["backlinks_status_type"], "lost");
    let alerts = call(&platform, "list_alerts", json!({ "project_id": id }));
    assert_eq!(alerts["alerts"][0]["kind"], "links");
    assert_eq!(alerts["alerts"][0]["detail"]["lost"], 1);
    let prospects = call(&platform, "link_prospects", json!({ "project_id": id }));
    assert!(
        prospects
            .as_array()
            .unwrap()
            .iter()
            .any(|row| row["domain"] == "kayip.com" && row["source"] == "lost")
    );
    assert!(
        call(&platform, "alert_setup", json!({ "project_id": id }))["settings"]["links_last_run"]
            .is_string()
    );
}

#[test]
fn mcp_work_tools_read_the_project_and_price_every_action() {
    let dir = tempfile::tempdir().unwrap();
    let transport = Arc::new(FakeTransport::new());
    let platform = Arc::new(platform(dir.path(), &transport));
    let server = crate::mcp::McpServer::new(Arc::clone(&platform), "octopuson-platform");
    let id = project(&platform);
    let tools = mcp(&server, "tools/list", json!({}))["result"]["tools"].clone();
    let names: Vec<&str> = tools
        .as_array()
        .unwrap()
        .iter()
        .map(|tool| tool["name"].as_str().unwrap())
        .collect();
    let free = [
        "project_dashboard",
        "opportunities",
        "alerts",
        "keyword_lists",
        "content_briefs",
        "link_work",
        "core_web_vitals",
    ];
    for name in free.iter().chain(&[
        "metric_history",
        "keyword_insights",
        "track_keywords",
        "check_rankings",
        "start_site_audit",
        "run_ai_visibility",
        "create_content_brief",
    ]) {
        assert!(names.contains(name), "{name} missing");
    }
    // The free reads answer even before anything is kept.
    for name in free {
        let answer = tool(&server, name, json!({ "project_id": id }));
        assert_eq!(answer["isError"], false, "{name}: {answer}");
    }
    assert_eq!(
        tool(
            &server,
            "keyword_lists",
            json!({ "project_id": id, "list_id": 999 })
        )["isError"],
        true
    );
    assert_eq!(tool(&server, "opportunities", json!({}))["isError"], true);

    // Tracking is free and immediate.
    let tracked = tool(
        &server,
        "track_keywords",
        json!({ "project_id": id, "keywords": ["spor ayakkabı", " ", "koşu ayakkabısı"] }),
    );
    assert_eq!(tracked["structuredContent"]["added"], 2, "{tracked}");

    // Without a key nothing can be paid: told so, nothing bought.
    let blocked = tool(&server, "check_rankings", json!({ "project_id": id }));
    assert_eq!(blocked["isError"], true);
    assert!(
        blocked["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("cannot be paid now")
    );
    let no_questions = tool(&server, "run_ai_visibility", json!({ "project_id": id }));
    assert_eq!(no_questions["isError"], true);

    // With a key every paid action gives its price first.
    call(
        &platform,
        "set_key",
        json!({ "provider": "dataforseo", "name": "credentials", "value": "me@example.com:secret" }),
    );
    let priced = tool(&server, "check_rankings", json!({ "project_id": id }));
    assert_eq!(
        priced["structuredContent"]["status"], "needs_approval",
        "{priced}"
    );
    assert!(
        priced["structuredContent"]["price"]["usd"]
            .as_f64()
            .unwrap()
            > 0.0
    );
    let brief = tool(
        &server,
        "create_content_brief",
        json!({ "project_id": id, "keyword": "spor ayakkabı" }),
    );
    assert_eq!(
        brief["structuredContent"]["status"], "needs_approval",
        "{brief}"
    );
    let too_little = tool(
        &server,
        "check_rankings",
        json!({ "project_id": id, "allow_spend_usd": 0.000_1 }),
    );
    assert_eq!(too_little["structuredContent"]["status"], "needs_approval");
    assert_eq!(
        transport.requests().len(),
        0,
        "nothing is bought without an allowance"
    );
}

#[test]
fn the_program_shows_the_release_version_of_the_desktop_program() {
    let dir = tempfile::tempdir().unwrap();
    let platform = platform(dir.path(), &Arc::new(FakeTransport::new()));
    let config: Value = serde_json::from_str(
        &std::fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../apps/geo-seo-platform/src-tauri/tauri.conf.json"),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        call(&platform, "app_info", Value::Null)["version"],
        config["version"]
    );
}
