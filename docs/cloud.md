# Account portal and credits gateway

People who don't want their own provider accounts can buy credits in TL at `my.octopuson.com`. The desktop programs then send their paid-source requests through `api.octopuson.com`, which uses our provider accounts and charges the credits. Using one's own keys stays free and local.

```
 browser ──▶ apps/portal   my.octopuson.com   (Next.js 16 + Better Auth: sign-up, credits,
                 │                              tokens, device sign-in, admin panel)
                 ▼
            PostgreSQL ◀── packages/cloud-core (schema, migrations, ledger, connectors, pricing)
                 ▲
                 │
 programs ──▶ apps/gateway  api.octopuson.com  (Hono: catalog, balance, device and one-click
                                                sign-in, /v1/p/<connector>/… forwarding,
                                                /v1/meter for server work)
```

Both apps share the database and `OCTOPUS_MASTER_KEY`, which seals our provider credentials and device tokens.

**Metered server work.** The web GEO-SEO Platform crawls sites and prints report PDFs on its own server, so it pays for that work by the unit through `POST /v1/meter { sku, units, idempotency_key }` with the account's token. Each account gets the month's free units of every unit first.

| Unit | Price setting (default) | Free each month (default) |
|---|---|---|
| `crawl.page` | `crawlPageMicros` (200 = $0.0002, 200 credits per 1,000 pages at the default credit unit) | `freeCrawlPages` (1,000) |
| `report.pdf` | `reportPdfMicros` (20,000 = $0.02, 20 credits) | `freeReportPdfs` (30) |

- **One transaction.** `meterUsage` locks the wallet, so two batches can never use the same free pages or money.
- **Records.** It writes a `meter_events` row (migration `0004_meter_events`) and a `usage_events` row with connector `server`, so the portal's usage page shows it. A charge also adds a `capture` ledger entry.
- **Retries.** A repeated idempotency key (namespaced by account) returns the first batch's figures and charges nothing.
- **Not enough credits.** The answer is 402 `insufficient_credits` and nothing is recorded.
- **Prices for programs.** `/v1/catalog` lists the prices under `meters`.
- **Admin.** Admin → Settings shows the four settings: the crawl price per 1,000 pages and the PDF price in credits, and the free pages and PDFs a month.

**AI visibility routes.** The seeded DataForSEO connector also allows the GEO-SEO Platform's AI visibility requests:
- `/v3/serp/google/ai_mode/live/advanced` (Google AI Mode);
- `/v3/ai_optimization/*/llm_responses/live` (ChatGPT, Claude, Gemini and Perplexity answers);
- `/v3/ai_optimization/llm_mentions/*/live` (LLM Mentions).

**Holds per route.** A route can reserve its own amount (`"hold_usd"` on the route) instead of the connector's `pricing.hold_usd`; the gateway reserves the matching route's hold before forwarding and charges the real price after. LLM Mentions holds $2, since a task with 1,000 rows costs about $1.10 upstream.

Queued `task_post` requests and the LLM Scraper stay closed. `seed` adds only missing connectors, so a DataForSEO connector already in the database needs these routes added when the platform is released. Assistants asked with the user's own OpenAI, Anthropic, Gemini or Perplexity key never pass through the gateway.

## Running locally

Requirements: Node 22.18 or newer (the gateway and the CLI run their TypeScript directly) and pnpm. CI uses Node 26.

1. Copy `apps/portal/.env.example` to `apps/portal/.env.local` and `apps/gateway/.env.example` to `apps/gateway/.env.local`. Use the same `DATABASE_URL` and `OCTOPUS_MASTER_KEY` (`openssl rand -base64 32`) in both.
2. Prepare the database: `cd packages/cloud-core && DATABASE_URL=… node src/cli.ts seed` applies the migrations and adds the connectors as drafts.
3. Start `pnpm --filter @octopusseo/portal dev` (port 3030) and `pnpm --filter @octopusseo/gateway dev` (port 8787).
4. Sign up, then make the account an admin with `node src/cli.ts promote you@example.com`. The admin panel asks for two-step sign-in (TOTP) first.

Without `SMTP_URL`, development prints the e-mails (verification, password reset) to the console. In production, sign-up fails without it, because the verification e-mail cannot be sent.

Tests: `pnpm --filter @octopusseo/cloud-core check`, `pnpm --filter @octopusseo/gateway check`, `pnpm --filter @octopusseo/portal check`. They run on PGlite; with `TEST_DATABASE_URL` set, the ledger's parallel-spending test also runs on a real PostgreSQL server.

## Languages

The portal speaks every language the CMS lists as active that also has a messages file. Nothing in the code names a language.

- **Texts.** `apps/portal/messages/<code>.json`, one file per language with the same keys (tr.json is the reference; a test checks the others). Each file also names its language (`language`) and translates the address segments (`routes`).
- **Languages and default.** From the CMS (`CMS_API_URL`, `/api/v1/languages`), cached for a minute. The CMS's default language is the portal's default; a visitor's last language is remembered in the `lang` cookie. Without the CMS, every messages file is offered.
- **Addresses.** Every page lives under its language with translated segments: `/tr/kredi/onay`, `/en/credits/confirm`, `/de/credits/bezahlen`. The folders under `app/[lang]` carry the English route names (`src/i18n/routes.ts`). `src/proxy.ts` redirects addresses without a language (the old `/giris`, the programs' `/cihaz`) and addresses in another language or in English to the right one, and checks the session cookie. `next.config.ts` rewrites each language's addresses to the folders; it reads the messages files when the portal is built.
- **Links in code.** `const { t, url } = await getDictionary()` in server code and `useUrl()` in client components: `url("credits/confirm", {}, { amount })`. Server code that cannot know the language (payment callbacks, `/baglan`) redirects to the English route name without a language (`/credits/result`), which the proxy sends on in the visitor's language.
- **Legal texts.** From the CMS (pages of type `legal`, see [cms.md](cms.md#data-model)), with the seller's details from the portal's `COMPANY_*` settings. `LEGAL_LANGUAGE` (Turkish unless set) is the binding language; the others carry a translation note.
- **E-mails.** Sent in the language of the page the request came from (the `lang` cookie).

To add a language: add it in the CMS admin, copy `messages/en.json` to `messages/<code>.json` and translate it (keep the keys and `{placeholders}`), translate the legal texts in the CMS, then deploy.

## Deploying

Both apps run on the same Contabo server as the website (207.180.209.30, managed by CloudPanel alongside other sites; see [cms.md](cms.md#deploying)). Cloudflare sits in front with SSL mode "Full", so the origin's self-signed `octopuson.com` certificate is enough for both subdomains.

| Part | Where |
|---|---|
| Code | `/home/octopuson/octopusOnPageSEO`, the clone the website uses (site user `octopuson`) |
| Portal | `octopuson-portal.service` runs the Next.js standalone server on `127.0.0.1:3030` from `/home/octopuson/portal-releases/current` |
| Gateway | `octopuson-gateway.service` runs `node src/main.ts` in `apps/gateway` on `127.0.0.1:8787` |
| nginx | `/etc/nginx/sites-enabled/octopuson-account.conf`, a hand-written file (not a CloudPanel site) with both server blocks |
| Settings | `/etc/octopuson/cloud.env` (shared) and `/etc/octopuson/portal.env`, owned by root, mode 600 |
| Database | PostgreSQL 16 on the server, database `octopus`, role `octopuson`, reached over the local socket without a password (peer authentication) |
| Logs | `journalctl -u octopuson-portal` / `-u octopuson-gateway`; nginx writes `/home/octopuson/logs/nginx/my-*.log` and `api-*.log` |

The units and the nginx file are kept in [`deploy/account`](../deploy/account). Node is the server's `/opt/node-v22.23.3-linux-x64`, the same as the website's. The portal reads its languages and legal texts from the CMS at `127.0.0.1:8095` (`CMS_API_URL` in the unit), so when both changed, deploy the CMS first (`scripts/deploy-website.sh`).

### Settings

`/etc/octopuson/cloud.env`, read by both services and by `scripts/account-cli.sh`:

```bash
DATABASE_URL=postgresql://octopuson@/octopus?host=/var/run/postgresql
OCTOPUS_MASTER_KEY=<openssl rand -base64 32>
```

Our provider credentials can live here too instead of the admin panel, one line each, `CONNECTOR_<ID>_<NAME>` (upper case, dashes as underscores); they win over credentials saved in the panel, which then shows "from the server settings":

```bash
CONNECTOR_DATAFORSEO_CREDENTIALS=login:API-password
CONNECTOR_SERPER_API_KEY=…
CONNECTOR_SERPAPI_API_KEY=…
CONNECTOR_YANDEX_SEARCH_API_KEY=…
CONNECTOR_YANDEX_SEARCH_FOLDER_ID=b1g…
CONNECTOR_WORDSTAT_API_KEY=…
CONNECTOR_WORDSTAT_FOLDER_ID=b1g…
```

`/etc/octopuson/portal.env` holds `BETTER_AUTH_SECRET` (`openssl rand -base64 32`) and the rest of `apps/portal/.env.example`: `SMTP_URL`, `MAIL_FROM`, the payment provider, Paraşüt and the `COMPANY_*` lines. Leave `LEGAL_APPROVED=0` until a lawyer has approved the texts; until then the legal pages carry a draft banner. The addresses (`BETTER_AUTH_URL`, `PORTAL_URL`), ports and `TRUST_PROXY=1` are set in the units.

Things to know about the master key:
- It cannot be replaced: without it, the stored provider credentials and every desktop sign-in are lost. Keep a copy outside the server.
- Both services must see the same key, which is why it lives in the shared file.

**Support.** `POST /v1/support { message, context: { program, version, page, error } }` (signed in, at most 10 an hour per account) keeps a message the programs send from an error's "report to support" button. When a provider refuses our own account (401, 402 or 403, such as a spent DataForSEO balance), the gateway notes it once per half hour per connector with the provider's reason. Both appear under Admin → Support (the audit log's `support.message` and `connector.upstream_problem` entries).

**Prices in dollars.** When buyers pay by hand into a dollar account (`TRANSFER_CURRENCY=USD`) or through Payoneer, credits are sold in US dollars: the packages, minimum and first-purchase cap are the `…UsdCents` settings (Admin → Settings; $10, $25, $50 and $100 by default, VAT included), and credits follow straight from the dollars with no exchange margin. A dollar order's `package_id` is `usd:<cents>`; its lira fields hold the equivalent at the bank's rate for invoices and the books. iyzico and PayTR sell in lira.

**Payments by bank transfer.** `PAYMENT_PROVIDER=transfer` with `TRANSFER_HOLDER`, `TRANSFER_IBAN`, optionally `TRANSFER_BANK` and `TRANSFER_CURRENCY` (`TRY`, or `USD` for a dollar account: the order's lira at its rate, rounded up to the cent) sells credits without a card processor. The buyer confirms an amount as usual and gets the bank details with the order's reference (`OCT…`) to write in the description; the order waits. When the money arrives, Admin → Orders → **Transfer received** marks it paid, adds its credits once and hands the invoice to Paraşüt (payment type EFT/HAVALE) or to the accountant; **Cancel** closes a transfer that will not come. `PAYONEER_LINK` (a Payoneer payment link, from Payoneer → Receive → Request a payment) adds Payoneer beside the bank, or alone: the buyer opens the link and pays the order's dollars by card, bank or PayPal with the reference in the description, and **Payoneer payment received** marks the order paid, recorded as a Payoneer sale. `EMAIL_VERIFICATION=off` lets accounts in without confirming their address, while no SMTP server is set up.

After changing a settings file, restart the service that reads it: `systemctl restart octopuson-portal` (and `octopuson-gateway` for `cloud.env`).

### Updating

After pushing to `main`, run on the server as root:

```bash
bash /home/octopuson/octopusOnPageSEO/scripts/deploy-account.sh
```

The script:
1. pulls `main` and installs the portal's and the gateway's dependencies;
2. applies the database migrations;
3. builds the portal and copies the build to a new directory in `/home/octopuson/portal-releases`, so the build never touches the running server;
4. refreshes the systemd units from `deploy/account` when they changed;
5. restarts both services, keeps the three newest releases and checks the sign-in page and `/health`.

To roll back the portal, point `current` at an older release and restart it:

```bash
ln -sfn 20261018-213000 /home/octopuson/portal-releases/current && systemctl restart octopuson-portal
```

Database chores run with the services' settings through `scripts/account-cli.sh`:

```bash
bash /home/octopuson/octopusOnPageSEO/scripts/account-cli.sh seed                      # add missing connectors as drafts
bash /home/octopuson/octopusOnPageSEO/scripts/account-cli.sh promote you@example.com   # make a signed-up account an admin
bash /home/octopuson/octopusOnPageSEO/scripts/account-cli.sh adjust you@example.com 5000 "reason"   # add credits by hand (negative takes), audited
```

### First installation

What was done once on the server, in this order:

1. **Database.** As root: `sudo -u postgres createuser octopuson` and `sudo -u postgres createdb -O octopuson octopus`. The role has no password; Ubuntu's `pg_hba.conf` lets the `octopuson` system user in over the socket.
2. **Settings.** `mkdir -m 700 /etc/octopuson`, then the two files above with `chmod 600`. The secrets were generated on the server with `openssl rand -base64 32`.
3. **Services.** `install -m 644 deploy/account/*.service /etc/systemd/system/`, `systemctl daemon-reload`, `systemctl enable octopuson-portal octopuson-gateway`.
4. **nginx.** Copy `deploy/account/octopuson-account.conf` to `/etc/nginx/sites-enabled/`, then `nginx -t && systemctl reload nginx`. Keep a copy of any changed nginx file in `/root/nginx-backup/` first.
5. **Deploy.** `scripts/deploy-account.sh`, then `account-cli.sh seed` once.
6. **DNS.** In Cloudflare, proxied `A` records for `my` and `api` pointing at 207.180.209.30.

### nginx notes

- The visitor's address comes from Cloudflare's `CF-Connecting-IP`, trusted only on connections from Cloudflare's published ranges. The server's `nginx.conf` takes `X-Real-IP` from any address, so the file matches on `$realip_remote_addr`. If Cloudflare publishes new ranges (<https://www.cloudflare.com/ips/>), update the `geo` block.
- The services receive that address as the only `X-Forwarded-For` entry. Better Auth's rate limits and the gateway's per-address limits depend on it.
- The access logs leave out query strings and referrers, because those can carry verification links, device codes and search terms. The gateway's own log keeps metadata only.

### Backups

Back up the `octopus` database (`sudo -u postgres pg_dump -Fc octopus > …`) and `/etc/octopuson`. The orders and invoices must be kept for the tax law's retention period.
