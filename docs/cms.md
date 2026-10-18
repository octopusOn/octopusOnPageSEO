# Website CMS (`apps/cms`)

The content of the OctopusOn SEO website lives in a Laravel admin. `apps/web` (Next.js) renders every page from its JSON API. Design decisions are in [ADR 0073](adr/0073-website-cms.md). The editor's guide in Turkish is [cms-kilavuz.md](cms-kilavuz.md).

```
 editors ──▶ apps/cms  /admin       (Laravel 13, Blade + Tailwind 4 + Alpine)
                  │  /api/v1/*      read-only JSON (pages, menus, languages, sitemap)
                  │  POST /api/revalidate  ◀── after every change (shared secret)
                  ▼
 visitors ──▶ apps/web  /{lang}/{path}  (Next.js, cached with the `cms` tag)
```

## Running both apps locally

Requirements:
- PHP 8.3 or newer with the pdo_sqlite extension (Herd works) and Composer.
- Node 22 or newer and pnpm.

```bash
cd apps/cms
composer install
cp .env.example .env
php artisan key:generate
touch database/database.sqlite
php artisan migrate --seed
php artisan storage:link
pnpm install
pnpm --filter @octopusseo/cms build
pnpm --filter @octopusseo/cms serve
```

`migrate --seed` creates:
- the first admin account (`CMS_ADMIN_EMAIL`). With `CMS_ADMIN_PASSWORD` empty, the seeder generates a password and prints it once.
- the website content: 13 pages in English, Turkish, German and Spanish, from `database/seeders/content.json`.

To create another admin or reset a password, run `php artisan cms:admin someone@example.com --name="Ayşe"`.

The admin is at <http://localhost:8000/admin>. `pnpm --filter @octopusseo/cms dev` runs Vite with hot reload for admin styles and scripts.

Then point the website at the CMS. In `apps/web/.env.local`:

```bash
CMS_API_URL=http://localhost:8000
CMS_REVALIDATE_SECRET=<same as in apps/cms/.env>
CMS_PREVIEW_SECRET=<same as in apps/cms/.env>
```

Start the site with `pnpm web` (port 3000). If 3000 is taken, use `pnpm --filter @octopusseo/web dev --port 3010` and set `CMS_WEB_URL` and `CMS_REVALIDATE_URL` in `apps/cms/.env` to that port.

The two secrets can be any long random strings, for example from `openssl rand -hex 24`.

### Tests

```bash
cd apps/cms && php artisan test
pnpm --filter @octopusseo/web check
```

The CMS tests cover paths and redirects, the API (drafts, schedules, previews, listings, menus, sitemap), the admin flows, media, languages, the revalidation call and the seed.

## Data model

| Table | What it holds |
|---|---|
| `pages` | The tree, shared by every language. Fields: `type` (home, page, category, product, post, doc, legal), `key` (legal texts), `parent_id`, `position`, `product`, `icon`, `in_header`, `footer_column`. |
| `page_translations` | One row per page and language. Fields: `slug`, full `path`, `title`, `nav_title`, `excerpt`, `blocks` (JSON), SEO fields, `status`, `published_at`. |
| `languages` | `code` (URL prefix and hreflang), names, `locale_tag` (BCP 47), default and active flags. |
| `media` | Uploaded images and PDFs on the `public` disk (`CMS_MEDIA_DISK`). SVG is refused. |
| `redirects` | Old addresses. Automatic ones point at a translation and follow it. Manual ones name a target. |

Categories are pages:
- A page of type `category` lists its published children after its own blocks. Posts are listed newest first, everything else by position.
- Any page can show its children with a `children` block.
- `/{lang}/{category}/page/2` is the second page of a category's list.

Pages linked to a program (`product`) feed the product cards, the header's program menu and the download page. The program ids (`seo-crawler`, `log-analyser`, `serp-checker`, `seo-keyword-research`, `index-checker`) match `apps/web/src/lib/products.json`.

Blocks the site shows for the business side:
- `api_options`: free / your own API keys / one OctopusOn connection. An empty link leads to `CMS_ACCOUNT_URL` (my.octopuson.com).
- `credit_calculator`: one slider per paid request type.
  - The credits per unit are `number` fields edited in the admin. The seed uses list prices × 1.35 ÷ $0.001.
  - Visitors see credits and dollars.

A program page opens with a `hero` whose `screenshot` visual floats the program's window and types the hero's `code` lines in a terminal.

Comparison pages:
- The `comparison` block is a feature table: our column, the other tool's column, and one row per feature with a state for each side (yes, partly or paid, no, or none) and a short text.
- The seed's comparison pages sit under the **Compare** category (`/en/compare/…`): Screaming Frog SEO Spider vs SEO Crawler, and Screaming Frog Log File Analyser vs Log Analyser. The two program pages carry a short version of the table and a link to the full page.
- Write the other tool's facts with their source and date in the block's note, and update them when its prices change. The seed's figures were checked on screamingfrog.co.uk on 18 October 2026.

Legal texts of the account site (my.octopuson.com):
- They are pages of type `legal` at the top of the tree, with a `key` that is the same in every language: `credit-terms`, `pre-contract-information`, `distance-sales`, `privacy`, `cookies`. The account site links to them by key, so a slug can change freely.
- The website never shows them: resolve, the sitemap, menus, listings and redirects skip them. The admin's "view" link opens them on the account site.
- Write the text as one Markdown block. `{{company.title}}`, `{{company.address}}`, `{{company.tax_office}}`, `{{company.tax_number}}`, `{{company.mersis}}`, `{{company.email}}`, `{{company.phone}}`, `{{company.kep}}` and `{{company.site}}` are filled from the account site's settings; missing details show as bracketed placeholders in the reader's language.
- The menu title (`nav_title`) is the short name in the account site's footers.
- The Turkish text is binding; the account site marks the other languages as translations. A text missing in a language is shown in the default language.
- The first texts, in four languages, are `database/seeders/legal/<key>.<language>.md`. Their migration adds the missing ones to an existing site and never overwrites an edited text.

## API

All endpoints are public and read-only. They return only live translations (published, with a publication time that has come) of active languages. A request with `Authorization: Bearer <CMS_PREVIEW_SECRET>` also sees drafts.

| Endpoint | Returns |
|---|---|
| `GET /api/v1/languages` | Active languages, the default first. |
| `GET /api/v1/site?locale=tr` | Languages, the header and footer menus of the language, and the download page's address. |
| `GET /api/v1/pages/resolve?locale=tr&path=blog/yazi` | `{"data": page}`, `{"redirect": {"to", "status"}}` or 404. |
| `GET /api/v1/sitemap` | Every indexable live address with its language versions. |
| `GET /api/v1/legal?locale=tr` | The account site's legal texts: key, slug, title, short label, language. |
| `GET /api/v1/legal/{slug}?locale=tr` | One legal text by its slug, its key or another language's slug, in that language when it has one: its blocks and the slugs of its other languages. |

A resolved page carries:
- its blocks, with media ids replaced by files and product cards filled in;
- its SEO fields, hreflang alternates, the x-default address and breadcrumbs;
- the page of children for a category.

## How the website uses it

- `apps/web/src/app/[lang]/[[...slug]]/page.tsx` resolves every address. It renders the page's blocks with `src/components/blocks`, follows redirects (308) and returns 404 otherwise.
- `src/lib/cms.ts` is the API client. Responses are cached under the `cms` tag and revalidate hourly. In draft mode they skip the cache.
- `POST /api/revalidate` (bearer `CMS_REVALIDATE_SECRET`) expires the tag. The admin calls it after every change to a page, translation, language, media file or redirect, once per request and after the response. The dashboard warns when the last call failed.
- `GET /api/preview?path&expires&signature` checks the admin's HMAC link, turns on draft mode and opens the page. `POST /api/preview` (the banner's button) turns it off.
- `src/proxy.ts` sends addresses without a language to the visitor's language. It reads the language list from the CMS and caches it for a minute.
- Interface texts (buttons, table headings, platform notes) stay in `src/i18n/dictionaries`. Languages without a dictionary fall back to English.

## Adding a program

1. Add an entry to `apps/web/src/lib/products.json`. It gives the program's id and key, its
   name, its tag prefix, the CLI and its Cargo package, the app folder, its ground (`data-product`
   colour) and its folders for release notes.
   - A new ground also needs its colours in `apps/web/src/components/blocks/blocks.css`.
2. Add the id and brand name to `products` in `apps/cms/config/cms.php`.
3. In the admin, create a page of type **Product** with that program in every language. Flag it
   for the footer's product column.

The home page cards, the header's program menu, the download page and the release workflow pick
it up from there.

## Adding a block type

1. Define it in `apps/cms/app/Cms/Blocks.php`: label, icon, hint and fields. The admin's editor and validation pick it up.
2. Render it in `apps/web/src/components/blocks/Blocks.tsx`. Section blocks with their own full-width background also go in `SECTIONS`.
3. Add a test with a sample block to `apps/cms/tests`.

New icons go in `config/cms.php` (`icons`) and `apps/web/src/components/blocks/icons.tsx`. Then run `pnpm --filter @octopusseo/cms icons`.

## Deploying

The live site runs on the Contabo server (207.180.209.30), which CloudPanel manages alongside other sites. Cloudflare sits in front of it with SSL mode "Full", so the origin's self-signed CloudPanel certificate is enough.

| Part | Where |
|---|---|
| Code | `/home/octopuson/octopusOnPageSEO`, a clone of this repository owned by the CloudPanel site user `octopuson` |
| Website | `octopuson-web.service` (systemd) runs `next start` on `127.0.0.1:3000`; `apps/web/.env.production.local` holds its settings |
| octopuson.com | The CloudPanel vhost `/etc/nginx/sites-enabled/octopuson.com.conf` proxies `/` to port 3000 and serves `/storage/` (uploads) from the CMS. The original is in `/root/nginx-backup/` |
| CMS | `/etc/nginx/sites-enabled/cms.octopuson.com.conf`, a hand-written vhost and not a CloudPanel site. It uses the `octopuson` PHP 8.5 FPM pool (`127.0.0.1:20028`) and also listens on `127.0.0.1:8095`, where the website reads the API |
| Database | SQLite, `apps/cms/database/database.sqlite`, with WAL mode and a 5 s busy timeout |
| Uploads | `apps/cms/storage/app/public`, published as `https://octopuson.com/storage/…` (`CMS_MEDIA_URL`) |

Production settings in `apps/cms/.env`, besides the secrets:
- `APP_ENV=production`, `APP_DEBUG=false`, `APP_URL=https://cms.octopuson.com`
- `CMS_WEB_URL=https://octopuson.com`, `CMS_REVALIDATE_URL=http://127.0.0.1:3000/api/revalidate`
- `CMS_MEDIA_URL=https://octopuson.com/storage`
- `CACHE_STORE=file`, because the API's rate limiter writes to the cache on every request.

The website's `.env.production.local` has `CMS_API_URL=http://127.0.0.1:8095` and the same `CMS_REVALIDATE_SECRET` and `CMS_PREVIEW_SECRET`.

To update both apps after pushing to `main`, run on the server as root:

```bash
bash /home/octopuson/octopusOnPageSEO/scripts/deploy-website.sh
```

The script pulls, installs dependencies, migrates the CMS, rebuilds both apps and restarts the website.

Things to know:
- **Admin accounts.** Create one with `php8.5 artisan cms:admin you@example.com --name="…"`, run as the `octopuson` user in `apps/cms`. It asks for the password.
- **CloudPanel and the vhost.** Saving octopuson.com's vhost in CloudPanel can bring back its PHP template. The 443 server block needs these two locations instead of the template's `location /` and static-file location, then `nginx -t && systemctl reload nginx`:

  ```nginx
  location ^~ /storage/ {
    alias /home/octopuson/octopusOnPageSEO/apps/cms/storage/app/public/;
    expires 30d;
    access_log off;
  }

  location / {
    proxy_pass http://127.0.0.1:3000;
    proxy_set_header X-Forwarded-Proto https;
    proxy_set_header Host $host;
    proxy_set_header X-Forwarded-Host $host;
    proxy_set_header X-Real-IP $remote_addr;
    proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
  }
  ```
- **Backups.** Back up `apps/cms/database/database.sqlite` (with its `-wal` file) and `apps/cms/storage/app/public`.
