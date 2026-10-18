use std::time::Duration;

use seonexus_core::http::{HttpMethod, HttpRequest, HttpResponse, HttpTransport, TransportError};
use ureq::{Agent, Proxy};

use crate::NetError;

/// Largest accepted API response body.
const MAX_BODY_BYTES: u64 = 32 * 1024 * 1024;

/// Redirects followed within one origin.
const MAX_REDIRECTS: usize = 5;

/// HTTP adapter of the [`HttpTransport`] port for web APIs.
///
/// Every status is returned as a response. Redirects are followed only within the origin of
/// the request, so headers carrying API keys never reach another host; a redirect elsewhere
/// is returned as it is.
#[derive(Debug, Clone)]
pub struct ApiClient {
    agent: Agent,
}

impl ApiClient {
    /// A client sending `user_agent`, giving up after `timeout` per request, optionally
    /// through an HTTP(S) proxy (`http://host:port`, with `user:password@` if needed).
    ///
    /// # Errors
    /// [`NetError`] for an invalid proxy address.
    pub fn new(user_agent: &str, timeout: Duration, proxy: Option<&str>) -> Result<Self, NetError> {
        let proxy = proxy
            .map(str::trim)
            .filter(|proxy| !proxy.is_empty())
            .map(|address| {
                Proxy::new(address).map_err(|error| NetError {
                    url: address.to_owned(),
                    message: format!("invalid proxy: {error}"),
                })
            })
            .transpose()?;
        let config = Agent::config_builder()
            .user_agent(user_agent)
            .timeout_global(Some(timeout))
            .max_redirects(0)
            .max_redirects_will_error(false)
            .http_status_as_error(false)
            .proxy(proxy)
            .build();
        Ok(Self {
            agent: Agent::new_with_config(config),
        })
    }

    fn send_once(&self, request: &HttpRequest, url: &str) -> Result<HttpResponse, TransportError> {
        let mut response = match request.method {
            HttpMethod::Get => {
                let mut call = self.agent.get(url);
                for (name, value) in &request.headers {
                    call = call.header(name.as_str(), value.as_str());
                }
                if let Some(timeout) = request.timeout {
                    call = call.config().timeout_global(Some(timeout)).build();
                }
                call.call()
            }
            HttpMethod::Post => {
                let mut call = self.agent.post(url);
                for (name, value) in &request.headers {
                    call = call.header(name.as_str(), value.as_str());
                }
                if let Some(timeout) = request.timeout {
                    call = call.config().timeout_global(Some(timeout)).build();
                }
                call.send(request.body.as_deref().unwrap_or_default())
            }
        }
        .map_err(transport_error)?;
        let headers = response
            .headers()
            .iter()
            .map(|(name, value)| {
                (
                    name.as_str().to_ascii_lowercase(),
                    String::from_utf8_lossy(value.as_bytes()).into_owned(),
                )
            })
            .collect();
        let status = response.status().as_u16();
        let body = response
            .body_mut()
            .with_config()
            .limit(MAX_BODY_BYTES)
            .read_to_vec()
            .map_err(|error| match error {
                ureq::Error::BodyExceedsLimit(_) => TransportError::TooLarge,
                other => transport_error(other),
            })?;
        Ok(HttpResponse {
            status,
            headers,
            body,
            url: url.to_owned(),
        })
    }
}

impl HttpTransport for ApiClient {
    fn send(&self, request: &HttpRequest) -> Result<HttpResponse, TransportError> {
        let origin = url::Url::parse(&request.url)
            .map_err(|error| TransportError::Invalid(format!("{}: {error}", request.url)))?;
        let mut current = origin.clone();
        for _ in 0..=MAX_REDIRECTS {
            let response = self.send_once(request, current.as_str())?;
            let next = (300..400)
                .contains(&response.status)
                .then(|| response.header("location"))
                .flatten()
                .and_then(|location| current.join(location).ok())
                .filter(|next| next.origin() == origin.origin());
            match next {
                // Only GET is repeated; a redirected POST is reported to the caller.
                Some(next) if request.method == HttpMethod::Get => current = next,
                _ => return Ok(response),
            }
        }
        Err(TransportError::Other(format!(
            "more than {MAX_REDIRECTS} redirects from {}",
            request.url
        )))
    }
}

fn transport_error(error: ureq::Error) -> TransportError {
    match error {
        ureq::Error::Timeout(_) => TransportError::Timeout,
        ureq::Error::HostNotFound => TransportError::Connect("host not found".to_owned()),
        ureq::Error::ConnectionFailed => TransportError::Connect("connection refused".to_owned()),
        ureq::Error::Io(error) => TransportError::Connect(error.to_string()),
        ureq::Error::Tls(message) => TransportError::Connect(format!("TLS: {message}")),
        ureq::Error::Rustls(error) => TransportError::Connect(format!("TLS: {error}")),
        ureq::Error::BadUri(message) => TransportError::Invalid(message),
        other => TransportError::Other(other.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use std::io::{BufRead, BufReader, Read, Write};
    use std::net::TcpListener;
    use std::sync::mpsc;
    use std::thread;

    use super::*;

    /// Serves one canned response per connection, sends each raw request to the channel and
    /// returns the base URL.
    fn serve(responses: Vec<String>) -> (String, mpsc::Receiver<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let (sender, receiver) = mpsc::channel();
        thread::spawn(move || {
            for (stream, response) in listener.incoming().zip(responses) {
                let mut stream = stream.unwrap();
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut head = String::new();
                let mut length = 0;
                loop {
                    let mut line = String::new();
                    reader.read_line(&mut line).unwrap();
                    if let Some(value) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                        length = value.trim().parse().unwrap();
                    }
                    head.push_str(&line);
                    if line == "\r\n" {
                        break;
                    }
                }
                let mut body = vec![0; length];
                reader.read_exact(&mut body).unwrap();
                head.push_str(&String::from_utf8_lossy(&body));
                sender.send(head).unwrap();
                stream.write_all(response.as_bytes()).unwrap();
            }
        });
        (format!("http://{address}"), receiver)
    }

    fn response(status: &str, headers: &str, body: &str) -> String {
        format!(
            "HTTP/1.1 {status}\r\n{headers}Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
    }

    fn client() -> ApiClient {
        ApiClient::new("octopusSEO-test", Duration::from_secs(5), None).unwrap()
    }

    #[test]
    fn posts_bodies_and_returns_error_statuses() {
        let (base, requests) = serve(vec![response(
            "429 Too Many Requests",
            "Retry-After: 7\r\n",
            r#"{"message":"slow"}"#,
        )]);
        let request = HttpRequest::post(format!("{base}/search"), br#"{"q":"kahve"}"#.to_vec())
            .header("X-API-KEY", "secret")
            .header("Content-Type", "application/json");
        let reply = client().send(&request).unwrap();
        assert_eq!(reply.status, 429);
        assert_eq!(reply.retry_after(), Some(7));
        assert_eq!(reply.text(), r#"{"message":"slow"}"#);
        let raw = requests.recv().unwrap();
        assert!(raw.starts_with("POST /search HTTP/1.1"));
        assert!(raw.to_ascii_lowercase().contains("x-api-key: secret"));
        assert!(raw.ends_with(r#"{"q":"kahve"}"#));
    }

    #[test]
    fn follows_redirects_only_within_the_origin() {
        let (base, requests) = serve(vec![
            response("302 Found", "Location: /final\r\n", ""),
            response("200 OK", "", "done"),
            response("302 Found", "Location: http://elsewhere.invalid/x\r\n", ""),
        ]);
        let first = client()
            .send(&HttpRequest::get(format!("{base}/start")).header("X-API-KEY", "k"))
            .unwrap();
        assert_eq!(first.status, 200);
        assert!(first.url.ends_with("/final"));
        assert_eq!(first.text(), "done");
        let _ = requests.recv().unwrap();
        assert!(requests.recv().unwrap().starts_with("GET /final"));

        let away = client()
            .send(&HttpRequest::get(format!("{base}/away")).header("X-API-KEY", "k"))
            .unwrap();
        assert_eq!(away.status, 302);
        assert_eq!(away.header("location"), Some("http://elsewhere.invalid/x"));
    }

    #[test]
    fn rejects_invalid_proxies_and_urls() {
        assert!(ApiClient::new("ua", Duration::from_secs(1), Some("not a proxy")).is_err());
        assert!(ApiClient::new("ua", Duration::from_secs(1), Some("  ")).is_ok());
        assert!(matches!(
            client().send(&HttpRequest::get("no scheme")),
            Err(TransportError::Invalid(_))
        ));
    }
}
