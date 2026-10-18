# ADR 0035 — Hreflang finding comparison coverage

Status: Accepted

## Context

Hreflang comparisons used the general indexable-HTML absence guard. An older archive without retained HTTP annotations could therefore make a newly observed HTML finding look introduced, or an older finding look no longer detected. Conversely, fully captured PDF observations could never qualify as comparable because the guard required HTML and sometimes JavaScript rendering.

## Decision

For the seven known hreflang rules, infer one-sided finding changes only when both archives record `http_hreflang_observed=1`, in addition to the existing completed/unlimited Spider, matching-plan, scope and catalog checks. Missing coverage yields an unverified difference and names the side without recorded coverage. Recurring positive findings and one-sided URL membership keep their existing states.

Accept indexable 200 PDFs as hreflang counterpart observations once capture coverage is established. Their annotations come from response headers, so body truncation and required JavaScript rendering do not invalidate header observations. HTML still requires parsed page facts plus the existing HTTP, indexability, completeness and rendering guards. Other content types remain unverified. This does not broaden non-hreflang rules or prove historical rule-definition identity; the existing provenance warning remains.

Summary cards, pages and full exports share this classification. Add Turkish/English explanations for missing coverage, unsupported response types and HTML that was only status-checked; use a general indexability explanation that also fits PDFs. No archive writes, schema migration or recrawl is required to apply these comparison guards. Original findings remain unchanged.

## Validation

Three regressions reproduce legacy coverage, PDF eligibility and unparsed-HTML mistakes before the fix. They verify both directions, recurring presence, unrelated rule behavior, filtered state totals, all 16 export columns, exports independent of page limits, source archive byte preservation, PDF truncation and required-JS exemptions, and conservative noindex/render-failure handling. Frontend tests verify both side-specific reason mappings and rendered Turkish coverage explanations. Existing comparison, scope, filtering and export regressions remain.

Native QA uses two four-URL real Chrome crawls with identical plans. In an isolated desktop instance, their five PDF hreflang pairs partition into one introduced, one not detected and three persisting. Filtering PDF URLs also includes three persistent HTTP warnings, giving eight total pairs; the 16-column CSV matches these states and exact finding URLs. State-card clicks select the correct single PDF, reversal swaps its state, and a selected self-reference rule shows one introduced plus one persisting row. Source archives and QA copies retain their hashes. The private app closes after verification.
