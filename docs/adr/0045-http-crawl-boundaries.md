# ADR 0045 — HTTP crawl boundaries and consistent redirect identities

Status: Accepted

## Context

Internal classification compared hosts without checking the scheme. XML sitemap entries and HTTP Location redirects could bypass the HTML extractor's HTTP filtering: same-host FTP, WebSocket, file and custom targets were fetched as internal pages. External checking could also enqueue unsupported targets without a matching host. These invented requests consumed page budgets, hid legitimate pages and produced false network failures and misleading URL/error counts.

Application tests also exposed a saved identity mismatch. The frontier normalized redirect destinations, but redirect_url retained fragments and stripped query parameters. Read-only redirect traces and graph audits could therefore fail to find a successfully fetched destination, a self-loop or a downstream HTTP error.

## Decision

Use one HTTP/HTTPS scheme predicate for internal web classification, final page/resource enqueue and sitemap-source admission. Reject unsupported schemes at the queue boundary before they consume dispatch slots; a non-HTTP matching host is not internal. Keep observed non-web Location and link references and unmodified sitemap entries as evidence. They have no fabricated response/status/error records and remain explicitly unchecked. An enabled external-status setting still applies to genuine HTTP targets.

For new HTTP redirects, normalize the saved destination with the same scope normalizer used for the frontier and graph links. Strip fragments and only the configured query parameters, preserving other query values. Keep unsupported destinations unchanged. The recorded HTTP source status and optional original stored Location header are untouched. This makes new saved graph edges and fetched identities agree without changing schema, preferences, dependencies or historical archives.

## Validation

Three regressions first reproduce incorrect scheme classification, sitemap-only page budget loss and unsupported redirect dispatch with external checks on/off. Application tests cover HTML, JavaScript and genuine external HTTP checks with source/header storage disabled. They compare progress, overview response groups, internal/external/error/redirect totals and full exports; retain unsupported Location/link/sitemap evidence with unknown responses; find the valid orphan page at the exact URL budget; and verify normalized successful redirect traces. A separate normalized chain fixture asserts self-loop detection and error propagation through two redirects to a real 404.

Five native loopback scenarios pass: HTML and real Chrome with external checking both off/on, plus normalized loop/error chains. The four normal runs save twelve internal URLs and zero/one checked external URL, retain seven unsupported redirect references and seven unchecked sitemap entries, find the orphan page at the exact page budget and have no HTTP/render errors. Each real page is fetched once. Saved destinations strip the fragment and configured tracking parameter while preserving the meaningful query. The chain fixture saves five URLs with one intentional HTTP 404, one loop finding and two redirect-to-error findings. The server is closed afterwards. All 560 Rust workspace tests pass (three ignored); the 98 affected-package tests also pass after adding direct overview and original-header assertions. Strict clippy, formatting and desktop/CLI builds pass. Frontend code is unchanged, retaining its prior 133-test baseline.
