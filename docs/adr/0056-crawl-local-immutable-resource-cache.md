# ADR 0056 — Reuse immutable JavaScript and CSS during a crawl

Status: Accepted

## Problem

The renderer isolates every URL in a new browser context and disables Chrome's cache. Shared Next.js chunks and stylesheets were downloaded for every page and each download consumed the shared request ceiling. In a saved Nexusmedya crawl, 65 rendered pages averaged 973 ms for HTTP and 31,566 ms for rendering including browser admission waits. Increasing HTTP workers alone would not resolve this bottleneck.

## Decision

Keep fresh per-URL contexts, cookies, storage and service-worker bypass. Add a bounded in-memory resource cache owned by one crawl's Chrome renderer. Cache only same-origin GET scripts and stylesheets with an HTTP 200 response, explicit `public`, `immutable`, a single valid positive `max-age` and a valid Date. Date and Age reduce remaining freshness. Preserve full URL identity and original response security headers. Remove transfer/content encoding and length headers when fulfilling the decoded body through DevTools.

Do not reuse navigations, HTML, XHR/fetch/API responses, cross-origin assets, credential/range/revalidation requests, responses setting cookies, private/no-store/no-cache responses, responses varying on anything other than Accept-Encoding, unsupported character encodings, or CSS with an explicit @charset. Cache misses and unavailable Chrome response bodies use normal networking; a cache population failure does not fail rendering. Entries expire and use least-recently-used eviction: 2 MiB per decoded body, 64 MiB retained content/headers/URL bytes, and 1,024 entries per crawl. Nothing is written to disk or carried to a later crawl.

Extend the renderer's request policy with an explicit cached-resource kind. Cache hits must still pass Stop, active deadline and internal robots checks. Only actual network requests reserve request-rate slots. Original crawl resource checks and HTTP metadata stay independent: a resource followed into the frontier still gets its own HTTP status check.

`rendering.cache_resources` defaults to true, including older partial settings when a new crawl is started. It can be disabled in rendering settings or the new/repeat dialog, JSON, or CLI `--no-render-cache`. Saved databases and metadata are not rewritten. No database schema migration is required. Promote the already-locked time dependency for HTTP Date parsing; no package version is upgraded.

## Validation

Compare eight JS-only linked pages loading 20 shared JavaScript chunks and one stylesheet, with five HTTP workers, a shared 5 requests/second limit, two browser slots and a 1,000 ms DOM wait. The original binary took 42,067 ms of crawl time and 177 network requests (168 assets). The cache-enabled implementation took 13,561 ms and 30 network requests (21 assets): approximately 3.1 times the crawl throughput and 87.5% fewer asset requests. Every page retained its expected rendered title and generated link; all eight rendered without errors. Browser startup is excluded from crawl time in both cases; end-to-end CLI measurements are retained separately in the QA report. These are local fixture measurements, not a guarantee for live websites.

Unit tests cover cache eligibility, freshness, eviction, byte limits, encodings, HTTP outcomes, request identity and policy. Real Chrome checks cover gzip script reuse, fresh API/no-store responses, isolated page state and a policy-blocked cache hit. Existing async rendering, viewport, queue timeout, cancellation and fallback checks remain required.
