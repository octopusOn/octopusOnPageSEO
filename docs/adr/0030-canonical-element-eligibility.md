# ADR 0030 — Eligible HTML canonical declarations

Status: Accepted

## Context

The HTML parser collected every canonical relation in the document, including body and inert template elements. It also accepted alternate-version attributes and counted repeated relation tokens as separate declarations. These values affected crawl discovery, indexability, stored raw/rendered metadata and canonical findings.

## Decision

Accept a canonical element only when its parsed parent is the document head and it has none of `hreflang`, `lang`, `media` or `type`, including empty attributes. Evaluate relation tokens case-insensitively and count each eligible element once. Preserve separate eligible declarations and existing URL/base resolution. Other link relations and body stylesheets keep their existing behavior.

This follows [Google Search Central's canonical documentation](https://developers.google.com/search/docs/crawling-indexing/consolidate-duplicate-urls). The implementation uses the HTML5 parsed tree, so source text written after markup that closes the head does not qualify. HTTP Link canonical handling remains available as the existing fallback when HTML contains no eligible canonical.

The same parser handles original HTML, rendered DOM and the fallback after a render failure. The correction applies to new crawls. No schema change or automatic archive rewrite is performed: saved canonical fields do not retain enough placement/attribute information to safely repair old observations without fetching and parsing again. Repeating a saved plan creates a separate corrected crawl.

## Validation

Three parser regressions first reproduce the previous failures, then cover body/template placement, empty and populated alternate-version attributes, independent alternate/stylesheet relations, repeated tokens, separate declarations and base URL resolution. HTML and injected-renderer integration tests check discovery, indexability, original metadata, HTTP fallback, canonical findings and structured JSON exports with HTML storage disabled. Native local-fixture QA exercises the same scenarios with real HTML and Chrome-rendered crawls.
