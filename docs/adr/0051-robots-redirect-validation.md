# ADR 0051 — Validate robots.txt redirect targets and avoid repeat requests

Status: Accepted

## Context

The robots fetcher followed every 3xx with Location, passed fragments and unsupported schemes to its HTTP port, and requested cyclic targets repeatedly until the five-hop budget expired. A 304 carrying Location could therefore apply unrelated rules. Fragment variants could bypass cycle detection and use the wrong identity in a fetcher. These decisions affect both page requests and JavaScript resources through the shared robots cache.

## Decision

Follow only 301, 302, 303, 307 and 308. Resolve each Location against the current hop, accept HTTP(S) targets only, strip literal fragments and retain queries/escaped hash bytes. Track the at-most-six requested URLs and stop before repeating one. The initial response plus five redirect destinations remain fetchable, including cross-origin chains. Parsed rules apply to the initial origin, with destination-origin caching kept independent.

Keep the existing conservative Unreachable classification for invalid, cyclic, non-redirect 3xx and exhausted chains. Respect blocks affected pages/resources for this crawl. The [REP standard](https://www.rfc-editor.org/rfc/rfc9309.html#section-2.3.1.2) permits following redirects across authorities and requires applying reached rules to the original authority. Its optional unavailable treatment after more than five redirects, and [Google's documented unrestricted treatment](https://developers.google.com/crawling/docs/robots-txt/robots-txt-spec#handling-of-errors-and-http-status-codes), are not adopted here. Explain the application policy in the UI and documentation without claiming full Googlebot emulation.

Keep the last received HTTP status when a later hop fails, so the report may show 302 beside Unavailable. Do not fabricate a terminal response or parse redirect/error bodies. Update the existing unreachable finding's explanation and recovery guidance. No schema, rule ID, priority, historical observation or override-policy changes.

## Validation

Ten focused cases cover all five accepted codes, every other 3xx, unsupported/malformed targets, fragment/query identity, relative cycles, missing/empty/self references, five/six-hop boundaries, cross-origin cache separation and network failures after redirects. Five fail before the fix. Full workspace and native desktop/Chrome verification results are recorded with the QA state for this change.

All 606 Rust workspace tests pass (three opt-in browser cases ignored), alongside 135 frontend tests, strict workspace clippy, formatting and frontend/CLI/desktop builds. Seven native loopback scenarios verify five-hop HTML and real Chrome runs, cycles, self-fragments, unsupported schemes, 304 and exhausted chains. HTTP requests match the expected set exactly; successful rules apply to the source origin without blocking the destination origin's own page. Chrome does not request the denied script, and the rendered title proves its error handler ran. CSV and JSON robots exports agree; fixture servers close.

The native UI shows two successful pages and one robots block for the valid chain, and two robots blocks with zero request errors for the cycle. The cycle's finding opens one Unavailable robots record with its real last HTTP 307 and the updated chain explanation. Private test selection and filters are restored before closing. Both installed apps have verified backups and signatures. The main app reopens the unchanged Nexus overview with 864 URLs and no active crawl. All 100 pre-existing database/metadata files and four new private fixture files remain byte-identical after UI inspection.
