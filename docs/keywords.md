# OctopusOn SEO Keyword Research

The SEO Keyword Research discovers keywords, groups them into clusters that one page can target, and maps the clusters to your site. Design decisions are in [ADR 0069](adr/0069-keyword-miner.md); a Turkish guide is in [keywords-kilavuz.md](keywords-kilavuz.md).

The CLI (`apps/snx-kw`) and the desktop app (`apps/keywords`) share the use cases in `crates/seonexus-kw-app` and one data folder:
- `OCTOPUSSEO_KW_DATA_DIR`, or the platform's application data directory for "octopusSEO Keyword Miner".
- It is separate from the SERP Checker's: keys, budgets and result pages are not shared.

```sh
pnpm keywords             # native desktop app (Vite + Tauri)
pnpm keywords:dev         # browser preview with sample data on http://localhost:1460
pnpm keywords:build       # native installers for the current platform
cargo run -p snx-kw -- --help
```

## Projects

A project is one market: country, language and engines, the first engine being primary. It can also name your site and competitor domains.

```sh
octopuson-keywords project create "Örnek Kahve" --country tr --language tr --engine google --engine yandex \
  --site ornek.com.tr --competitor rakip.com
octopuson-keywords project list
```

The site and competitors are stored as registrable domains (`https://www.Örnek.com.tr/` becomes `xn--rnek-4qa.com.tr`).

## Getting keywords

### Typing and files

- `octopuson-keywords add 1 "türk kahvesi" "filtre kahve"`, or pipe one keyword per line.
- `octopuson-keywords import 1 export.csv --preview` shows how a file will be read. Without `--preview` it imports.

Supported files:
- CSV, TSV and TXT in UTF-8, UTF-16 (Keyword Planner) or Windows-1254.
- Excel workbooks: the first sheet's cached values. Formulas, macros and links are never run.
- Search Console export archives: the `Queries.csv` inside.

Header recognition:
- The header is found below title lines, and headers are recognised in English, Turkish and Russian: Ahrefs, Semrush, Keyword Planner, Search Console and Wordstat exports.
- Recognised columns bring their numbers: volume, a range like `1K – 10K`, CPC, competition, difficulty, and Keyword Planner's monthly columns.
- Volumes, cost and difficulty are stored as source `import` for the project's primary engine.
- Search Console clicks, impressions and positions are not volumes and are not stored as such.

Variant handling:
- Keywords that differ only in case, diacritics, dashes or spacing are one keyword (`Türk Kahvesi`, `turk kahvesi`). The first text wins.
- A keyword added again gains the new source, and an excluded keyword stays excluded.

### Paid sources

| Provider | Keys (`octopuson-keywords providers set-key …`) | Gives |
|---|---|---|
| `wordstat` (Yandex Search API) | `api_key`, `folder_id` | up to 2,000 phrases containing a seed and its associations, with monthly counts (Yandex) |
| `dataforseo` | `credentials` (`login:password`) | Google Ads volumes and ideas; Labs ideas, related searches, suggestions, a domain's ranking keywords, difficulty and intent; Google autocomplete |
| `serpapi` | `api_key` | Google autocomplete |

Keys and spending:
- Keys go into the system keychain (a 0600 file on Linux or with `OCTOPUSSEO_SECRET_STORE=file`). They are bound to the provider's address and read from standard input, never from the command line.
- `OCTOPUSSEO_SECRET_<PROVIDER>_<NAME>` overrides a key for scripts.
- `octopuson-keywords providers budget dataforseo 20` sets a monthly budget, and `providers disable` switches a provider off.

```sh
octopuson-keywords discover 1 --seed "türk kahvesi" --seed "kahve makinesi" \
  --source wordstat,related,suggestions,autocomplete --expand questions,commercial \
  --exclude falı --min-volume 20 --estimate
octopuson-keywords discover 1 --seed "türk kahvesi" --source related,competitors --yes
octopuson-keywords enrich 1 --ask volume,difficulty,intent
```

How a run proceeds:
- It shows the requests and estimated cost per source first. It asks above `confirm_above_usd` (default 1 USD) unless `--yes` is given.
- Requests go one at a time.
- A source without keys, switched off, or past its monthly budget is reported and skipped.
- A refused key or an exhausted balance leaves the provider out for the rest of the run. One rate limit is waited out, then the provider is left out.
- Wordstat's quota (100 requests an hour unless Yandex raised it; `wordstat_hourly_quota` in the settings) is kept by waiting, and Ctrl-C stops the run.

What is recorded:
- Every keyword found carries its source (`wordstat`, `dataforseo-related`, …) and seed.
- Numbers are stored per source: `wordstat` for Yandex; `dataforseo-ads`, `dataforseo-labs` and `google-ads` for Google; difficulty alone as `dataforseo-kd`.
- A keyword's volume is the newest from the project's primary engine, else any. Cost, competition and difficulty are the newest known.

### On OctopusOn SEO credits

The paid sources also run without keys of your own: connect an OctopusOn SEO account (Sources → OctopusOn SEO account → Connect, or `octopuson-keywords account login`) and approve the code shown in the program on the page that opens in your browser. Credits are bought at my.octopuson.com.

- Each paid source has a payment mode: **Automatic** (your key when you have one, else credits), **My key** or **Credits** (`octopuson-keywords providers mode serper credit`).
- On credits, requests go to the OctopusOn SEO gateway (`api.octopuson.com`), which sends them on with OctopusOn SEO's provider account and charges your credits. The program never sends your own keys there. Request contents are not stored; only the source, endpoint, status and amount are kept.
- Estimates show the credit price (the provider's price times the multiplier shown on the source's card). The spending recorded is what the gateway charged; `octopuson-keywords usage` and the Sources page show the part paid with credits.
- When credits run out (402), a source on credits stops; one in Automatic mode keeps using your key if you have one.
- The account token is kept in one file shared by every OctopusOn SEO program on the computer (`account.json` in the common octopusSEO data folder, readable by you only; `OCTOPUSSEO_ACCOUNT_DIR` moves it). Connecting in one program connects the others, and a program that is already open notices within seconds. `octopuson-keywords account logout` disconnects all of them; devices can also be switched off at my.octopuson.com.
- `OCTOPUSSEO_CLOUD_URL` points the program at another gateway (for development; https only, or http on localhost).

### Your Google data

Use your own OAuth client: Google Cloud → APIs & Services → Credentials → OAuth client ID → *Desktop app*. Enable the Search Console API, and the Google Ads API for keyword planning.

```sh
octopuson-keywords google client 123-abc.apps.googleusercontent.com   # the secret is read from standard input
octopuson-keywords google login            # Search Console; add --ads for Google Ads
octopuson-keywords google sites
octopuson-keywords google import 1 sc-domain:ornek.com.tr --days 90
octopuson-keywords google ads-account 123-456-7890
octopuson-keywords discover 1 --seed "türk kahvesi" --source google-ads
octopuson-keywords enrich 1 --ask volume --google-ads
```

Signing in:
- Sign-in opens Google's consent page in your browser. The program listens on `127.0.0.1` only and uses PKCE and a random state.
- The refresh token goes into the keychain. A revoked grant is forgotten, and `google logout` revokes it.

Search Console:
- It imports query × page rows of web search, final data only, ending three days ago: 25,000 rows per request and at most 50,000 per import (the API's daily limit per property).
- By default only searches from the project's country are read.
- Queries become keywords (source `gsc`) and their pages are kept for mapping.

Google Ads:
- Keyword Planner needs at least Basic access for your Google Cloud project, and its terms describe tools for Ads campaigns. Low-spending accounts may see volume ranges only.
- Requests go once a second. A developer token is sent only if you saved one (`providers set-key google-ads developer_token`).

### Free suggestion endpoints

Google's and Yandex's suggestion endpoints are not official APIs; Yandex's terms forbid automated queries without permission.

They are off until you switch them on with the terms accepted:

```sh
octopuson-keywords suggest --google on --accept-terms
octopuson-keywords discover 1 --seed kahve --source google-suggest --expand questions
```

Then the SEO Keyword Research asks them with its own user agent, one request every 2 seconds at least, and at most 2,000 per engine and day (these can only be made slower or lower). The first refusal (429, 403, a check page) stops that engine until the next day, and it is never retried.

## Clustering

```sh
octopuson-keywords cluster 1 --method lexical --threshold 0.6
octopuson-keywords serp 1 --per-cluster 3 --estimate
octopuson-keywords serp 1 --per-cluster 3
octopuson-keywords cluster 1 --method hybrid --serp-threshold 3 --mode soft
octopuson-keywords clusters 1
octopuson-keywords keywords 1 --cluster 4
```

### Methods

- **Wording (`lexical`)**: free and instant.
  - Keywords are compared by their word stems: Turkish suffixes are removed, and intent words like `nasıl` and `fiyat` are kept so they separate.
  - The measure is IDF-weighted Jaccard, the threshold is 0.6 by default, and candidates come from an inverted index.
  - Pillars are taken by searches, and each takes the free keywords at or above the threshold.
- **Result pages (`serp`)**: exact.
  - Keywords whose top ten share at least `serp_threshold` results (1–9, default 3) group together.
  - Soft mode compares with the pillar, hard mode with every member.
  - Comparison is by page or by site. Google results that hold sites only (browser-mode data) switch it to sites.
- **Hybrid**: result pages where fetched, wording elsewhere. Members placed by wording are marked unverified.

50,000 keywords cluster in about two seconds in a release build.

### Result pages

`octopuson-keywords serp` fetches the top ten for the chosen keywords through the SEO Keyword Research's own chain per engine (`settings.json` → `serp.chains`). The defaults are:
- Google: DataForSEO, Serper, SerpApi.
- Yandex: Yandex Search API, DataForSEO, SerpApi.
- Bing: DataForSEO, SerpApi.

Fetching rules:
- Monthly budgets apply.
- Pages younger than `serp.max_age_days` (30) are reused unless `--refresh` is given.
- A run stops after three keywords in a row without a page.
- `--top N` limits to the most searched keywords, and `--per-cluster K` to the most searched members of each cluster of the newest clustering.

### Browser mode (attended)

For small lists, result pages can come from a visible Chrome or Edge window instead of an API. It is not in any default chain; add it yourself, in the app (Sources → Browser mode) or with:

```sh
octopuson-keywords browser add google      # last in Google's chain, after the APIs
octopuson-keywords browser set --delay-secs 45 --daily-cap 60
octopuson-keywords browser status
octopuson-keywords serp 1 --per-cluster 2
```

How it works:
- The window has the program's own profile (`<data>/browser-profile`), so your own browser profile is never touched. `octopuson-keywords browser reset-profile` deletes it (cookies, consent choices).
- One query at a time, with a fixed pause (30 s by default, at least 10). The pause doubles after each check page, up to eight times as long.
- A daily limit per engine (100 pages by default), counted from the recorded attempts.
- A consent page or CAPTCHA brings the window forward and waits for you, up to 10 minutes. The job card (and the CLI) says so. Once you clear it the run goes on; if nobody does, the run stops.
- It never solves CAPTCHAs, hides automation, changes the user agent or fingerprint, or rotates proxies. One proxy of your own can be set for the window (`browser.proxy`); the network proxy is not used by it.
- Only desktop results, read with the built-in parser profiles.

Google hides result addresses in the browser, so its pages give sites only. Clustering then compares sites, and mapping does not use those pages. Bing and Yandex keep full addresses.

Automated searching may conflict with the engines' terms. Use an API source for large lists.

### Cluster totals

Each cluster stores:
- Its searches and size.
- Its intent: the stated intent with the most searches, unless more searches state none. The cluster is marked *mixed* when a second intent has a quarter of them.
- Search-weighted difficulty and cost per click.
- Whether result pages verified it.

The newest ten clusterings of a project are kept.

### Moving keywords by hand

In the app, select members of a cluster and **Move to** another cluster, make them a cluster of their own (**Own cluster**), or **Undo move**.

Moves are stored as placements (keyword → pillar). They are applied to the newest clustering at once and to every later clustering. A cluster whose pillar moves away is led by its most searched remaining keyword.

## Site mapping

Mapping uses the site's pages from Search Console (`gsc`) and from stored result pages (`serp`).

For each cluster it shows:
- **Target page**: the site's page with the most impressions (or keywords), then the best position.
- **Competing pages**: other pages of the site with at least 10% of the cluster's impressions. This is cannibalisation.
- **Gap**: competitors rank and the site does not. It is shown only when there is evidence: site pages or result pages.
- **Competitors**: the five domains ranking for most members.

`octopuson-keywords map 1` maps the newest clustering again after new Search Console data or result pages, without reclustering. Clustering maps by itself.

## Exports

```sh
octopuson-keywords export keywords 1 keywords.xlsx      # or .csv, .json; --excluded for the left-out ones
octopuson-keywords export clusters 1 clusters.xlsx      # one row per keyword with its cluster's totals and mapping
```

## Data folder

```
settings.json          defaults, budgets, switched-off providers, Wordstat quota, free suggestions,
                       Google client id and Ads account, result-page chains, network
secret-bindings.json   which address each saved key belongs to (not the keys)
kw.duckdb              projects, keywords, sources, tags, numbers, intents, result pages,
                       clusterings, placements, site pages, runs and provider attempts
```

The CLI and the app can be used one after the other; the store is locked while one of them writes.
