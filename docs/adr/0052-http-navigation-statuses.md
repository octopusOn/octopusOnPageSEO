# ADR 0052 — Distinguish HTTP 3xx families from automatic navigation

Status: Accepted

## Context

Page fetching and redirect inspection previously treated every 3xx Location as a navigation. A 304 with an erroneous Location could enqueue another URL, inflate redirect/link findings and fabricate a chain. Blank Location values resolved to the current document, generating a false self-loop. SQL checks and the overview's redirect count also classified every 3xx as a navigation.

## Decision

Use one shared `follows_location` predicate for 301, 302, 303, 307 and 308 in page, robots and sitemap fetching and read-only page-chain inspection. Page Location values must be nonblank and parseable. Existing HTTP scope/normalization applies to valid targets; non-HTTP references remain stored evidence without requests, and an explicit fragment-only self redirect remains a real loop. Responses outside these five codes retain their actual status, do not create navigation edges and do not render/parse a 3xx body. Their indexability is OtherHttp rather than Redirected. No cache revalidation or cached-body substitution is introduced for 304.

The [HTTP semantics standard](https://www.rfc-editor.org/rfc/rfc9110.html#section-15.4) distinguishes URI redirects, choices and stored representations. It permits automatic use of Location for additional 3xx codes; selecting only these five is an application policy. A 300 Multiple Choices and unknown 3xx remain recorded without automatic selection. A 304 does not establish that this crawl obtained a 200 representation.

Restrict internal-redirect, links-to-redirects, redirect-chain and graph error/loop propagation to the same five statuses. Both source and destination must qualify for the two-hop finding. The overview's `redirects` total counts those statuses, while disjoint response groups and HTTP filters still retain every 3xx. Label the latter HTTP 3xx, preserving the API group identifier and exact CSV/JSON codes. The existing rule IDs and priorities remain stable.

Read-only traces stop at any non-navigation response, even if a historical archive recorded a spurious redirect target. Preserve that stored target as evidence; do not rewrite it or claim a missing Location. Trace Complete means a recorded terminal response, not successful content. Explain the distinction in the inspector. Prepared historical archives retain their original observations/findings; new crawls collect corrected edges and audits. No schema migration.

## Validation

Engine regressions exercise every 3xx code and missing/empty/whitespace/malformed Location. Store regressions cover legacy targets, false cycles/error propagation, source/destination SQL eligibility, and the difference between the automatic redirect count and complete HTTP 3xx response group. The original engine test fails before the fix by following HTTP 300. End-to-end HTML/Chrome and native UI verification are recorded in the accompanying QA state.

All 611 Rust workspace tests pass (three opt-in browser cases ignored), alongside 135 frontend tests, strict workspace clippy, formatting and frontend/CLI/desktop builds. Native HTML and real Chrome crawls save 16 and 17 URLs respectively; the additional URL is discovered from a JavaScript-created hyperlink. Both have 14 HTTP 3xx responses, eight automatic redirect statuses, six recorded redirect edges, three chain findings and one explicit self-loop. No ignored Location target is requested, no blank-Location loop is fabricated, and there are zero page/render errors. CSV/JSON statuses and indexability agree; the fixture server closes.

The native desktop shows 14 rows from the HTTP 3xx overview group. Its read-only trace ends after 301 → 302 → 304 with the revised explanation and no invented target. Private selection, filters and temporary window size return to their previous state before closing. Both installed binaries have verified backups and signatures. The main app reopens the unchanged Nexus crawl with 864 URLs and no active crawl. Its current Explorer selection is preserved. All 118 pre-existing database/metadata files and both new private fixture files retain their hashes.
