# ADR 0021 — Disjoint response counts and matching URL filters

Status: Accepted

## Context

The overview inferred successful URLs by subtracting redirects, errors and robots blocks from the total. Missing responses became successes, and overlapping diagnostic flags could reduce the inferred count twice. HTTP 4xx/5xx, request failures and robots blocks shared one summary row. The response-codes explorer could filter only HTTP status classes.

## Decision

Count eight disjoint outcomes directly from stored URL records: 2xx, 3xx, 4xx, 5xx, other non-null codes, robots-blocked without a code, request errors without a code/block, and no recorded code/error/block. A recorded HTTP status takes precedence over diagnostic flags; robots blocks take precedence over errors only when no status exists. A 2xx count describes the recorded HTTP response and does not imply successful JavaScript rendering. The missing-response category makes no claim about why a response is absent. Counts cover stored URL rows, not the unpersisted frontier, references, or browser subresources.

A typed response-group enum supplies the same predicates to the overview, response-codes table and filtered streaming export. Unsupported tab/group combinations are rejected; values are whitelisted by enum deserialization. Search, sorting, pagination and other existing filters compose with the group. Overview counts open the exact group; the response-codes toolbar exposes all groups, including null-status outcomes. New columns show the stored error and robots flag. Crawl changes and unrelated tab/issue navigation clear the group. No site requests, migration or audit refresh are required. Existing aggregate fields retain their previous semantics for consumers outside this summary. An older backend without exact counts displays an unavailable message rather than inferring success.

## Validation

Backend fixtures include HTTP boundary codes, external URLs, overlapping flags, request errors, robots blocks, absent responses and an empty crawl. Tests establish that each stored URL appears once, overview counts match filtered tables/exports, literal search is safe, invalid groups/tabs are rejected, and export includes 1,505 matching rows beyond the table page. Frontend coverage checks exact counts, empty/older-backend presentation and demo filter/export parity. Native QA verifies the summary-to-table-to-export flow with stored fixtures and the user's existing completed crawl without changing its database or metadata.
