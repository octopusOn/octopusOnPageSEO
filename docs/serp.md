# OctopusOn SERP Checker

The SERP Checker reads Google, Bing and Yandex result pages, crawls the pages that rank with the Spider engine, scores their content and sites, and explains keyword difficulty. The CLI (`apps/snx-serp`) and the desktop app (`apps/serp`) share the use cases in `crates/seonexus-serp-app` and one data folder: `OCTOPUSSEO_SERP_DATA_DIR`, or the platform's application data directory for "octopusSEO SERP Finder". Design decisions are in [ADR 0064](adr/0064-serp-finder.md).

```sh
pnpm serp                 # native desktop app (Vite + Tauri)
pnpm serp:dev             # browser preview with sample data on http://localhost:1450
pnpm serp:build           # native installers for the current platform
cargo run -p snx-serp -- --help
```

## Sources

Every engine asks its sources in order (`settings.json` → `chains`) and moves to the next when one is switched off, has no key, is over its monthly budget, out of quota, rate limited or offline. A check page in the browser never moves on silently: it waits for you. One snapshot always comes from one source.

| Source | Engines | Key |
|---|---|---|
| Browser mode (free) | Google, Bing, Yandex | none |
| Yandex Search API | Yandex | `api_key`, `folder_id` |
| DataForSEO | Google, Bing, Yandex | `credentials` (`login:password`) |
| Serper | Google | `api_key` |
| SerpApi | Google, Bing, Yandex | `api_key` |
| Your own provider | as declared | as declared |

Keys are entered in the app (Sources) or with `octopuson-serp providers set-key <provider> <name>` (read from standard input). They are stored in the system keychain (macOS Keychain, Windows Credential Manager; a 0600 file on Linux or with `OCTOPUSSEO_SECRET_STORE=file`) and bound to the address the provider had when the key was saved: a spec that is changed to send requests elsewhere asks for the key again. `OCTOPUSSEO_SECRET_<PROVIDER>_<NAME>` overrides a key for scripts. Keys are never written to settings, exports or logs.

Depth costs requests differently per source: DataForSEO and the Yandex Search API return up to 100 results in one request, while Google serves ten results per page (it ignores `num=100` since September 2025), so Google depth 100 through the browser mode, Serper or SerpApi takes ten requests. In the browser mode Bing and Yandex also serve ten results per page (`count=50` and `numdoc=50` were ignored in a live check on 2026-10-18).

Before a run the app shows the expected requests and cost of the first usable source and asks above `confirm_above_usd` (default 1 USD). `octopuson-serp estimate` prints the same; `octopuson-serp usage` lists requests and spending per provider and month.

### On OctopusOn SEO credits

The built-in paid sources (Serper, SerpApi, DataForSEO, Yandex Search API) also run without keys of your own: connect an OctopusOn SEO account (Sources → OctopusOn SEO account → Connect, or `octopuson-serp account login`) and approve the code on the page that opens in your browser. Credits are bought at my.octopuson.com.

- Each built-in source has a payment mode: **Automatic** (your key when you have one, else credits), **My key** or **Credits** (`octopuson-serp providers mode serper credit`). Your own providers never run on credits.
- On credits, requests go to the OctopusOn SEO gateway (`api.octopuson.com`), which sends them on with OctopusOn SEO's provider account and charges your credits; your own keys are never sent there and request contents are not stored. Volumes and backlink summaries through DataForSEO or Wordstat follow the same mode.
- Estimates show the credit price; the recorded cost is what the gateway charged, and `octopuson-serp usage` shows the part paid with credits.
- When credits run out (402) the chain moves on to the next source, as with any refused key.
- The account token is kept in one file shared by every OctopusOn SEO program (`account.json` in the common octopusSEO data folder, readable by you only); connecting in one program connects the others within seconds, and `octopuson-serp account logout` disconnects them all.

### The browser mode

The browser mode opens a visible Chrome or Edge window with its own profile (`<data>/browser-profile`) and searches one query at a time:

- a fixed pause between searches (30 seconds by default, at least 10) and a daily limit per engine (100 by default); after a check page the pause doubles;
- when an engine shows a consent or check page (Google `/sorry/`, reCAPTCHA, Yandex SmartCaptcha, Bing challenges), the window comes to the front and the search waits up to 10 minutes for you to clear it;
- the app never solves CAPTCHAs, never changes the browser's fingerprint or user agent and never rotates proxies; one proxy of your own is allowed;
- desktop results only; mobile results come from the APIs;
- **Google hides result addresses**: since 2026 its result links are encrypted (`/goto?url=…`), so the browser mode reads the site of each Google result from its displayed URL and marks it *site only*. Positions by domain or host are exact; URL-prefix matching and the page crawl of keyword analysis need a Google API source (Serper, DataForSEO, SerpApi). Bing and Yandex results keep their full addresses;
- Google shows six to nine organic results on its first page, so choose depth 20 to see the first ten.

Automated searching may conflict with a search engine's terms. Use it for low volumes while you are at the computer, and an API for regular tracking.

### Your own provider

A provider spec describes the request as a template and the response as a mapping. Placeholders (`{keyword}`, `{gl}`, `{hl}`, `{device}`, `{page}`, `{num:int}`, `{secret:token}`) are encoded for the place they appear in; secrets may only be used in the provider's declared origin. Results and SERP features are read with JSONPath (`$.organic[*]`, RFC 9535 filters), CSS selectors (`a @href`, `h3 ::text`) or a regular expression with named groups. Add a spec in the app (Sources → Your providers, with an offline **Test** on a pasted response) or with `octopuson-serp providers add spec.json`; `providers test spec.json --response body.json` parses a saved response without a request.

```json
{
  "version": 1, "id": "acme", "name": "Acme SERP", "engines": ["google"],
  "secrets": [{ "name": "token" }],
  "request": { "method": "POST", "url": "https://api.acme.example/v1/search?page={page}",
    "headers": { "Authorization": "Bearer {secret:token}" },
    "body": { "q": "{keyword}", "gl": "{gl}", "hl": "{hl}", "num": "{num:int}" } },
  "paging": { "per_page": 10, "first_page": 1, "max_pages": 2 },
  "response": { "format": "json", "error": "$.error.message",
    "results": { "items": "$.organic[*]", "fields": { "url": "$.link", "title": "$.title", "snippet": "$.snippet" } },
    "features": [{ "kind": "people-also-ask", "items": "$.related_questions[*]", "fields": { "text": "$.question" } }] },
  "cost": { "per_request_usd": 0.002 }
}
```

### When an engine changes its HTML

The browser mode reads Google, Bing and Yandex pages with built-in, versioned selector profiles (`google-html@1` …). Every response is stored compressed under `<data>/raw/`. When results stop being read (the app warns about low coverage and saves the page under `<data>/diagnostics/`), patch the selectors in Sources → Result page selectors with a JSON merge patch, test it on a stored page, save, and read old snapshots again: SERP snapshots → **Read again**, or `octopuson-serp reparse <snapshot>`. Rank checks that used a snapshot get its corrected position.

## Keyword analysis

`octopuson-serp analyze "<keyword>" --engine google --country tr --site example.com`, or Keyword analysis in the app:

1. The keyword is searched (depth 10).
2. The top results (10 by default, up to 20) and your page, if given, are crawled with the Spider engine: robots.txt respected, OctopusOn SEO or Chrome user agent, optional JavaScript rendering. Crawls are kept under `<data>/crawls`.
3. Domain facts are looked up and cached: age from RDAP, WHOIS (`whois.iana.org` referral, for `.tr`, `.de`, `.ru` and others) or the first Wayback capture (180 days); Open PageRank score, rank and referring domains with a free key (30 days); DataForSEO backlink summaries if asked (paid); Tranco rank if the list was downloaded (Settings, non-commercial licence).
4. Pages, sites, difficulty, intent and opportunity are scored and stored; `octopuson-serp rescore <id>` scores a stored analysis again with newer models without a request.

Every score lists its components with the measured value, weight, contribution and a reason. Missing signals are left out, the remaining weights are renormalised and the confidence drops.

- **Content (`content-1`)**: keyword placement (stems in title, H1, first 100 words, URL, description) 20, topic coverage (terms at least 40% of the other top pages use) 25, depth against the median 15, structure 10, questions answered 5, media 5, schema 10, snippet fit 5, freshness 5.
- **Technical (`tech-1`)**: page health from the Spider audit (12/6/2 points per high/medium/low finding), HTTPS/HSTS and server response; non-200 or noindex pages score 0.
- **Site (`site-1`)**: Open PageRank 30, referring domains 25, Tranco 20 (optional), age 15, HTTPS/HSTS 10.
- **Keyword difficulty (`kd-1`)**: strength of each result = 0.55 × site + 0.30 × content + 0.15 × targeting; the base is the click-weighted average over the top ten, adjusted by up to +10 for brand dominance, −10 for forums and user content, +5 for three or more ads. Bands: 0–14 very easy, 15–29 easy, 30–49 medium, 50–69 hard, 70–84 very hard, 85+ extreme.
- **Opportunity (`opp-1`, with your site)**: chance to win against the difficulty 40, demand (volume) 25, clicks left by AI answers, local packs, others' snippets and ads 20, position 15.
- **Intent (`intent-1`)**: Turkish and English modifiers, places, brands, SERP features and schema of the top pages.

Recommendations give the word count and H2 ranges of the top pages, common subheading topics, schema types, title length and pattern, questions to answer, a short answer when the result page shows an AI answer or a snippet, content gaps and related keyword ideas.

Monthly searches come from Wordstat (Yandex, with the Yandex Search API key) or DataForSEO (Google Ads data) when they are set up.

## Rank tracking

```sh
octopuson-serp project create "Acme" --site acme.com --engine google --country tr --language tr --depth 20
octopuson-serp project add-keywords 1 "türk kahvesi" "filtre kahve" --tag kahve
octopuson-serp track 1            # check now (asks above the cost limit; --yes skips)
octopuson-serp track --due        # every project whose schedule is due, for cron
octopuson-serp export rank 1 ranks.xlsx
```

A project tracks one site (registrable domain, exact host or URL prefix) on one or more engines and locales, daily, weekly or on request. The desktop app checks due projects 20 seconds after it starts and then every 10 minutes while it is open, one project at a time; projects missed while it was closed are checked then. A check page that is not cleared stops the run, so a blocked engine is not asked again and again.

The rank report shows the latest position, change, best position, history, ranking URL, volume and cannibalisation (several URLs of the site ranking for one keyword), visibility by day (volume-weighted click share against first place everywhere), and the share of voice of your site and its competitors.

## Data folder

```
settings.json            defaults, source order, budgets, browser mode, proxy, timeout
providers.json           your provider specs
parser-overrides.json    selector patches
secret-bindings.json     which address each saved key belongs to (not the keys)
serp.duckdb              runs, snapshots, results, features, attempts, analyses, projects, rankings
raw/YYYY-MM/             compressed responses
crawls/                  Spider crawls of analysed pages
datasets/                RDAP bootstrap, Tranco
browser-profile/         the browser mode's profile (Settings → Reset browser profile clears it)
diagnostics/             pages no result could be read from
```

The CLI and the app can be used one after the other; the store is locked while one of them writes.
