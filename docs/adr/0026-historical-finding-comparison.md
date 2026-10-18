# ADR 0026 — Historical finding observations

Status: Accepted

## Context

URL field comparison cannot answer which archived SEO findings appeared or disappeared between crawls. A missing finding may reflect unavailable content, changed scope, changed rule definitions or a page becoming ineligible. Archives retain positive findings but no definition fingerprint or per-check evaluation/pass log.

## Decision

Add a Finding changes view to the existing read-only comparison, using exact `(issue_id, URL)` full-outer joins of saved audit_findings. Preserve unknown historical IDs and include locally excluded findings. Use the current shared catalog for priority, category and translated explanations, with an explicit provenance warning.

Classify each pair exactly once: introduced, not-detected, persisting, new-url, missing-url or unverified. Presence on both sides remains persisting. A finding on an only-current/only-previous URL is a scope observation and cannot establish remediation. Site-file findings without URL records remain available under all-URL scope; recurring records persist and one-sided observations remain unverified.

Shared-URL one-sided differences require completed, unlimited Spider runs with identical seeds and settings. Reject changed internal/external classification and unknown check applicability. Nine HTTP outcome checks require a usable counterpart HTTP observation. Other known rules use conservative successful, complete, indexable 200 HTML and successful required rendering as an absence guard. The original-noindex-removal check additionally requires a recorded original robots baseline. These guards establish observation suitability, not historical coverage or a passing check. No state is called fixed.

Summary totals cover the selected internal/all scope and URL/check/priority predicates, independently of state and pagination, and count URL/check pairs. Previous/current totals use the same predicates. This supersedes the initial whole-scope indicators that misleadingly remained at 105 when a selected check returned 100 new findings. Selecting a card preserves other filters. Summary query identity contains every non-state predicate; stale totals are hidden during refresh and exports wait for both summary and page. Translate actual archive warning codes instead of implying a settings mismatch for any capture failure. Keep the rule picker available for the full selected URL scope; preview the first 500 IDs by priority/ID with full rule total. Omit whole-scope counts beside filtered picker options. Table filtering by state, exact rule ID, priority and literal case-insensitive URL substring shares predicates with full export. Page 250 rows with backend bounds of 1–1000. Order by priority, state, rule and URL. CSV/XLSX/JSON stream all matches with 16 presence/context columns, ignoring pagination and retaining atomic destination preservation. Existing 30-field URL detail and 65-column URL export remain stable.

Use a composite serialized row key so different checks on the same URL open independently. Detail shows exact recorded presence, comparison reason, catalog explanation/fix and stored URL facts. A rule action clears unrelated filters. Pair, scope or view changes reset local table/detail state. Direction, pair and URL-scope changes preserve the selected URL/finding view; only the keyed result subtree resets. This prevents an unexpected switch to URL indicators while reviewing finding direction. Native live-job guards and read-only archive validation apply to all three new IPC commands. No schema or dependency update is needed.

## Validation

Store tests cover six disjoint groups and reversal, HTTP/non-HTML outcomes, noindex/404/render failures, required JS and unknown original robots baselines, partial/mismatched plans, orphan/unknown IDs, literal injection-like searches, reclassification, 1505-row export, 500-rule preview bounds and archive byte preservation. App tests check all three formats, ignored page limits and preservation of existing output on failure. IPC rejects invalid archives and unknown states. Frontend tests verify composite identity, filters, reason rendering, recurring/missing-scope semantics and reversible demo counts.

Native QA uses two real isolated loopback crawls of 505 URLs each. The fixture intends 400 successful title changes and 100 new missing descriptions. The recorded run has 399 comparable missing-title disappearances; one additional loopback request fails and is retained as unverified along with the new 404 and noindex page. All 100 missing descriptions are introduced. Verify table pagination, check/priority filters, exact-row detail, reversed direction and all-match CSV/XLSX/JSON equivalence beyond the first 250 rows. Compare source archive hashes before/after.

## Limits

This compares stored positive observations. It does not prove successful SEO fixes, historical audit coverage or identical rule definitions, and does not map rewritten URLs. Conservative guards can leave legitimate changes unverified. Joins/aggregation require resources proportional to archives; no million-URL benchmark is claimed.

Indicator regression tests compare every card against its matching state page across rule, priority, literal URL search, zero-result and pagination cases, for URL fields and finding pairs. Native regression QA reuses the same read-only 505-URL archives to verify 100/100 descriptions, 399/399 titles and three unverified title differences.
