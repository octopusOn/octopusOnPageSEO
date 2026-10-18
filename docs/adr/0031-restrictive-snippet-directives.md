# ADR 0031 — Restrictive snippet directives

Status: Accepted

## Context

Repeated `max-snippet` declarations used the last parsed value. A later unlimited value, a larger finite limit or an invalid value could therefore erase an earlier restriction. The same overwrite occurred when combining separate HTTP headers and HTML meta tags, making a `max-snippet:0` finding disappear.

## Decision

Combine valid snippet limits by their restrictiveness: zero is the strongest restriction, positive limits use their minimum, and `-1` supplies no explicit limit. Ignore malformed, overflowing and unsupported negative values without removing an existing valid limit. Use the same combination for repeated declarations and `Directives::merge`, so HTTP, matching meta tags and analyzed JavaScript DOM share the behavior.

This implements the restrictive-rule principle and special snippet values in [Google Search Central's robots specifications](https://developers.google.com/search/docs/crawling-indexing/robots-meta-tag). Existing `nosnippet` flags and crawler scoping remain independent. A snippet restriction alone does not make a page non-indexable.

The correction applies to newly collected analysis. Existing archives retain their observations; there is no schema migration or automatic rewrite. Repeat a saved plan into a separate crawl for corrected measurements. Original/rendered metadata gains no invented fields.

## Validation

Parser regressions first reproduce the previous overwrite and then cover declaration order, unlimited/zero/finite boundaries, the largest stored integer, invalid values, matching crawler scope and multiple meta tags. HTML and injected-renderer integration tests check saved limits, indexability, exact finding URLs and full JSON issue exports with HTML storage disabled, including a DOM-generated restriction.

Native CLI QA uses a temporary local HTTP server and the shared production crawl engine. HTML and real Chrome runs each store ten indexable URLs; their complete JSON issue exports contain six and seven expected snippet findings respectively, including the DOM-generated restriction only in JavaScript mode. Repeated HTTP header fields, finite/unlimited controls and invalid values are verified in saved facts. The server closes after the check, and the main desktop host is rebuilt and reopened with its original saved crawl.
