//! Pausing and stopping a running crawl from another thread.

use std::sync::atomic::{AtomicBool, Ordering};

/// Shared switches of a running crawl; share it with an `Arc` or a scoped reference.
#[derive(Debug, Default)]
pub struct CrawlControl {
    stop: AtomicBool,
    pause: AtomicBool,
}

impl CrawlControl {
    /// A control that neither stops nor pauses.
    pub fn new() -> Self {
        Self::default()
    }

    /// Stops the crawl: requests in flight finish and are recorded, nothing new starts.
    pub fn stop(&self) {
        self.stop.store(true, Ordering::Relaxed);
    }

    /// Whether [`stop`](Self::stop) was called.
    pub fn is_stopped(&self) -> bool {
        self.stop.load(Ordering::Relaxed)
    }

    /// Pauses or resumes starting new requests.
    pub fn set_paused(&self, paused: bool) {
        self.pause.store(paused, Ordering::Relaxed);
    }

    /// Whether the crawl is paused.
    pub fn is_paused(&self) -> bool {
        self.pause.load(Ordering::Relaxed)
    }
}
