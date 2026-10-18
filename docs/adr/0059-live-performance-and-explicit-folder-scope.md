# ADR 0059 — Inspect live page timing and choose a starting-folder scope

Status: Accepted

## Context

The real Nexus Medya test was an HTML crawl at five workers and five requests/second, initially observing about 2.1 URLs/second and mostly two-to-three-second page response headers. It also followed other language paths because its starting-folder restriction was disabled. A request ceiling is not a guaranteed page throughput, and a starting URL containing `/tr/` does not itself restrict discovery to that language.

## Decision

Show a read-only performance block in the live workspace: actual job rendering mode, saved worker/request limits, effective browser concurrency when JavaScript is active, and whether discovery is limited to starting folders. Resolve settings by the running job's crawl identity rather than the selected historical archive or global draft. Omit unknown settings until metadata arrives.

Compute independent means for response-header time and complete assigned page-operation time from successful HTTP 2xx page responses in the existing latest-100-operation feed. Exclude resources, external URLs, HTTP errors, blocked/cancelled work and missing/invalid timing data. Successful HTTP responses with a JavaScript error still contribute their HTTP observation. Keep sample size visible; operation time also contains rate/browser waits, body download, parsing and any rendering. Neither metric identifies server-only or JavaScript-only duration. Feed filters do not change the sample; no extra network request, database read, growing history or estimated completion time is introduced.

Expose the existing backend `urls.start_folder_only` boolean in Configuration and new/repeat Spider dialogs. Keep it off for missing legacy values and require an explicit choice. Dialog edits are local; cancel leaves source and global settings intact. Explain trailing-slash/parent-folder semantics, preserved outgoing links, unchecked out-of-folder internal targets, and separately configured external checks. Keep advanced URL rules and the current running job unchanged. List drafts retain the copied flag while hiding the Spider control. No native protocol, dependency or archive migration is needed.

## Validation

Check mixed HTTP/JavaScript outcomes, bounded timing samples, zero/missing/invalid times, unknown settings, browser bounds, explicit unlimited rates, HTML scope labels and legacy/malformed folder flags. Verify real crawl observations and new folder drafts through the native UI without restarting the user's active crawl. Run frontend checks and production build, and exercise the existing backend folder rules with a separate local fixture.

The real HTML crawl completed naturally with 737 URL records in 223,782 ms (about 3.3 URLs/second): 266 internal and 471 external URLs. This is a real observation with JavaScript disabled, not a before/after JavaScript benchmark. The local fixture fetched all 11 Turkish pages with the folder restriction on, retained outgoing English links without fetching their target, and fetched the English target as a twelfth page when unrestricted. Native repeat-dialog edits were cancelled and the original folder flag remained enabled. The live panel showed 2,002 ms response-header time and 2,208 ms total operation time for its initial successful-page sample. All 163 frontend checks, 23 relevant Rust scope tests and the frontend production build passed.
