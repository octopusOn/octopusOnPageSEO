# octopusSEO

Open-source, cross-platform SEO tooling written in Rust. (Crates keep the internal `seonexus-` prefix of the project's first name.)

| Product | Status | Programs |
|---------|--------|----------|
| **octopusSEO Log Analyser** — server log analysis for SEO: verified search-engine and AI crawlers, crawl budget, status codes, orphan pages | ✅ usable | `snx-log` CLI, desktop app (macOS, Windows, Linux) |
| octopusSEO Spider — site crawler | planned | — |

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
- **Saved queries**: name any query (desktop "Save…" or `snx-log saved add`), reuse it on every site, share it as a JSON file.
- **Query language + 39 report templates** (including Screaming Frog-style URL, response-code, user-agent, referer, directory, IP, country and bytes tables with last status, days since last crawl, per-bot columns, 1xx–5xx columns and estimated CO2): `bot:googlebot* verification:verified status:4xx group:url metric:hits,last_seen` — see [docs/query-language.md](docs/query-language.md).
- **Analyses**: crawl-budget overview, URL list / sitemap comparison (uncrawled and orphan pages, with the crawl export's other columns and sitemap `lastmod` joined like a VLOOKUP), period comparison, inconsistent status codes.
- **Exports**: CSV (formula-injection safe), XLSX (native dates/numbers), JSON — streamed, so large results never sit in memory.
- **Desktop app** (Tauri 2 + React) in Turkish and English, sharing the data directory with the CLI. Filters are built in a simple form (field, operator, value, min/max ranges, grouping, sorting) that writes the query text, which stays visible and editable; value boxes suggest the site's most frequent bots, user agents, URLs, status codes, … with hit counts for multi-selection, and accept any typed value or `*` pattern (`googlebot*`, `*/amp`) as well.

## Install

Release builds are attached to GitHub releases (`v*` tags): desktop installers (`.dmg`, `.msi`/`.exe`, `.deb`/`.AppImage`) and `snx-log` archives. To build from source see [Development](#development).

## CLI quick start

```sh
# Check how a log is parsed (format detection, skipped lines) without storing anything
snx-log inspect access.log.gz

# Organise logs per client and site; the format is stored with the site
snx-log client add "Acme"
snx-log site add acme.com --client "Acme" --format combined
snx-log site add shop.acme.com --client "Acme" \
  --nginx-format '$remote_addr - $remote_user [$time_local] "$request" $status $body_bytes_sent "$http_referer" "$http_user_agent"'

# Import files or folders; re-importing is safe
snx-log import acme.com /var/log/nginx/

# Dashboards and reports
snx-log overview acme.com --period last30d
snx-log templates
snx-log query acme.com --template googlebot-errors
snx-log query acme.com 'bot_category:ai-* group:bot,day' --rows 100
snx-log query acme.com --template client-errors 'date:last7d' -o 404s.xlsx

# Saved queries (shared by all sites and with the desktop app)
snx-log saved add "Googlebot 404" 'bot:googlebot* verification:verified status:404 group:url'
snx-log query acme.com --saved "Googlebot 404" 'date:last7d'
snx-log saved export team-queries.json      # share with colleagues: saved import

# Analyses
snx-log compare-urls acme.com https://acme.com/sitemap.xml crawl-export.csv -o comparison.xlsx
snx-log compare-periods acme.com --before 2026-09-01..2026-09-30 --after 2026-10-01..2026-10-31 --group directory

# Bots
snx-log bots list --category ai-training
snx-log bots check-ip 66.249.66.1 --bot googlebot-smartphone
snx-log bots update            # download current crawler IP lists
snx-log bots verify acme.com --all
snx-log bots custom add acme-monitor --category monitoring --pattern AcmeMonitor --ip-range 203.0.113.0/24

# robots.txt compliance, countries and networks
snx-log robots acme.com --filter 'date:last30d' -o robots.xlsx
snx-log geo update             # optional: DB-IP Lite country/ASN data (~20 MB)
snx-log query acme.com --template spoofed-bot-networks

# Site settings and archives
snx-log site settings acme.com --traffic bots --strip-param sid
snx-log site export acme.com acme.snxsite
snx-log site import acme.snxsite --name acme-copy.com
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

## Website

`apps/web` is the octopusSEO website (Next.js): home, features, downloads, docs and changelog in English, Turkish, German and Spanish. Visitors are sent to their language by cookie or `Accept-Language`.

```sh
pnpm web              # development server on http://localhost:3000
pnpm web:build        # production build
```

Downloads and the changelog come from the GitHub releases of `OCTOPUSSEO_GITHUB_REPO` (refreshed hourly, no redeploy needed). Without releases there, `pnpm web:downloads` copies the installers and `snx-log` binaries built on this machine into `apps/web/public/downloads` (ignored by git; run it before `pnpm web:build`) and the site offers those, with release notes from `docs/releases/<version>.md`; with neither, the download page says releases are coming soon. The download button picks the installer for the visitor's system and, where the browser tells, CPU architecture. See [apps/web/.env.example](apps/web/.env.example) for all settings.

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
apps/desktop                Desktop app (React UI + src-tauri shell)
apps/web                    octopusSEO website (Next.js)
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
