# ADR 0025 — XML sitemap builder

Status: Accepted

## Context

The Spider reads existing sitemaps and audits their entries but could not create one from observed eligible pages. A generator must respect [the sitemap protocol](https://www.sitemaps.org/protocol.html), avoid unsupported URL/canonical observations, and preserve saved archives and existing output on failure. [Google's guidance](https://developers.google.com/search/docs/crawling-indexing/sitemaps/build-sitemap) favors canonical URLs, requires truthful modification dates and ignores priority/changefreq.

## Decision

Add an XML sitemap tool to the desktop Tools navigation for the selected finished crawl. Verify live job status before enabling it, reject active/finalizing jobs in native commands, and reject running archive metadata in the shared app/store. A dedicated archive reader opens only current, fully analyzed schemas READ_ONLY; it never migrates or refreshes an archive. Older/pending archives must first be opened in the normal explorer.

Stream URL observations in exact address order, including one rendering join and duplicate URL count. Include only internal, unblocked, HTTP 200 HTML records with no request error, no observed effective noindex before/after JavaScript, no incomplete response/DOM, no failed rendering and no conflicting/non-self HTML or HTTP canonical. JavaScript crawls additionally require a saved successful rendering observation. Other non-indexable records, duplicate stored addresses, unsupported/unnormalized URLs, credentials, fragments, malformed percent escapes and loc values of 2048 or more characters are excluded. Canonical absence is allowed; this generator does not assert Google's canonical or indexing decision.

Treat scheme, host and non-default port as separate origins. Select one origin per export; the default is the origin with the most eligible URLs (lexical tie-break). Show eligible counts by origin and disjoint first-reason exclusions covering all stored records, including external records and eligible URLs from other origins. Preview the first 100 selected URLs while exporting every eligible URL. Report partial/list coverage and explain that undiscovered pages are not included. URL validation streams; origin counters, fixed-size exclusion counters, a bounded preview and at most 50,000 part filenames are retained, not every eligible URL.

Generate deterministic UTF-8 urlset XML with escaped text. Do not infer lastmod from crawl time or emit priority/changefreq. Enforce at most 50,000 URLs and 52,428,800 uncompressed bytes per document, including the envelope. Planner and ZIP writer share the part-boundary tracker. A single XML export is disabled/rejected when multiple documents are needed. ZIP exports stream protocol-sized sitemap-00001.xml parts and sitemap.xml as an index, with independently checked index URL/count/byte limits. Index locations refer to the selected origin's root. Explain that the index and every part must be extracted/uploaded there; no publishing or submission takes place.

Both formats write a sibling temporary file, finish/flush and sync it before atomic replacement. Require an output extension matching the chosen format. Failures preserve existing destinations and archive data. ZIP reuses the existing workspace zip 9.0.4 dependency; it adds no new package version. Browser demo supports labeled sample preview/XML only; native app supports ZIP and real saved results.

## Validation

Store tests cover disjoint eligibility, canonical conflicts, raw noindex, rendering failures/missing observations/truncation, exact origins, empty scopes, literal invalid input, normalized URL/percent/length checks, all XML entities, 50,001-URL plans, 50 MiB boundaries, bounded previews and read-only pending/running archive guards. App tests parse generated ZIP parts/index, verify 50,001 URLs split 50,000/1, preserve existing output on invalid selection/format, and verify archive bytes before/after XML generation. Desktop IPC checks command argument names and format deserialization; frontend tests verify live-status/export guards, escaped preview, disjoint totals, coverage warnings and sample demo eligibility.

Native UI QA uses an isolated byte-identical copy of the completed Nexusmedya crawl. Compare the 177-URL XML and ZIP exports against an independent saved-data SQL selection; parse the XML namespace/index/parts and ensure URL equality beyond the 100-row preview. Check original and QA archive hashes after export and restart.

## Limits

This release exports HTML page URLs only. No image/video/news/hreflang extensions, manual URL inclusion, modification dates or custom upload directory mapping. ZIP packages assume publication at the origin root; cross-host sitemap submissions are not modeled. Eligibility reflects the saved crawl, including historical rendering failures; a new crawl is needed for changed site observations. No million-URL performance claim.
