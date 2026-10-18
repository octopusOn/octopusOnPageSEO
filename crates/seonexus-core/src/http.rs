//! The HTTP port: requests and responses as plain data, sent through an [`HttpTransport`].
//!
//! Domain crates that call web APIs (SERP providers, domain lookups) depend on this port only;
//! `seonexus-net` implements it, and tests replace it with canned responses.

use std::fmt;
use std::time::Duration;

/// Request method.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HttpMethod {
    /// GET.
    Get,
    /// POST with a body.
    Post,
}

impl HttpMethod {
    /// Upper-case name.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Get => "GET",
            Self::Post => "POST",
        }
    }
}

/// A request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpRequest {
    /// Method.
    pub method: HttpMethod,
    /// Absolute URL.
    pub url: String,
    /// Headers in order.
    pub headers: Vec<(String, String)>,
    /// Body of a POST.
    pub body: Option<Vec<u8>>,
    /// Time limit of the whole exchange, overriding the transport's default.
    pub timeout: Option<Duration>,
}

impl HttpRequest {
    /// A GET request.
    pub fn get(url: impl Into<String>) -> Self {
        Self {
            method: HttpMethod::Get,
            url: url.into(),
            headers: Vec::new(),
            body: None,
            timeout: None,
        }
    }

    /// A POST request with a body.
    pub fn post(url: impl Into<String>, body: Vec<u8>) -> Self {
        Self {
            method: HttpMethod::Post,
            url: url.into(),
            headers: Vec::new(),
            body: Some(body),
            timeout: None,
        }
    }

    /// Adds a header.
    #[must_use]
    pub fn header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.push((name.into(), value.into()));
        self
    }
}

/// A response, whatever its status.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpResponse {
    /// Status code.
    pub status: u16,
    /// Headers in order, names lowercase.
    pub headers: Vec<(String, String)>,
    /// Body.
    pub body: Vec<u8>,
    /// URL the body came from (after followed redirects).
    pub url: String,
}

impl HttpResponse {
    /// The first value of a header, by case-insensitive name.
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }

    /// Whether the status is 2xx.
    pub fn is_success(&self) -> bool {
        (200..300).contains(&self.status)
    }

    /// The body as UTF-8, invalid sequences replaced.
    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }

    /// Seconds from a `Retry-After` header given in seconds.
    pub fn retry_after(&self) -> Option<u64> {
        self.header("retry-after")?.trim().parse().ok()
    }
}

/// Why no response arrived.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransportError {
    /// The time limit passed.
    Timeout,
    /// DNS, connection or TLS failure.
    Connect(String),
    /// The body was larger than allowed.
    TooLarge,
    /// The request could not be built (invalid URL or header).
    Invalid(String),
    /// Anything else.
    Other(String),
}

impl fmt::Display for TransportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Timeout => f.write_str("the request timed out"),
            Self::Connect(message) => write!(f, "cannot connect: {message}"),
            Self::TooLarge => f.write_str("the response is too large"),
            Self::Invalid(message) => write!(f, "invalid request: {message}"),
            Self::Other(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for TransportError {}

/// Something that sends requests.
pub trait HttpTransport: Send + Sync {
    /// Sends a request and returns the response, whatever its status.
    ///
    /// # Errors
    /// [`TransportError`] when no response arrived.
    fn send(&self, request: &HttpRequest) -> Result<HttpResponse, TransportError>;

    /// US dollars a metered transport (OctopusOn SEO credits) charged since the last call;
    /// `None` when nothing was metered.
    fn take_charge_usd(&self) -> Option<f64> {
        None
    }
}

pub mod testing {
    //! A transport that answers from a queue of canned responses and records requests.

    use std::collections::VecDeque;
    use std::sync::{Mutex, PoisonError};

    use super::{HttpRequest, HttpResponse, HttpTransport, TransportError};

    type Reply = Result<(u16, Vec<(String, String)>, Vec<u8>), TransportError>;

    /// Canned responses in order; every request is recorded.
    #[derive(Debug, Default)]
    pub struct FakeTransport {
        replies: Mutex<VecDeque<Reply>>,
        requests: Mutex<Vec<HttpRequest>>,
    }

    impl FakeTransport {
        /// A transport without responses.
        pub fn new() -> Self {
            Self::default()
        }

        /// Queues a response.
        #[must_use]
        pub fn reply(self, status: u16, body: impl Into<Vec<u8>>) -> Self {
            self.reply_with_headers(status, Vec::new(), body)
        }

        /// Queues a response with headers.
        #[must_use]
        pub fn reply_with_headers(
            self,
            status: u16,
            headers: Vec<(&str, &str)>,
            body: impl Into<Vec<u8>>,
        ) -> Self {
            let headers = headers
                .into_iter()
                .map(|(name, value)| (name.to_ascii_lowercase(), value.to_owned()))
                .collect();
            self.replies
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .push_back(Ok((status, headers, body.into())));
            self
        }

        /// Queues a failure.
        #[must_use]
        pub fn fail(self, error: TransportError) -> Self {
            self.replies
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .push_back(Err(error));
            self
        }

        /// Requests sent so far.
        pub fn requests(&self) -> Vec<HttpRequest> {
            self.requests
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .clone()
        }
    }

    impl HttpTransport for FakeTransport {
        fn send(&self, request: &HttpRequest) -> Result<HttpResponse, TransportError> {
            self.requests
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .push(request.clone());
            let reply = self
                .replies
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .pop_front()
                .unwrap_or_else(|| {
                    Err(TransportError::Other("no canned response left".to_owned()))
                });
            reply.map(|(status, headers, body)| HttpResponse {
                status,
                headers,
                body,
                url: request.url.clone(),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_headers_and_retry_after() {
        let response = HttpResponse {
            status: 429,
            headers: vec![("retry-after".to_owned(), " 30 ".to_owned())],
            body: b"slow down".to_vec(),
            url: "https://api.example/".to_owned(),
        };
        assert_eq!(response.header("Retry-After"), Some(" 30 "));
        assert_eq!(response.retry_after(), Some(30));
        assert!(!response.is_success());
        assert_eq!(response.text(), "slow down");
        let request = HttpRequest::post("https://api.example/", b"{}".to_vec()).header("a", "b");
        assert_eq!(request.method.as_str(), "POST");
        assert_eq!(request.headers.len(), 1);
    }
}
