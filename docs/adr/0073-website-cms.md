# ADR 0073 — Website content in a Laravel CMS, one pages table for every kind of page

- Status: accepted
- Date: 2026-10-18

## Context
The octopusSEO website (`apps/web`, Next.js) kept every text in four TypeScript dictionaries (en, tr, de, es) and had one hand-written route per page. A new page, a blog post or a fixed typo needed a developer and a deploy, and every language had to reuse the English slugs (`/tr/features`).

The owner asked for:
- a content admin in Laravel with Tailwind;
- dynamic, multilingual pages, each language a version of the same page;
- categories (and similar groupings: product groups, doc sections, blog categories) as pages, with no separate categories table.

The credits portal (`apps/portal`) already has an admin; it stays separate and is not touched. Hosting is the owner's Contabo server (Docker Compose, Caddy, Postgres).

## Decision
- **`apps/cms`**: a Laravel 13 app (PHP 8.4). Blade, Tailwind 4 and Alpine render the admin, styled with the shared Chromatophore components (`@octopusseo/design`, the "Deep Sea" look of the desktop apps). It has no public pages, only `/admin` and a read-only JSON API under `/api/v1`. It uses SQLite locally and Postgres in production.
- **Data model:**
  - `pages` holds the language-independent tree. Its `type` is one of home, page, category, product, post or doc. It also has `parent_id`, position, the program it presents (`product`), an icon and menu flags (`in_header`, `footer_column`). A category is a page whose children the site lists, so there is no categories table.
  - `page_translations` holds one row per page and language: slug, full `path`, title, menu title, excerpt, blocks (JSON), SEO fields, status and publication time. `unique(page_id, locale)` and `unique(locale, path)` apply.
  - `languages` is managed in the admin, and its code is the URL prefix.
  - `media` holds uploaded files. SVG is refused because it can carry scripts.
  - `redirects` holds old addresses.
- **Paths:**
  - A translation's path is its parent's path in the same language plus its slug, so slugs are translated: `/en/features`, `/tr/ozellikler`.
  - A child can only be translated after its parent.
  - When a slug or parent changes, every translation below it moves too. Each old address gets a redirect that points at the translation, not at a path, so a page that moves again never leaves a chain.
  - The seed adds redirects from the previous English-only addresses (`/tr/features`).
- **Content blocks:**
  - Pages are ordered lists of typed blocks (`app/Cms/Blocks.php`): hero, page header, feature grid, sections, FAQ, CTA, markdown text, image, code, downloads and changelog, among others.
  - The same definitions drive three things: the admin's block editor, the server-side cleaning (unknown types and fields are dropped, and every error names its block and field), and the API's expansion of media ids and product cards.
  - Blocks that need releases (download buttons, download tables, changelog) take their texts from the CMS and their data from GitHub releases, as before.
- **Delivery:**
  - `apps/web` has one optional catch-all route, `[lang]/[[...slug]]`. It asks the API to resolve `/{lang}/{path}` and gets back a page, a redirect or a 404. `<category>/page/N` pages through a category's children.
  - Fetches are cached with the `cms` tag and revalidate hourly.
  - After every change, model events make the admin post to the site's `/api/revalidate` with a shared secret. The site calls `revalidateTag("cms", { expire: 0 })`, so the next visit renders the new content.
  - Pages known at build time are prerendered. New ones render on their first visit.
- **SEO:**
  - hreflang alternates come from a page's published language versions, and x-default is the default language's version.
  - The sitemap is built from the API, with alternates on every entry.
  - Each page can set a canonical URL and noindex.
  - The site adds JSON-LD for breadcrumbs, the program a page presents (SoftwareApplication), FAQs and posts (BlogPosting).
- **Previews:** the admin links to `/api/preview` with the path, an expiry time and an HMAC-SHA256 made with `CMS_PREVIEW_SECRET`. The site checks the signature, turns on Next.js draft mode and then reads drafts with the same secret as a bearer token. Previews are noindex and show an exit banner.
- **Interface texts:** buttons, labels and table headings stay in the site's dictionaries, which shrank to about 100 lines per language. A language added in the CMS shows them in English until it gets a dictionary.
- **Menus:** header and footer links are pages flagged in the CMS. GitHub, contact and issue links still come from the site's settings.

## Consequences
- Editors add pages, posts, categories and languages without a deploy, and every language gets its own slugs.
- The site depends on the CMS. If the CMS is unreachable, cached pages keep being served and new or expired pages fail until it is back. Builds work without the CMS: languages fall back to the built-in four, and pages render on demand.
- Next.js answers permanent redirects with 308, which search engines treat like 301.
- Two runtimes (PHP and Node) run on the server, and both need the shared secrets.
- The block types are a contract between `apps/cms/app/Cms/Blocks.php` and `apps/web/src/components/blocks`. A new block type needs both sides.
