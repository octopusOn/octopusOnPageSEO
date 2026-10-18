# ADR 0069 — octopusSEO Keyword Miner: keyword discovery and clustering

- Status: accepted
- Date: 2026-10-18

## Context
Agencies plan content around groups of keywords that one page can rank for. Today that work is split across tools:
- suggestion scrapers and Keyword Planner exports to collect keywords;
- spreadsheets to merge variants;
- paid clustering services that charge per keyword.

Turkish keywords add problems those tools handle poorly:
- inflection: `kahvesi`, `kahveler`, `kahvesinin`;
- diacritic-less typing: `turk kahvesi`;
- Yandex next to Google.

The owner asked for a fifth program that discovers keywords and clusters them, independent of the SERP Finder: own data folder, keys and budgets.

External facts that shape the design (checked 2026-10-18):
- **Google Ads API:** keyword planning (v26) needs Basic access since developer tokens were retired on 2026-09-09. Its terms describe tools for Ads campaigns.
- **Yandex Wordstat:** statistics (Search API v2) are limited to 100 requests per hour by default.
- **Bing:** Bing Search APIs, including Autosuggest, were retired on 2025-08-11.
- **Autocomplete:** Google and Yandex have no official autocomplete API. DataForSEO and SerpApi sell sanctioned access.
- **SERP-overlap clustering:** this is the industry standard (SE Ranking, Serpstat, Keyword Insights). It uses 3–4 shared URLs in the top ten, in soft (against the pillar) or hard (against every member) mode, and costs one result page per keyword.

## Decision
- A separate program, **octopusSEO Keyword Miner** (`snx-kw` CLI, `apps/keywords` desktop, data folder "octopusSEO Keyword Miner"). It shares code with the SERP Finder but never its data.
- **`seonexus-keywords`** (domain, pure and deterministic):
  - Normalisation gives each keyword a display text, an identity that merges case, diacritic, dash and spacing variants, and word stems (Turkish suffixes removed with `seonexus-text`, stop words kept because `nasıl` and `fiyat` carry intent).
  - It also holds the discovery expansions (a–z, Turkish letters, question and buying words) and filters (whole-word phrases ignoring case and diacritics, expressions, volume and length).
- **Two clusterings, combined:**
  - **Wording:** IDF-weighted Jaccard similarity of stem sets, with candidates from an inverted index. Pillars are taken by search volume and each takes the free keywords at or above the threshold (default 0.6). Keywords with the same stems always meet. It is free and instant, for tens of thousands of keywords.
  - **Result pages:** shared top-ten URLs, soft or hard, threshold 1–9 (default 3). It is exact but costs a page per keyword.
  - **Hybrid:** wording first, then result pages for the chosen keywords; keywords without a page stay in their wording cluster, marked unverified.
- **Intent of a keyword alone:** `seonexus_serp_score::keyword_intent` reuses the SERP Finder's modifier lists and returns nothing when a keyword states no intent, instead of a default.
- **Sources:** Wordstat, DataForSEO (Keywords Data, Labs, autocomplete), Google Search Console and Google Ads through the user's own Google OAuth client (loopback and PKCE), SerpApi autocomplete, and file imports. The free Google and Yandex suggest endpoints are off by default and opt-in, with a terms warning, an honest user agent, a fixed slow pace, a daily cap and a stop on refusals.
- Further milestones (store, sources, OAuth, desktop, release) are listed in the plan and recorded below as they land.

## Consequences
- Clusterings can be repeated and explained: every member carries its similarity, or its shared URLs, to the pillar.
- Wording clusters cost nothing, so whole discovery runs can be grouped before any paid result page is fetched.
- Google Ads remains optional because of its access requirements and terms; DataForSEO and Wordstat provide volumes by default.

## Implementation status (2026-10-18)
- **KW1** (done): `seonexus-keywords` with normalisation, expansions, filters and wording clustering (property tests), plus `Analyzer::all_stems` and `keyword_intent`.
- **KW2** (done): result-page clustering (soft/hard, page/site), hybrid, cluster totals and site mapping. 50,000 keywords cluster in about 2 s in a release build.
  - A cluster's intent is the stated intent with the most searches, unless more searches state none. So a modifier on a long-tail member does not label a pillar that states no intent.
- **KW3** (done):
  - `seonexus-secrets` holds the keychain, file and environment secret store, bound to addresses. The SERP Finder moves to it once the workspace builds again.
  - `seonexus-kw-store` keeps `kw.duckdb`. Large writes go through appenders and staging tables, and reads come in bounded pages. Numbers prefer the primary engine's newest value per column. The newest ten clusterings are kept.
  - Manual placements (`pins`) are applied by `seonexus_keywords::place` after every clustering.
  - `seonexus-kw-app` provides the workspace, settings, projects and imports:
    - CSV/TSV/TXT in UTF-8, UTF-16 or Windows-1254, Excel cached values, and Search Console ZIPs.
    - Headers from Ahrefs, Semrush, Keyword Planner (ranges, monthly columns), Search Console and Wordstat are recognised.
    - It also clusters and exports.
  - `snx-kw` offers `project`, `add`, `import`, `keywords`, `exclude`/`include`, `cluster`, `clusters`, `export` and `usage`.
- **KW4** (done):
  - `seonexus-keyword-sources` sends one request per call over the HTTP port and returns the keywords with the requests and cost. Clients:
    - Wordstat: top requests up to 2,000 with associations, monthly dynamics and the region tree.
    - DataForSEO: Google Ads volumes and ideas, Labs ideas, related, suggestions, a domain's ranking keywords, bulk difficulty, search intent and autocomplete.
    - SerpApi autocomplete.
  - The SERP Finder keeps its own volume clients; they ask fewer fields.
  - The runner in `seonexus-kw-app` sends requests one at a time and releases the store during network waits. It keeps monthly budgets per provider and Wordstat's hourly quota (waiting, cancellable).
  - A provider that refuses the key or is out of balance is left out for the rest of the run, and one rate limit is waited out.
  - Discovery filters what it finds and records the source and seed. It stores numbers per source: `wordstat` for Yandex, `dataforseo-ads` and `dataforseo-labs` for Google. Enrichment keeps difficulty apart as `dataforseo-kd` so it never replaces a volume.
- **KW5** (done):
  - `seonexus-oauth`: the user's own Desktop OAuth client, a loopback listener on `127.0.0.1` with PKCE S256 and a checked state, the code exchange, refresh and revocation. The refresh token stays in the keychain, bound to `oauth2.googleapis.com`, and a revoked grant is forgotten.
  - Search Console: properties, and query × page rows (25,000 per request, at most 50,000, country filter, final data ending three days ago). The queries become keywords (`gsc`) and the pages site pages for mapping.
  - Google Ads REST v26: ideas and historical metrics, one request per second. A developer token is sent only when the user saved one.
- **KW6** (done):
  - Result pages come through the Keyword Miner's own chain per engine (DataForSEO, Serper, SerpApi, the Yandex Search API) using the SERP Finder's provider code. Fresh pages are reused (30 days), the budgets apply, and a run stops after three keywords in a row without a page.
  - The site's ranking pages are stored as `serp` site pages. `map` maps the newest clustering again without reclustering.
  - The free Google and Yandex suggestion endpoints are off until switched on with the terms accepted. Requests carry the program's user agent and go one per 2 s at least, at most 2,000 a day per engine. They stop until the next day at the first refusal and are never retried.
  - Browser-mode result pages (KW10, below).
- **KW7–KW8** (done): `apps/keywords` (Tauri 2 + React, `seonexus-kw-desktop`, ports 1460/1461) on the deep emerald `[data-product="keywords"]` ground.
  - Pages: Projects, Discover (typing, file import with preview, paid discovery with estimates, numbers), Keywords (a virtual table for 50,000 rows with filters, sorting, bulk exclude/tag and exports), Clusters (method and thresholds, result pages, treemap, detail with moves, splits and a content brief), Site mapping (Search Console import, gaps, cannibalisation, mapped), Sources (keys, budgets, switches, Google sign-in and Ads account, the free suggestions with their warning, usage) and Settings.
  - Background jobs report progress and block closing while they run.
  - A browser preview with sample data, and an IPC contract test.
- **KW9** (done): the website page `/keyword-miner` in four languages, `products.json` (`keyword-miner`, `kw-v`), release tags `kw-v*`, docs (`docs/keywords.md`, `docs/keywords-kilavuz.md`, `docs/releases/kw-0.1.0.md`) and the README.
- **KW10** (done): the attended browser mode as a result-page source, reusing the SERP Finder's `BrowserProvider` and `seonexus-render` window.
  - It is out of the default chains; the user adds `browser` to an engine's chain (Sources page or `snx-kw browser add`).
  - A visible Chrome or Edge window with its own profile (`<data>/browser-profile`), one query at a time, a fixed pause (`browser.delay_secs`, 30 s, at least 10) and a daily limit per engine (`browser.daily_cap`, 100), counted from `provider_attempts` (schema 2 adds its `engine` column). One optional proxy of the user's.
  - A consent page or CAPTCHA brings the window forward and waits up to 10 minutes; the job carries the wait (`check`) for the job card and the CLI. A check nobody clears stops the run.
  - Never CAPTCHA solving, stealth, fingerprint or user-agent changes, `navigator.webdriver` patching or proxy rotation.
  - Google's browser results are `site_only`, so clustering compares sites and mapping leaves them out.

