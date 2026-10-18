# ADR 0063 — SEO anatomy and infrastructure scoring exceptions

Status: Accepted; extends ADR 0062 with model v2.

## Context

Historical crawls contain Cloudflare email-protection placeholders that produce a broken-link finding on many legitimate pages. Excluding these addresses from future crawls does not correct the score of an existing archive. Users also need to see where every score deduction comes from rather than only the top three rules.

## Decision

Reuse the strict shared default URL-exclusion patterns for known Cloudflare infrastructure. Exclude matching internal URL records from the scoring sample. On source pages with such references, recheck only the eight applicable reference-based finding types against remaining link evidence. Retain a stored finding when genuine evidence remains. Keep all other recorded findings authoritative, including unknown rules, and preserve raw audit evidence unchanged. Do not introduce another setting or silently turn arbitrary user patterns into retroactive scoring exceptions.

Keep the severity weights, per-URL cap and global contribution accounting of ADR 0062. Return excluded URL/reference/finding counts and counts by rule. Version the changed sample/evidence policy as v2. All calculations are read-only and require no crawl or archive migration.

Return all 21 catalog categories with independent branch health, eligible/evidenced and affected URL counts, defined-check count and complete global-loss attribution. Use recorded HTML, URL, image, rendering, pagination and sitemap observations appropriate to each branch; never claim measured branch health without evidence. Site-file-only findings outside the internal sample remain outside the global score. Defined checks without recorded findings are labeled accordingly, not presented as proven passes.

Group branches into nine interactive systems on a human-shaped SVG map. System selection, branch expansion, search and loss-only filtering expose every catalog check and a direct link to the original findings. Distinguish a branch's independent health from its attributed global loss. Category contributions sum to the exact global loss before display rounding. Very small positive losses remain visible, and zero losses never display negative zero.

## Validation

Test strict pattern boundaries, Cloudflare-only and mixed genuine link failures, empty/generic anchors including Turkish Unicode, unchanged stored findings, all-category coverage and loss conservation. Test the UI's system coverage, excluded counts, no-data states and small/zero point formatting. Verify the native app against an independent calculation on a private copy of a real archive; preserve user archive hashes and retire the owned fixture.

## Follow-up — actionable remediation

At the user's clarification, remove the human-shaped map and keep the nine systems as ordinary filters. Add a prioritized solution plan using the existing localized audit catalog, without changing scoring. Rank recognized positive scored findings by severity, current global point attribution, affected URL count and rule ID. Show the first three actions and allow expansion to all; include solutions, explanations and evidence access. Show the same solution and global action rank beside each contributing branch check. Ignored-only findings and unavailable assessments must not produce recommendations. Displayed deductions remain attribution rather than guaranteed recoverable points. Label the raw archive list separately so it does not contradict the recommended order.
