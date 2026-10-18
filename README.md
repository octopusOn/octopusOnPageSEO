# OctopusOn SEO

Open-source, cross-platform SEO tooling written in Rust. (Crates keep the internal `seonexus-` prefix of the project's first name.)

| Product | Status | Programs |
|---------|--------|----------|
| **OctopusOn Log Analyser** — server log analysis for SEO: verified search-engine and AI crawlers, crawl budget, status codes, orphan pages | ✅ usable | `octopuson-log` CLI, desktop app (macOS, Windows, Linux) |
| **OctopusOn SEO Crawler** — technical SEO crawler and site audits | beta | `octopuson-crawler` CLI, desktop app (macOS, Windows, Linux) |

## Log Analyser highlights

- **Formats**: Nginx/Apache combined and common, custom Nginx `log_format` / Apache `LogFormat` strings, Microsoft IIS and CloudFront (W3C, read from the `#Fields:` header), AWS ALB/ELB, HAProxy, Traefik (CLF + JSON), JSON logs with any key names (Caddy, Cloudflare Logpush, Google Cloud load balancers, Nginx JSON, Akamai, …) and CSV/TSV with a header row — detected automatically. Plain, `.gz`, `.zst`, `.bz2` and `.zip` files or whole folders.
- **Import settings per site**: store bots only, include/exclude URL patterns, strip parameters (`sid`, `utm_*`, …); delete a wrong import again; move or share a site as one `.snxsite` archive.
- **Scale**: ~0.7–0.8 M lines/s per import on a laptop; monthly ZSTD Parquet partitions (≈ 8 % of raw size) plus rollups answer dashboards in milliseconds independent of site size.
- **Safe re-imports**: files already imported are skipped; overlapping rotated logs are de-duplicated line by line.
- **~110 bots identified** (Googlebot smartphone/desktop/image/…, Bingbot, GPTBot, ChatGPT-User, OAI-SearchBot, ClaudeBot, PerplexityBot, Applebot, YandexBot, AhrefsBot, …) in categories search / AI training / AI search / AI assistant / ads / SEO tools / social / monitoring.
- **Verification**: official IP lists of Google, Bing, OpenAI, Anthropic, Perplexity, Apple, DuckDuckGo, Common Crawl, Mistral, Ahrefs and Amazon (bundled snapshot + `bots update`) and forward-confirmed reverse DNS. Every bot hit is `verified`, `spoofed` or `unverified`. **Custom bots** (own crawlers, monitors) with IP ranges, IP list URLs or reverse DNS.
- **robots.txt compliance**: which bots request disallowed URLs (spoofed bots, AI crawlers ignoring rules), which bots robots.txt blocks site-wide — evaluated like Google (most specific group, longest match).
- **AI visibility**: AI crawler activity and errors per bot, plus human visits referred by ChatGPT, Perplexity, Gemini, Copilot, Claude and others (`referer_type:ai`) with their landing pages.
- **Countries and networks** of client IPs (optional DB-IP Lite data, looked up locally): find scraper clouds and where fake bots come from.
- **Alerts**: sudden crawl drops/spikes of Googlebot, search engines and AI bots, and server-error bursts.
- **Saved queries**: name any query (desktop "Save…" or `octopuson-log saved add`), reuse it on every site, share it as a JSON file.
- **Query language + 39 report templates** (including Screaming Frog-style URL, response-code, user-agent, referer, directory, IP, country and bytes tables with last status, days since last crawl, per-bot columns, 1xx–5xx columns and estimated CO2): `bot:googlebot* verification:verified status:4xx group:url metric:hits,last_seen` — see [docs/query-language.md](docs/query-language.md).
- **Analyses**: crawl-budget overview, URL list / sitemap comparison (uncrawled and orphan pages, with the crawl export's other columns and sitemap `lastmod` joined like a VLOOKUP), period comparison, inconsistent status codes.
- **Exports**: CSV (formula-injection safe), XLSX (native dates/numbers), JSON — streamed, so large results never sit in memory.
- **Desktop app** (Tauri 2 + React) in Turkish and English, sharing the data directory with the CLI. Filters are built in a simple form (field, operator, value, min/max ranges, grouping, sorting) that writes the query text, which stays visible and editable; value boxes suggest the site's most frequent bots, user agents, URLs, status codes, … with hit counts for multi-selection, and accept any typed value or `*` pattern (`googlebot*`, `*/amp`) as well.

## Install

Release builds are attached to GitHub releases (`v*` tags): desktop installers (`.dmg`, `.msi`/`.exe`, `.deb`/`.AppImage`) and `octopuson-log` archives. To build from source see [Development](#development).

## CLI quick start

```sh
# Check how a log is parsed (format detection, skipped lines) without storing anything
octopuson-log inspect access.log.gz

# Organise logs per client and site; the format is stored with the site
octopuson-log client add "Acme"
octopuson-log site add acme.com --client "Acme" --format combined
octopuson-log site add shop.acme.com --client "Acme" \
  --nginx-format '$remote_addr - $remote_user [$time_local] "$request" $status $body_bytes_sent "$http_referer" "$http_user_agent"'

# Import files or folders; re-importing is safe
octopuson-log import acme.com /var/log/nginx/

# Dashboards and reports
octopuson-log overview acme.com --period last30d
octopuson-log templates
octopuson-log query acme.com --template googlebot-errors
octopuson-log query acme.com 'bot_category:ai-* group:bot,day' --rows 100
octopuson-log query acme.com --template client-errors 'date:last7d' -o 404s.xlsx

# Saved queries (shared by all sites and with the desktop app)
octopuson-log saved add "Googlebot 404" 'bot:googlebot* verification:verified status:404 group:url'
octopuson-log query acme.com --saved "Googlebot 404" 'date:last7d'
octopuson-log saved export team-queries.json      # share with colleagues: saved import

# Analyses
octopuson-log compare-urls acme.com https://acme.com/sitemap.xml crawl-export.csv -o comparison.xlsx
octopuson-log compare-periods acme.com --before 2026-09-01..2026-09-30 --after 2026-10-01..2026-10-31 --group directory

# Bots
octopuson-log bots list --category ai-training
octopuson-log bots check-ip 66.249.66.1 --bot googlebot-smartphone
octopuson-log bots update            # download current crawler IP lists
octopuson-log bots verify acme.com --all
octopuson-log bots custom add acme-monitor --category monitoring --pattern AcmeMonitor --ip-range 203.0.113.0/24

# robots.txt compliance, countries and networks
octopuson-log robots acme.com --filter 'date:last30d' -o robots.xlsx
octopuson-log geo update             # optional: DB-IP Lite country/ASN data (~20 MB)
octopuson-log query acme.com --template spoofed-bot-networks

# Site settings and archives
octopuson-log site settings acme.com --traffic bots --strip-param sid
octopuson-log site export acme.com acme.snxsite
octopuson-log site import acme.snxsite --name acme-copy.com
```

| Command | Purpose |
|---------|---------|
| `inspect` | Parse files without importing; format, counts, skipped-line examples |
| `client add/list/remove` | Manage clients |
| `site add/list/show/set-format/settings/remove-import/export/import/remove` | Manage sites, import settings and history, archives |
| `import` | Import files/folders (`--no-dns` for offline verification) |
| `query` | Run a query or template; table, `--csv`, `--json` or `-o file.{csv,json,xlsx}` |
| `templates` | List built-in reports |
| `saved list/add/show/remove/export/import` | Named queries reused on every site; JSON files to share them |
| `overview` | Traffic and crawl-budget summary |
| `compare-urls` | URL lists / sitemaps vs. logs |
| `compare-periods` | Two periods per URL, directory, bot, … |
| `bots list/identify/check-ip/status/update/verify/custom` | Bot catalog, IP lists, verification, custom bots |
| `robots` | Bot requests vs. robots.txt |
| `geo update/status` | IP geolocation data (country, network) |

Global options: `--data-dir` (`OCTOPUSSEO_DATA_DIR`), `--memory-limit <MiB>` (`OCTOPUSSEO_MEMORY_LIMIT_MB`, default 1024), `-v`/`-q`. Most commands accept `--json`.

A Turkish user guide is in [docs/kullanim-kilavuzu.md](docs/kullanim-kilavuzu.md).

## Desktop app

```sh
pnpm install
pnpm desktop          # development (Vite + Tauri)
pnpm desktop:build    # installers for the current platform
```

`pnpm --filter @seonexus/desktop dev` alone serves the UI in a browser with a built-in demo backend, so the interface can be developed without the Rust toolchain.

## SEO Crawler

The Spider uses the Log Analyser's **Chromatophore** design package with its own deep teal ground. Its Rust crawler and audit engine are shared by the CLI and the Tauri desktop app.

- Spider and URL-list modes; parallel requests, rate limits, user-agent presets, robots.txt, sitemaps, scope patterns and query-parameter removal. No built-in URL count limit.
- Saved crawls in DuckDB on disk, with separate directories reserved atomically for concurrent sessions.
- More than 100 technical checks: response codes, redirects and loops, titles and descriptions, headings, canonical and hreflang relationships, images, links, orphan pages, structured data, security and response times. AI crawler access and `llms.txt` are checked from actual fetched responses. Findings are materialized after the crawl finishes.
- Turkish/English desktop overview, priority filters and per-crawl issue exclusions, URL tables with bounded database queries and virtual rows, and an inspector for page facts, incoming/outgoing links, headers and optional saved HTML.
- Live crawl activity: assigned URLs, the latest 100 completions, HTTP/JS outcomes, literal search and operation/error filters. Startup, crawling and final analysis/saving have distinct states. Pause/resume/stop, active-job reconnects and a native memory monitor are included; multiple crawls can run independently.
- Optional JavaScript rendering through an installed Chrome, Chromium or Edge: JS/AJAX content and generated hyperlinks are analyzed, with raw versus rendered comparisons and explicit per-page rendering errors.
- CSV, XLSX and JSON exports shared with the CLI. The desktop export writes the entire selected backend tab, including findings excluded from the UI.

```sh
pnpm spider               # native desktop app (Vite + Tauri)
pnpm spider:dev           # browser demo on http://localhost:1430
pnpm spider:build         # native installers for the current platform
pnpm --filter @octopusseo/spider check

cargo run -p snx-spider -- crawl https://example.com --rps 5 --store-headers
cargo run -p snx-spider -- show latest --tab issues
cargo run -p snx-spider -- export latest --tab issues --out issues.xlsx
```

The CLI and desktop share `OCTOPUSSEO_SPIDER_DATA_DIR`, or the platform's Spider application-data directory. Browser builds use explicitly labeled sample data; live crawling and XLSX export require the native app. JavaScript rendering requires an installed Chrome/Chromium/Edge; it is enabled per crawl. Custom rendering CSS, near-duplicate detection, crawl comparisons and the self-hosted service are not implemented yet. A million-URL performance benchmark has not been run; there is no claim that the current crawler uses constant memory.

See [docs/spider.md](docs/spider.md) for the current behavior and limits.

## SERP Checker

The SERP Checker reads Google, Bing and Yandex results, crawls the pages that rank with the Spider engine and scores them, with its own deep blue ground in the Chromatophore design.

- Sources in an order you choose per engine: a free, attended browser mode (a visible Chrome or Edge window, one query at a time, a fixed pause and a daily limit; consent and check pages are cleared by you, never solved or evaded), the Yandex Search API, DataForSEO, Serper, SerpApi, or your own provider described with a request template and a JSONPath, CSS or regex mapping. Monthly budgets and a cost estimate before every run; keys in the system keychain, bound to the provider's address.
- Keyword analysis: content, technical and site scores (domain age from RDAP, WHOIS or Wayback; Open PageRank; referring domains; optional Tranco), an explainable keyword difficulty with every component's contribution, search intent, opportunity, content gaps and what it takes to rank.
- Rank tracking: projects on several engines and locales, daily or weekly checks while the app is open or from cron, visibility, share of voice and cannibalisation; Excel, CSV and JSON exports.
- Every result page is stored; selector patches are tested on stored pages and old snapshots are read again without a request.

```sh
pnpm serp                 # native desktop app (Vite + Tauri)
pnpm serp:dev             # browser preview with sample data on http://localhost:1450
pnpm serp:build           # native installers for the current platform

cargo run -p snx-serp -- analyze "türk kahvesi nasıl yapılır" --engine google --country tr --site example.com
cargo run -p snx-serp -- project create "Example" --site example.com --engine google --country tr
cargo run -p snx-serp -- track --due
```

See [docs/serp.md](docs/serp.md) (Turkish: [docs/serp-kilavuz.md](docs/serp-kilavuz.md)) and [ADR 0064](docs/adr/0064-serp-finder.md).

## SEO Keyword Research

The SEO Keyword Research discovers keywords, groups them into clusters one page can target and maps the clusters to your site, on its own deep emerald ground. Its data folder, keys and budgets are separate from the SERP Checker's.

- Discovery from Yandex Wordstat, DataForSEO (Google Ads, Labs, autocomplete), SerpApi, Search Console and Google Ads Keyword Planner (your own OAuth client, loopback with PKCE), and imports of Ahrefs, Semrush, Keyword Planner, Search Console and Wordstat exports. Monthly budgets and a cost estimate before every run; keys in the system keychain.
- The free Google and Yandex suggestion endpoints are off until switched on with a terms warning, then slow, capped per day and stopped at the first refusal.
- Clustering by wording (Turkish-aware, 50,000 keywords in seconds), by shared top-ten results (soft or hard, through its own result-page chain) or both; manual moves kept across clusterings.
- Site mapping: target pages, cannibalisation, gaps and competitors; content briefs; Excel, CSV and JSON exports.

```sh
pnpm keywords             # native desktop app (Vite + Tauri)
pnpm keywords:dev         # browser preview with sample data on http://localhost:1460
pnpm keywords:build       # native installers for the current platform

cargo run -p snx-kw -- project create "Example" --site example.com --engine google --country tr --language tr
cargo run -p snx-kw -- import 1 ahrefs-keywords.csv
cargo run -p snx-kw -- cluster 1 --method lexical
```

See [docs/keywords.md](docs/keywords.md) (Turkish: [docs/keywords-kilavuz.md](docs/keywords-kilavuz.md)) and [ADR 0069](docs/adr/0069-keyword-miner.md).

## Design language

`packages/design` (`@octopusseo/design`) is **Chromatophore**, the design language every OctopusOn SEO program and the website share: design tokens (`tokens.json` → `tokens.css`: a deep product-colored ground with glass on top, one ground color per program), the fonts and the `oc-*` component styles. See [packages/design/README.md](packages/design/README.md) and [ADR 0008](docs/adr/0008-shared-design-language.md).

## Website

`apps/web` is the OctopusOn SEO website (Next.js): home, product pages (Log Analyser features, Spider, SERP Checker, SEO Keyword Research), downloads, docs and changelog in English, Turkish, German and Spanish. Visitors are sent to their language by cookie or `Accept-Language`.

```sh
pnpm web              # development server on http://localhost:3000
pnpm web:build        # production build
```

Downloads and the changelog come from the GitHub releases of `OCTOPUSSEO_GITHUB_REPO` (refreshed hourly, no redeploy needed); the tag prefix names the product (`v…` Log Analyser, `spider-v…` Spider, `serp-v…` SERP Checker, `kw-v…` SEO Keyword Research; see [docs/release.md](docs/release.md) and [ADR 0065](docs/adr/0065-multi-product-releases.md)). Without releases there, `pnpm web:downloads -- --product <log-analyser|spider|serp-finder|keyword-miner>` copies the installers and CLI binaries built on this machine into `apps/web/public/downloads` (ignored by git; run it before `pnpm web:build`) and the site offers those; with neither, a product's page and download section say it is not released yet. The download button picks the installer for the visitor's system and, where the browser tells, CPU architecture. See [apps/web/.env.example](apps/web/.env.example) for all settings.

## Repository layout

```
crates/seonexus-core        Shared value types (timestamps, dates, result tables)
crates/seonexus-logparse    Streaming log parsers, compression, format detection
crates/seonexus-query       Query AST, text language, planner, SQL compiler, templates
crates/seonexus-botverify   Bot catalog, user-agent classification, IP lists, verification
crates/seonexus-url         URL normalization, robots.txt
crates/seonexus-geo         IP → country / ASN lookups (DB-IP Lite data)
crates/seonexus-analytics   URL-list comparison, period comparison, crawl overview
crates/seonexus-store       DuckDB/Parquet storage adapter
crates/seonexus-report      CSV / XLSX / JSON exporters
crates/seonexus-net         HTTP and DNS adapters
crates/seonexus-log-app     Use-cases shared by the CLI and the desktop app
apps/snx-log                CLI
apps/desktop                Log Analyser desktop app (React UI + src-tauri shell)
apps/spider                 Spider desktop app (React UI + src-tauri shell)
apps/snx-spider              Spider CLI
apps/serp                    SERP Checker desktop app (React UI + src-tauri shell)
apps/snx-serp                SERP Checker CLI
apps/keywords                SEO Keyword Research desktop app (React UI + src-tauri shell)
apps/snx-kw                  SEO Keyword Research CLI
crates/seonexus-serp         SERP model, declarative mappings, Google/Bing/Yandex parsers, providers
crates/seonexus-serp-score   Content, technical, site, difficulty, intent and opportunity scores
crates/seonexus-serp-store   DuckDB store of runs, snapshots, raw responses, analyses and rankings
crates/seonexus-serp-app     SERP Checker use cases shared by both frontends
crates/seonexus-keywords     SEO Keyword Research domain: normalisation, expansions, filters, clustering, mapping
crates/seonexus-keyword-sources  Wordstat, DataForSEO, SerpApi, Search Console, Google Ads, suggestions
crates/seonexus-kw-store     SEO Keyword Research storage (DuckDB)
crates/seonexus-kw-app       SEO Keyword Research use cases shared by both frontends
crates/seonexus-oauth        Google OAuth for desktop apps (loopback, PKCE)
crates/seonexus-secrets      keychain, file and environment secret store
crates/seonexus-text         Tokenizing, Turkish-aware folding, stems and term statistics
crates/seonexus-domain-intel Domain age (RDAP, WHOIS, Wayback), Open PageRank, Tranco
crates/seonexus-crawl        Crawl engine, scope, robots.txt and sitemaps
crates/seonexus-html         HTML extraction and content fingerprints
crates/seonexus-audit        Bilingual technical SEO issue catalog
crates/seonexus-crawl-store  DuckDB crawl storage, audit rules and bounded table queries
crates/seonexus-spider-app   Spider use cases shared by both frontends
apps/web                    OctopusOn SEO website (Next.js)
packages/design             Chromatophore, the shared design language (tokens, fonts, component CSS)
docs/                       ADRs, query language, user guide, release notes
fuzz/                       cargo-fuzz targets
xtask/                      Developer automation
```

Architecture decisions are recorded in [docs/adr](docs/adr).

## Development

Requirements: Rust stable (via [rustup](https://rustup.rs)); for the desktop app Node 22+ and pnpm, plus the [Tauri system prerequisites](https://tauri.app/start/prerequisites/) on Linux.

```sh
cargo xtask ci          # fmt + clippy + tests + cargo-deny + frontend checks
cargo xtask frontend    # frontend only: type-check, tests, build
cargo xtask bot-ranges  # refresh the bundled crawler IP list snapshot
cargo run -p snx-log -- --help
```

See [CONTRIBUTING.md](CONTRIBUTING.md) for conventions.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option. Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as
above, without any additional terms or conditions.

Country and network data, downloaded on request, is "IP Geolocation by DB-IP"
(<https://db-ip.com>), licensed under CC BY 4.0.
