# ADR 0046 — Sitemap page associations with original evidence

Status: Accepted

## Context

The frontier normalized sitemap page URLs, while storage retained original entries and joined them directly to fetched URL strings. Fragments, URL parser normalization and configured query stripping broke this equality. Successful, noindex and failed pages appeared unknown in sitemap reports. Orphan/non-indexable findings could be missed and indexable pages falsely reported absent from the sitemap.

## Decision

Keep original sitemap entry URLs, source map URLs and lastmod. In new crawls, persist a nullable crawl_url beside each entry, computed with the same validated CrawlPlan normalization used by the sitemap frontier. This is a potential HTTP page identity, independent of scope/fetch eligibility. Unsupported or unparseable entries receive no fabricated identity. Sitemap source files keep their separate query-preserving fetch identity.

Schema 8 adds this optional column and joins reports through coalesce(crawl_url,url). Apply the same association to orphan, non-indexable and missing-indexable audits. Preserve all raw entry rows, including duplicates; emit page findings once per observed page. Filters and streaming exports use the same identities. No response, indexability or crawl depth is inferred for an unfetched target.

Expose Sitemap URLs in the explorer with original/crawl addresses, source, lastmod and page facts. Search both addresses and the source map. Select the normalized identity for inspection; legacy schemas retain their original selection behavior. Add Turkish/English labels and explain unknown observations in the table hint. The browser demo uses explicitly sample data with the same table/filter shape.

Prepared version 7 is still a supported read format. Opening/exporting/comparing it must not force migration or recompute saved findings. Versions below 7 and pending audits retain their existing preparation path; if migrated to 8, old entries get NULL rather than guessed associations. Bump the minimum read version whenever a future mandatory read contract requires it. No preference or dependency changes.

## Validation

Three regressions first reproduce unknown statuses for fetched pages. HTML, injected JavaScript and strip-all-query runs retain seven raw entries, map/source queries and lastmod while matching five actual URL observations. Duplicate references do not multiply findings. Noindex, HTTP 404, excluded and unsupported entries retain the correct known/unknown states. Tests cover orphan and sitemap finding counts, filters, paginated previews and complete filtered exports.

Store tests open, export and compare a prepared version 7 archive with version 8 while asserting byte-identical database and metadata files. A pending legacy audit migrates without inventing entry identities. All 565 Rust workspace tests pass (three ignored), and 134 frontend tests pass, including demo filtering/export and normalized inspection facts.

Native loopback runs exercise HTML, real Chrome and strip-all-query settings with optional header/source capture off. Each saves five actual page observations from seven raw entries, preserves the original map source including its query, and fetches each resource once. JSON and CSV agree. All runs report three orphan pages, two non-indexable sitemap pages and only the genuinely missing root entry; the single intended HTTP 404 remains a real failure and rendering succeeds.

The native explorer shows all seven raw entries with the normalized address and known/unknown facts. Its 4xx filter returns the one correct row, and selecting that row opens the normalized 404 URL without its stripped tracking parameter/fragment. An unobserved entry is not falsely highlighted or made interactive when no crawl identity exists. The screenshot records the complete table state with filters cleared. Strict clippy, formatting and frontend/desktop/CLI builds pass. The fixture server is closed and the test application's previous archive/view is restored.
