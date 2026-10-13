# ADR 0006 — URL normalization and analyses on top of queries

- Status: accepted
- Date: 2026-10-12

## Context
SEO audits compare log data with other URL sources: crawler exports (Screaming Frog
`Address` column), plain lists and XML sitemaps. Logs record request targets
(`/a?utm_source=x`), the other sources absolute URLs (`https://www.example.com/a`). Agencies
also compare periods (before/after a migration) and want a crawl-budget summary.

## Decision
- `seonexus-url` normalizes URLs into a comparable key: scheme and host are split off (host
  lower-cased, default ports dropped), fragments removed, RFC 3986 percent-encoding and dot
  segments normalized, well-known tracking parameters (`utm_*`, `gclid`, `fbclid`, `srsltid`,
  …) removed. Lower-casing, trailing-slash folding and dropping all parameters are opt-in,
  because they change which resource a server returns on many sites. Normalization is
  idempotent (property-tested and fuzzed).
- `seonexus-analytics` holds the pure parts (matching, period trends, crawl indicators) and
  works on plain values; `seonexus-log-app` feeds it with query results, so analyses reuse the
  planner, rollups and verification of the query engine instead of having their own SQL.
- URL comparison runs in memory on the distinct URLs of a site (millions at most), not on hits.
  Logged URLs are restricted by a query filter; the default is verified search-engine crawlers,
  the population that "not crawled" refers to.
- URL lists are read leniently: sitemap and sitemap index XML (`<loc>`), CSV/TSV with an
  `Address`/`URL` column, or one URL per line; files or URLs, gzip-compressed or not. Sitemap
  indexes are followed up to 1,000 sitemaps.
- Results are tables (`TableSink`), so every analysis exports to CSV/XLSX/JSON like queries.

## Consequences
- Analyses stay consistent with reports: the same filter text means the same hits.
- Lists are not persisted; comparing again re-reads them (sitemaps change anyway).
- Very large sites with tens of millions of distinct URLs would need a DuckDB-side join; the
  boundary is `compare_url_list` and can be swapped without changing callers.
