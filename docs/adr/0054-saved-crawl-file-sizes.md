# ADR 0054 — Show saved crawl file sizes without opening databases

Status: Accepted

## Context

Saved crawl cards showed URL counts and duration but gave no indication of file size.
Users could not identify the largest archives before deciding which ones to remove.
Development also accumulated very large copies of bundled DuckDB with C++ debug symbols.

## Decision

Add a filesystem-only `CrawlLibrary::storage` snapshot, exposed through the shared
workspace and native `crawl_storage` command. Measure regular file lengths throughout
each saved crawl folder, including metadata, WAL and nested temporary/spill files.
Do not connect to DuckDB, replay its WAL, migrate an archive or rewrite metadata.
An unreadable, disappearing, non-regular or symbolic-link entry makes the whole
measurement unknown. Individual failures do not prevent other crawl sizes from loading.
The ordinary saved-metadata listing remains the source of visible crawl IDs.

These are logical file lengths, not allocated physical storage or a guarantee of
space reclaimed by deleting a folder. A running crawl can change during measurement.
Show this limitation in the saved-crawls screen and delete dialog's size remains a
file-size label. Existing user confirmation and active/unfinished deletion protection
remain in place. This feature never automatically removes an archive.

Display a workspace total, a third per-crawl statistic, newest/largest sorting and
manual refresh. Unknown sizes sort last and are never rendered as zero. A partial
total is explicitly labeled known size, with the number of unavailable measurements.
Exclude already-deleted IDs from cached totals; reject unsafe numeric values and
overflow instead of displaying imprecise counts. Keep measurements out of persisted
`CrawlMeta`, schemas, audit rules and comparison fields.

Request sizes only while the saved-crawls screen is visible. Refresh every ten seconds
while a job is active, on crawl lifecycle invalidation, and after deletion. Show errors
independently of the crawl list, so an old native backend or failed measurement does
not prevent opening archives. Browser demo sizes are unknown rather than invented.

Disable debug symbols specifically for `libduckdb-sys` in development and test
profiles. Rust application debugging and release settings remain as before. This
reduces regenerated bundled-C++ artifacts substantially after cache cleanup.

## Validation

Filesystem regressions include a deliberately invalid running database, unchanged
metadata/WAL bytes, nested spill files, unrelated exports, remeasurement after file
removal, invalid metadata IDs, library I/O errors, and directory/file symlinks. Native
IPC checks the exact response shape and preserves running-crawl delete protection.
Frontend checks cover total scope, missing/invalid/unsafe sizes, partial and failed
measurements, zero versus unknown, stable sorting, localization and the honest demo.

All 621 Rust workspace tests pass (three opt-in browser tests ignored), alongside
142 frontend tests, strict workspace/all-target clippy, formatting and frontend,
desktop and CLI builds. Native inspection of both original Nexus archives matches
10,417,700 bytes in total; their database, WAL and metadata hashes remain unchanged.
Two disposable records in the private app verify actual sorting, an added 1 MB file
appearing after manual refresh, an unchanged unfinished-delete guard and file size in
the cancelled delete dialog. The private app closes and both test records are removed.
The main app stays open on the user's chosen screen. Native proof and results are
recorded in `target/qa/storage-size-state.json`.

The user-requested cleanup manifest at `target/qa/duckdb-cleanup-state.json` supersedes
older QA hashes for intentionally removed generated databases. Do not recreate those
retired fixtures merely to satisfy historical verification manifests.
