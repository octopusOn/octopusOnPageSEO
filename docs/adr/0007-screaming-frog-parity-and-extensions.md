# ADR 0007 — Screaming Frog Log File Analyser parity and extensions

- Status: accepted
- Date: 2026-10-13

## Context
Agencies compare the Log Analyser with Screaming Frog's Log File Analyser (SF). SF offers tabs
per URL / response code / user agent / referer / directory / IP / country / bytes, more input
formats (W3C/IIS, ELB, HAProxy, JSON, CSV), import-time filters, custom user agents with
verification, project export, and "Imported URL Data" joined to log URLs. We wanted parity
without adding a second query engine, plus analyses SF lacks.

## Decision
- **Reports are queries.** SF's tabs became metrics and fields of the query language
  (`last_status`, `days_since_last`, `errors`, `hits_1xx`…`hits_5xx`, `inconsistent`,
  `avg_bytes`, `co2_mg`, per-bot hit columns, `content_type`, `referer_host`, `referer_type`,
  `country`, `asn`, `as_org`) and templates, so they combine with any filter, run on rollups
  where possible and export like every query. Expressions stay compile-time constants
  (`concat!` of macro literals); per-site values the SQL needs (the site's own host) live in
  `site_meta`.
- **Formats with headers** (W3C `#Fields:`, CSV header rows) are built from the sample during
  detection; header and directive lines are a separate skip reason (`comment or header line`),
  not errors. JSON and CSV logs map field names through one synonym table (`structured.rs`)
  with priorities, so new vendors usually need a name, not a parser.
- **Site settings** (catalog `sites.settings`, JSON) filter at import time (bots only, URL
  regexes, parameter stripping); stored data is never rewritten. Deleting an import removes its
  rollups and history in one transaction and its Parquet files afterwards; leftover files of
  deleted imports are removed on every write open, so a crash cannot resurrect them.
- **Custom bots** live in `custom-bots.json` beside the data, are validated in the domain crate
  and matched before the catalog. Their definitions are interned (process-lifetime strings) so
  the classifier keeps `&'static BotDef`; the classifier version includes a stable hash of the
  custom set, which reuses the existing lazy re-classification of stored user agents.
- **Geolocation** uses DB-IP Lite (CC BY 4.0, attribution shown with results), downloaded only on
  request. Answers are stored per site in `ip_geo` (one row per client IP) with the data
  release; new IPs are resolved after imports, a new release re-resolves lazily on the next read.
  Lookups never leave the machine.
- **robots.txt compliance** evaluates the current robots.txt against `(bot, url, verdict)`
  groups from the daily rollup, Google-style (most specific group, longest match, `Allow` wins
  ties); operator-specific tokens (Googlebot family, AdsBot, Applebot fallback, Yandex) are
  domain knowledge in `seonexus-botverify::robots`.
- **Alerts** are a pure analysis (median of the previous 14 days and MAD with minimum volumes)
  over daily series fetched through queries.
- **Site archives** (`.snxsite`) are ZIP files with a manifest, the checkpointed database and the
  Parquet files; restore paths are whitelisted so entries cannot escape the site directory.
- **Imported URL data** stays transient (ADR 0006), but CSV columns and sitemap `lastmod` are
  joined to the comparison rows; the desktop remembers sources per site.

## Consequences
- The query language grew; its tables in `docs/query-language.md` are the reference and every
  template is executed against real data in tests.
- `country`/`asn`/`as_org`, `referer_*` and `ip` need raw hits; they are slower on large sites
  than rollup-backed fields, as documented.
- robots.txt history is not kept: older requests are judged by the current file.
- Interned custom bot definitions are never freed; each distinct set costs a few kilobytes.
