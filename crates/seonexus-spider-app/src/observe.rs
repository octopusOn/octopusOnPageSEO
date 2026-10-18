//! Notifications shared by native clients; observations never contain complete page bodies.

use seonexus_crawl::{CrawlActivity, CrawlProgress};
use seonexus_crawl_store::CrawlMeta;
use serde::{Deserialize, Serialize};

/// The real phase of a running crawl, including work after the last HTTP request.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CrawlPhase {
    /// Validating configuration and starting the HTTP/browser adapters.
    #[default]
    Starting,
    /// Fetching, rendering and discovering URLs.
    Crawling,
    /// Flushing results, computing SEO audits and saving the crawl.
    Finalizing,
}

/// A synchronous observation emitted by the shared use case on its calling thread.
pub enum CrawlEvent<'a> {
    /// A persistent crawl exists; identifies the job across frontend reloads.
    Created(&'a CrawlMeta),
    /// The phase changed.
    Phase(CrawlPhase),
    /// A bounded activity snapshot, followed by its counters.
    Activity(&'a CrawlActivity),
    /// Current counters.
    Progress(&'a CrawlProgress),
}
