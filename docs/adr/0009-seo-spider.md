# ADR 0009 — octopusSEO Spider, the site crawler

- Status: accepted
- Date: 2026-10-16

## Context
Agencies audit sites with Screaming Frog's SEO Spider (500-URL free tier, £199 per year and seat) or open-source tools such as LibreCrawl (a Python/Playwright web app). We want a second octopusSEO program that crawls like a search engine, finds the technical SEO issues those tools find, and is faster, unlimited, private and built to work with the Log Analyser, so that what a crawler *can* find and what bots *actually* fetch are compared in one place.

## Decision
A separate program, **octopusSEO Spider** (`snx-spider` CLI and a desktop app), in this workspace and on the same layering as the Log Analyser:

- `seonexus-html` (domain): turns an HTML document into page facts (titles, descriptions, robots directives, canonicals, hreflang, headings, links with anchors and rel values, images, structured data, social tags, word count, content hashes) using `scraper` (html5ever, the parser browsers specify).
- `seonexus-crawl` (domain): crawl configuration and scope, the breadth-first frontier, robots.txt (the existing Google-compatible matcher), redirects recorded hop by hop, sitemaps (URLs only sitemaps list are crawled after the link graph, without a depth, which marks them as orphans), and the crawl engine driving a `Fetcher` port on a pool of worker threads.
- `seonexus-net` (adapter): the HTTP fetcher (ureq with redirects disabled, gzip, size and time limits).
- `seonexus-crawl-store` (adapter): each crawl in its own directory with a DuckDB database (`urls`, `links`, `images`, `headings`, `hreflang`, `structured_data`, robots.txt files and sitemaps, plus `tab_*` views for the tabs and exports), written through appenders in batches; a `crawl.json` beside it holds name, settings, status and counters, so listing crawls never opens a database.
- `seonexus-audit` (domain): issue rules with type, priority and explanation, computed over the stored crawl (per-page checks and site-wide ones: duplicates, broken links, redirect chains, orphan and deep pages, internal link score).
- `seonexus-spider-app` (use-cases), `apps/snx-spider` (CLI), `apps/spider` (Tauri desktop on the shared design language, `data-product="spider"`).

Milestones:

1. **S1 crawler core**: crawl modes (spider, list, sitemap), scope rules, robots.txt, user agents, rate limits, redirects, HTML facts, storage, CLI crawl and Excel/CSV/JSON export.
2. **S2 audit**: 100+ issues across response codes, redirects, indexability, titles, descriptions, headings, canonicals, hreflang, pagination, links and anchors, images, URLs, security, duplicates (exact and near), structured data, sitemaps, depth and link score; AI readiness (AI crawler access, `llms.txt`).
3. **S3 desktop app**: crawl setup, live progress, issue overview, URL tabs, URL detail (inlinks, outlinks, headers, source), filters, exports, saved crawls.
4. **S4 rendering**: optional JavaScript rendering through an installed Chrome or Edge (DevTools protocol), raw versus rendered comparison.
5. **S5 extensions**: custom extraction and search, crawl comparison, XML sitemap generation, PageSpeed Insights, scheduled crawls, site structure visualisations, and crawl data shared with the Log Analyser.

## Alternatives considered
- Async I/O (tokio + reqwest): more throughput per thread but a second runtime in the workspace; polite crawls are bounded by per-host rate limits long before threads are, and blocking workers keep the engine simple and testable.
- A headless browser for every page: slow and heavy; rendering stays optional per crawl, as in the tools it replaces.
- In-memory crawl data: limits crawl size by RAM; DuckDB keeps millions of URLs on disk with fast analytical queries.

## Consequences
- No URL limit, no licence; crawls are stored locally and can be reopened.
- The Spider and the Log Analyser share URL normalization, robots.txt evaluation, exports and the design language.

## Implementation status (2026-10-17)

S1 and the S3 desktop workflow are implemented in `apps/snx-spider` and `apps/spider`, with the same crawl workspace and exporters. S2 materializes 113 of the catalog's 114 rules where the required observations exist; near-duplicate detection is pending. Saved-crawl schema upgrades, bounded table queries, per-crawl UI exclusions, independent jobs and a native resident-memory sampler are included. S4 JavaScript rendering is implemented via the separate rendering adapter (ADR 0010); custom rendering CSS and S5 remain planned. Browser preview is explicitly a sample-data demo, and the crawler's frontier/discovered set still grows with site size; a million-URL scalability claim is not yet validated.
