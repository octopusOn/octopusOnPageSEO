# ADR 0010 — JavaScript rendering through a local browser

- Status: accepted
- Date: 2026-10-17

## Context

The HTTP crawler cannot observe content, metadata or hyperlinks that a SPA creates after executing JavaScript. The CLI and desktop must analyze the same representation without putting browser/process/network dependencies in the crawl domain. HTTP status codes and redirect records must remain trustworthy.

## Decision

- The domain exposes a `Renderer` port alongside `Fetcher`. Rendering settings deserialize with defaults, so old configs and saved crawls continue to use server HTML. Only internal, successfully fetched HTML page tasks invoke the renderer; resources, external status checks, robots.txt, sitemaps and HTTP redirects retain their HTTP path.
- `seonexus-render` is a process/network adapter that starts an installed Chrome/Chromium/Edge in a temporary private profile and uses the [Chrome DevTools Protocol](https://chromedevtools.github.io/devtools-protocol/) over a loopback WebSocket. `tungstenite` 0.28 supplies blocking WebSocket framing/handshakes with no async runtime or WebSocket TLS dependency; the endpoint is always loopback. Browser discovery covers standard application locations and PATH, with a config/environment override.
- DevTools Fetch interception fulfills the initial main-document request from the crawler's already fetched, decompressed response. Transport encoding/length headers are removed when fulfilling that response; stored original headers and metrics stay unchanged. This avoids a duplicate request and preserves the URL origin and page CSP. Browser security/certificate checks are not disabled.
- A fresh context per URL isolates cookies, storage and service workers. Browser resources use normal Chrome networking with crawl rate/internal robots checks; document navigations also honor scope. Parallel pages are bounded separately from HTTP threads. Admission is FIFO and cancellable. Each page's execution budget starts after admission, so slow earlier pages cannot consume a queued page's timeout. The renderer passes that active-page deadline to the domain's request policy for rate-limited browser requests. Stop is checked while queuing and receiving protocol messages; contexts and slots are released on success/failure.
- DOMContentLoaded, a minimum configurable wait and 500 ms of finite-request network idle determine readiness. The UTF-8 DOM has the same body-size ceiling as fetched HTML. The resulting DOM, using its post-execution URL as the base, enters the existing parser/frontier/audit path.
- Migration 3 stores rendering diagnostics and raw/title/word/link comparisons. A Rendering tab is exported through the normal exporters. Store HTML keeps both the raw response and the analyzed DOM. Render failures fall back to server HTML with an explicit finding; truncation has its own finding. Browser launch failure stops the crawl before creating a saved crawl.

## Consequences

JavaScript is optional and slower/more memory-intensive than HTTP-only crawling. The browser must be installed and is not bundled or downloaded. Resident memory of the native app excludes separate browser processes. No user browser profile or authenticated session is reused. The renderer does not click or scroll, and parent DOM serialization does not flatten shadow roots or iframe documents. Original HTTP metadata and the final browser URL can differ after client navigation; the Rendering tab exposes that difference. Additional robots checks retain their HTTP timeout.

Tests inject a deterministic renderer into the shared use case to verify facts, JS-only link discovery, scope/robots, fallback diagnostics, storage and exports. Opt-in real-browser tests verify async JS/XHR execution, no duplicate main fetch, request blocking, context isolation, truncation/cancellation and timeout recovery. A local CLI smoke crawl validates the complete Chrome-to-DuckDB/export path, including gzip and original HTTP robots headers.

Queue regression: three URLs using one browser slot, a two-second active timeout and 1,100 ms DOM settling must all render, even though total elapsed time exceeds the timeout. Previously two URLs failed without receiving their full execution budget. Unit tests also cover admission order and cancellation without releasing another page's slot.
