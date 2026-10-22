//! A desktop syncing with a web workspace in the same process: requests go straight to
//! [`hub_answer`] through a transport, never to a network.

use std::path::Path;
use std::sync::{Arc, Mutex, PoisonError};

use octopuson_cloud::TokenStore;
use octopuson_core::http::{HttpRequest, HttpResponse, HttpTransport, TransportError};
use octopuson_secrets::{FileBackend, Secrets};
use serde_json::{Value, json};
use tempfile::TempDir;

use super::*;
use crate::jobs::NoEvents;
use crate::router::Opener;
use crate::{Host, Platform, Workspace};

/// A gateway address nothing listens on: tests never reach a real one.
const LOCAL_GATEWAY: &str = "http://127.0.0.1:9";

/// The web side: one account's workspace answering sync requests.
struct Web {
    workspace: Mutex<Option<Arc<Workspace>>>,
    account: Mutex<String>,
}

impl Web {
    fn workspace(&self) -> Arc<Workspace> {
        self.workspace
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
            .unwrap()
    }
}

impl HttpTransport for Web {
    fn send(&self, request: &HttpRequest) -> Result<HttpResponse, TransportError> {
        assert!(request.url.ends_with(SYNC_PATH), "{}", request.url);
        assert!(
            request
                .headers
                .iter()
                .any(|(name, value)| name == "Authorization" && value == "Bearer oct_test")
        );
        let sync: SyncRequest = decode(request.body.as_deref().unwrap_or_default()).unwrap();
        let account = self
            .account
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        let (status, body) = match hub_answer(&self.workspace(), &account, &sync, now_ms()) {
            Ok(answer) => (200, encode(&answer.response).unwrap()),
            Err(error) => (400, error.to_string().into_bytes()),
        };
        Ok(HttpResponse {
            status,
            headers: Vec::new(),
            body,
            url: request.url.clone(),
        })
    }
}

fn workspace(dir: &Path, host: Host, transport: Arc<dyn HttpTransport>) -> Workspace {
    let secrets = Secrets::new(
        Box::new(FileBackend::new(dir.join("secrets.json"))),
        dir.join("secret-bindings.json"),
    )
    .with_environment(|_| None);
    let tokens = TokenStore::new(
        Box::new(FileBackend::new(dir.join("account.json"))),
        LOCAL_GATEWAY,
    );
    Workspace::new(dir.to_owned(), host, secrets, tokens).with_transport(transport)
}

struct Pair {
    _dirs: (TempDir, TempDir),
    web: Arc<Web>,
    desktop: Platform,
}

fn pair() -> Pair {
    let web_dir = tempfile::tempdir().unwrap();
    let desktop_dir = tempfile::tempdir().unwrap();
    let web = Arc::new(Web {
        workspace: Mutex::new(None),
        account: Mutex::new("user-1".to_owned()),
    });
    let nothing: Arc<dyn HttpTransport> = Arc::new(Web {
        workspace: Mutex::new(None),
        account: Mutex::new(String::new()),
    });
    let hub = workspace(web_dir.path(), Host::Web, nothing);
    *web.workspace.lock().unwrap() = Some(Arc::new(hub));
    let link: Arc<dyn HttpTransport> = web.clone();
    let open: Opener = Arc::new(|_| false);
    let desktop = Platform::new(
        workspace(desktop_dir.path(), Host::Desktop, link),
        Arc::new(NoEvents),
        open,
    );
    desktop
        .workspace()
        .set_account_token(Some("oct_test"))
        .unwrap();
    Pair {
        _dirs: (web_dir, desktop_dir),
        web,
        desktop,
    }
}

fn create(platform: &Platform, domain: &str) -> i64 {
    platform
        .call(
            "create_project",
            json!({ "spec": { "domain": domain, "country": "tr", "language": "tr" } }),
        )
        .unwrap()["id"]
        .as_i64()
        .unwrap()
}

fn domains(workspace: &Workspace) -> Vec<String> {
    let mut domains: Vec<String> = workspace
        .store()
        .unwrap()
        .projects()
        .unwrap()
        .into_iter()
        .map(|project| project.domain)
        .collect();
    domains.sort();
    domains
}

fn done(outcome: RoundOutcome) -> RoundReport {
    match outcome {
        RoundOutcome::Done(report) => report,
        other => panic!("the round did not run: {other:?}"),
    }
}

#[test]
fn a_desktop_and_the_web_keep_the_same_projects() {
    let pair = pair();
    let desktop = pair.desktop.workspace();
    assert!(desktop.schedules_here());
    let id = create(&pair.desktop, "ornek.com");
    pair.desktop
        .call(
            "track_keywords",
            json!({ "project_id": id, "keywords": [{ "keyword": "spor ayakkabı" }] }),
        )
        .unwrap();
    let report = done(round(desktop).unwrap());
    assert!(report.sent >= 2, "{report:?}");
    let web = pair.web.workspace();
    assert_eq!(domains(&web), ["ornek.com"]);

    // Syncing now: scheduled work moves to the web.
    let status = status(desktop).unwrap();
    assert!(status.active && status.enabled && status.signed_in);
    assert_eq!(status.unsent, 0);
    assert!(status.last_at.is_some());
    assert!(!desktop.schedules_here());
    assert!(web.schedules_here());

    // A project made on the web comes to the desktop.
    web.store()
        .unwrap()
        .create_project(
            &octopuson_platform::ProjectSpec {
                domain: "web.com".to_owned(),
                country: "tr".to_owned(),
                language: "tr".to_owned(),
                ..octopuson_platform::ProjectSpec::default()
            },
            "2026-10-22T10:00:00Z",
        )
        .unwrap();
    let report = done(round(desktop).unwrap());
    assert_eq!(report.received, 1);
    assert_eq!(domains(desktop), ["ornek.com", "web.com"]);
    let again = done(round(desktop).unwrap());
    assert_eq!((again.sent, again.received), (0, 0));

    // Switched off, nothing runs and schedules come back here.
    set_enabled(desktop, false).unwrap();
    assert_eq!(round(desktop).unwrap(), RoundOutcome::Off);
    assert!(desktop.schedules_here());
}

#[test]
fn two_filled_workspaces_wait_for_a_choice() {
    for choice in ["merge", "upload", "download"] {
        let pair = pair();
        let desktop = pair.desktop.workspace();
        create(&pair.desktop, "masaustu.com");
        let web = pair.web.workspace();
        web.store()
            .unwrap()
            .create_project(
                &octopuson_platform::ProjectSpec {
                    domain: "web.com".to_owned(),
                    country: "tr".to_owned(),
                    language: "tr".to_owned(),
                    ..octopuson_platform::ProjectSpec::default()
                },
                "2026-10-22T10:00:00Z",
            )
            .unwrap();
        let RoundOutcome::Choose(waiting) = round(desktop).unwrap() else {
            panic!("no choice asked");
        };
        assert_eq!(
            (waiting.reason.as_str(), waiting.here, waiting.web),
            ("first", 1, 1)
        );
        // Nothing moved while waiting.
        assert_eq!(domains(&web), ["web.com"]);
        assert!(status(desktop).unwrap().choice.is_some());
        assert!(desktop.schedules_here());

        choose(desktop, choice).unwrap();
        done(round(desktop).unwrap());
        let expected: &[&str] = match choice {
            "merge" => &["masaustu.com", "web.com"],
            "upload" => &["masaustu.com"],
            _ => &["web.com"],
        };
        assert_eq!(domains(desktop), expected, "{choice}");
        assert_eq!(domains(&web), expected, "{choice}");
    }
}

#[test]
fn another_account_or_a_reset_web_waits_for_a_choice() {
    let pair = pair();
    let desktop = pair.desktop.workspace();
    create(&pair.desktop, "ornek.com");
    done(round(desktop).unwrap());

    *pair.web.account.lock().unwrap() = "user-2".to_owned();
    let RoundOutcome::Choose(waiting) = round(desktop).unwrap() else {
        panic!("no choice asked");
    };
    assert_eq!(waiting.reason, "account");
    choose(desktop, "pause").unwrap();
    assert_eq!(round(desktop).unwrap(), RoundOutcome::Off);
    set_enabled(desktop, true).unwrap();
    *pair.web.account.lock().unwrap() = "user-1".to_owned();
    done(round(desktop).unwrap());

    // The web's sync records start over (a restore): the desktop asks before sending.
    pair.web
        .workspace()
        .store()
        .unwrap()
        .sync_forget(&[])
        .unwrap();
    let RoundOutcome::Choose(waiting) = round(desktop).unwrap() else {
        panic!("no choice asked");
    };
    assert_eq!(waiting.reason, "reset");
    choose(desktop, "merge").unwrap();
    done(round(desktop).unwrap());
    assert_eq!(domains(&pair.web.workspace()), ["ornek.com"]);
}

#[test]
fn commands_that_change_data_make_the_desktop_sync_soon() {
    let pair = pair();
    let workspace = pair.desktop.workspace();
    let before = workspace.generation();
    pair.desktop.call("list_projects", Value::Null).unwrap();
    pair.desktop.call("sync_status", Value::Null).unwrap();
    assert_eq!(workspace.generation(), before);
    create(&pair.desktop, "ornek.com");
    assert!(workspace.generation() > before);

    let status = pair.desktop.call("sync_now", Value::Null).unwrap();
    assert_eq!(status["active"], true);
    assert_eq!(status["unsent"], 0);
    assert!(
        pair.desktop
            .call("sync_choose", json!({ "choice": "merge" }))
            .is_err()
    );
}

#[test]
fn requests_are_checked() {
    let pair = pair();
    let web = pair.web.workspace();
    let request = SyncRequest {
        protocol: PROTOCOL + 1,
        replica: "0123456789abcdef".to_owned(),
        epoch: None,
        cursor: 0,
        changes: Vec::new(),
        last: true,
        pull: true,
        replace: false,
    };
    assert!(hub_answer(&web, "user-1", &request, 0).is_err());
    let bad_replica = SyncRequest {
        protocol: PROTOCOL,
        replica: "../x".to_owned(),
        ..request.clone()
    };
    assert!(hub_answer(&web, "user-1", &bad_replica, 0).is_err());
    let ahead = SyncRequest {
        protocol: PROTOCOL,
        cursor: 99,
        ..request
    };
    assert!(
        hub_answer(&web, "user-1", &ahead, 0)
            .unwrap()
            .response
            .reset
    );

    let packed = encode(&json!({ "a": "x".repeat(1000) })).unwrap();
    assert!(packed.len() < 100);
    assert_eq!(
        decode::<Value>(&packed).unwrap()["a"]
            .as_str()
            .unwrap()
            .len(),
        1000
    );
    assert!(decode::<Value>(b"not zstd").is_err());
    assert!(pair.web.workspace().store().is_ok());
}
