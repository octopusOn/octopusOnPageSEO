# ADR 0017 — Original and rendered SEO signals

Status: Accepted

## Context

Rendering diagnostics retained original titles and counts but omitted descriptions, canonical and robots directives. A rendered page could remove an original noindex without an audit identifying that difference. Failures use original HTML for analysis, which must not be presented as a successfully captured DOM.

## Decision

Capture the original description, first HTML canonical (HTTP Link canonical fallback), and effective noindex/nofollow before invoking the renderer. Effective robots directives merge HTTP and page directives using the configured agent. Persist this compact baseline independently of HTML storage. Schema 5 adds nullable fields; historical rows remain unknown, rather than inventing false flags or absent metadata. Pending-audit migration behavior remains transactional.

Use the existing detail facts for an inspector comparison of URL, title, description, canonical, robots flags, words and hyperlinks. Mark changed values only when both observations exist. Rendering rows open the comparison directly. The rendering table and streaming exports carry the additional signals. Failed renders have null after-values; raw fallback facts remain available. Truncated DOM has an explicit incomplete warning.

Add the high-priority `render-noindex-removed` rule for successful, untruncated internal 200 HTML pages whose known original noindex becomes false. Header noindex that continues to apply, failures, truncation and historical unknowns do not qualify. This follows [Google Search Central](https://developers.google.com/search/docs/crawling-indexing/javascript/javascript-seo-basics#use-robots-meta-tags-carefully), which explains that initial noindex may prevent rendering. Measured DOM indexability remains unchanged; this warning does not assert how any search engine indexed the page.

## Validation

Integration tests cover changed and absent metadata, HTTP canonical fallback and restrictive headers, added/removed/unchanged noindex, render failures, truncated DOM, comparison without saved HTML, exports, and old-schema upgrades with unknown baselines. Frontend tests distinguish failures, legacy values, absent values and partial DOM, and verify safe text rendering. Native QA uses a separate localhost crawl with real Chrome before updating the main application.
