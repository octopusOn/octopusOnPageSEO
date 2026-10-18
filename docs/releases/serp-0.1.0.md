First release of the OctopusOn SERP Checker: a desktop app and the `octopuson-serp` command line for rankings, keyword analysis and keyword difficulty you can explain.

## Highlights

- **Your sources**: Google, Bing and Yandex through the free browser mode, the Yandex Search API, DataForSEO, Serper or SerpApi — or your own provider, described with a request template and a JSONPath, CSS or regular-expression mapping. Each engine tries its sources in the order you set; monthly budgets and a cost estimate come before every run.
- **Keyword analysis**: the top results are crawled with the Spider engine and scored. Content scores cover keyword placement, topic coverage, depth, structure, questions, media, schema, snippet fit and freshness; site scores cover domain age (RDAP, WHOIS, Wayback), Open PageRank, referring domains and, optionally, Tranco popularity.
- **Explainable keyword difficulty**: the click-weighted strength of the ten results, adjusted for brands, forums and ads, with every component, its weight and its contribution shown. Search intent, SERP features, your opportunity, content gaps and what it takes to rank come with it.
- **Rank tracking**: projects with daily, weekly or manual checks on several engines and locales; positions, best positions, history, visibility, share of voice and cannibalisation. Due checks run while the app is open, and `octopuson-serp track --due` runs them from cron.
- **Stored result pages**: every response is kept compressed. When an engine changes its HTML, patch the selectors, test them on a stored page and read old snapshots again without a request.
- **Exports** of rank reports and analyses to Excel, CSV and JSON.
- Keys are kept in the system keychain (a protected file on Linux), bound to the address they were entered for, and never exported.

## The free browser mode

Google encrypts its result links, so in the browser mode Google results carry their site, not the page address: rankings by domain are exact, while page-level work needs a Google API source. Bing and Yandex results keep their full addresses.

It searches in a visible Chrome or Edge window with its own profile, one query at a time, with a fixed pause (30 seconds by default, at least 10) and a daily limit per engine. When an engine shows a consent or check page, the window comes to the front and the search waits for you to clear it; the app never solves, hides from or works around such pages. Automated searching may conflict with a search engine's terms; use an API for heavy tracking.

## Installation notes

- **macOS**: the app is not notarized yet. If macOS says it cannot verify the developer, open **System Settings → Privacy & Security** and choose **Open Anyway**.
- **Windows**: SmartScreen may warn about an unknown publisher; choose **More info → Run anyway**.
- The browser mode and JavaScript rendering need an installed Chrome, Chromium or Edge.
