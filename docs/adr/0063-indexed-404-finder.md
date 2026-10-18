# ADR 0063 — octopusSEO 404 Finder, the indexed 404 & thin-content auditor

- Status: accepted
- Date: 2026-10-18

## Context
A traditional crawler (`octopusSEO Spider`) discovers URLs linked internally or listed in current sitemaps. However, search engines (Google, Yandex, Bing, DuckDuckGo) and historical archives (Wayback Machine) often retain URLs in their indexes that are no longer linked on the site and now return `404`/`410`, `4xx`/`5xx`, Soft 404 (200 OK with "not found" text or deep URLs redirected to `/`), or thin `200 OK` pages with very little visible text.

## Decision
Add a third standalone product to the workspace, **octopusSEO 404 Finder**, sharing the Chromatophore design language (`data-product="finder"`), HTTP adapter (`seonexus-net`), HTML parser (`seonexus-html`), and report exporters (`seonexus-report`):

- `crates/seonexus-finder`:
  - **Discovery (`serp.rs`)**: Queries free search engine result pages (`site:domain.com` on Google, Yandex, Bing with `ck/a` base64 decoding, and DuckDuckGo HTML with `uddg` decoding), the free Internet Archive Wayback Machine CDX API, user seed lists, and optional search APIs (Google Custom Search JSON API, Yandex XML/Search API, Serper.dev), plus automatic top-level directory query expansion (`site:domain.com/<segment>`).
  - **Verification (`verify.rs`)**: Multi-threaded HTTP verifier following redirects up to `max_redirects`, extracting `<title>`, `<h1>`, `word_count`, `canonical`, and `noindex` via `seonexus-html`, and classifying each indexed URL into `error_404`, `error_4xx`, `error_5xx`, `soft_404`, `thin_200` (`word_count < min_word_count`), `network_error`, or `healthy_200`.
  - **Storage (`store.rs`)**: Saves each scan in its own directory (`scan.json` + `scan.duckdb`), supports paginated/filtered queries, CSV/XLSX/JSON exports, and Nginx / Apache `.htaccess` 301 redirect rule generation.
- `apps/snx-finder`: Command-line interface (`scan`, `list`, `show`, `export`, `redirects`).
- `apps/finder`: Tauri 2 + React 19 desktop app with bilingual TR/EN support and browser demo preview.
