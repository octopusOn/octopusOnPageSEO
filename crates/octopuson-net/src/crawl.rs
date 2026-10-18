use std::io::Read;
use std::time::{Duration, Instant};

use octopuson_crawl::{FetchError, FetchRequest, FetchResponse, Fetcher};
use ureq::Agent;

/// What crawl requests accept, like a browser.
const ACCEPT: &str = "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8";

/// HTTP adapter of the Spider's [`Fetcher`] port: one request per call, redirects reported
/// instead of followed, every status returned as a response.
#[derive(Debug, Clone)]
pub struct CrawlFetcher {
    agent: Agent,
    accept_language: Option<String>,
}

impl CrawlFetcher {
    /// A fetcher sending `user_agent` and giving up after `timeout` per request.
    pub fn new(user_agent: &str, timeout: Duration, accept_language: Option<String>) -> Self {
        let config = Agent::config_builder()
            .user_agent(user_agent)
            .accept(ACCEPT)
            .timeout_global(Some(timeout))
            .max_redirects(0)
            .max_redirects_will_error(false)
            .http_status_as_error(false)
            .max_idle_connections_per_host(16)
            .build();
        Self {
            agent: Agent::new_with_config(config),
            accept_language,
        }
    }
}

impl Fetcher for CrawlFetcher {
    fn fetch(&self, request: &FetchRequest<'_>) -> Result<FetchResponse, FetchError> {
        let started = Instant::now();
        let mut call = self.agent.get(request.url);
        if let Some(language) = &self.accept_language {
            call = call.header("Accept-Language", language);
        }
        let mut response = call.call().map_err(fetch_error)?;
        let response_time = started.elapsed();
        let headers = response
            .headers()
            .iter()
            .map(|(name, value)| {
                (
                    name.as_str().to_owned(),
                    String::from_utf8_lossy(value.as_bytes()).into_owned(),
                )
            })
            .collect();
        let mut fetched = FetchResponse {
            status: response.status().as_u16(),
            headers,
            body: Vec::new(),
            truncated: false,
            http_version: format!("{:?}", response.version()),
            response_time,
        };
        if request.body_limit > 0 {
            let reader = response.body_mut().with_config().limit(u64::MAX).reader();
            let mut limited = reader.take(request.body_limit.saturating_add(1));
            // A connection dropped mid-body keeps what arrived and counts as cut short.
            let complete = limited.read_to_end(&mut fetched.body).is_ok();
            let limit = usize::try_from(request.body_limit).unwrap_or(usize::MAX);
            fetched.truncated = !complete || fetched.body.len() > limit;
            fetched.body.truncate(limit);
        }
        Ok(fetched)
    }
}

fn fetch_error(error: ureq::Error) -> FetchError {
    match error {
        ureq::Error::Timeout(_) => FetchError::Timeout,
        ureq::Error::HostNotFound => FetchError::Dns,
        ureq::Error::ConnectionFailed => FetchError::Connection("refused".to_owned()),
        ureq::Error::Io(error) => FetchError::Connection(error.to_string()),
        ureq::Error::Tls(message) => FetchError::Tls(message.to_owned()),
        ureq::Error::Rustls(error) => FetchError::Tls(error.to_string()),
        other => FetchError::Other(other.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use std::io::{BufRead, BufReader, Write};
    use std::net::TcpListener;
    use std::thread;

    use super::*;

    /// Serves one canned response per connection and returns the base URL.
    fn serve(responses: Vec<&'static str>) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        thread::spawn(move || {
            for (stream, response) in listener.incoming().zip(responses) {
                let mut stream = stream.unwrap();
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut line = String::new();
                while reader.read_line(&mut line).unwrap() > 2 {
                    line.clear();
                }
                stream.write_all(response.as_bytes()).unwrap();
            }
        });
        format!("http://{address}")
    }

    #[test]
    fn reports_redirects_and_errors_as_responses() {
        let base = serve(vec![
            "HTTP/1.1 301 Moved Permanently\r\nLocation: /new\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
            "HTTP/1.1 404 Not Found\r\nContent-Type: text/html\r\nX-Robots-Tag: noindex\r\nContent-Length: 4\r\nConnection: close\r\n\r\nnope",
            "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: 10\r\nConnection: close\r\n\r\n0123456789",
        ]);
        let fetcher = CrawlFetcher::new("test-agent", Duration::from_secs(5), None);
        let url = format!("{base}/old");
        let redirect = fetcher
            .fetch(&FetchRequest {
                url: &url,
                body_limit: 1024,
            })
            .unwrap();
        assert_eq!(redirect.status, 301);
        assert_eq!(redirect.header("location"), Some("/new"));

        let missing = fetcher
            .fetch(&FetchRequest {
                url: &url,
                body_limit: 1024,
            })
            .unwrap();
        assert_eq!(
            (missing.status, missing.body.as_slice()),
            (404, b"nope".as_slice())
        );
        assert_eq!(missing.header("x-robots-tag"), Some("noindex"));

        let cut = fetcher
            .fetch(&FetchRequest {
                url: &url,
                body_limit: 4,
            })
            .unwrap();
        assert_eq!(cut.body, b"0123");
        assert!(cut.truncated);
    }

    #[test]
    fn unreachable_hosts_are_errors() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        drop(listener);
        let fetcher = CrawlFetcher::new("test-agent", Duration::from_secs(5), None);
        let url = format!("http://{address}/");
        let error = fetcher
            .fetch(&FetchRequest {
                url: &url,
                body_limit: 0,
            })
            .unwrap_err();
        assert!(matches!(error, FetchError::Connection(_)), "{error:?}");
    }
}
