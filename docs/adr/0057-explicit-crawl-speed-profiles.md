# ADR 0057 — Explicit speed profiles and visible crawl limits

Status: Accepted

## Context

New/repeat crawl dialogs exposed HTTP worker count but hid the request ceiling and browser concurrency. Increasing workers alone could leave JavaScript crawls constrained by 5 requests/second and two browser pages. The effective browser bound is the smaller of HTTP workers and configured browser concurrency.

## Decision

Share speed controls between Configuration and new/repeat/JS-failure retry dialogs. Display HTTP worker count (1–64), the request ceiling (finite nonnegative; explicit zero means unlimited), and browser concurrency (1–8) in JavaScript mode. Accept custom and fractional rates. Clearing a number field produces an invalid draft rather than silently enabling unlimited requests. Validate the complete dialog draft before starting a job.

Offer explicit Gentle (2 workers, 2 requests/second, 1 browser), Balanced (5, 5, 2), and Fast (10, 20, 4) buttons. Do not apply a profile automatically: identify an existing exact match or show Custom. Applying a profile changes only these speed limits; HTML mode retains its hidden browser settings. Keep JavaScript settling time, active timeout, viewport, resource cache, robots policy, limits and discovery coverage unchanged. Profiles do not promise a measured throughput; server latency and the rate shared by page/resource network requests still matter.

Dialog edits stay local, and cancellation changes neither saved settings nor source crawl metadata. Submitted numeric settings use the existing IPC/domain validation and metadata format; no dependencies, Rust behavior, archive migration or global-default changes are required. Show the effective simultaneous browser-page bound and explain that URL throughput differs from the request ceiling. Keep the new-crawl dialog header/footer visible while its body scrolls.

## Validation

Test profile application against a one-second timeout with a short existing wait, preserve source configuration and coverage, retain hidden HTML browser settings, accept custom fractional/subnormal/unlimited rates, compute both concurrency bounds, and reject invalid/cleared drafts. Verify profile selection, numeric draft validation, local cancellation, saved-plan replay and a real JavaScript crawl through the native desktop UI. Build and type-check the frontend, and remove generated QA archives after closing the test instance.
