//! Share a pending cache fill without blocking another page's DevTools event loop.

use std::collections::HashMap;
use std::sync::{
    Arc, Mutex, PoisonError,
    atomic::{AtomicBool, Ordering},
};

const MAX_PENDING: usize = 1024;
const MAX_URL_BYTES: usize = 2048;

#[derive(Default)]
pub(crate) struct Downloads(Mutex<HashMap<String, Arc<Flight>>>);

#[derive(Default)]
pub(crate) struct Flight(AtomicBool);

impl Flight {
    pub(crate) fn finished(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}

pub(crate) enum Admission<'a> {
    /// Another page is fetching this URL; its response may or may not be cacheable.
    Wait(Arc<Flight>),
    /// Start normal networking, tracking a fill only while bounded capacity is available.
    Network(Option<Lease<'a>>),
}

pub(crate) struct Lease<'a> {
    downloads: &'a Downloads,
    url: String,
    flight: Arc<Flight>,
}

impl Downloads {
    #[cfg(test)]
    pub(crate) fn has_waiter(&self, url: &str) -> bool {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(url)
            .is_some_and(|flight| Arc::strong_count(flight) > 2)
    }

    pub(crate) fn reserve(&self, url: &str) -> Admission<'_> {
        if url.len() > MAX_URL_BYTES {
            return Admission::Network(None);
        }
        let mut pending = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(flight) = pending.get(url) {
            return Admission::Wait(Arc::clone(flight));
        }
        if pending.len() >= MAX_PENDING {
            return Admission::Network(None);
        }
        let flight = Arc::new(Flight::default());
        pending.insert(url.to_owned(), Arc::clone(&flight));
        Admission::Network(Some(Lease {
            downloads: self,
            url: url.to_owned(),
            flight,
        }))
    }
}

impl Drop for Lease<'_> {
    fn drop(&mut self) {
        let mut pending = self
            .downloads
            .0
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if pending
            .get(&self.url)
            .is_some_and(|current| Arc::ptr_eq(current, &self.flight))
        {
            pending.remove(&self.url);
        }
        // Completion means "retry the cache or use your own network request", never
        // permission to reuse private/failed/unsupported responses.
        self.flight.0.store(true, Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_dropped_owner_releases_waiters_and_allows_a_new_generation() {
        let downloads = Downloads::default();
        let Admission::Network(Some(owner)) = downloads.reserve("/app.js") else {
            panic!("first owner")
        };
        let Admission::Wait(first) = downloads.reserve("/app.js") else {
            panic!("shared request")
        };
        assert!(!first.finished());
        drop(owner);
        assert!(first.finished());
        let Admission::Network(Some(second)) = downloads.reserve("/app.js") else {
            panic!("new owner")
        };
        let Admission::Wait(next) = downloads.reserve("/app.js") else {
            panic!("new waiter")
        };
        assert!(first.finished());
        assert!(!next.finished());
        drop(second);
        assert!(next.finished());
        assert!(downloads.0.lock().unwrap().is_empty());
    }

    #[test]
    fn pending_urls_are_bounded_without_restricting_unrelated_networking() {
        let downloads = Downloads::default();
        let mut owners = Vec::new();
        for n in 0..MAX_PENDING {
            let Admission::Network(Some(owner)) = downloads.reserve(&n.to_string()) else {
                panic!("available capacity")
            };
            owners.push(owner);
        }
        assert!(matches!(
            downloads.reserve("overflow"),
            Admission::Network(None)
        ));
        assert!(matches!(
            downloads.reserve(&"x".repeat(MAX_URL_BYTES + 1)),
            Admission::Network(None)
        ));
        assert!(matches!(downloads.reserve("0"), Admission::Wait(_)));
        drop(owners);
        assert!(downloads.0.lock().unwrap().is_empty());
    }
}
