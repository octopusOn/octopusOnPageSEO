# ADR 0013 — Filtered exports

Status: Accepted

## Context

The desktop exported a complete backend tab. Users investigating a filtered URL list could not download that selection with the displayed columns and order. Client-side export of a paginated response would silently omit later pages and could collect a large crawl in webview memory.

## Decision

The explorer publishes a settled export selection scoped to its crawl and UI tab. Search debounce, pending queries, placeholder rows and stale crawl/tab selections disable filtered export. The selection is copied before opening the native save dialog. Full-tab export remains an explicit choice, and other screens retain it.

`CrawlReader` builds one validated filter/order query for both pagination and streaming export. Export ignores pagination, whitelists projected columns, preserves their order and rejects duplicates. The shared application use case streams into the existing CSV/JSON/XLSX writers. IPC accepts an optional selection, retaining callers that omit it, and rejects mismatched tab names. The demo mirrors filtering and column projection.

Writers target a temporary file in the destination directory. The file replaces the destination after successful writer finalization and file synchronization. Failure leaves the previous report intact and removes the temporary output. `tempfile`, already present in the lockfile, moves from a development dependency to a normal application dependency without requiring a package version change. Separate ZIP security updates are documented in ADR 0014.

## Validation

Tests cover more than 1,500 rows, output beyond the current page, combined predicates, literal search, sorting, projected columns, nulls, invalid/duplicate columns, stale UI selections, demo parity, all three formats and preservation after a partial write fails. Native UI verification checks a filtered result export and its downloaded file.
