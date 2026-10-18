# ADR 0033 — HTTP Link annotation parsing

Status: Accepted

## Context

The HTTP Link parser split parameters on every semicolon, toggled quotation state on escaped quotes and let later `rel` parameters replace earlier ones. A quoted title could become a false canonical, a comma inside escaped quotation marks could hide a valid annotation, and a response could incorrectly receive a canonical intended for a different `anchor` context. These observations affected discovery, indexability and reports in both HTML and JavaScript crawls, including non-HTML files.

## Decision

Read parameters with quote-aware delimiters and decode HTTP quoted-pairs. Preserve the first `rel` parameter, including an empty or valueless first occurrence, and use the first `anchor`. Resolve header targets and contexts against the fetched URL, independently of any HTML base. Accept an explicit context only when it resolves to that same document; a different document or fragment context cannot be attributed to the fetched page and the entire link is ignored. Strip target fragments using the existing URL policy.

This follows the relation/context and quoted-string rules in [RFC 8288 sections 3.2–3.3 and appendix B](https://www.rfc-editor.org/rfc/rfc8288.html). Restricting extracted targets to HTTP(S) and discarding malformed parameter lists rather than applying partial annotations are application choices. Header canonical fallback, original-render baselines and final DOM canonical precedence remain in the shared crawl engine. HTML and PDF responses use the same header parser.

Existing archive measurements are retained. No schema migration or automatic reparse is introduced; repeat a saved plan as a new crawl to collect corrected observations.

## Validation

Four regressions reproduce quoted-title, escaped-quote, repeated-relation, context and malformed/non-web target failures before the fix. The existing comma-in-target and canonical/alternate test remains. Two integration tests exercise HTML and rendered DOM with HTML storage disabled: exact requested URLs, saved header canonicals, raw baselines, DOM precedence, indexability overview totals, missing-canonical findings and complete URL/link exports, including PDF alternates.

Native CLI QA against a temporary local site verifies the shared production engine in HTML and real Chrome modes. HTML stores 11 URLs and exports eight links; JavaScript adds the DOM canonical target, stores 12 URLs and exports ten links. Both runs retain the two header alternates (one from a PDF), report the five expected missing-canonical pages, and never request ignored annotation targets. JavaScript preserves the original header baseline and reports the intended DOM/header canonical mismatch. The server closes afterward, and the updated desktop host restores the user's original crawl and selected issue view. Source archive hashes remain unchanged.
