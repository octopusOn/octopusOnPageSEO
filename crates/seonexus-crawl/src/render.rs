//! Optional browser rendering port, independent of browser/process/network adapters.

use std::time::{Duration, Instant};

use crate::{CrawlControl, FetchError, FetchResponse};

/// What the browser is about to load.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrowserRequest {
    /// A document navigation, subject to crawl scope and the network request ceiling.
    Document,
    /// A subresource fetched from the network.
    Resource,
    /// A public immutable resource reused locally; still subject to robots policy.
    CachedResource,
}

/// Crawl policy callback: target URL, request kind and active-page deadline.
/// The renderer supplies the deadline after acquiring a browser slot.
pub type RequestPolicy<'a> = dyn Fn(&str, BrowserRequest, Instant) -> bool + Sync + 'a;

/// A fetched HTML document to execute in its original HTTP origin.
pub struct RenderRequest<'a> {
    /// The original document URL.
    pub url: &'a str,
    /// The already fetched response; renderers must not fetch the main document again.
    pub response: &'a FetchResponse,
    /// Maximum UTF-8 bytes of serialized DOM returned.
    pub body_limit: u64,
    /// Optional crawl policy for browser HTTP requests, using the active-page deadline.
    pub request_allowed: Option<&'a RequestPolicy<'a>>,
    /// Stop signal, also checked while waiting for a browser slot.
    pub control: &'a CrawlControl,
}

/// The page after JavaScript execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderedPage {
    /// The serialized DOM, in UTF-8.
    pub html: String,
    /// The document URL, including changes from history/navigation APIs.
    pub url: String,
    /// Time spent waiting for a slot and rendering.
    pub elapsed: Duration,
    /// The serialized DOM exceeded the body limit.
    pub truncated: bool,
}

/// Renders HTML through an optional browser adapter.
pub trait Renderer: Send + Sync {
    /// Executes the page and returns the resulting DOM.
    ///
    /// # Errors
    /// Browser errors, navigation failures, cancellation or rendering timeouts.
    fn render(&self, request: &RenderRequest<'_>) -> Result<RenderedPage, FetchError>;
}
