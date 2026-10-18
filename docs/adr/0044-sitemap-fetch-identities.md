# ADR 0044 — Sitemap fetch identities and usable discovery hints

Status: Accepted

## Context

The sitemap queue deduplicated complete URL strings, including fragments that are never sent to HTTP servers. Configuration and nested indexes could refer to one file with several fragments or a fragment cycle, consuming max_sitemaps slots and excluding other maps and their orphan pages. Unsupported scheme URLs consumed the same budget and created failed map requests. Unsupported configured or robots hints also suppressed the default /sitemap.xml discovery.

## Decision

Accept HTTP and HTTPS sitemap sources only. Remove the literal URL fragment before checking the sitemap budget and inserting into the shared queue's seen set. Apply this in the common queue entry point used by configuration, robots discovery and nested indexes, so HTML and JavaScript crawls share the correction.

Preserve complete queries, query order and encoded characters, including %23 in paths and query values. Do not use page scope normalization for sitemap source identities: page query stripping must not collapse distinct map resources. Keep current limits, scheduling, scope and orphan-depth semantics. Skip unsupported schemes without inventing HTTP responses or errors. Only usable HTTP/HTTPS configured or robots hints suppress the existing default sitemap lookup. A usable but failing HTTP sitemap keeps the existing error behavior.

Original robots text retains unsupported hints and comments; parsed robots hints still follow robots comment handling. Map and sitemap URL exports identify the actual fragmentless source, including its query. No schema, dependency or preference changes. Saved historical crawl observations are not rewritten; a new crawl is required to collect previously missed pages.

## Validation

Three crawler regressions first reproduce budget loss from fragment duplicates/cycles, unsupported sources taking slots, and unsupported hints preventing default discovery. Three application regressions exercise HTML, injected JavaScript and fallback discovery with headers/HTML storage disabled. They verify exact map and page counts, no false sitemap errors, orphan findings/depth, original robots text, complete JSON exports, lastmod and the correct sitemap source for every discovered page. Source queries and an encoded hash path remain distinct even when page query stripping is enabled.

Native loopback HTML and real Chrome crawls each fetch four unique maps exactly once and save four pages, including three sitemap-only pages. Both preserve a query map containing an encoded hash and a map with an encoded hash in its path. A separate HTML crawl with only unsupported hints fetches the default map once and finds its orphan page. All exported map sources, page facts, orphan findings, raw/DOM titles and robots evidence agree; no HTTP or render errors are invented. The local server is closed afterwards. All 553 Rust workspace tests pass (three ignored), along with strict clippy for crawl/application targets, formatting and desktop/CLI builds. Frontend code is unchanged; its previous 133 passing tests remain the baseline.
