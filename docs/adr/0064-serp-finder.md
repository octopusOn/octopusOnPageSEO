# ADR 0064 — octopusSEO SERP Finder, keyword difficulty from crawled competitors

- Status: accepted
- Date: 2026-10-18

## Context
Rank trackers and keyword tools (Ahrefs, Semrush, SE Ranking, Serpstat) show where a site ranks and a keyword difficulty derived from the backlinks of the top results. They do not read the ranking pages themselves, so their difficulty says little about how good the competing content is, and the numbers cannot be explained or reproduced. Agencies working in Turkey also need Yandex next to Google and Bing, Turkish text handling, and control over where their SERP data comes from and what it costs.

External facts that shape the design (checked 2026-10-18):
- Google's Custom Search JSON API is closed to new customers and retires on 2027-01-01, so it is not used.
- Since January 2025 Google serves results only to clients that run JavaScript, so free fetching needs a real browser.
- Since September 2025 Google ignores `num=100`, so every ten results are a separate request and depth multiplies cost.
- Yandex publishes a Search API (`searchapi.api.cloud.yandex.net/v2/web/search`, XML or HTML responses, `SEARCH_TYPE_TR` for yandex.com.tr) and Wordstat volumes.
- IANA's RDAP bootstrap covers generic TLDs but not `.tr`, `.de` or `.ru`, so domain age needs WHOIS and Wayback fallbacks.
- The Tranco list contains CC BY-NC data.

## Decision
A third program, **octopusSEO SERP Finder** (`snx-serp` CLI and a desktop app), on the same layering as the Log Analyser and the Spider:

- **`seonexus-serp`** (domain): the model and the result parsers.
  - Model: `SerpQuery` (engine, keyword, country, language, location, device, depth) and `SerpSnapshot` (organic results by position, feature blocks, warnings).
  - Every reader is a declarative mapping, JSONPath / JSON Pointer, CSS with `@attr` / `::text` picks, or regular expressions, with alternatives tried in order. The Google, Bing and Yandex HTML parsers are such mappings in versioned JSON profiles embedded in the binary; Yandex XML has its own reader.
  - Users patch profiles with JSON merge patches and re-read stored pages, and define their own providers as a request template plus a mapping. Template values are encoded for the context they land in, and secrets are bound to a URL origin that placeholders cannot change.
  - Rank matching by registrable domain (Public Suffix List), host or URL prefix.
- **`seonexus-text`** (domain): tokenizing, Turkish-aware case folding, stop words, stemming, n-grams and term statistics, for content gap analysis.
- **`seonexus-domain-intel`** (domain): domain age from RDAP, WHOIS referrals and Wayback CDX, Open PageRank (score, rank, referring domains), optional Tranco rank.
- **`seonexus-serp-score`** (domain): versioned, explainable scores. Each score returns its components with value, weight, contribution and a reason key; missing signals renormalise the weights and lower a reported confidence.
  - Page content: keyword placement, topical coverage, depth, structure, PAA coverage, media, schema, snippet fit, freshness.
  - Technical: audit findings by priority.
  - Site: Open PageRank, referring domains, Tranco, age, technical health.
  - Keyword difficulty: a CTR-weighted strength of the top ten, adjusted for brand dominance, forum results and ads.
  - Opportunity, search intent and recommendations.
- **`seonexus-serp-store`** (adapter): one DuckDB database (projects, keywords, runs, snapshots, results, features, rankings, provider attempts with cost, analyses, page analyses and terms, domain facts with expiry, volumes), raw responses as zstd files.
- **`seonexus-serp-app`** (use cases): providers and fallback chains, secrets (OS keychain, a 0600 file fallback, environment variables for the CLI), budgets, analysis and rank runs, the scheduler.
  - Built-in providers: the browser, the Yandex Search API, DataForSEO, Serper, SerpApi.
  - The top results are crawled with the Spider engine in list mode, one site per URL, respecting robots.txt and identifying as octopusSEO or Chrome, never as Googlebot.
- **Apps:** `apps/snx-serp` (CLI) and `apps/serp` (Tauri desktop on the shared design language, `data-product="serp"`, dev ports 1450/1451).

**Browser mode limits.** Browser mode opens the user's Chrome or Edge in a visible window with a dedicated persistent profile.
- One query at a time, with a fixed delay (30 s default, 10 s minimum), a daily cap per engine, and a delay that doubles after each challenge.
- On a consent page or CAPTCHA, the run pauses and the window comes to the front until the person solves it, cancels, or ten minutes pass.
- Never built: automated challenge solving, stealth or fingerprint changes, user-agent changes, proxy rotation. A single user-configured proxy is allowed.
- Persistent profiles are an exception to ADR 0010's temporary profiles, limited to this mode; the profile is locked while in use and can be reset.

**Milestones.**
- SF1: model, mappings, parsers.
- SF2: HTTP port and API providers.
- SF3: store, workspace, CLI search.
- SF4: browser mode.
- SF5: text analysis and crawling the ranking pages.
- SF6: domain intelligence.
- SF7: scoring and `analyze`.
- SF8: rank tracking.
- SF9–SF10: desktop app.
- SF11: multi-product releases and website.

## Alternatives considered
- **Backlink-only difficulty (industry standard):** needs a paid backlink index for every keyword and ignores content. Here referring domains are one site signal, free from Open PageRank or optional from DataForSEO.
- **Hard-coded parsers:** break silently when engines change markup and can only be fixed by a release. Mappings make the breakage visible (coverage warnings) and fixable by the user.
- **Headless scraping with evasion:** against the engines' terms and a moving target. An attended visible browser at a low rate is the free option; APIs are recommended for tracking at volume.

## Consequences
- Every score can be traced to its inputs and recomputed from stored data when the model changes.
- The Index 404 Finder (`seonexus-finder`, a separate program) has its own discovery queries; both programs can share `seonexus-serp` parsers once they settle.

## Implementation status (2026-10-18)
SF1–SF11 are implemented:

- **Crates:** `seonexus-serp`, `seonexus-text`, `seonexus-domain-intel`, `seonexus-serp-score`, `seonexus-serp-store` and `seonexus-serp-app`.
- **Frontends:** the `snx-serp` CLI and the `apps/serp` desktop app, with the in-app scheduler, editors, reparse and exports.
- **Releases and website:** multi-product releases and the website, see [ADR 0065](0065-multi-product-releases.md). User guides are `docs/serp.md` and `docs/serp-kilavuz.md`.

Deviations from the plan:

- **SF5 left the crawl engine unchanged.** Ranking pages are crawled with the Spider's list mode, followed by a second list crawl for redirect targets on other hosts. Internal and external links are classified per page when the results are read back. There is therefore no `list_scope` setting and no separate ADR.
- **The store is held only while it is read or written.** Searches and domain lookups release it while they wait for the network or for the person, and `SerpWorkspace::store` waits up to five seconds for a store in use. This lets the desktop app's views read while an analysis runs.

Live check in browser mode (2026-10-18, two Turkish queries per engine, the owner clearing the check pages):

- **Yandex:** read correctly, with ten results and full addresses after a SmartCaptcha.
- **Bing:** two defects found and fixed (profile `bing-html@2`).
  - Its Turnstile check page (`#turnstile-widget`) was not recognised, so the search did not wait for the person.
  - Its cookie banner sits on the result page, yet it was treated as a consent page that blocks reading.
  - A consent challenge over readable results no longer stops the search.
  - Feature headings are matched without the `<style>` elements Bing puts first.
- **Google:** after a `/sorry/` check, Google served encrypted result links (`/goto?url=…`) with no page address in the page.
  - Results whose link is hidden keep their site from the displayed URL and are marked `site_only`. This is stored as results column `site_only`, migration 5, with a snapshot warning.
  - Analyses score them on their site only.
  - Rank tracking by domain or host stays exact; page addresses need a Google API source.

Not yet done:

- **Live calls to the API providers with real keys.** The owner enters the keys.
- **The first `serp-v0.1.0` and `spider-v0.1.0` tags.**
