# ADR 0050 — Treat robots.txt rate limits as unavailable access and retain browser observations

Status: Accepted

## Context

Every robots.txt 4xx response previously became Missing, including HTTP 429. Respect mode therefore fetched pages while the server was rate limiting robots access. Browser request policy also discarded robots responses first fetched for a new internal resource origin: the access decision worked, but the Robots.txt report and audit lost its evidence.

## Decision

Classify HTTP 429 as Unreachable before ordinary 4xx handling, retaining its original HTTP code and an explicit rate-limit diagnostic on affected page records. Error bodies remain unparsed. Existing Respect behavior for unavailable robots blocks page requests and in-scope browser resources for this crawl; Ignore-but-report and Ignore keep their requested semantics. Other 4xx results remain unrestricted. Preserve once-per-origin caching, including redirect destinations, and isolate scheme/host/port origins. Do not invent a page HTTP response or request failure for an unsent blocked page.

The exception follows [Google's robots status guidance](https://developers.google.com/crawling/docs/robots-txt/robots-txt-spec) and [HTTP 429 interpretation](https://developers.google.com/crawling/docs/troubleshooting/http-status-codes). Applying a conservative per-crawl denial to all presets is an application policy. It does not implement Google's historical cache or 12-hour/30-day recovery behavior, automatic Retry-After retries or cross-crawl recovery.

Browser-only checks place newly observed robots records into a shared deferred collection. After all workers have joined, a successful or normally stopped engine run sends those records through the normal sink before final storage/auditing. The cache's single initializer owns each observation, so a page/side-job record is not duplicated. The final audit can report the affected robots URL without creating a page row for a browser resource. Sink failures retain the existing error propagation.

Translate found/missing/unreachable states in the desktop table while keeping raw values in exports. Explain policies and recovery in the Robots.txt hint and update the existing missing/unreachable catalog descriptions. Rule IDs, priorities and schema remain unchanged. Preserve prepared historical archives; recrawl to collect corrected access observations.

## Validation

Regressions reproduce both unrestricted 429 access and missing browser-only evidence before their fixes. Unit cases cover all five agent presets, all other 4xx responses, redirect-to-429, scheme/port isolation, one cached request, and exactly-once deferred delivery for successful, missing and failed responses. Integrated cases cover HTML, JavaScript, Respect, Ignore-but-report, Ignore, genuine 404, cached browser denial, newly encountered browser origins and Stop. Assertions compare saved statuses, audit findings, disjoint response totals, exports and actual fetch calls.

All 596 Rust workspace tests pass (three opt-in browser tests ignored), alongside 135 frontend tests, strict clippy, formatting and frontend/CLI/desktop builds. Seven native loopback cases include two real Chrome runs: a cached 429 and an origin first discovered by a browser resource. Neither sends the denied resource request; the latter saves one real page and both robots observations, without fabricating a blocked page row. Genuine 404 and both override policies retain their expected behavior. CSV/JSON agree, every robots origin is requested once and both fixture servers close.

The native desktop shows one blocked page and one successful page, with zero request-error/no-response rows. Its unreachable finding opens exactly one robots record. Clearing that filter shows the two original statuses, HTTP 429/Unavailable and HTTP 200/Read successfully, with translated headings and recovery guidance. The private app returns to its previous Responsive JavaScript overview and closes. Both installed binaries have verified backups and signatures; the main app returns to the original Nexus overview with 864 URLs and no active job. All 84 baseline archive/metadata files and both new UI fixture files retain their original hashes.
