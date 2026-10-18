# ADR 0038 — Structural XML sitemap discovery

Status: Accepted

## Context

Sitemap parsing searched strings for tag names. Valid prefixed sitemap/index elements and numeric character references were missed, while comments, nested extension elements and HTML containing sitemap-like text could create phantom discoveries. CDATA content was incorrectly entity-decoded. A malformed XML document could publish its valid-looking prefix as a successful sitemap. A cut HTTP response or the gzip reader's byte cap could also silently publish partial observations.

## Decision

Read XML events with namespace resolution using quick-xml 0.42.0, already present in the workspace lockfile. Support the standard sitemap namespace with default or named prefixes, preserving existing unnamespaced XML compatibility. Read only direct core entries under the actual urlset/sitemapindex root and direct core loc/lastmod children. Ignore comments, processing instructions and unrelated extension vocabularies. Join XML text, CDATA and predefined/numeric references correctly, decoding references once and retaining literal CDATA contents.

Require one closed sitemap root and matching elements, valid UTF-8/XML characters and references, valid attributes, and bounded nesting. Reject DTD/entity declarations rather than resolving external content. This is structural parsing, not XSD/protocol validation: absent/empty loc entries remain skipped and empty maps remain representable. Non-sitemap roots, malformed/truncated documents, unknown entities and unsupported encodings are errors and publish no partial entries.

Keep text sitemap support. Apply the existing 50 MiB uncompressed cap to both plain and gzip inputs, reading one extra byte to detect overflow. Read all gzip members and validate the trailer/checksum even at the exact byte cap; reject corruption instead of accepting a prefix. Reject a known truncated HTTP sitemap response before parsing. Preserve its actual HTTP status and parsing error in the sitemap report and live activity.

No schema or preference migration is needed. Existing archives are unchanged; a fresh crawl collects corrected discoveries. Scope, URL budgets, normalization, backlog order and the configured sitemap count limit continue to apply.

## Validation

Regression tests first reproduce missed prefixed files, phantom/comment URLs, wrong CDATA decoding, accepted incomplete XML and discovery from a known truncated HTTP response. Tests cover default/prefixed namespaces, foreign extensions, mixed text/CDATA/references, lastmod, empty maps, malformed XML, UTF-8, DTD/unknown references, excessive nesting, plain/gzip size boundaries, checksum/trailer failure and concatenated gzip members. An engine regression follows a prefixed index, crawls the legitimate decoded orphan URL, retains lastmod and errors, and sends no request for commented, foreign or incomplete entries.

Native CLI QA crawls an isolated loopback site in HTML and real Chrome JavaScript modes, with source/header storage disabled. Both save six HTTP 200 pages: the seed plus five sitemap-only pages from a prefixed gzip URL set and a multi-member gzip text list. Seven sitemap operations retain four distinct errors (incomplete XML, HTML root, truncated HTTP body and corrupt gzip checksum) with no partial entries. Complete JSON exports preserve decoded query strings, literal CDATA, lastmod and unknown click depth. All six JavaScript pages render successfully; each original response is requested once and no phantom URL is requested. The fixture server is closed afterwards. Native UI shows all seven sitemap rows and the matching four-row error filter. The workspace passes 501 tests (three ignored), strict clippy, formatting and desktop/CLI builds.

References: [Sitemaps protocol](https://www.sitemaps.org/protocol.html), [XML 1.0](https://www.w3.org/TR/xml/).
