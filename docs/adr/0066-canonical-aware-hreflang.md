# ADR 0066 — Canonical context for hreflang absence checks

Status: Accepted; extends scoring to model v3.

## Context

A parameterized URL can declare its primary URL as canonical and repeat the primary language cluster. Exact source-URL reciprocity/self checks then create duplicate warnings and penalties even when that primary cluster is reciprocal. Numeric pagination parameters also occur on genuinely distinct content; dropping all parameters or treating every canonical declaration as authoritative would hide real problems.

## Decision

Share one read-only SQL context across new audits, current issue reports, URL inspection/filtering, hreflang tables/exports, scoring and remediation. Resolve a canonical source only with observed, successful, untruncated internal HTML; unambiguous declarations; a healthy, indexable, self-canonical representative; compatible page languages; a language-specific canonical self annotation; and source annotations present in the representative's cluster. Failed/incomplete renders, canonical chains/conflicts, unknown targets and noindex pages do not establish this context.

Keep exact URL returns separate from canonical returns. Remove a source's historical missing-return finding only when all confirmed missing-return witnesses resolve through this context. A mixed genuine failure stays actionable with only its unresolved witnesses. Missing-self and optional default copies require the corresponding canonical evidence. Never silently canonicalize an alternate target; its invalid language, response or indexability findings remain. Never require translations of single-language content.

Numeric pagination is a review hint, not proof of duplicate content. Expose it and observed text-hash equality separately, retaining existing canonical/pagination findings. Canonical remediation explains the distinct-content/self-canonical case and genuine-duplicate case; it does not propose return tags to a parameterized source before canonical intent is resolved. Link [Google's pagination guidance](https://developers.google.com/search/docs/specialty/ecommerce/pagination-and-incremental-page-loading), [canonical guidance](https://developers.google.com/search/docs/crawling-indexing/consolidate-duplicate-urls) and [localized-version guidance](https://developers.google.com/search/docs/specialty/international/localized-versions).

The canonical exception is an application policy for avoiding duplicate absence warnings, not a claim about Google's chosen canonical. Do not infer content equivalence from a declaration or query string. Return null rather than false for exact return observations on unchecked targets.

Preserve saved database bytes and original `audit_findings`; current built-in reports apply the context at read time. SQL and historical comparisons retain recorded findings/provenance and their existing non-indexable absence guards. No schema migration, network recrawl or archive mutation is required. Score v3 retains weights, per-URL caps, Cloudflare exclusions and contribution accounting, and adds a separate count of historical canonical-context exclusions. Frontend accepts known models v2/v3 and rejects unknown models.

## Validation

Regression fixtures cover canonical duplicates, distinct-content pagination, exact versus canonical returns, mixed real failures, self-canonical pagination, header canonical fallback, optional default context, broken alternates, untranslated pages and nine invalid/unobserved canonical states. Current findings, URL filters, overview counts, score deductions, remediation witnesses and raw-record preservation are checked together. Existing archive, comparison, parser, export, desktop and CLI tests remain applicable. Frontend checks cover v3 compatibility, exclusion copy and conditional remediation guidance.
