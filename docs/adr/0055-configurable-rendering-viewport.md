# ADR 0055 — Configure the JavaScript rendering viewport

Status: Accepted

## Context

Rendering previously selected a fixed 412×915 mobile layout for Googlebot Smartphone
and a 1280×800 desktop layout for every other agent. Users could not inspect the DOM,
metadata and generated links at another responsive breakpoint.

## Decision

Add optional `rendering.viewport` with width, height and Chrome mobile-layout mode.
When omitted or null, retain the exact historical agent defaults. Explicit layouts
override those dimensions and mobile-layout mode independently of the HTTP/browser
user-agent identity, robots decisions, rendering deadlines, concurrency and body cap.
Allow integer dimensions from 240 to 3840 CSS pixels in the shared crawl-plan validator
and frontend, rejecting invalid settings before creating a crawl or starting Chrome.

Apply the chosen layout before navigating each isolated browser context, using
[Chrome's Device Metrics override](https://raw.githubusercontent.com/ChromeDevTools/devtools-protocol/master/pdl/domains/Emulation.pdl).
The mobile flag controls mobile layout and viewport-meta processing; pixel ratio stays
one. Touch, hardware, UA Client Hints and full device simulation are outside this change.
Page markup, viewport metadata and layout overflow can affect the resulting inner
dimensions, as in an ordinary browser. The setting is a requested browser layout.

The desktop offers automatic, 1280×800 desktop, 390×844 mobile and custom choices.
Custom sizes have independent width, height and mobile-layout controls. Explain that
layout does not change the chosen crawler identity. Show the saved layout in the
overview and repeated-plan review. Persist it as part of the existing config JSON,
preserve it during full repeats and rendering retries, and include it in the existing
settings-difference comparison policy. Omit absent overrides when serializing native
settings, so legacy automatic plans remain automatic.

The same viewport control is available in global rendering settings and in the new,
repeat and rendering-retry dialogs. Dialog edits stay local to that draft, validate
before starting, and appear in its repeated-plan review without changing saved
preferences or the source archive.

CLI supports `--render-viewport WIDTHxHEIGHT`, with `--render-mobile` to request mobile
layout. Explicit CLI dimensions otherwise use desktop layout. The mobile flag requires
dimensions instead of being silently ignored. JSON configurations support automatic
layout via omission/null. HTML mode keeps these settings for a later JavaScript crawl
but does not invoke a browser. No database schema, audit rule or historical observation
is rewritten. A new crawl collects a different viewport's content.

## Validation

Domain tests cover unchanged agent defaults, explicit precedence, bounds and historical
JSON round trips. CLI tests cover parsing, rejected/ignored options and independent
agent identity. Native IPC rejects invalid sizes before a job/archive is created.
Frontend checks cover validation, default/preset/custom controls, repeat/retry cloning
and overview context. Real-Chrome tests check five layouts with media-query content,
exact generated titles and links, preserved navigator identity and no duplicate main
fetch. Full CLI-to-store checks compare mobile and desktop discoveries and CSV/JSON
exports. Generated QA databases are removed after verification, following the cleanup
policy in ADR 0054.

All 626 workspace tests pass, plus 151 frontend tests and all four opt-in Chrome tests.
Strict workspace/all-target clippy, formatting and frontend/CLI/desktop builds pass.
Two native CLI crawls produce distinct 390×844 narrow and 1280×800 wide content and
generated links. A third crawl launched from the desktop repeat dialog uses a custom
768×1024 mobile layout and saves the matching DOM, plan and CSV/JSON exports. A later
repeat retains those exact controls. Comparison with the desktop snapshot reports a
settings warning, one changed root and one unchanged branch.

The ongoing main Nexus scan was left running. Once it was saved stopped at 66 URLs
and the application had closed, the signed update was installed and reopened on that
saved crawl. Historical and new main database/metadata hashes remain unchanged. The
three private QA archives, test server and redundant staged binary are cleaned up;
no follow-up schedule is needed. Results are in `target/qa/viewport-state.json`.
