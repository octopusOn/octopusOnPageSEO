# ADR 0034 — HTTP hreflang reporting and PDF audits

Status: Accepted

## Context

HTTP Link alternates entered discovery and the link graph but were absent from the saved hreflang table. Reports and language-version audits consequently missed header-only annotations, including PDFs, and HTML return links supplied through headers could be falsely reported missing. Optional header storage did not solve this gap. A Link entry also retained only its last hreflang parameter.

## Decision

Keep header hreflang observations on each URL record independently of optional HTTP-header or HTML storage. Preserve every hreflang parameter on an alternate Link entry, including invalid/empty codes for validation. Store the union of analyzed HTML/DOM and response-header annotations in the existing hreflang table. Merge identical language/target mappings per source URL, comparing language codes without ASCII case and retaining the first spelling; distinct languages or targets remain separate. Metadata targets retain their resolved URLs independently of frontier normalization and follow settings. The link graph continues to record declarations separately.

The Hreflang explorer and streaming exports use the same saved table. Existing invalid-code, target-response and indexability checks now receive header annotations. Self-reference, x-default and return-link checks also cover successful internal PDF responses. New crawls record `http_hreflang_observed=1` in crawl_info; without it, historical PDF header absence remains unknown. Return-link absence requires an analyzed internal HTML target or an internal 200 PDF with this capture marker. External checks and unparsed HTML status checks do not prove missing returns; external annotations do not create site hreflang findings. Unknown or unfollowed targets remain unknown.

[Google's localized-versions guidance](https://developers.google.com/search/docs/specialty/international/localized-versions) supports HTTP annotations for PDFs and requires bidirectional/self annotations. Repeated language hints follow [RFC 8288 section 3.4.1](https://www.rfc-editor.org/rfc/rfc8288.html#section-3.4.1). The coverage marker and deduplication policy are application decisions. The existing x-default finding remains a recommendation.

No schema migration, old-archive backfill or automatic audit refresh is introduced. New crawls collect corrected measurements; existing archives keep their original data and coverage.

## Validation

Four integration regressions first reproduce missing report rows, then verify HTML, generated DOM, failed-render fallback and list/unfollowed modes with both optional source stores disabled. They cover mixed HTML/header returns, PDF pairs, repeated headers, multiple language parameters, invalid codes, broken/noindex targets, foreign anchor exclusion, exact report/export rows and target statuses, and affected finding URLs. A parser regression retains multiple/empty language parameters. An audit regression toggles the capture marker to verify conservative historical PDF behavior while preserving HTML checks, excluding external findings and treating unparsed HTML as unknown.

Native CLI QA verifies HTML, real Chrome and list crawls against a temporary local server. They retain 18, 22 and 11 unique annotations respectively; every CSV mapping matches its JSON counterpart. Both full crawls report the six intended findings on the problematic PDF, while list mode reports only three findings and keeps five target statuses unknown. Ten PDF annotation rows are retained in each full crawl, despite storing neither headers nor HTML. The JavaScript run renders five HTML pages and discovers the generated alternate. The server closes afterward; source archive hashes remain unchanged.
