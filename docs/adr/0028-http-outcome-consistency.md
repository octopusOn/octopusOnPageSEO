# ADR 0028 — Consistent HTTP outcome classification

Status: Accepted

## Context

The overview partitions observed HTTP responses into bounded status classes. The server-error audit and the crawler's indexability classification instead accepted every status of 500 or above. A saved 600 or 999 response therefore appeared under Other HTTP in the overview and as a 5xx error elsewhere. Informational statuses could also receive a no-response label despite having a stored response.

## Decision

Bound internal/external server-error findings and the crawler's server-error category to 500–599 inclusive. Add the non-indexable `other-http` indexability category for observed responses outside 200–599. Keep robots blocking precedence and reserve `no-response` for absent HTTP responses. Translate the new category using the existing Other HTTP label in result tables; retain the stable serialized value in exports and comparisons.

Schema 7 normalizes only the derived indexability label for these unusual HTTP responses, preserving robots blocks, and requests the existing transactional audit refresh. Ordinary explorer access updates finished archives without fetching their websites. Read-only archive tools retain their existing requirement to open older archives in the explorer first. Stored HTTP codes, content, links, rendering facts, crawl settings, timestamps and metadata are not rewritten by this correction.

## Validation

Boundary checks cover 199/200/299/300/399/400/499/500/599/600/999, absent responses and robots-block precedence. Store regressions verify internal/external finding totals, overview category counts and complete filtered exports beyond a one-row page. A simulated schema-6 archive refresh removes stale 600/999 server findings, corrects derived categories and preserves every other URL column and metadata byte.

Native QA uses a local site with internal and external 499/500/599/600/999 endpoints. Its eleven URL records partition into one 2xx, two 4xx, four 5xx and four Other HTTP responses. Each internal/external server-error finding covers only 500 and 599. Other HTTP rows retain their actual 600/999 codes and use the Other HTTP indexability label. No external website is involved.

The browser-only sample archives use matching 4xx/5xx indexability labels and bounded finding classes. Their saved crawl/error counters match their actual rows, and every sample URL has a complete record so response-time averages remain finite. During simulated crawling, progress errors update together with the activity counter. Regression tests check both snapshots across overview, paginated filters, full exports and URL findings, plus intermediate and completed simulated progress. These fixtures do not fetch sites or update native archives.
