# ADR 0011 — Bounded URL activity during crawls

Status: Accepted

## Context

The desktop previously received counters only. While a large JavaScript crawl ran, users could not see which URLs were assigned or distinguish a slow HTTP request, rendering failure and final analysis. Full database tables remained unavailable until completion. Reloading the frontend could also lose its selected active job.

## Decision

- The crawl domain exposes `CrawlSink::activity`, separately from the existing copyable progress counters. The coordinator correlates worker assignments and outcomes with per-crawl operation IDs, including robots, sitemap and site-file work. Observations are produced on the coordinator thread with no UI or network dependencies.
- Snapshots contain active assignments (bounded by the maximum 64 workers), the latest 100 completions, and lifetime render/block counters. Bodies, headers and link graphs are excluded. URL/title/error strings are bounded and truncated on Unicode boundaries. Processing time includes assigned-worker waits, while the original HTTP time remains separate.
- The shared application use case has an observed API for identity, phase, activity and counters; existing progress-only callers remain supported. Finalization includes flushing, audits and durable save, and is visibly distinct from network crawling.
- Tauri jobs carry the saved crawl ID, activity and monotonic revision. Counters publish the preceding activity snapshot once per reporting interval. Terminal jobs release activity rows, preserve their final metadata, and retain the newest 64 completions for reconnects. Stopped results remain stopped.
- The frontend reconciles polling with newer events, reconnects to the job of a selected running crawl, and provides current assignments plus a searchable/filterable recent-operation table. HTTP/network errors, JS fallback and robots blocks are distinguished. Full historical results still use the saved database after completion.
- Native window-close and application-exit requests are prevented while any job is active or finalizing, with a localized explanation. This guard does not prevent force termination, OS failures or restore a lost frontier.
- On macOS the predefined Quit item sends `NSApplication terminate:` directly. Its menu item and accelerator are replaced with a regular action that checks active crawls before requesting a cancellable Tauri exit.

## Validation

Domain tests exercise bounded history, Unicode truncation, assignment correlation and HTTP/robots outcomes. Application tests exercise identity/phase notifications, stopped persistence and DOM-derived activity/fallback diagnostics. Native job tests cover lifecycle revisions, finalization controls and retention. Frontend tests cover polling/event reconciliation, literal filters, rendering diagnostics, escaping remote text and finalization controls. A real native UI crawl is required to verify the event-to-screen path and close protection.
