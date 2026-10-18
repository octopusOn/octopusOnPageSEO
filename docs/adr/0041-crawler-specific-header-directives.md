# ADR 0041 — Crawler-specific HTTP indexing directives

Status: Accepted

## Context

The Chrome crawler preset intentionally has no crawler tokens. The shared directive parser treated an empty token list as matching every named X-Robots-Tag scope. A response meant only for Googlebot or Bingbot could therefore make Chrome report noindex, stop hyperlink discovery through nofollow, or inherit another crawler's snippet restriction. The same incorrect flags entered saved facts, raw/rendered comparisons, audits and exports, including non-HTML responses.

## Decision

An empty crawler-token list accepts only unscoped directives. Named scopes require an explicit matching token, using the existing case-insensitive header parsing. The scope continues through its comma-separated directives until another named scope begins. Each separate HTTP field starts unscoped. General directives and matching named directives still merge restrictively; index/follow and unlimited snippet values cannot erase applicable restrictions.

Keep the existing identities for Chrome, Googlebot Desktop/Smartphone, Bingbot and octopusSEO, including the existing octopusSEO alias. Meta tag names continue to select applicable crawler directives independently. The same selected identity governs original HTML, rendered DOM, failed-render fallback and HTTP-only resources such as PDFs. Preserve original X-Robots-Tag evidence even when it does not apply to the selected crawler and when full headers/HTML are not saved.

No schema, dependencies, configuration or interface changes are needed. Historical archives keep their original observations; a new crawl is required for corrected Chrome flags. Re-running an audit over old stored booleans cannot reconstruct missing crawl requests or replace source observations.

## Validation

The regression tests reproduce Chrome inheriting named noindex/nofollow and max-snippet values. Parser tests then cover mixed scopes, case differences, general directives, field boundaries and restrictive merging. Application tests exercise all five presets in HTML and injected JavaScript, plus failed-render fallback. They verify HTML/PDF flags, named and generic meta tags, raw versus rendered noindex/nofollow, exact audit URL sets, original header evidence in complete exports, and whether a child linked only from a Googlebot-nofollow page is actually fetched. Headers and HTML storage are disabled throughout.

Six native loopback crawls exercise Chrome, Googlebot Desktop and Bingbot with HTML and real Chrome rendering. SQL facts and complete internal/rendering/issue exports match each expected scope, including original header strings, PDF flags, source/DOM metadata and exact finding sets. The child is requested once by Chrome/Bingbot and never by Googlebot; every seed main document is requested once, with the configured HTTP user agent. No HTTP or render error is invented. The fixture server is closed afterwards. All 526 Rust workspace tests pass (three ignored), together with strict clippy, formatting and desktop/CLI builds. Frontend code is unchanged; its prior 133 passing tests remain the baseline.

Reference: [Google Search Central: robots meta and X-Robots-Tag scopes](https://developers.google.com/search/docs/crawling-indexing/robots-meta-tag#xrobotstag).
