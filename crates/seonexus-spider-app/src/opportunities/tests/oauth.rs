use super::*;
use seonexus_core::http::testing::FakeTransport;
#[test]
fn tokens_are_form_encoded_and_expired_authorization_is_redacted() {
    let http = FakeTransport::new().reply(
        400,
        br#"{"error":"invalid_grant","secret":"do-not-log"}"#.to_vec(),
    );
    let error = exchange_with_client(
        &http,
        &[
            ("grant_type", "refresh_token"),
            ("refresh_token", "sensitive&value"),
        ],
        "fixture.apps.googleusercontent.com",
        None,
    )
    .unwrap_err();
    assert!(error.contains("expired"));
    assert!(!error.contains("sensitive"));
    assert!(!error.contains("do-not-log"));
    let request = &http.requests()[0];
    assert_eq!(request.url, "https://oauth2.googleapis.com/token");
    assert!(String::from_utf8_lossy(request.body.as_ref().unwrap()).contains("sensitive%26value"));
}
#[test]
fn cache_rejects_missing_access_token_and_invalidation_removes_memory_copy() {
    let id = "oauth-unit-token";
    assert!(cache_token(id, &serde_json::json!({"expires_in":3600})).is_err());
    cache_token(
        id,
        &serde_json::json!({"access_token":"unit-access","expires_in":3600}),
    )
    .unwrap();
    let http = FakeTransport::new();
    assert_eq!(access_token(id, &http).unwrap(), "unit-access");
    assert_eq!(http.requests().len(), 0);
    invalidate_access(id);
    assert!(!access().lock().unwrap().contains_key(id));
}
#[test]
fn cancelled_listener_returns_without_an_authorization_code() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    assert!(
        receive_code(&listener, "state", &AtomicBool::new(true))
            .unwrap_err()
            .contains("cancelled")
    );
}
#[test]
fn expired_callback_reports_timeout_and_does_not_accept_late_codes() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    assert_eq!(
        receive_code_until(&listener, "state", &AtomicBool::new(false), Instant::now())
            .unwrap_err(),
        "Google sign-in timed out"
    );
}
#[test]
fn loopback_callback_checks_state_and_handles_denied_consent() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let address = listener.local_addr().unwrap();
    let handle = std::thread::spawn(move || {
        receive_code(&listener, "expected-state", &AtomicBool::new(false))
    });
    let mut stream = std::net::TcpStream::connect(address).unwrap();
    std::thread::sleep(Duration::from_millis(25));
    write!(stream,"GET /oauth/callback?state=expected-state&error=access_denied HTTP/1.1\r\nHost: localhost\r\n\r\n").unwrap();
    assert!(handle.join().unwrap().unwrap_err().contains("denied"));
    assert!(!constant_time_eq(b"a", b"b"));
    assert!(constant_time_eq(b"same", b"same"));
}
