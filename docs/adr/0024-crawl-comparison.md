# ADR 0024 — Read-only crawl comparison

Status: Accepted

## Context

Saved crawls could be inspected individually, but users could not measure changes between snapshots. [Screaming Frog's crawl comparison](https://www.screamingfrog.co.uk/seo-spider/tutorials/how-to-compare-crawls/) is an important workflow for validating releases and investigating SEO changes. A URL absent from a crawl is not evidence that it was deleted, and differing crawl settings affect observations.

## Decision

Add a desktop Comparison page with explicit previous/current snapshot selectors, direction reversal and internal/all URL scope. Exclude running and finalizing jobs; require a successful live-jobs snapshot before enabling selection. The native commands independently reject selected active jobs. The shared store rejects running metadata, equal/invalid identifiers, duplicate URL records and archives needing schema/audit updates. Comparison does not perform migrations: existing finished archives can be opened in the normal explorer to update their analysis first.

Open an ephemeral DuckDB connection with extensions disabled and attach both archives READ_ONLY. Match exact stored URL strings with a full outer join. Track 30 fields: HTTP outcome, content kind, indexability, redirects, titles/descriptions/headings and counts, canonical/header canonical, robots directives, word count/text fingerprint, depth, link counts, rendering state/errors and truncation. Exclude timestamps, response latency and render duration from change detection. Cast values to text inside DuckDB before returning them, preserving unsigned 64-bit fingerprints without JavaScript numeric rounding. NULL and empty strings remain distinct. This reports extracted-text fingerprint changes, not a content similarity percentage or link-target graph equality.

Provide four disjoint groups: changed shared URLs, unchanged tracked fields, only-current and only-previous URLs. Field-change counts include shared URLs only. Filtering by field also requires shared membership; absent-side values never become inferred field changes or SEO fixes. Scope applies independently to each snapshot, so internal/external reclassification can affect membership. Summary counts use the selected URL scope and the same search/field predicates as the table, across all membership states. Selecting a card or changing state preserves the other filters. This supersedes the initial whole-scope indicators, which were confusing after filtering. Hide stale counts during summary refresh and disable export until summary and results settle. Explain partial, list, seed/settings mismatch, request/rendering failures, truncated content and no-shared-URL conditions.

Page 250 rows by exact URL (backend clamped to 1–1000), virtualize visible rows, and fetch a 30-field before/after detail only for the selected URL. Reset result filters, pagination and detail on pair/scope changes. Query identities include both ordered IDs and every filter. Disable export until the current result query settles. Export every matching URL with all 65 membership/before/after columns, using the same predicates and order and ignoring page limits. Reuse the atomic streaming CSV/XLSX/JSON writer; destination files survive failures. No recrawl, HTML storage, new schema or new dependency is needed.

## Validation

Store tests cover exact/literal URL matching, SQL-like input, NULL/empty transitions, large unsigned fingerprints, scope reclassification, link counts, direction, disjoint membership, unsupported selections, running/pending/same/invalid archives, duplicate stored URLs, scope warnings, simultaneous readers/comparisons, 1505-row exports and byte preservation. App tests cover all formats, ignored pagination and failed-export preservation. Desktop IPC verifies the new command argument names and enum validation. Frontend tests cover finalizing-job eligibility, identities, escaped before/after values, missing/NULL distinction and reversible demo snapshots.

Native UI QA uses two real isolated loopback crawls of 303 URLs each. The result has 270 changed, one only-current, one only-previous and 32 unchanged URLs. Verify noindex detail, all-fields toggle, 250-row pagination, field reset, membership detail and reversed direction. CSV, XLSX and JSON exports each contain all 270 changes and 65 columns; every cell and exact URL set agrees across formats. Verify source DB/metadata hashes, including the user's original Nexus crawls.

## Limits

No URL rewriting/mapping, arbitrary custom-field selection or content similarity computation. Historical finding observations are added separately in ADR 0026. Database joins and aggregation still require resources proportional to the archives; this is not a million-URL performance claim. Original crawl settings are compared as a whole, so some settings differences may not affect the selected fields.
