# ADR 0012 — Rendering retries and hreflang verification

Status: Accepted

## Context

A completed crawl can contain JavaScript failures alongside useful saved results. Repeating a full crawl is unnecessary when investigating these failures. A focused crawl also has less evidence for checks involving other pages. The previous hreflang return-link predicate treated any recorded target without stored annotations as a confirmed missing return, including HTTP errors, redirects, blocked pages, non-HTML resources and external status checks.

## Decision

The Rendering tab prepares an editable list crawl using the existing bounded page API and `render-error` audit filter. It retrieves every page sorted by the unique URL address, verifies the totals, and rejects incomplete or inconsistent responses. It clones the original settings, enables JavaScript and clears URL/depth limits. Normal crawl defaults and saved source results remain unchanged. The existing start command runs the reviewed list as a separate crawl; preparation performs no network crawl or storage mutation.

List reports display their scope limitation. Hreflang return checks join only successfully analyzed internal HTML targets. Other target states remain eligible for their separate response, robots and indexability findings. This is an evidence boundary; external language variants are still valid annotations, but the current external status-check path does not parse their content.

## Validation

Frontend tests cover more than 2,500 failures across multiple pages, empty results, active-crawl rejection, immutable configuration, inconsistent responses and safe list previews. An integration fixture covers confirmed missing and valid returns, 404, redirects, robots blocks, non-HTML and external status-only targets. It reproduces five false findings before the predicate change. Actual native UI verification covers preparation, cancellation and starting a focused retry into a new saved record.
