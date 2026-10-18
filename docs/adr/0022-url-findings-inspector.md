# ADR 0022 — Stored findings in the URL inspector

Status: Accepted

## Context

The inspector showed page facts, links, response diagnostics and duplicate groups, but not the audit findings for the selected URL. Reviewing a page's findings required returning to the site-wide issues list and repeatedly reopening filtered results. Navigation from a finding also needed to clear unrelated URL/link/response filters so the affected set remained complete.

## Decision

Extend the existing URL-detail response with a findings table selected by exact stored URL equality, sorted by rule ID, with at most 1,000 rows and the full total. This shares the inspector's existing bounded table contract. Do not infer findings from URL substrings, response facts or catalog eligibility. Unknown historical rule IDs remain in the response. No audit recomputation, migration, source writes or network requests are needed.

Add a URL findings tab showing the stored count and a priority-ordered list. Known rules use the shared Turkish/English catalog's title, category, severity, explanation and correction. Unknown rule IDs remain visible with an explicit missing-catalog message. Empty stored results do not imply that every check ran or that a page is error-free; an unrecorded URL and an older backend without this field receive separate messages. A truncated preview shows the full count.

Use the crawl's existing local exclusions to hide excluded findings initially, with a reversible Show excluded toggle. Selecting an excluded rule explicitly reveals it; stored findings and exclusions remain unchanged by inspection. Each known rule offers the existing full affected-URL workflow. Clear URL search, HTTP/indexability filters, exact link/duplicate scopes, sort, selection and pagination before opening that rule; the app clears the response outcome. Preserve the dedicated rendering, duplicate and content-match entry points. Other issue-filtered rows open this tab directly with the selected rule expanded.

## Validation

Backend tests cover exact URL equality with quotes and literal search metacharacters, neighboring URL isolation, unknown URLs, unknown historical IDs, a 1,005-result bounded preview, and parity with real crawler audit output. Frontend tests cover priority ordering, stored counts, explanations, exclusions and selected excluded rules, unknown IDs, safe text rendering, incomplete previews, unavailable data and demo parity. Native QA uses an isolated copy of the user's completed crawl to verify selection, accordion explanations, exclusions, complete affected-URL navigation and export without altering the source files.
