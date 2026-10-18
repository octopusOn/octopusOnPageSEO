# ADR 0036 — Cancellable HTTP rate waits

Status: Accepted

## Context

HTTP workers reserved a request slot and slept until it without checking Stop. At low request rates, stopping could wait through multiple future slots and still send requests that had not begun when Stop was pressed. Browser resource waits already checked cancellation; page/resource/external requests, sitemaps and well-known site files did not. In a regression crawl, four additional URLs were fetched after stopping following the first observed page.

## Decision

Check Stop before reserving an HTTP slot and while waiting, sleeping in slices of at most 50 ms. Return cancellation to the worker instead of issuing an unsent request. Use the existing skipped outcome and cancelled live-activity state, without writing a URL response, sitemap failure, site-file failure or HTTP error for unsent work. Preserve a completed robots response if it was obtained before cancelling its page's rate wait. Keep the shared slot reservation/order and existing request spacing.

Responses already being fetched still finish under their configured HTTP timeout and are saved. Stop does not abort their sockets. Existing browser cancellation/deadline handling and Pause's assignment semantics remain. Audit/final-save time is separate from draining rate waits. No schema, preference or dependency changes are needed; saved archives are unchanged.

## Validation

Three worker regressions first reproduce page, sitemap and site-file requests being sent after Stop, then verify cancellation, bounded wait completion, no fetch and no request error. A separate regression preserves an already completed robots.txt response while cancelling its page. Existing rate-spacing tests cover normal/unlimited operation and now also check stopped controls.

HTML and injected-JavaScript integration tests stop after the first page is observed. They preserve that page and its rendered result, cancel waiting work, enter finalization without waiting through future request slots, and verify stopped metadata, live cancellation, zero HTTP/render errors, exact saved URL count and complete JSON export. These tests initially stored five URLs instead of one.

Native UI QA uses an isolated local site at 0.1 requests/second. Stop is pressed with two pages already observed and two pages waiting. The same UI interaction enters finalization, both waiting pages become cancelled, the server records no HTTP request after Stop, and the saved stopped crawl and complete JSON export retain exactly the two successful responses with zero request errors. The private QA speed and sitemap preferences are restored afterwards. All 488 workspace tests pass (three ignored), along with strict clippy, formatting and desktop/CLI builds. Existing source archive hashes remain unchanged.
