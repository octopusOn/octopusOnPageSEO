# ADR 0029 — Prepare selected comparison archives

Status: Accepted

## Context

Comparison and sitemap tools require current, finished archives and open them read-only. After an analysis schema update, selecting an older archive reports that it must first be opened in the explorer. The comparison page had no direct recovery action, requiring users to navigate away and reopen both archives separately.

## Decision

Add an explicit **Prepare selected archives** action beside the selected comparison pair. Reuse the normal saved-analysis overview operation to open the baseline, then the current archive. Those operations already perform any required schema migration and transactional audit refresh from saved data; they do not crawl the website. Comparison queries and exports themselves remain read-only. No new backend command, schema or dependency is required.

Before opening either archive, validate two different finished selections and request a fresh jobs snapshot. Refuse active, paused, stopping or finalizing selected jobs, even if metadata already says completed. If status cannot be verified, open neither archive. Perform updates sequentially and stop at the first failure. Updates are atomic per archive, not across the pair; a completed first update remains valid if the second fails, and preparation can be retried.

Disable pair/direction/scope/view controls during preparation and remove result/export controls until it ends. Preserve the chosen archive pair, URL scope and URL/finding view. Reset result filters, pagination and open details when results remount. Invalidate cached results on success or failure before remounting, including after partial updates, so the 30-second query cache cannot reuse obsolete findings.

## Validation

Orchestration tests verify exact selected identifiers, sequential opening, metadata immutability, duplicate/unfinished rejection, fresh active-job checks on either side, status-read failure, unrelated active jobs, and first/second archive failures. Native QA uses isolated copies of two completed 505-URL schema-6 archives. The old-archive error is reproduced in both comparison views, preparation disables the relevant controls, and both archives become usable without a new crawl. The chosen finding view/pair remains selected. The original 4,556 finding-pair partition remains consistent; the no-longer-recorded card/table total is 399 and native CSV exports all 399 rows beyond the first 250-row page. URL comparison also opens after preparation. Stored observation-table fingerprints and metadata are checked against the source copies; original user archives are untouched.
