use std::io::{Read, Write};
use std::net::TcpStream;

use octopuson_core::http::testing::FakeTransport;

use super::*;

fn client() -> OAuthClient {
    OAuthClient {
        client_id: "123.apps.googleusercontent.com".to_owned(),
        client_secret: "desktop-secret".to_owned(),
    }
}

fn query(url: &str, name: &str) -> String {
    Url::parse(url)
        .unwrap()
        .query_pairs()
        .find(|(key, _)| key == name)
        .map(|(_, value)| value.into_owned())
        .unwrap()
}

/// Sends the requests to the listener from another thread, each once the previous
/// one was answered, and returns the answers in order.
///
/// In order, so the listener cannot stop on the redirect while an earlier request
/// still waits in its queue.
fn visit(redirect: &str, targets: &[&str]) -> std::thread::JoinHandle<Vec<String>> {
    let address = redirect.trim_start_matches("http://").to_owned();
    let targets: Vec<String> = targets.iter().map(|&target| target.to_owned()).collect();
    std::thread::spawn(move || {
        targets
            .iter()
            .map(|target| {
                let mut stream = TcpStream::connect(&address).unwrap();
                write!(stream, "GET {target} HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n").unwrap();
                let mut answer = String::new();
                stream.read_to_string(&mut answer).unwrap();
                answer
            })
            .collect()
    })
}

#[test]
fn the_challenge_is_the_s256_of_the_verifier() {
    // RFC 7636, appendix B.
    let pkce = Pkce::from_verifier("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk".to_owned());
    assert_eq!(
        pkce.challenge,
        "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
    );
    let random = Pkce::new().unwrap();
    assert_eq!(random.verifier.len(), 43);
    assert_ne!(random.verifier, Pkce::new().unwrap().verifier);
}

#[test]
fn takes_the_code_of_the_redirect_and_exchanges_it() {
    let authorization = begin(&client(), &[scopes::SEARCH_CONSOLE, scopes::ADS]).unwrap();
    let url = authorization.url.clone();
    assert!(url.starts_with(AUTH_URL));
    assert_eq!(query(&url, "code_challenge_method"), "S256");
    assert_eq!(query(&url, "access_type"), "offline");
    assert_eq!(
        query(&url, "scope"),
        format!("{} {}", scopes::SEARCH_CONSOLE, scopes::ADS)
    );
    let redirect = authorization.redirect_uri().to_owned();
    assert!(redirect.starts_with("http://127.0.0.1:"));
    let state = query(&url, "state");
    let browser = visit(
        &redirect,
        &[
            "/favicon.ico",
            &format!("/?state={state}&code=4%2Fabc&scope=x"),
        ],
    );
    let code = authorization
        .wait(Duration::from_secs(10), &|| false)
        .unwrap();
    let answers = browser.join().unwrap();
    assert!(answers[0].starts_with("HTTP/1.1 404"));
    assert!(answers[1].contains("Signed in"));

    let transport = FakeTransport::new().reply(
        200,
        r#"{"access_token":"ya29.a","expires_in":3599,"refresh_token":"1//r","scope":"https://www.googleapis.com/auth/webmasters.readonly","token_type":"Bearer"}"#,
    );
    let tokens = exchange(&transport, &client(), &code).unwrap();
    assert_eq!(tokens.refresh_token.as_deref(), Some("1//r"));
    assert!(tokens.has_scope(scopes::SEARCH_CONSOLE));
    assert!(!tokens.has_scope(scopes::ADS));
    let body = String::from_utf8(transport.requests()[0].body.clone().unwrap()).unwrap();
    assert!(body.contains("grant_type=authorization_code"));
    assert!(body.contains("code=4%2Fabc"));
    assert!(body.contains("code_verifier="));
    assert!(body.contains(&format!(
        "redirect_uri={}",
        url::form_urlencoded::byte_serialize(redirect.as_bytes()).collect::<String>()
    )));
    assert!(!format!("{tokens:?}").contains("ya29"));
}

#[test]
fn refuses_foreign_states_denials_and_timeouts() {
    let authorization = begin(&client(), &[scopes::SEARCH_CONSOLE]).unwrap();
    let redirect = authorization.redirect_uri().to_owned();
    let stranger = visit(&redirect, &["/?state=other&code=x"]);
    assert_eq!(
        authorization.wait(Duration::from_secs(10), &|| false),
        Err(OAuthError::StateMismatch)
    );
    stranger.join().unwrap();

    let authorization = begin(&client(), &[scopes::SEARCH_CONSOLE]).unwrap();
    let state = query(&authorization.url, "state");
    let redirect = authorization.redirect_uri().to_owned();
    let denied = visit(
        &redirect,
        &[&format!("/?state={state}&error=access_denied")],
    );
    assert_eq!(
        authorization.wait(Duration::from_secs(10), &|| false),
        Err(OAuthError::Denied("access_denied".to_owned()))
    );
    assert!(denied.join().unwrap()[0].contains("not completed"));

    let authorization = begin(&client(), &[scopes::SEARCH_CONSOLE]).unwrap();
    assert_eq!(
        authorization.wait(Duration::from_millis(150), &|| false),
        Err(OAuthError::Timeout)
    );
    let authorization = begin(&client(), &[scopes::SEARCH_CONSOLE]).unwrap();
    assert_eq!(
        authorization.wait(Duration::from_secs(10), &|| true),
        Err(OAuthError::Cancelled)
    );
}

#[test]
fn refreshes_and_reports_an_expired_grant() {
    let transport = FakeTransport::new()
        .reply(200, r#"{"access_token":"ya29.b","expires_in":3599}"#)
        .reply(
            400,
            r#"{"error":"invalid_grant","error_description":"Token has been expired or revoked."}"#,
        )
        .reply(200, "");
    let tokens = refresh(&transport, &client(), "1//r").unwrap();
    assert_eq!(tokens.access_token, "ya29.b");
    assert_eq!(
        refresh(&transport, &client(), "1//r"),
        Err(OAuthError::Expired)
    );
    revoke(&transport, "1//r").unwrap();
    assert!(transport.requests()[2].url.starts_with(REVOKE_URL));
}
