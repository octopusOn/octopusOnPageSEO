# ADR 0037 — Safe request-rate scheduling and explicit worker validation

Status: Accepted

## Context

A finite positive request rate can have a reciprocal larger than `Duration` or the monotonic clock can represent. The shared rate limiter converted that reciprocal and added future slots with panicking operations, so a very small rate could crash a crawl instead of waiting. The public crawl-plan boundary also accepted negative/nonfinite rates and silently clamped HTTP worker counts, despite the desktop validating finite nonnegative rates and 1–64 workers.

## Decision

Validate HTTP workers as 1–64 and request ceilings as finite and nonnegative before browser launch, job registration or archive creation. Report the invalid field instead of silently changing it. Keep zero as unlimited and accept all finite positive rates, including fractional and subnormal values; no arbitrary minimum or maximum request rate is introduced.

Use fallible duration conversion and checked clock addition. A slot beyond the clock's range has an explicit unavailable time rather than wrapping, panicking or disabling the ceiling. The first available request still proceeds. Later HTTP work keeps waiting with Stop checks at most 50 ms apart; browser resources reject slots beyond their active render deadline. A cancelled wait still creates no saved HTTP failure. Normal rate spacing, rendering deadlines and unlimited-rate behavior remain unchanged.

No schema, dependency or saved-preference migration is needed. Reading existing archives is unchanged. Repeating a saved plan with an invalid worker count or rate now reports that error before starting.

## Validation

The new configuration and rate tests first reproduce the old acceptance of zero workers and the duration-conversion panic. They then cover invalid worker/rate values, valid boundary workers, fractional/unlimited/extreme positive rates, bounded browser waits and cancellable HTTP waits. HTML and injected-JavaScript use-case regressions at subnormal/very small rates retain the first observed response, cancel the second URL, preserve a complete JSON export and report zero request/render errors. Invalid settings cannot launch even an explicitly missing browser or create an archive. Desktop IPC tests verify field-specific rejection without registering a job.

Native CLI QA against an isolated loopback HTTP server verifies stopped HTML at `5e-324` and real Chrome rendering at `1e-300`: each retains one successful response and sends no request after Stop. The JavaScript-generated title and link are present in the complete exports. Unlimited and 20 requests/second list crawls both complete with two responses; the latter preserves request spacing. Zero/65 workers and a negative request ceiling fail before browser startup, any HTTP request or archive creation. The fixture server is closed afterwards. The workspace passes 493 tests (three ignored); strict clippy, formatting, desktop IPC and desktop/CLI builds pass.
