# ADR 0058 — Coordinate shared resource downloads across browser pages

Status: Accepted

## Context

The crawl-local immutable cache helps later pages, but simultaneously admitted pages can all miss an empty cache and download the same framework chunks. A controlled eight-URL list crawl, with eight HTTP workers, four browser pages, 10 requests/second and a 1,000 ms DOM wait, downloaded the 21 shared assets four times before the cache warmed.

## Decision

Coordinate pending eligible same-origin script/CSS requests in the existing renderer. A request owns a bounded, RAII-managed pending-download lease. Other pages can temporarily defer that URL while continuing to receive DevTools events, collect completed responses and process unrelated requests. Reuse still requires the existing successful, fresh, public immutable response checks; a pending download by itself never authorizes sharing a body.

Each page holds at most 128 deferred requests; at most 1,024 distinct downloads are coordinated per crawl, with URLs up to 2,048 bytes. Longer URLs retain ordinary networking. A deferred request's sharing deadline is the smaller of two seconds and a quarter of its remaining active-page budget. When that deadline is serviced, or the owner finishes without a reusable body, the page performs its own policy-checked network request. Private, non-cacheable or rejected response headers release waiters before the owner's body completes. Owners use [Fetch response-stage interception](https://github.com/ChromeDevTools/devtools-protocol/blob/master/pdl/domains/Fetch.pdl) to inspect headers before Chrome's body/MIME buffering, continuing the original response without modifying headers or body. Failed requests, denied policy, cancellation, timeouts and session disposal release leases. Waiting never holds the cache/download mutex, and readiness also requires an empty deferred queue.

Check the cache again before network dispatch, including after a rate wait: a concurrent fill may have completed meanwhile. Every reused response still passes robots, Stop and active-deadline checks. Actual network dispatch retains the shared request ceiling; a previously reserved slot can remain unused when a late cache hit avoids that download. Keep page contexts, cookies, storage, service-worker bypass, body caps, original HTTP observations and DOM readiness rules unchanged.

The existing resource-cache switch controls this optimization. No new setting, dependency, archive schema or historical data rewrite is introduced. Only public immutable resources can be shared; response latency and cache headers determine the gain. The bounded wait may fall back to duplicate networking, avoiding a dependency on another page's full render budget.

## Validation

The original parallel fixture completed in 11,314 ms with 84 asset requests. The verified implementation completed in 5,316 ms with 21 asset requests, at identical settings: approximately 2.13 times the crawl throughput and 75% fewer asset downloads. All eight JavaScript-generated titles and hyperlinks were retained with no render/request failures. Intermediate coordinated runs completed in 5,633–5,764 ms with 23–24 asset requests; bounded fallback intentionally permits duplicate networking. These are local measurements, excluding browser startup, not a live-site guarantee.

The sequential JavaScript-only link-discovery fixture still completed in 13,437 ms with 21 asset requests and all eight generated titles. The installed desktop app displayed the completed parallel archive, including its generated titles, eight indexable pages and five-second crawl summary. Temporary QA archives were removed after preserving reports.

Unit tests cover lease completion, later generations, capacity/URL bounds and cleanup. Real Chrome regressions cover public asset sharing, distinct no-store responses, immediate release on non-cacheable headers, per-page storage isolation, cancellation of an owner while another page waits, bounded fallback while an owner remains active, and a failed fill followed by a successful independent response. All nine opt-in browser tests, 632 default Rust tests, 156 frontend checks, workspace formatting and strict Clippy passed. Existing queue budgets, request policy, gzip cache, async DOM, truncation, Stop and viewport checks remain required.
