# ADR 0077 — OctopusOn GEO-SEO Platform: one Rust core, one interface, desktop and web

- Status: accepted
- Date: 2026-10-21

## Context
The owner asked for an Ahrefs/Semrush alternative in the style of open-seo. Requirements:
- Both a web application and a desktop program.
- Users either enter their own API keys or spend OctopusOn SEO credits bought at my.octopuson.com.

Decisions taken with the owner:
- **Name.** A separate, sixth product, "OctopusOn GEO-SEO Platform" (`geo-seo-platform`); the owner wrote "Geo-Seo Platform".
- **Existing programs.** The five free programs stay as they are.
- **First release.** Every module:
  - domain overview and competitors
  - keyword explorer, backlinks, gap analysis
  - rank tracking, site audit, AI visibility (GEO), Search Console
  - white-label reports and MCP
- **Web address.** app.octopuson.com, signed in with the my.octopuson.com account.

The SEO engine already exists in Rust:
- provider clients, the credit transport and source modes;
- the crawler with 118 audits, rank tracking, clustering;
- about 990 tests.

## Decision
- **One Rust core, two shells.**
  - `octopuson-platform-app` offers every operation as a command of one router: `dispatch(name, JSON) -> JSON`, with typed error codes.
  - The desktop calls it through one Tauri command (`platform_call`).
  - The web server (`octopuson-platform-server`, axum) calls it through `POST /api/call/{command}`.
  - Job progress goes to an `EventSink`: Tauri events on the desktop, a per-account server-sent event stream on the web.
  - The React interface (`apps/geo-seo-platform`) is one build that detects where it runs: desktop, web, or sample data.
- **Workspaces.**
  - A workspace is one data folder with `platform.duckdb`, settings, own keys and the account token.
  - The desktop has one. The web has one per account under `users/<account id>/`, which isolates accounts and makes deletion simple.
  - The server's own `server.duckdb` keeps accounts, sessions (hashes only) and when each account's scheduled work is due next, so the minute tick reads one table instead of opening every workspace.
- **Paying for data.**
  - Every paid question goes through one helper: kept answer while fresh, source (own key or credits, as in the other programs), monthly budget, request, record.
  - Pages show the kept answer and the estimate first, and pay only when asked.
  - A cache shared across accounts is designed for, but stays off until DataForSEO confirms it in writing.
  - Questions that are always asked fresh (rank checks) are paid and recorded the same way but not kept as answers; the module keeps their readings.
- **Scheduled work.** Hosts call `Platform::run_due` every minute: the desktop program while it is open, the web server for the accounts that are due. Schedules start as manual, so nothing is paid until the user picks a schedule.
- **Site audits reuse the SEO Crawler.** The platform crawls with `octopuson-crawl` into `octopuson-crawl-store` and its 118 checks. GEO readiness is a read-only reader on the crawl, so the SEO Crawler's findings stay what they were.
- **Server crawls.**
  - The web server crawls addresses people type, so its fetcher resolves names itself and connects only to public addresses (no loopback, private, link-local or metadata address, on any redirect).
  - At most three crawls run at once.
  - Pages are metered on the account (`POST /v1/meter`): a month's free pages, then credits, charged in idempotent batches of 100. A crawl stops when the credits run out.
- **AI visibility asks each assistant the cheapest way the user has.**
  - With the user's own key for an assistant, `octopuson-llm` asks its API directly with web search (OpenAI Responses, Anthropic Messages, Gemini, Perplexity's Agent API); otherwise DataForSEO's LLM Responses asks it, on the user's DataForSEO key or credits. Google AI Mode comes only from DataForSEO.
  - Answers are read on our side for every brand: whole-word name matches, place among the brands named, citation of its pages, and a lexicon tone in four languages. The same reading applies to every route, so assistants and routes can be compared.
  - Runs are always fresh and paid per answer within the budget, each source on its own `PaidRun`, assistants in parallel. Every answer is kept with its text and sources so the report can be rebuilt and any answer reopened.
  - Server-side AI runs are not metered: they cost the server little, and the provider's price is already paid on the user's key or credits.
- **Search Console and Analytics are the user's own Google data, read free and replaced per sync.**
  - Read-only scopes only. The desktop signs in with the user's own "Desktop app" OAuth client (or one built into the program) through a loopback listener, as the other OctopusOn programs do; the web uses the server's own Web application client and returns to `/auth/google/callback`, where the signed-in account's workspace checks the state it started.
  - A sync reads the chosen period and the one before it and replaces the project's numbers: Google keeps 16 months of history itself, so the platform does not.
  - The report joins the numbers with the rank tracker's keywords and the latest audit's crawl instead of copying them into either module.
- **Reports are drawn, not stored.** A report keeps only its definition; its numbers come from the modules' own report commands when it is shown, so a report never pays a source and always matches the module pages.
  - One self-contained HTML page (inline styles and SVG, its own CSP with no requests or scripts) serves the preview, the share link, the e-mail and the PDF, which a private headless Chrome prints (`Page.printToPDF`). Web PDFs are metered (`report.pdf`).
  - Share links are bearer tokens: the server indexes only their hash; the workspace decides whether a token still opens a report.
  - E-mails are sent by the web server (SMTP over TLS with `lettre`) through a `Mailer` the host gives the workspace; the desktop sends none.
- **AI assistants get tools, not a second API.** The MCP server is the platform's own commands behind tools, in `octopuson-platform-app`, so the desktop's standard input and output and the web's `/mcp` answer alike.
  - It is a small JSON-RPC 2.0 layer written here rather than the `rmcp` crate: the server needs only `initialize`, `ping`, `tools/list` and `tools/call` over two transports, and a hand-written layer has no async runtime or macro dependency in the core and is fully covered by the core's tests.
  - Paid tools never buy on their own. Without fresh kept data they return the price, and buy only when called with `allow_spend_usd` at least that high; budgets still apply. Assistants are told to ask the user first.
  - The web takes personal access tokens (`opt_…`, at most 10 per account) instead of sessions. The server keeps their hashes only; a token stops when revoked or when the account's OctopusOn SEO token is. Requests from other sites' pages (`Origin`) are refused.
  - The web endpoint answers with JSON and keeps no MCP session or event stream: every tool answers within its call, and jobs report through the pages.
- **The desktop program is its own MCP command.** Started with `mcp`, it serves the tools on standard input and output without a window, so desktop users point their assistant at the program they installed; the `octopuson-platform` archives serve servers and scripts.
- **The web server ships in the release, the server only installs it.** The release workflow builds `octopuson-platform-server` with the built interface on Ubuntu 22.04; `scripts/deploy-platform.sh` checks its SHA-256 and swaps release folders. The shared server, which also hosts client sites, never compiles Rust, and a rollback is a symlink.
- **Shared rank measures.** Visibility, keyword trends and competitor shares live in `octopuson-serp-score::rank`, used by the SERP Checker and the platform alike; each keeps its own storage.
- **Web sign-in.**
  - The portal's one-click flow (`/baglan`, PKCE) also accepts exact https return addresses listed in `CONNECT_WEB_REDIRECTS`.
  - `/v1/me` names the account (`user_id`).
  - The token is sealed with AES-256-GCM under the server's master key, the value's name bound as associated data.
  - Scheduled work runs on the token while nobody is signed in. Signing out ends the browser session; revoking the token on the account site ends all of them.
- **Modules in their own files.** Each module has:
  - a Rust module, `modules/<module>.rs`;
  - an interface folder;
  - its translation files (tr, en, de, es).

  After the foundation the modules can be built in parallel.
- **Shared React components** live in `packages/ui`, on the Deep Sea design. The platform has its own ground: indigo with an aqua accent.

## Consequences
- **Same answers everywhere.** The desktop and the web cannot drift: they call the same commands, and tests check them in both shells.
- **No second backend.** The web server is a thin layer: sign-in, sessions, CSRF, one platform per account. It needs no TypeScript backend.
- **Scaling.** Per-account DuckDB files suit one server. Growing means moving account folders to more servers by account id.
- **Server-side risks.**
  - Web crawls are guarded against private addresses and metered (G7).
  - Still to do: other server resources (PDFs) and the admin fields for meter prices.
  - The seeded DataForSEO connector allows the AI Optimization and AI Mode routes (G8); the connector already in the live database still needs them added.
- **Prerequisites.**
  - DataForSEO's written permission (resale, shared cache).
  - Google's verification of a web OAuth client for Search Console.
  - An updated privacy notice for data kept on the server.
