use std::time::Duration;

use ureq::Agent;

/// Largest accepted response body.
const MAX_BODY_BYTES: u64 = 32 * 1024 * 1024;

/// A download failed.
#[derive(Debug, thiserror::Error)]
#[error("cannot download `{url}`: {message}")]
pub struct NetError {
    pub(crate) url: String,
    pub(crate) message: String,
}

/// Blocking HTTP client for small text documents.
#[derive(Debug, Clone)]
pub struct HttpClient {
    agent: Agent,
}

impl HttpClient {
    /// Creates a client sending `user_agent` and giving up after `timeout` per request.
    pub fn new(user_agent: &str, timeout: Duration) -> Self {
        let config = Agent::config_builder()
            .user_agent(user_agent)
            .timeout_global(Some(timeout))
            .build();
        Self {
            agent: Agent::new_with_config(config),
        }
    }

    /// Downloads `url` as raw bytes (e.g. gzip-compressed sitemaps). Non-2xx statuses are
    /// errors.
    ///
    /// # Errors
    /// Network, TLS and status failures, or bodies over 32 MiB.
    pub fn get_bytes(&self, url: &str) -> Result<Vec<u8>, NetError> {
        let error = |message: String| NetError {
            url: url.to_owned(),
            message,
        };
        let mut response = self
            .agent
            .get(url)
            .call()
            .map_err(|e| error(e.to_string()))?;
        response
            .body_mut()
            .with_config()
            .limit(MAX_BODY_BYTES)
            .read_to_vec()
            .map_err(|e| error(e.to_string()))
    }

    /// Downloads `url` allowing bodies up to `limit` bytes (large data files).
    ///
    /// # Errors
    /// Network, TLS and status failures, or bodies over `limit`.
    pub fn get_bytes_up_to(&self, url: &str, limit: u64) -> Result<Vec<u8>, NetError> {
        let error = |message: String| NetError {
            url: url.to_owned(),
            message,
        };
        let mut response = self
            .agent
            .get(url)
            .call()
            .map_err(|e| error(e.to_string()))?;
        response
            .body_mut()
            .with_config()
            .limit(limit)
            .read_to_vec()
            .map_err(|e| error(e.to_string()))
    }

    /// Downloads `url` as text. Non-2xx statuses are errors.
    ///
    /// # Errors
    /// Network, TLS, status and decoding failures, or bodies over 32 MiB.
    pub fn get_text(&self, url: &str) -> Result<String, NetError> {
        let error = |message: String| NetError {
            url: url.to_owned(),
            message,
        };
        let mut response = self
            .agent
            .get(url)
            .call()
            .map_err(|e| error(e.to_string()))?;
        response
            .body_mut()
            .with_config()
            .limit(MAX_BODY_BYTES)
            .read_to_string()
            .map_err(|e| error(e.to_string()))
    }
}
