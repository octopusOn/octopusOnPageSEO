# ADR 0060 — Split link counterparts and expose exact finding targets

Status: Accepted

## Context

Incoming/outgoing references were grouped together in the inspector, and the URL grid did not distinguish both endpoint classes. The `links-to-broken` finding counts affected source URLs. Opening those sources without their failing destinations made a repeated site-wide link look like hundreds of unidentified broken targets.

## Decision

Derive enriched link rows at read time from recorded URL IDs and references. Retain destination status, request error, anchor text and both endpoint classifications. Incoming internal/external selection uses the source page; outgoing and unscoped selection uses the destination. The saved crawl's classifications govern the boundary. Unchecked targets retain their recorded reference classification and no invented HTTP outcome.

Expose four inspector tabs with complete counts: internal/external incoming and internal/external outgoing. Previews are capped at 250 rows; their full filtered selection opens in the paginated Links explorer. Preview totals count every recorded reference kind. The URL grid and its exports add separate internal/external incoming/outgoing hyperlink counts, with an explanation of this difference. Missing older-backend counts remain unknown rather than being inferred from a capped preview. Recorded external inlinks do not imply internet-wide backlink discovery.

Route internal/external broken-link and redirect-link findings to Links. Keep the stored source finding filter and the matching target predicate: hyperlinks with recorded failing HTTP/request errors, or automatic redirect statuses. Include source, exact destination, HTTP status/error and anchor text even if a custom column layout hid these fields. Selecting a finding row inspects the destination; its incoming references identify referring pages. Unknown, blocked-only and successful targets do not become broken evidence.

Show distinct source URLs, distinct destination URLs and reference records over the entire filtered selection, independently of pagination. Search/status/class/scope filters and exports use the same query. Clearing an incoming URL scope also clears its counterpart classification, so it cannot silently become an outgoing target filter. Hide stale summary counts while replacement data is loading. Keep original audit counts and URL/link data intact; no archive migration, re-crawl or dependency is required.

## Validation

Cover mixed internal/external sources, duplicate references, non-hyperlink annotations, HTTP 404/503, request timeout without an HTTP status, successful/blocked/unchecked targets, redirects and literal URL quoting. Verify full counts, classification, pagination and complete ordered exports beyond preview/page limits. Existing export and duplicate fixtures exercise added URL counts. Verify the native UI against a private copy of a completed archive, including a second page and clearing a source scope.

The completed Nexus HTML archive contains one matching broken target, `https://nexusmedya.com/cdn-cgi/l/email-protection`, recorded as HTTP 404, with 260 distinct sources and 751 hyperlink records. These are stored observations, not a new live request or 260 distinct failing destinations. All user archive files are hash-checked; the private verification copy is removed after closing it.
