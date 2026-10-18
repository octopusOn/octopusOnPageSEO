//! Bounded, transient URL activity for a live crawl view.

use std::collections::{HashMap, VecDeque};

use serde::{Deserialize, Serialize};

/// Completed URL operations retained for live observers; full results belong in the store.
pub const RECENT_URL_LIMIT: usize = 100;

/// The operation assigned to a crawl worker.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActivityKind {
    /// An internal page, including optional JavaScript processing.
    Page,
    /// A referenced image, script, stylesheet or other resource.
    Resource,
    /// An external link status check.
    External,
    /// An origin's robots.txt.
    Robots,
    /// A sitemap document.
    Sitemap,
    /// A well-known site file.
    SiteFile,
}

/// The state of one assigned URL operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActivityState {
    /// Assigned to a worker, including rate-limit and browser waits.
    Processing,
    /// Finished without an HTTP, network or rendering error.
    Completed,
    /// An HTTP, network or rendering error occurred.
    Error,
    /// Skipped because robots.txt disallows it.
    Blocked,
    /// Cancelled before its HTTP request began, or while waiting for JavaScript work.
    Cancelled,
}

/// A small URL summary; never includes HTML, headers or the page's link graph.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UrlActivity {
    /// Unique assignment number within this crawl, also for non-page operations.
    pub id: u64,
    /// URL, bounded to 2,048 characters for the live view.
    pub url: String,
    /// What is being processed.
    pub kind: ActivityKind,
    /// Current outcome.
    pub state: ActivityState,
    /// Milliseconds from crawl start until assignment.
    pub started_ms: u64,
    /// Time assigned to the worker, including rate-limit and rendering waits.
    pub elapsed_ms: u64,
    /// Original HTTP status, when available.
    pub status: Option<u16>,
    /// Analyzed page title, bounded to 256 characters.
    pub title: Option<String>,
    /// Network or protocol error, bounded to 512 characters.
    pub error: Option<String>,
    /// Whether JavaScript processing succeeded.
    pub rendered: bool,
    /// JavaScript processing failure; the crawler falls back to original HTML.
    pub render_error: Option<String>,
    /// Original HTTP response time, separate from the total processing time.
    pub response_ms: Option<u32>,
    /// Response size, when known.
    pub size_bytes: Option<u64>,
}

/// A bounded snapshot for live observers, not persisted as crawl metadata.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct CrawlActivity {
    /// Assigned operations, oldest first; bounded by the worker count (at most 64).
    pub active: Vec<UrlActivity>,
    /// Most recent completed operations, newest first, at most [`RECENT_URL_LIMIT`].
    pub recent: Vec<UrlActivity>,
    /// Successfully rendered pages over the whole crawl.
    pub rendered: u32,
    /// Pages whose JavaScript processing failed over the whole crawl.
    pub render_errors: u32,
    /// HTTP, network or protocol failures, including site-file and sitemap operations.
    pub request_errors: u32,
    /// URLs blocked by robots.txt over the whole crawl.
    pub blocked: u32,
}

#[derive(Default)]
pub(crate) struct ActivityTracker {
    active: HashMap<u64, UrlActivity>,
    recent: VecDeque<UrlActivity>,
    rendered: u32,
    render_errors: u32,
    request_errors: u32,
    blocked: u32,
}

impl ActivityTracker {
    pub(crate) fn start(&mut self, id: u64, url: String, kind: ActivityKind, now: u64) {
        self.active.insert(
            id,
            UrlActivity {
                id,
                url: bounded(url, 2048),
                kind,
                state: ActivityState::Processing,
                started_ms: now,
                elapsed_ms: 0,
                status: None,
                title: None,
                error: None,
                rendered: false,
                render_error: None,
                response_ms: None,
                size_bytes: None,
            },
        );
    }

    pub(crate) fn finish(&mut self, id: u64, now: u64, describe: impl FnOnce(&mut UrlActivity)) {
        let Some(mut entry) = self.active.remove(&id) else {
            return;
        };
        entry.state = ActivityState::Completed;
        entry.elapsed_ms = now.saturating_sub(entry.started_ms);
        describe(&mut entry);
        // Match the saved audit's cancellation treatment: stopping is not a JS failure.
        if entry
            .render_error
            .as_deref()
            .is_some_and(|message| message.contains("cancelled"))
        {
            entry.state = ActivityState::Cancelled;
        }
        if entry.state == ActivityState::Completed
            && (entry.error.is_some()
                || entry.render_error.is_some()
                || entry.status.is_some_and(|s| s >= 400))
        {
            entry.state = ActivityState::Error;
        }
        entry.title = entry.title.map(|s| bounded(s, 256));
        entry.error = entry.error.map(|s| bounded(s, 512));
        entry.render_error = entry.render_error.map(|s| bounded(s, 512));
        self.rendered += u32::from(entry.rendered);
        self.render_errors +=
            u32::from(entry.render_error.is_some() && entry.state != ActivityState::Cancelled);
        self.request_errors +=
            u32::from(entry.error.is_some() || entry.status.is_some_and(|status| status >= 400));
        self.blocked += u32::from(entry.state == ActivityState::Blocked);
        self.recent.push_front(entry);
        self.recent.truncate(RECENT_URL_LIMIT);
    }

    pub(crate) fn snapshot(&self, now: u64) -> CrawlActivity {
        let mut active: Vec<_> = self.active.values().cloned().collect();
        active.sort_by_key(|entry| entry.id);
        for entry in &mut active {
            entry.elapsed_ms = now.saturating_sub(entry.started_ms);
        }
        CrawlActivity {
            active,
            recent: self.recent.iter().cloned().collect(),
            rendered: self.rendered,
            render_errors: self.render_errors,
            request_errors: self.request_errors,
            blocked: self.blocked,
        }
    }
}

fn bounded(mut value: String, limit: usize) -> String {
    if let Some((end, _)) = value.char_indices().nth(limit - 1)
        && value[end..].chars().nth(1).is_some()
    {
        value.truncate(end);
        value.push('…');
    }
    value
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_a_bounded_recent_window_and_lifetime_totals() {
        let mut tracker = ActivityTracker::default();
        for id in 0..250 {
            tracker.start(id, "https://example.com/".into(), ActivityKind::Page, id);
            tracker.finish(id, id + 10, |entry| entry.rendered = true);
        }
        let snapshot = tracker.snapshot(300);
        assert_eq!(snapshot.active, Vec::<UrlActivity>::new());
        assert_eq!(snapshot.recent.len(), RECENT_URL_LIMIT);
        assert_eq!(snapshot.recent[0].id, 249);
        assert_eq!(snapshot.recent[99].id, 150);
        assert_eq!(snapshot.rendered, 250);
    }

    #[test]
    fn assignments_are_distinct_and_diagnostics_are_bounded_utf8() {
        let mut tracker = ActivityTracker::default();
        for id in [2, 1] {
            tracker.start(id, "🙂".repeat(3000), ActivityKind::Page, 10);
        }
        let live = tracker.snapshot(25);
        assert_eq!(live.active.iter().map(|e| e.id).collect::<Vec<_>>(), [1, 2]);
        assert_eq!(live.active[0].elapsed_ms, 15);
        assert_eq!(live.active[0].url.chars().count(), 2048);
        tracker.finish(1, 30, |entry| {
            entry.status = Some(200);
            entry.title = Some("界".repeat(400));
            entry.render_error = Some("🙂".repeat(1000));
        });
        tracker.finish(2, 5, |entry| entry.state = ActivityState::Cancelled);
        let done = tracker.snapshot(40);
        assert_eq!(done.active, Vec::<UrlActivity>::new());
        assert_eq!(done.render_errors, 1);
        assert_eq!(done.recent[0].elapsed_ms, 0);
        assert_eq!(done.recent[1].state, ActivityState::Error);
        assert_eq!(done.recent[1].title.as_ref().unwrap().chars().count(), 256);
        assert_eq!(
            done.recent[1]
                .render_error
                .as_ref()
                .unwrap()
                .chars()
                .count(),
            512
        );
    }

    #[test]
    fn stopping_a_render_is_not_a_javascript_error() {
        let mut tracker = ActivityTracker::default();
        tracker.start(1, "https://example.com/".into(), ActivityKind::Page, 0);
        tracker.finish(1, 10, |entry| {
            entry.status = Some(200);
            entry.render_error = Some("rendering: rendering cancelled".into());
        });
        let done = tracker.snapshot(10);
        assert_eq!(done.recent[0].state, ActivityState::Cancelled);
        assert_eq!((done.render_errors, done.request_errors), (0, 0));
    }
}
