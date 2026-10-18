# 0047: Record sitemap redirect chains through the existing crawl queue

Date: 2026-10-17

Status: Accepted

## Context

Sitemap requests previously treated every 3xx response as an error. Sites moving their sitemap endpoint could lose all discovered page URLs. Following redirects inside the HTTP client would hide source responses and bypass the unique sitemap budget and live operation reporting.

## Decision

Follow only 301, 302, 303, 307 and 308 through the existing sitemap queue. Record every observed response separately. Resolve relative Location values against their source; permit cross-origin HTTP(S), remove literal fragments and retain complete query strings and percent-encoded hashes. Store the original Location independently of optional response-header storage. Invalid/missing Location and unsupported schemes remain explicit source errors. Other HTTP statuses, including 304, do not navigate.

Every unique hop uses the normal sitemap limit, rate scheduler, cancellation and live activity. Shared targets and cycles are fetched once. The configured global sitemap budget bounds long chains; there is no separate arbitrary redirect cutoff. A completed final target uses the existing XML, plain-text and gzip parser and only that target owns its parsed entries/children.

Schema 9 adds nullable redirect observations to sitemaps and a derived sitemap_redirects table. During the transactional audit, an iterative, memoized graph walk resolves each node once, without recursion or further network requests. The report distinguishes complete parsed destinations, known failures, cycles and unobserved targets. Known destination failures and cycles propagate the existing sitemap-error finding to their redirect sources. An unobserved destination, including a map-budget or Stop boundary, is unverified and contributes no invented HTTP failure. Loops have NULL final URL, final status and hop count. The original response status remains separate from the final status; a broken final XML response with status 200 still fails.

The Sitemaps explorer shows source status, outcome, final address/status and parsed counts, with original Location, next target and hop count available as optional columns and in full exports. Search includes all recorded addresses and Location. Status filters apply to the source response. Filters and streaming exports use the same query.

Prepared schema 7 and 8 archives keep read-only compatibility for exploration, exports and comparison without mutation. Older or pending-audit archives retain the existing migration path. Historical observations cannot reconstruct an absent Location or recover pages from an unvisited redirect; a fresh crawl is required.

## Validation

Regression coverage exercises all five supported statuses, relative/protocol-relative and cross-origin targets, query preservation, shared targets, cycles, malformed/missing Location, unsupported schemes, 304, failed destinations, malformed XML, map limits and Stop. A 5,000-hop saved graph verifies iterative resolution without a fixed trace cutoff. Archive compatibility checks preserve prepared schema 7/8 byte-for-byte, including cross-version comparisons. Query tests cover final-address search, source status, finding filters, bounded previews and full exports.

Native fixtures run both original HTML and actual headless Chrome against a local HTTP server, including a cross-origin redirect and gzipped URL set. A limited third run checks unobserved targets. Request logs verify each actual resource is requested once, and CSV/JSON exports agree. The native desktop explorer is checked separately before replacing the main application binary; existing crawl archives are hash-checked and preserved.

All 575 Rust workspace tests pass (three opt-in browser tests ignored), alongside 135 frontend tests, strict clippy, formatting and frontend/CLI/desktop builds. Native HTML and Chrome runs each save 16 sitemap operations and two page observations, with ten intentional sitemap-error findings and no page or rendering errors. The two-map limit run saves two unverified sources without false sitemap findings. The desktop finding card opens ten matching rows, final-target search returns three rows and its source 3xx filter retains the two successful 307/308 sources. The fixture server is stopped and the private application's previous Responsive JavaScript overview is restored. Both app binaries are backed up and signature-verified after replacement. The main app reopens the original Nexus overview with 864 URLs and no active job; all 66 pre-existing archive/metadata files and both new fixture files remain byte-identical.
