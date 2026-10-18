# ADR 0062 — Overall technical SEO score on the overview

Status: Accepted

## Context

The overview lists URL counts and findings but lacks an overall technical-health indicator with visible causes. Raw finding counts cannot be converted into a trustworthy score without preserving URL identity, overlap, scope and missing observations.

## Decision

Calculate model v1 on demand from saved internal URL observations and materialized audit findings, without migrations, archive writes or network requests. Each distinct internal URL starts at 100; each distinct recognized high/medium/low-priority finding deducts 20/7/2 points. Cap each URL's total deduction at 100, then average over the observed internal sample and round to one decimal. External URL volume and site-level file findings outside that sample cannot dilute or change it. Resources and HTTP failures within the sample are included. UI hiding/exclusion of findings does not alter stored evidence or the score.

Apportion each URL's capped deduction proportionally among its findings, then aggregate by rule. These contributions sum to the unrounded overall deduction and explain the top drivers. They are attribution, not a promise that fixing one rule recovers that exact amount while other caps remain active.

Return no score when the sample is empty, no HTTP 200 HTML page with observed page facts exists, or an internal finding uses an unknown rule. Mark stopped, failed, URL-limited and list crawls, or incomplete HTTP/render observations, as limited assessments. A completed crawl is still only its configured sample. Grade thresholds are 90/75/50, with lower values requiring priority improvement. The score is an application heuristic, not a Google ranking score or a full-site coverage claim.

Expose a 0–100 accessible meter above existing overview cards, a grade, sample/finding counts, the three largest deductions with links to their findings, and an expandable explanation of the formula and scope. Old backends without score data show unavailable rather than an invented value. The browser demo mirrors the same versioned model; production consumes the native result.

## Validation

Test severity weights, deduplication, caps, driver conservation, improvement after removing evidence, absence of external dilution, unknown rules, empty/unobserved samples and incomplete rendering. Test UI unavailable/version states, partial-crawl labels, the formula and independence from hidden findings. Verify the native UI on a private copy of the current Nexus archive against an independent calculation, preserve user archive hashes and retire the private fixture.
