# OctopusOn GEO-SEO Platform

The GEO-SEO Platform is the all-in-one SEO and GEO program: domain overview, keyword research, backlinks, gap analysis, rank tracking, site audits, AI visibility, Search Console and white-label reports. It runs as a desktop program and as a web application at app.octopuson.com, on the user's own API keys or on OctopusOn SEO credits. Design decisions are in [ADR 0077](adr/0077-geo-seo-platform.md); a Turkish guide is in [platform-kilavuz.md](platform-kilavuz.md).

The first release is built in milestones G1–G12:
- **G1, the foundation:** projects, sources (own key or credits), the account and jobs.
- **G2, the domain overview (Site Explorer):** summary, 24-month history, organic keywords, top pages, competitors with a traffic comparison, and subdomains.
- **G3, the Keyword Explorer:**
  - keyword numbers, one keyword or up to 700 at once;
  - ideas: matching, questions, related and broad;
  - the live result page, Yandex Wordstat, free autocomplete;
  - keyword lists with grouping into topics.
- **G4, Backlinks:**
  - the link profile and authority (OA);
  - backlinks: live, new, lost and broken, filterable by dofollow, one per site;
  - referring domains, anchors and the most linked pages;
  - referring domains won and lost over 12 months;
  - OA of many sites at once.
- **G5, gap analysis:** keywords competitors rank for and the site does not, shared keywords with weak spots, link gaps, and many sites compared at once.
- **G6, rank tracking:** keywords in up to 10 places on a schedule, with visibility, competitor shares, AI Overview citations and cannibalisation.
- **G7, the site audit:** the SEO Crawler's 118 checks, GEO readiness, and on the web crawls that reach only public addresses and are metered.
- **G8, AI visibility:** the project's questions asked to ChatGPT, Claude, Gemini, Perplexity and Google AI Mode, with mention and citation rates, share of voice against rivals, the sources cited, and LLM Mentions.
- **G9, Search Console and Google Analytics:** queries, pages, devices and countries against the period before, organic sessions and key events by landing page, joined with rank tracking and the latest site audit.
- **G10, reports:** white-label reports drawn from the modules' data, as a page, a PDF and an Excel workbook; on the web, read-only links and scheduled e-mails.
- **G11, AI assistants and the command line:** an MCP server for Claude, ChatGPT, Cursor and other assistants (the desktop's `octopuson-platform mcp`, the web's `/mcp` with access tokens), five agent skills, and the `octopuson-platform` command.
- **G12, release preparation:** the release workflow and its Linux web server archive, the server's deploy files, script and backups, the gateway's per-route holds and metered prices in the admin, the website's program page and the program's GitHub project files.

Version 0.1.1 adds sync: a desktop signed in to the account keeps its projects the same as the account's web workspace ([Sync](#sync), [ADR 0078](adr/0078-platform-sync.md)).

The other modules show "soon" in the sidebar until their milestone lands.

## Layout

| Part | Path | What it is |
|---|---|---|
| DataForSEO clients | `crates/octopuson-dataforseo` | Live tasks with costs; Labs (`domain_rank_overview` so far). |
| Domain and storage | `crates/octopuson-platform` | Projects, kept answers (cache) and spending in `platform.duckdb`. |
| Use cases | `crates/octopuson-platform-app` | Every operation as a command of one router; jobs; own key or credits per source. |
| Desktop shell | `apps/geo-seo-platform/src-tauri` | One Tauri command, `platform_call`, and the `platform-job` event. |
| Web server | `apps/octopuson-platform-server` | axum: account sign-in, one workspace per account, `/api/call/{command}`, `/api/events`, `/mcp`. |
| Command line | `apps/octopuson-platform-cli` | `octopuson-platform`: any command with JSON, and the MCP server over standard input and output. |
| MCP tools | `crates/octopuson-platform-app/src/mcp/` | The JSON-RPC server and its tools, shared by both transports. |
| Agent skills | `distribution/geo-seo-platform/skills/` | Instructions that teach assistants to use the tools. |
| Server deployment | `deploy/platform/`, `scripts/deploy-platform.sh`, `scripts/backup-platform.sh` | The systemd units, the nginx site and the scripts that install releases and back up data. |
| Interface | `apps/geo-seo-platform` | React; the same build on the desktop, on the web and with sample data. |
| Shared components | `packages/ui` | Brand mark, buttons, dialogs, fields, states on the Deep Sea design. |

## Running it

```sh
pnpm --filter @octopusseo/geo-seo-platform dev     # interface on http://localhost:1470 (sample data without a server)
pnpm --filter @octopusseo/geo-seo-platform tauri dev
cargo run -p octopuson-platform-server             # web server on http://127.0.0.1:8796
cargo run -p octopuson-platform-cli -- projects    # the command line, on the desktop's data folder
```

The interface's dev server passes `/api` and `/auth` to the web server (`PLATFORM_SERVER`, default `http://127.0.0.1:8796`). Without a server it runs on sample data and says so.

A local web server without the account site:

```sh
PLATFORM_DEV_LOGIN=1 PLATFORM_UI_DIR=apps/geo-seo-platform/dist OCTOPUSSEO_CLOUD_URL=http://127.0.0.1:9 \
  cargo run -p octopuson-platform-server
```

`PLATFORM_DEV_LOGIN=1` offers `/auth/dev`, a sign-in as the local account `dev-local`, on a loopback address only.

### Web server settings

| Variable | Default | Meaning |
|---|---|---|
| `PLATFORM_BIND` | `127.0.0.1:8796` | Listening address; nginx sits in front. |
| `PLATFORM_DATA_DIR` | `platform-data` | `server.duckdb` and `users/<account id>/`. |
| `PLATFORM_PUBLIC_URL` | `http://localhost:<port>` | The public address; https outside localhost. |
| `PLATFORM_PORTAL_URL` | `https://my.octopuson.com` | The account site. |
| `OCTOPUSSEO_CLOUD_URL` | `https://api.octopuson.com` | The gateway. |
| `PLATFORM_MASTER_KEY` | — | 32 random bytes, base64 (`openssl rand -base64 32`). Required outside the development sign-in. |
| `PLATFORM_UI_DIR` | — | The built interface (`apps/geo-seo-platform/dist`). |
| `PLATFORM_DEV_LOGIN` | — | `1` offers the development sign-in. |
| `PLATFORM_GOOGLE_CLIENT_ID` | — | The server's Google OAuth client (Web application type), for Search Console and Analytics. Set both or neither. |
| `PLATFORM_GOOGLE_CLIENT_SECRET` | — | Its secret. |
| `PLATFORM_SMTP_URL` | — | SMTP server of report e-mails: `smtps://user:password@host:465`, or `smtp://user:password@host:587?tls=required`. Plain-text SMTP is refused. Set it with `PLATFORM_MAIL_FROM`, or neither. |
| `PLATFORM_MAIL_FROM` | — | Their sender: `OctopusOn SEO <rapor@octopuson.com>`. |

The account site must allow the return address: `CONNECT_WEB_REDIRECTS=https://app.octopuson.com/auth/callback` in the portal's settings. The Google client must list `https://app.octopuson.com/auth/google/callback` as an authorized redirect URI.

## Commands

The desktop window and the web call the same commands with the same JSON. A failed command answers `{ "code", "message", "retry_after"? }`. The codes are `invalid`, `not_found`, `busy`, `no_source`, `budget`, `auth`, `quota`, `rate_limited`, `unavailable`, `provider`, `unsupported`, `account` and `internal`; the web adds the matching HTTP status.

| Module | Commands |
|---|---|
| App | `app_info`, `get_settings`, `save_settings`, `open_url` |
| Projects | `list_projects`, `get_project`, `create_project`, `update_project`, `delete_project` |
| Sources | `list_providers`, `set_key`, `remove_key`, `set_source_mode`, `set_budget`, `usage` |
| Account | `account_status`, `start_account_login` (desktop), `account_logout` (desktop), `account_page` |
| Jobs | `list_jobs`, `cancel_job`, `clear_jobs` |
| Domain overview | `overview_estimate`, `get_overview`, `fetch_overview` (with `report`) |
| Keyword Explorer | `keywords_estimate`, `get_keywords`, `fetch_keywords` (with `report`), `keyword_autocomplete` |
| Backlinks | `backlinks_estimate`, `get_backlinks`, `fetch_backlinks` (with `report`) |
| Gap analysis | `gap_estimate`, `get_gap`, `fetch_gap` (with `report`) |
| Rank tracking | `rank_setup`, `save_rank_settings`, `add_rank_tracker`, `remove_rank_tracker`, `track_keywords`, `untrack_keywords`, `set_keyword_tags`, `rank_estimate`, `start_rank_check`, `rank_runs`, `rank_report`, `rank_keyword`, `rank_volumes_estimate`, `fetch_rank_volumes` |
| Site audit | `audit_setup`, `save_audit_settings`, `start_audit`, `audit_report`, `audit_issue`, `delete_audit` |
| Search Console | `google_status`, `set_google_client` (desktop), `start_google_login`, `google_logout`, `google_properties`, `gsc_setup`, `save_gsc_settings`, `start_gsc_sync`, `gsc_report`, `gsc_query` |
| Access tokens (web) | `list_access_tokens`, `create_access_token`, `revoke_access_token`; the server answers them itself |
| Reports | `reports_setup`, `save_report_branding`, `save_report`, `delete_report`, `report_html`, `report_pdf`, `report_xlsx`, `share_report` (web), `unshare_report`, `report_deliveries`, `send_report` (web) |
| AI visibility | `ai_setup`, `save_ai_settings`, `add_ai_prompts`, `remove_ai_prompts`, `suggest_ai_prompts`, `ai_estimate`, `start_ai_run`, `ai_runs`, `ai_report`, `ai_answer`, `ai_mentions_estimate`, `get_ai_mentions`, `fetch_ai_mentions` |
| Keyword lists | `keyword_lists`, `create_keyword_list`, `rename_keyword_list`, `delete_keyword_list`, `list_keywords`, `add_to_keyword_list`, `remove_from_keyword_list`, `cluster_keyword_list` |
| Sync (desktop) | `sync_status`, `set_sync`, `sync_now`, `sync_choose` |

A new module adds its commands in its own file, `crates/octopuson-platform-app/src/modules/<module>.rs`, and its page in `apps/geo-seo-platform/src/modules/<module>/`; its translations go in `src/i18n/<language>/<module>.json`.

## Domain overview reports

The three overview commands take `{ project_id, domain?, report, limit?, force? }`. `domain` looks at another domain (a competitor) in the project's market.

| `report` | DataForSEO Labs endpoint | What it holds |
|---|---|---|
| `summary` (default) | `domain_rank_overview` | Organic and paid keywords, traffic, traffic value, positions, movement. |
| `history` | `historical_rank_overview` | The last 24 months, oldest first. |
| `keywords` | `ranked_keywords` | Keywords by traffic: position and change, volume, KD, CPC, intent, address, SERP elements. |
| `pages` | `relevant_pages` | Pages by organic traffic. |
| `competitors` | `competitors_domain` | Domains sharing keywords, without the giants (`exclude_top_domains`). |
| `subdomains` | `subdomains` | Subdomains by organic traffic. |
| `compare` | `bulk_traffic_estimation` | The domain and the project's competitors (needs competitors). |

- **Row count.** Table reports take `limit`: 100, 500 or 1000 rows (10–1000). The cost grows with the rows, and every row count is kept separately.
- **Where filtering happens.** Filtering, sorting and the CSV download happen in the interface, on the rows fetched. They cost nothing.
- **Search.** Text search ignores case, accents and the Turkish dotted/dotless i ("ayakkabi" finds "ayakkabı").

## Keyword Explorer reports

The keyword commands take `{ project_id, report, keywords?, seed?, limit?, depth?, mobile?, force? }`. Every report is asked in the project's market.

| `report` | Source | What it holds |
|---|---|---|
| `overview` (default) | Labs `keyword_overview` | Volume, 12 months, trend, KD, CPC, competition, intents, SERP elements, the top ten's average backlinks (up to 700 `keywords`). |
| `ideas` | Labs `keyword_ideas` | Keywords on the topics of the seeds (`keywords`, up to 200). |
| `related` | Labs `related_keywords` | Google's related searches of `seed`, `depth` 1–4 (2 by default). |
| `suggestions` | Labs `keyword_suggestions` | Searches containing `seed`; the interface also picks the questions from them. |
| `serp` | SERP `google/organic/live/advanced` | The live top ten and the page's elements (`mobile` for the phone page). |
| `wordstat` | Yandex Wordstat `topRequests` | Yandex's most frequent searches with `seed`, and similar searches. Uses the `yandex-cloud` keys (`api_key`, `folder_id`) or the `wordstat` credit connector. |

Kept answers last as long as their kind of data: keyword numbers 30 days, result pages 1 day.

Free, never paid:
- **Autocomplete.** Google's or Yandex's suggestions for a seed (`keyword_autocomplete`).
- **Topics.** `cluster_keyword_list` groups a list's keywords by wording, with the project's language stems (`octopuson-keywords`). Each topic is the keywords one page can target.

**Keyword lists.** Lists belong to a project and hold up to 10,000 keywords, each with its numbers when it was added. Deleting a project deletes its lists.

## Backlink reports

The backlink commands take `{ project_id, report, target?, limit?, status?, dofollow_only?, one_per_domain?, targets?, force? }`. `target` is a domain or a page address (a page stays a page); the project's domain by default. Ranks are asked on the 0–100 scale and shown as **OA (Octopus Authority)**.

| `report` | Backlinks API | What it holds |
|---|---|---|
| `summary` (default) | `summary` | OA, backlinks, referring domains (and nofollow ones), IPs, subnets, broken links and pages, spam score; links by attribute, type, top-level domain, country and kind of site. |
| `backlinks` | `backlinks` | Linking pages, strongest first. `status`: `live`, `new` (`is_new`), `lost` (lost links), `broken` (`is_broken`). `dofollow_only`, `one_per_domain`. |
| `referring_domains` | `referring_domains` | Linking sites by OA; whether a site only gives nofollow links. |
| `anchors` | `anchors` | Anchor texts, used by the most sites first. |
| `pages` | `domain_pages` | The target's pages linked from the most domains (status codes show linked pages that are gone). |
| `new_lost` | `timeseries_new_lost_summary` | Backlinks and referring domains won and lost, month by month, 12 months. |
| `bulk` | `bulk_ranks` | OA of `targets` (up to 1,000), or of the project's domain and competitors. |

Kept answers last 3 days. On credits, the gateway's DataForSEO connector allows every live Backlinks report (`/v3/backlinks/*/live`); a connector already saved in the admin panel needs that route added there.

## Gap reports

The gap commands take `{ project_id, report, competitors?, targets?, limit?, force? }`. `competitors` defaults to the project's; the project's own domain and repeats are dropped, and at most 5 are compared. A gap asks DataForSEO once per competitor; the parts are estimated, kept and paid together (`estimate_many`, `kept_many`, `fetch_many` in `fetch.rs`), and the budget is checked on the total before the first part is asked.

| `report` | Endpoint | What it holds |
|---|---|---|
| `missing` (default) | Labs `domain_intersection`, competitor first, `intersections: false` | Keywords competitors rank for and the site does not. Keywords more competitors share come first, then the most searched. |
| `shared` | Labs `domain_intersection`, the site first, `intersections: true` | Keywords both rank for, with the site's position and each competitor's; `weak` where a competitor ranks higher. |
| `links` | Backlinks `domain_intersection` (`intersection_mode: all`, `exclude_targets`: the site) | Sites linking to every chosen competitor but not to the site, by OA, with the backlinks each competitor gets. |
| `bulk` | Labs `bulk_traffic_estimation` + Backlinks `bulk_ranks` | Organic traffic, keywords, paid traffic and OA of `targets` (up to 100), or of the project's domain and competitors. The interface draws each site's share of the summed traffic (share of voice). |

`limit` is the row count per competitor (100, 500 or 1000). A kept gap is fresh only while every part is; keyword gaps last 7 days, link gaps 3 days.

## Rank tracking

A project tracks keywords (up to 5,000, with tags and their monthly searches when known) in up to 10 **places**: engine (`google`, `bing`, `yandex`), country, language, device and an optional city or region in DataForSEO's location names (`Istanbul,Istanbul,Turkey`, checked to be in the place's country). The first keywords add the first place: Google in the project's market, desktop.

- **Checks.** A run checks every keyword in every place on DataForSEO's live result page (`serp/{engine}/organic/live/advanced`), through a `PaidRun` (`fetch.rs`): always fresh, paid within the month's budget and recorded, never kept in the answer cache. Each check's reading is kept in the rank history (`rank_checks`): the site's best position and page, every page of the site among the results (more than one is cannibalisation), the page's elements, whether an AI Overview is shown and cites the site (its references), whether the site holds the featured snippet, and the first 20 organic results for the shares. A subdomain counts as the site.
- **Price.** $0.002 per ten results looked at (`depth` 10, 20, 50 or 100; 20 by default), plus $0.002 on Google when AI Overviews that load later are asked for (`load_async_ai_overview`, refunded when there is none). `rank_estimate` gives one run and thirty days on the schedule, in credits on credits.
- **Runs.** A run is a job (`rank-check`); `start_rank_check` refuses without a source or over the budget before anything is paid. A failed check is recorded and the run goes on, unless the failure concerns every check (no key or credits, budget, refused key, empty balance, rate limit): then the run stops as `partial` or `failed` with the reason. A run left running by a program that stopped becomes `interrupted`.
- **Schedule.** `manual` (the default: nothing is paid on its own), `daily` (20 hours after the last run) or `weekly` (6.5 days). The interface asks for it when the first keywords are added. `Platform::run_due` starts the runs that are due and `Platform::next_due` says when the next one is: the desktop program asks every minute while it is open (a run missed while it was closed starts shortly after it opens); the web server keeps every account's next due time in `server.duckdb` (`schedules`), notes it again after the commands that move it, and every minute starts up to 8 due accounts.
- **Report.** `rank_report { project_id, tracker_id?, days? }` (the first place, 90 days by default, 7–365): per keyword its position, change, best, page, history by day and the page's signals; a summary; visibility by day; the shares of the site and its competitors (listed ones, else the five most visible domains) now and by day. Visibility is the share of the clicks the keywords would get in first place, weighted by monthly searches; the measures (`octopuson-serp-score::rank`) are the SERP Checker's.
- **Volumes.** Keywords added without monthly searches count as one search. `fetch_rank_volumes` asks Labs keyword numbers for them (700 a request, kept 30 days like the Keyword Explorer's).
- **Keyword Explorer.** Selected keywords go to the Rank Tracker with their volumes ("Track rankings").

## Site audit

An audit crawls the project's site (`https://<domain>/`) with the SEO Crawler's engine and stores it like the SEO Crawler does, in `crawls/<id>/` of the workspace; the platform keeps a row per audit (`audits`) with its health, GEO readiness, findings and what was metered.

- **The crawl.**
  - It identifies as the OctopusOn crawler (`octopusSEO-Spider`) and obeys robots.txt (the `octopusseo-spider`, `octopusseo` and `*` groups).
  - It follows the site's links, canonicals, hreflang, pagination and sitemaps, and checks its images. Other sites, stylesheets and scripts are not fetched.
  - Server HTML only (no JavaScript rendering).
  - `max_pages` (URLs fetched, 10 at least) is up to 100,000 on the desktop and 5,000 on the web.
- **Checks.** The 118 checks of the SEO Crawler (21 categories, `octopuson-audit`) and its health score, with English and Turkish texts (other languages see English). The report compares every check with the previous finished audit; `audit_issue` lists where a check was found, 100 addresses a page.
- **GEO readiness** (`CrawlReader::geo_readiness` in `octopuson-crawl-store`). It is read-only over the crawl and adds no findings, so the SEO Crawler's results stay the same. It has eight weighted signals (100 points); signals that cannot be measured are left out and the rest renormalised:

  | Signal | Weight | Passes when |
  |---|---|---|
  | `ai-search-access` | 25 | AI search crawlers (OAI-SearchBot, PerplexityBot, Claude-SearchBot …) may fetch `/` under the home page's robots.txt; an unreachable robots.txt blocks all |
  | `snippets-allowed` | 15 | an indexable page has no `nosnippet` and no `max-snippet` under 50 |
  | `structured-data` | 15 | an indexable page has valid JSON-LD |
  | `ai-assistant-access` | 10 | AI assistant fetchers (ChatGPT-User, Claude-User …) may fetch `/` |
  | `entity-schema` | 10 | the site has Organization (or LocalBusiness, Corporation, Store) and WebSite schema |
  | `question-headings` | 10 | a page of 300+ words has an H2/H3 that asks a question (question mark; question word first in English, German, Spanish; Turkish question words anywhere) |
  | `substantial-content` | 10 | an indexable page has 300+ words |
  | `llms-txt` | 5 | `/llms.txt` answers 200 with text |

  Each signal lists up to 20 failing pages, crawlers or missing types. The report also shows every AI crawler with its robots.txt decision and the schema types found.
- **On the web.**
  - **Public addresses only.** The crawl uses `CrawlFetcher::public_only` (`octopuson-net::guard`). Its resolver keeps only public addresses, so loopback, private, link-local (cloud metadata at 169.254.169.254), shared, documentation, multicast and reserved addresses, IPv4-mapped forms included, are refused on every request and redirect. The connection uses exactly the checked addresses, so DNS rebinding cannot get through.
  - **Limits.** At most 3 crawls run on the server at once (the others wait); each uses 3 threads at 4 requests a second.
  - **Metering.** Every URL fetched is metered on the account (`crawl.page`, see [cloud.md](cloud.md)), in batches of 100 while the crawl runs and the rest at the end. Each batch has the idempotency key `<crawl id>:<batch>`. When the credits run out the crawl stops; the audit is `stopped` and says why. The desktop crawls on the computer and is never metered.
- **Schedule.** `manual` (the default), `weekly` (6.5 days) or `monthly` (29 days). These run through `Platform::run_due` with the rank checks. Deleting an audit or the project deletes its crawl folders.

## Search Console and Google Analytics

A project reads its own Google data: one Search Console property and, optionally, one Google Analytics 4 property. Google's data is free, so nothing is paid or recorded as spending.

- **Connecting Google** (`google.rs`). Only read-only scopes are asked for: `webmasters.readonly`, and `analytics.readonly` when Analytics is wanted. The refresh token is kept with the user's keys, bound to Google's token address; access tokens stay in memory until shortly before they expire, and one Google refuses is renewed once.
  - **Desktop.** The user's own OAuth client of the "Desktop app" type (client id and secret on the page; the secret goes to the credential store), else the client the program was built with (`OCTOPUSON_GOOGLE_CLIENT_ID` and `OCTOPUSON_GOOGLE_CLIENT_SECRET`, at run or build time). Signing in is a job (`google-login`): Google's consent page opens in the browser and comes back to a one-time listener on `127.0.0.1` (PKCE S256, checked state).
  - **Web.** The server's own Web application client (`PLATFORM_GOOGLE_CLIENT_ID` and `PLATFORM_GOOGLE_CLIENT_SECRET`). `start_google_login` gives the consent page; the account's workspace remembers its state and PKCE verifier for ten minutes. Google returns to `/auth/google/callback`, which needs the account's session and the same state, then sends the browser to `/#/gsc`. Google must verify the app before people outside the test users can connect (sensitive scopes).
  - **Signing out** revokes the grant and forgets it.
- **Properties.** `google_properties { project_id }` lists the Search Console properties the user can read (not unverified ones), the GA4 properties of their accounts (when Analytics was granted), and the property that covers the project's domain (its `sc-domain:` property first, then an https prefix).
- **Syncs.** A sync is a job (`search-console`) over the chosen period (7, 28, 90 or 180 days, ending three days ago: Google's final data) and as many days before it (`octopuson-google`):
  - Search Console, web search, final data: clicks, impressions and positions by day (both periods), by query and by page (both periods, 5,000 rows each), by device and country, and by query on page (25,000 rows);
  - Analytics, when a property is chosen: organic search (`sessionDefaultChannelGroup` = Organic Search) sessions, engaged sessions, key events and revenue by day and by landing page (both periods). Sampled or thresholded numbers are marked. When Analytics fails the sync is `partial` and keeps the Search Console numbers.

  A sync replaces the project's numbers (`gsc_rows`, `gsc_query_pages`, `ga4_rows`, migration 6); Google keeps the history.
- **Schedule.** `weekly` (the default, 6.5 days), `daily` (20 hours) or `manual`, through `Platform::run_due`. Nothing is due without a Google sign-in.
- **Report.** `gsc_report { project_id }` reads the latest finished sync:
  - totals of both periods (CTR, position weighted by impressions) and Analytics' organic sessions, key events and revenue;
  - every day of the period beside the same day of the period before;
  - the top 500 queries with their changes, the pages each was shown on, whether the rank tracker follows it, `near_top` (average position 4–20 with enough impressions) and `low_ctr` (less than half the CTR its position usually gets);
  - the top 500 pages with their changes, Analytics' organic sessions, key events and revenue (matched by path), and how the latest site audit saw them (status, indexability, findings);
  - queries several pages share (cannibalisation), devices and countries;
  - how many pages with clicks the latest audit found broken, redirected or not indexable, and how many tracked keywords Search Console has numbers for.

  `gsc_query` lists the pages a query was shown on. Queries can go to the rank tracker from the page (`track_keywords`).

## Reports

Agencies send their clients reports with their own brand. A report never asks a paid source: it is drawn, when shown, from what the modules already hold.

- **Branding** (one per workspace, white label): company name, logo (PNG, JPEG, WebP or SVG up to about 300 KB, kept as a `data:` address), accent colour, website, a footer line, and whether "Prepared with OctopusOn SEO" shows.
- **A report** belongs to a project: a name, a language (Turkish, English, German, Spanish; its own words and number formats, whatever the interface's language), a period (7, 28 or 90 days) and its parts in order:

  | Part | Drawn from |
  |---|---|
  | `summary` | two headline numbers of each module that has data |
  | `rankings` | `rank_report`: visibility with its change, average position, ranked and top-3/top-10 counts, movement, the visibility trend, the 25 most searched tracked keywords |
  | `competitors` | `rank_report`: visibility shares against competitors |
  | `search_console` | `gsc_report`: clicks, impressions, CTR and position against the period before, Analytics' organic sessions and key events, daily clicks beside the period before, top queries and pages |
  | `site_audit` | `audit_report` (latest audit): health and GEO readiness against the audit before, pages, findings, the main issues by priority |
  | `ai_visibility` | `ai_report`: mention and citation rates, share of voice, mention rate by assistant, brands' shares |
  | `backlinks` | the kept backlink summary and domain overview (`get_backlinks`, `get_overview`), never fetched for the report |

  A part whose module has nothing yet says so.
- **The page** (`report_html`, `report/html.rs`): one self-contained HTML document, styles inline, charts as inline SVG, for the screen and A4 print. Its own Content-Security-Policy forbids every request and script, and every text is escaped. The app shows it in a sandboxed frame; links and e-mails show the same page.
- **PDF** (`report_pdf`): the page printed by an installed Chrome, Chromium or Edge (`octopuson-render::print_pdf`: a private headless browser, scripts off, `Page.printToPDF`). On the web each PDF is metered as `report.pdf` once the gateway prices it ([cloud.md](cloud.md)); a month's free PDFs come first.
- **Excel** (`report_xlsx`): the headline numbers on the first sheet, then a sheet per table, numbers as numbers. Files come to the page as base64; the desktop window saves them where the person chooses (save dialog), a browser downloads them. CSV exports of the other pages save the same way.
- **Read-only links** (web, `share_report`): a random token (32 bytes); `/r/{token}` shows the report, drawn from current data, to anyone with the link, without signing in, `no-store` and `noindex`. The server keeps only the token's SHA-256 and whose report it is; `unshare_report` (or a new link) takes the old one away, and opens are counted.
- **E-mails** (web with SMTP): `weekly` (6.5 days) or `monthly` (29 days) to up to 5 recipients, through `Platform::run_due`; `send_report` sends now. Each e-mail carries the PDF (the page itself when no browser is installed), the link when the report has one, and the account's address as Reply-To. Deliveries are kept (`report_deliveries`); a failed one waits for the next turn of the schedule. The desktop program sends no e-mail.

## AI visibility

A project asks AI assistants up to 200 **questions** (500 characters each, with tags) and reads every answer for its own brand and up to 10 **rival brands**. A rival is a domain plus the names answers may use; without rivals, the project's competitors stand in.

- **Assistants.** ChatGPT, Claude, Gemini, Perplexity and Google AI Mode. A project chooses which (all by default) and, per assistant, the model.
  - **The user's own key** for an assistant (Sources: OpenAI, Anthropic, Google Gemini, Perplexity) asks it directly, with web search, through `octopuson-llm`:

    | Assistant | API | Default model | Search | Sources from |
    |---|---|---|---|---|
    | ChatGPT | OpenAI Responses `/v1/responses` | `gpt-5.5` | `web_search` tool | `url_citation` annotations |
    | Claude | Anthropic Messages `/v1/messages` | `claude-opus-5-5` | `web_search_20260209` (basic `web_search_20250305` on older models), 5 searches at most; a paused turn is resumed | `web_search_result_location` citations |
    | Gemini | `models/{model}:generateContent` | `gemini-3.8-flash` | `google_search` | grounding chunks |
    | Perplexity | Agent API `/v1/agent` | the `fast` preset, or `provider/model` | `web_search` | `url_citation` annotations, else the search results its `[n]` markers point to |

  - **DataForSEO.** Without that key the assistant is asked through DataForSEO's LLM Responses (`ai_optimization/{chat_gpt,claude,gemini,perplexity}/llm_responses/live`), with web search in the project's country, on the user's DataForSEO key or on credits. Its default models are `gpt-5`, `claude-opus-5`, `gemini-2.5-flash` and `sonar`.
  - **Google AI Mode** is asked through DataForSEO only (`serp/google/ai_mode/live/advanced`, in the project's market).
- **Reading an answer** (`octopuson-llm::analysis`).
  - Brand names are found as whole words in any case or accent (Turkish ı and İ fold to i). The project's brand matches its brand names, its domain and the domain's first label.
  - For every brand: its place among the brands named (by first mention), whether one of its pages is a source, and the tone of the sentences naming it, from −1 to 1. Tone uses a small lexicon in Turkish, English, German and Spanish.
- **Runs.** A run is a job (`ai-visibility`). It asks every question on every chosen assistant, the assistants in parallel.
  - Each source is paid through its own `PaidRun`: always fresh, within the month's budget, and recorded. Own-key answers are priced from their tokens and searches at the providers' list prices; Perplexity reports its own cost.
  - An answer that fails is recorded with its error and the run goes on. A failure that concerns every answer of a source (no key or credits, budget, refused key, empty balance, rate limit) stops that assistant only.
  - Every answer is kept (`ai_answers`) with its text (up to 20,000 characters), the brands it names and its sources.
- **Price.** `ai_estimate` lists each assistant's route, model and price for one run, and for a month on the schedule. Own keys are estimated from typical answers: about $0.04 on ChatGPT, $0.074 on Claude Opus 5.5, $0.03 on Gemini and $0.012 on Perplexity. Through DataForSEO an answer is $0.0006 plus the model's price; AI Mode is $0.004.
- **Schedule.** `manual` (the default), `weekly` (6.5 days) or `monthly` (29 days). Runs start through `Platform::run_due` with the rank checks and audits.
- **Report.** `ai_report { project_id, days? }` (90 days by default, 7–365) reads the latest answer of every question on every assistant:
  - mention rate, citation rate, share of voice (the brand's share of every brand mention), average place and tone, overall and by assistant;
  - every brand's mentions, citations and share;
  - mention rate, citation rate and share of voice run by run, and each assistant's mention rate per run;
  - the questions × assistants matrix;
  - the most cited domains, and the project's own cited pages.

  `ai_answer` gives a question's last 10 answers on one assistant, with the brands to highlight.
- **Suggestions.** `suggest_ai_prompts` offers free questions in the project's language: about its brand, and about its most searched tracked keywords.
- **LLM Mentions.** `ai_mentions_estimate`, `get_ai_mentions` and `fetch_ai_mentions { project_id, report, limit?, force? }` read DataForSEO's database of AI answers (Google AI Overview and ChatGPT). Answers are kept 7 days, like domain data. A task is $0.10 plus $0.001 a row.

  | `report` | Endpoint | What it holds |
  |---|---|---|
  | `summary` (default) | `llm_mentions/target_metrics/live` | The site's mentions and AI search volume, by platform, with the domains cited alongside. |
  | `queries` | `llm_mentions/search_mentions/live` | The questions whose answers mention or cite the site, by AI search volume (`limit` 10–1000). |
  | `compare` | `target_metrics` for the site and up to 4 rivals | The same metrics side by side. |

## AI assistants (MCP)

Assistants that speak the Model Context Protocol read the workspace's data with the platform's own commands, so they answer like the pages. The server (`crates/octopuson-platform-app/src/mcp/`) is JSON-RPC 2.0 written for the platform: `initialize` (protocol versions 2025-11-25, 2025-06-18, 2025-03-26 and 2024-11-05), `ping`, `tools/list` and `tools/call`; notifications get no answer and batches are answered. It offers tools only. Every answer has the JSON as text and as `structuredContent`; long lists are cut to 200 rows (40 for very long answers).

| Tool | Paid | What it gives |
|---|---|---|
| `list_projects` | no | Projects with ids, domain, market, competitors and brand names. |
| `domain_overview` | yes | `get_overview`/`fetch_overview`: `summary`, `history`, `keywords`, `pages`, `competitors`, `subdomains`; another `domain` in the project's market. |
| `keyword_research` | yes | `get_keywords`/`fetch_keywords`: `overview` (up to 700 keywords), `ideas`, `related`, `suggestions`, `wordstat`. |
| `serp_overview` | yes | The live result page of a `keyword` (desktop or `mobile`). |
| `backlinks` | yes | `summary`, `backlinks`, `referring_domains`, `anchors`, `pages`, `new_lost` of the project's domain or a `target`. |
| `keyword_gap` | yes | `missing`, `shared`, `links`, `bulk` against the project's or the given `competitors`. |
| `ai_mentions` | yes | LLM Mentions: `summary`, `queries`, `compare`. |
| `rank_report` | no | The rank tracker's report over `days`. |
| `site_audit_issues` | no | The latest audit: scores, overview, AI crawler access, checks with findings sorted by priority (`lang` `en` or `tr`). |
| `ai_visibility` | no | The AI visibility report over `days`. |
| `search_console` | no | The latest Search Console and Analytics sync. |
| `estimate_cost` | no | What a paid tool call would cost now, and whether kept data is fresh. |
| `spending` | no | Sources, how each is paid, budgets and spending by month. |

**Spending is asked for, never assumed.** A paid tool answers from the kept answer while it is fresh (`status: "kept"`). Otherwise it buys nothing and answers `status: "needs_approval"` with the price (`usd`, `credits`, `paid_with`) and any stale kept data. Only a call with `allow_spend_usd` at least the price buys (`status: "fetched"`); `refresh: true` asks for fresh data even when the kept answer is fresh. The server's instructions tell assistants to ask the user and never raise the allowance themselves. Sources' modes and monthly budgets apply as on the pages; a source that cannot be used answers with what to set up under Sources.

**Transports.**
- **Desktop:** `octopuson-platform mcp` reads one JSON message per line on standard input and answers on standard output. Assistants start it from their MCP settings: `{"mcpServers":{"octopuson-platform":{"command":"octopuson-platform","args":["mcp"]}}}`, or `claude mcp add octopuson-platform -- octopuson-platform mcp`. It works on the desktop program's data folder; the program and the command take turns on the database (the store's lock), so both can run.
- **Web:** `POST https://app.octopuson.com/mcp` (Streamable HTTP, answered as `application/json`; notifications get `202`). Each request carries `Authorization: Bearer opt_…`, a personal access token made under Settings → AI assistants. `GET` and `DELETE` answer `405`: the server keeps no MCP sessions and sends no event stream.

**Access tokens (web).**
- `opt_` and 32 random bytes; shown once when made. `server.duckdb` keeps only the SHA-256, the name, when it was made and last used.
- At most 10 per account; `revoke_access_token` stops one at once.
- A token works only while its account is signed in to OctopusOn SEO on the server: revoking the program's access on the account site stops its tokens too.
- A request with an `Origin` header other than `PLATFORM_PUBLIC_URL` is refused (`403`), so web pages cannot drive it from a browser.

**Agent skills.** `distribution/geo-seo-platform/skills/` holds `seo-audit`, `keyword-research`, `competitor-analysis`, `geo-visibility` and `seo-coach` (`SKILL.md` with `name` and `description`); each tells the assistant which tools to call in which order and repeats the spending rule. A test keeps every tool and argument they name in step with the server.

## Command line

`octopuson-platform` is the platform without a window, on the desktop program's data folder (`--data-dir` or `OCTOPUSON_PLATFORM_DATA_DIR` for another):

```sh
octopuson-platform projects                                  # the projects
octopuson-platform commands                                  # every command
octopuson-platform call rank_report '{"project_id":1}'       # any command, JSON in and out
octopuson-platform call start_audit '{"project_id":1}'       # waits for the job; --no-wait returns its id
octopuson-platform mcp                                       # the MCP server for assistants
```

Progress goes to standard error (`-q` silences it); a failed command prints its error and exits with 1. `call` runs exactly what it is given, paid commands (`fetch_…`) included, under the sources' budgets.

## Paying for data

Every paid question goes through `Workspace::fetch`:

1. **Kept answer.** A fresh kept answer is returned free. Freshness depends on the kind of data: domains 7 days, backlinks 3 days, result pages and AI answers 1 day, keyword numbers 30 days.
2. **Source.** The source runs on the user's own key or on credits (`auto`, `own`, `credit`).
3. **Budget.** The month's budget, if set, must not be passed.
4. **Request.** The answer is kept, and its cost is recorded: the provider's own cost, or the gateway's charge in credits.

Pages first show what is kept (`get_…`) and the estimate (`…_estimate`, in credits on credits). They pay only when the user asks for fresh data (`fetch_…`, `force` to refresh a fresh answer).

On credits, the gateway also keeps answers for every account ([ADR 0079](adr/0079-shared-answer-cache.md)).
- **Estimates.** An estimate asks the gateway (`X-Octopus-Cache: peek`) whether it keeps the DataForSEO answer and shows its discounted price (`shared`).
- **Fetching.**
  - A fetch reports what the discount saved (`saved_usd`).
  - Refreshing an answer that is still fresh here passes the gateway's cache by (`Cache-Control: no-cache`).
- **Option.** Sources → "Use discounted answers from the cache" (`fresh_only` in `settings.json`) turns it off on this device.

## Sync

A desktop signed in to an OctopusOn SEO account keeps its data the same as the account's web workspace; [ADR 0078](adr/0078-platform-sync.md) has the design.

- **What travels.**
  - projects with their settings, competitors and brands;
  - tracked places and keywords;
  - AI questions and settings;
  - keyword lists;
  - the report branding, reports and their deliveries;
  - finished rank runs with their checks, AI runs with their answers, audits, Search Console syncs with the project's numbers;
  - kept answers for domains, backlinks, keywords and AI mentions (up to 2 MB each).

  Own API keys, the device's settings (payment modes, budgets, proxy), spending records and Google sign-ins stay on each side.
- **How.**
  - The desktop (`octopuson_platform_app::sync::round`) sends `POST /api/sync` with the account token. The body is JSON compressed with zstd (`application/vnd.octopuson.sync+zstd`).
  - The server checks the token at the gateway (remembered for 5 minutes), allows 240 requests a minute per account, and answers with `hub_answer` from the account's workspace.
  - A round greets the hub, sends the desktop's changes, then takes the others'.
  - It runs every 30 seconds, about 3 seconds after a change, and after a job ends; it waits up to 5 minutes while the web cannot be reached.
  - `PLATFORM_SYNC_URL` points a desktop at another server (http only for this computer), for example a local `octopuson-platform-server` with `PLATFORM_DEV_LOGIN=1`, whose development account takes the token `oct_dev`.
- **Records.** In `platform.duckdb` (migration 8):
  - every travelling record has a `uid`;
  - `sync_rows` keeps each record's last fingerprint and change number;
  - `sync_state` keeps the copy's id, the hub's epoch, the cursors and the clock offset;
  - `sync_pending` keeps changes waiting for the record they belong to.
- **Who wins.**
  - The later change, with the desktop's clock corrected by the hub's.
  - A deletion over an edit of a record made apart.
  - Times the program writes itself (last run, last sync, last e-mail) keep the later value.
  - A copy cannot make report links, only stop them.
- **Choices.** The first sync of a computer with projects, with an account whose web has projects too, waits for the person. So do another account and a reset hub. Settings → Sync and a dialog offer:
  - merge;
  - put this computer's data on the web;
  - take the web's data here;
  - pause.
- **Scheduled work.**
  - While a desktop syncs (switched on, signed in, synced once, no choice waiting), `Workspace::schedules_here` is false on the desktop and the web server runs the account's schedules.
  - Until the account signs in on the web, the server keeps the desktop's token (marked as borrowed in `sync-token.sha256`) for that. A later web sign-in replaces it without revoking it.
  - Paid schedules use credits or keys entered on the web.
- **Crawls.**
  - After a round, the computer that ran an audit sends its crawl to `POST /api/sync/crawls/{audit uid}` (packed, at most 30 MB). The web keeps at most 5 GB of crawls per account.
  - A desktop takes the latest audit's crawl of each project with `GET` on the same address.
- **nginx.** `/api/sync` takes bodies up to 32 MB and is not buffered (`deploy/platform/octopuson-platform.conf`).

## Web version

- **Sign-in.** The account site's one-click flow (`/baglan`) with PKCE, returning to `/auth/callback`.
- **Account token.**
  - The token is sealed (AES-256-GCM, master key) in the account's folder.
  - Scheduled work keeps using it while nobody is signed in. Signing out ends the browser's session only.
  - Revoking the token on the account site ends every session of the account.
- **Sessions.** Cookies are HttpOnly, SameSite=Lax, and `__Host-` on https. The database keeps only their SHA-256.
- **Writes.** Every command carries the session's CSRF token (`x-csrf-token`).
- **Own keys.** They are sealed in the account's folder. The server's environment never stands in for them.
- **Network.** No proxy setting on the web: requests leave from the server itself.
- **Scheduled work.** Rank checks, site audits, AI visibility runs and Search Console syncs run on the server for every account with a schedule, with the account's sealed token; see [Rank tracking](#rank-tracking), [Site audit](#site-audit), [AI visibility](#ai-visibility) and [Search Console](#search-console-and-google-analytics).
- **Google.** Accounts connect Google with the server's own client and return to `/auth/google/callback`; the refresh token is sealed in the account's folder.
- **Reports.** `/r/{token}` shows shared reports without a session; report e-mails go through `PLATFORM_SMTP_URL`. Chrome on the server prints the PDFs.
- **AI assistants.** `/mcp` takes personal access tokens instead of sessions; see [AI assistants](#ai-assistants-mcp).
- **Crawls.** Server crawls reach public addresses only and are metered on the account; see [Site audit](#site-audit).
- **Headers.** CSP with `frame-ancestors 'none'`, `nosniff`, `DENY` framing, `same-origin` referrers.

## Releasing

The platform is the sixth program of `apps/web/src/lib/products.json` (key `platform`, tag `platform-v<version>`, GitHub project `octopusOn/geo-seo-platform`); [release.md](release.md) has the whole procedure. `pnpm release platform` writes the notes and makes the tag; the tag must match `apps/geo-seo-platform/src-tauri/tauri.conf.json`. Each release carries:
- the desktop installers (`.dmg`, `.msi`, `.exe`, `.AppImage`, `.deb`, `.rpm`);
- `octopuson-platform` archives for Linux, macOS (Apple Silicon and Intel) and Windows;
- the web server for Linux: `octopuson-platform-server-platform-v<version>-x86_64-unknown-linux-gnu.tar.gz` with its `.sha256`, holding the binary and the built interface (`ui/`). It is built on Ubuntu 22.04, so it runs on that glibc and newer.

The desktop program serves MCP too: started with the single argument `mcp`, it answers on standard input and output without opening a window, and Settings → AI assistants shows its own path for the assistant's settings. Desktop users therefore need no separate command; the archives are for servers, scripts and other computers.

The website's program page comes from `apps/cms/database/seeders/programs/geo-seo-platform.json` with the screenshot `media/geo-seo-platform.jpg`. The CMS migration `add_program_pages` adds it to the live site as a draft in every language when the website is next deployed; publish it in the CMS admin once the release and app.octopuson.com are live. `node scripts/program-readmes.mjs` writes `distribution/geo-seo-platform/README.md` and its screenshot from the same page; the `Program projects` workflow copies that folder, skills included, to the GitHub project once it exists.

## Deploying

The web version runs on the same server as the website and the account site ([cms.md](cms.md#deploying), [cloud.md](cloud.md#deploying)), from release archives: the server builds nothing.

| Part | Where |
|---|---|
| Service | `octopuson-platform.service` runs `/home/octopuson/platform-releases/current/octopuson-platform-server` as `octopuson` on `127.0.0.1:8796` |
| Data | `/var/lib/octopuson/platform` (`server.duckdb`, `users/<account id>/`), made owner-only by systemd |
| Settings | `/etc/octopuson/platform.env`, root, mode 600 ([`platform.env.example`](../deploy/platform/platform.env.example)); the unit sets the addresses, port and data folder |
| nginx | `/etc/nginx/sites-enabled/octopuson-platform.conf`: `app.octopuson.com`, the event stream unbuffered, no query strings in the log. It uses the Cloudflare address mapping and log format of `octopuson-account.conf` |
| Logs | `journalctl -u octopuson-platform`; nginx writes `/home/octopuson/logs/nginx/app-*.log` |

**First installation** (as root, once):
1. Cloudflare: a proxied DNS record for `app.octopuson.com`; the origin certificate already covers the subdomains.
2. The account site must accept the return address: add `https://app.octopuson.com/auth/callback` to `CONNECT_WEB_REDIRECTS` in `/etc/octopuson/portal.env` and restart `octopuson-portal`.
3. `install -m 600 deploy/platform/platform.env.example /etc/octopuson/platform.env`, then set `PLATFORM_MASTER_KEY` (`openssl rand -base64 32`) and keep a copy of it off the server. Google, SMTP and Chrome are optional.
4. Report PDFs need Chrome or Chromium on the server (for example `apt install chromium`, then `OCTOPUSSEO_CHROME_PATH` if it is not found).
5. Deploy the account site first (`scripts/deploy-account.sh`; the website before it when the CMS changed): it brings the web sign-in, `/v1/me`'s `user_id`, `/v1/meter` and the gateway migration `0004_meter_events`. Then, in the portal admin, check the metered prices under Settings, and add the DataForSEO routes the platform uses (`/v3/backlinks/*/live`, the AI Optimization and AI Mode routes, with `"hold_usd": 2` on `llm_mentions`; see `packages/cloud-core/src/seeds.ts`) to the live connector. It stays a draft until DataForSEO confirms reselling in writing; until then the platform works with users' own keys.
6. Publish the release on GitHub (drafts cannot be downloaded), then deploy it as below.

**Updating:**

```bash
bash /home/octopuson/octopusOnPageSEO/scripts/deploy-platform.sh 0.1.0
```

The script pulls `main` (for the unit and nginx files), downloads the release archive, checks its SHA-256, unpacks it into `/home/octopuson/platform-releases/<version>-<time>`, points `current` at it, installs the unit and the nginx site when they changed (putting the old site back if `nginx -t` fails), restarts the service and asks `/health`. It keeps the three newest releases: to roll back, point `current` at an older one and `systemctl restart octopuson-platform`. Set `PLATFORM_RELEASE_REPO=octopusOn/geo-seo-platform` when releases go to the program's own project.

## Backups

`octopuson-platform-backup.timer` runs [`scripts/backup-platform.sh`](../scripts/backup-platform.sh) every night: it copies each account's folder while holding the account's lock (the lock every operation takes, so the database copy is consistent), then the crawls, and sends the copy to a restic repository, keeping 7 daily, 4 weekly and 6 monthly snapshots. To turn it on:
1. `apt install restic`, and an off-server repository (for example S3-compatible storage).
2. `/etc/octopuson/backup.env` (root, 600) with `RESTIC_REPOSITORY`, `RESTIC_PASSWORD` and the storage's credentials; `restic init` once.
3. Install `deploy/platform/octopuson-platform-backup.service` and `.timer` into `/etc/systemd/system`, then `systemctl enable --now octopuson-platform-backup.timer`.

The backup is useless without `PLATFORM_MASTER_KEY`, which seals every account's token and keys: keep it apart from the backup.

## Data folders

- **Desktop.** "OctopusOn GEO-SEO Platform" in the platform's application data folder, or `OCTOPUSON_PLATFORM_DATA_DIR`.
  - Keys go to the system credential store.
  - The account token is the one every OctopusOn SEO program shares.
- **Web.** `PLATFORM_DATA_DIR/users/<account id>/` holds `platform.duckdb`, `settings.json`, `secrets.sealed`, `account.sealed` (with `sync-token.sha256` when the token was borrowed from a syncing desktop) and the audits' `crawls/`.
- **Google.** `google.json` beside the database keeps the desktop's own client id and the scopes granted; the client secret and the refresh token are with the keys.
- **Desktop and web alike.** Each audit's crawl is a folder in `crawls/` beside `platform.duckdb`.
