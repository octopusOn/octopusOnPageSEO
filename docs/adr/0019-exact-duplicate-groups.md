# ADR 0019 — Inspect and export complete exact duplicate groups

Status: Accepted

## Context

Duplicate audits identified affected URLs without separating their individual groups. An affected-URL list can contain many unrelated repeated values, and the inspector did not identify which URLs shared a field with the selected page.

## Decision

Add a bounded `duplicate_groups` reader method and `url_duplicate_groups` IPC returning at most five field/count summaries. Match the existing title, description, first H1/H2 and normalized-text-hash audits exactly: internal, indexable 200 HTML pages, a nonempty string or positive word count with a stored content fingerprint. Counts include the source URL. No raw HTML, new HTTP requests, schema changes or audit recomputation are needed. Previews contain at most 512 Unicode characters; comparisons use the entire stored value. Content hashes are not exposed as readable text.

Add an optional typed `DuplicateScope { kind, url }` to internal table requests. The kind enum selects fixed SQL columns; unknown kinds, empty URLs and incompatible tabs fail. Source and peers must both qualify. Query URL values as parameters in summaries and as escaped literals in the shared table-filter path. Content fingerprints stay as unsigned database integers, avoiding frontend number precision loss. Search, status/indexability, issue filters, sorting and complete streaming exports compose with the group scope.

The inspector offers a complete-group action and duplicate findings open that inspector tab. Navigation clears unrelated search, issue and link filters; a removable chip describes the scope. The compared field is included in the table/export projection. Tab/crawl changes clear the scope. CSV/JSON/XLSX share the same backend predicates and the existing atomic-output mechanism. The browser demo supports its recorded title/description/H1 fields; fields it does not store remain unavailable.

## Validation

Tests cover 1,505 peers beyond a visible page, complete projected exports, filters/sorting, eligibility on both endpoints, blank fields, quoted/injection-like values, enum validation, unsigned 64-bit content fingerprints and previews sharing a 512-character prefix with distinct suffixes. Frontend tests cover safe summaries, empty results, exact scope preservation while saving, and demo summary/page/export agreement. Native QA uses a separate copy of the existing Nexus Medya crawl before updating the main application.
