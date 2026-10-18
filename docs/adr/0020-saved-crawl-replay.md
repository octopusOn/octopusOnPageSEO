# ADR 0020 — Repeat a saved crawl and distinguish saved from live state

Status: Accepted

## Context

The saved-crawl list had no way to reuse a complete plan. The new-crawl dialog defaulted to one Spider seed even when a saved plan used list mode or multiple seeds. An interrupted record retained `running` metadata indefinitely, and opening it showed a generic progress message despite the absence of a live job.

## Decision

Provide a saved-plan action that deep-copies the source configuration and seed URLs. Preserve scope, URL/depth limits, rate, storage, resources and browser settings. Historical plans without rendering settings receive the current HTML defaults. The dialog initializes with the saved mode, retains multiple Spider seeds, shows the source/date and copied settings, and supports the existing URL, renderer and worker-count edits. A successful start uses the existing new-crawl endpoint and creates a new record; no resume, source metadata rewrite or global-preference update occurs. Closing the dialog or returning to an ordinary new crawl clears the copied plan. JS-failure-only retry remains a separate workflow that clears old limits.

Derive display state from saved metadata plus this process's jobs snapshot. Matching running/paused/stopping/finalizing jobs take precedence over metadata, including the short finalization period when metadata is already completed. An unfinished record with no matching active job gets a process-scoped inactive label only after jobs are known. Unknown/unavailable jobs do not establish interruption and block saved-plan launch. The label does not claim another process has stopped. Unfinished-record deletion stays blocked by the existing backend. No database migration or new backend command is required.

## Validation

Frontend tests cover loading/error state, finalizing jobs with completed metadata, saved list/multi-seed initialization, deep-copy isolation, preserved limits and patterns, legacy defaults and safe previews. Native QA replays separate localhost Spider and list crawls with nondefault JS, rate, limit and scope settings; source-file hashes and new-record configurations verify preservation. An isolated unfinished record exercises the inactive state without altering the user's original records.
