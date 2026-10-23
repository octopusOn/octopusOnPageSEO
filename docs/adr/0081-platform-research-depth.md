# ADR 0081 — GEO-SEO Platform 0.2.0: navigation, server-side filters, markets, history and deeper research

- Status: accepted
- Date: 2026-10-23

## Context
Version 0.1 showed every module's "now", but not at the depth of Semrush or Ahrefs. A review on 2026-10-23 found these gaps:
- **Navigation.** The address carried only the page and the project. A keyword or domain could not be opened in another module, and there was no global search.
- **Tables.** Sorting was by one column, pages had 50 rows, columns could not be chosen and numbers had no explanations.
- **Filters.** They ran in the browser, over the first 1,000 rows only.
- **Market.** Every report was tied to the project's country.
- **History.** Domain and link numbers kept no history; the cache held only the latest answer.
- **Missing reports.** Intent and country splits, subfolders, new and lost keywords, paid keywords and ads, keyword difficulty, parent topic and traffic potential, the strength of each result, SERP clustering, link history, a toxic-link audit, link prospects, and Semrush's strong/weak/missing keyword groups.
- **A price bug.** `historical_rank_overview` was estimated about ten times too low.

The owner chose to deepen every research area (domain, keywords, backlinks, gap and market) and to ship everything as one release, 0.2.0 (plan phases P1–P13).

## Decision
- **Entity addresses.** The route carries the entity and tab (`#/overview?project=1&domain=x.com&tab=keywords`).
  - Every keyword, domain and address has an "open" menu (`lib/Entity.tsx`): look at the domain, the keyword or the backlinks; track it; add it to a list.
  - The title bar has a global search (domain, address or words open the right module) and the sidebar a project switcher.
- **Tables** (`packages/ui` `DataTable`):
  - a column chooser kept per table, page sizes of 50, 100 and 500;
  - totals, a sticky head, row menus and multi-select.

  Every metric has a `?` explanation from a glossary in four languages (`lib/glossary.tsx`).
- **Filters run where the rows are** (`octopuson-dataforseo/src/query.rs`, UI `lib/query.ts`, `FilterBar.tsx`).
  - A `ListQuery` (filters, order, offset) is translated into DataForSEO's `filters`, `order_by` and `offset` for keywords, ranked keywords, backlinks and referring domains.
  - When every row is already loaded, filtering is local and free. Otherwise the server is asked, with the price first.
  - `Fetched.total` comes from `total_count`.
  - **Cache.** A filtered or paged answer has its own cache key and never travels to other copies (`cache.travels = false`, migration 12). An unfiltered question keeps today's key, so kept answers and the shared cache stay valid.
- **Markets.** Site Explorer, Keyword Explorer and Gap take another `country-language` (`MarketPicker`); the market lives in the route and carries between them. The project's market is the default.
- **Metric history.** Each domain overview or backlink summary fetched leaves the day's numbers (`metric_points`, migration 13, sync kind `metric_point`; command `metric_history`). Trends grow from the user's own fetches beside the providers' histories.
- **Prices.** Every list price is collected in `prices.rs` and tested; the history price is fixed ($0.12 + $0.0012 a month).
- **Site Explorer 2.0.**
  - The summary shows the link profile, 24 months or full history of every organic series, OA history, the keyword mix (intent, SERP features, brand split), traffic by country, and a competitor bubble map.
  - Keyword sets: organic now, lost (`historical_serp_mode: lost`) and paid.
  - Ads seen in paid keywords are kept (`ad_copies`, migration 14), so the domain's ad history grows.
  - New tabs: rank changes, subfolders, paid search.
- **Keyword research 2.0** (`modules/research.rs`).
  - Reports: search history since 2021, the top 10 month by month, each result's authority, linking sites and traffic, the parent topic and traffic potential, and searches in other markets.
  - A Keyword Magic view (broad, phrase, exact, questions; word groups).
  - SERP-overlap clustering of a list (`start_serp_clustering`, about $0.60 for 1,000 keywords).
- **Backlinks 2.0.**
  - OA and referring-domain history since 2019 (`backlinks/history`).
  - A toxic-link audit (`link_audit.rs`: spam score, /24 networks, risky TLDs, site-wide and link-farm links, spam anchors). Each linking site gets a decision: keep, watch or disavow (`link_decisions`). The disavow file is in Google's format.
  - Link prospects with an outreach status (`link_prospects`, migration 15).
  - Outbound links from the audit crawl.
- **Gap and market.**
  - Semrush's groups (shared, missing, weak, strong, untapped, unique) from each domain's ranked keywords, which reuse the overview's kept answers.
  - A page gap (`page_intersection`).
  - Traffic share history (`historical_bulk_traffic_estimation`), and categories with their leading sites.
- **Reports and assistants (P12).**
  - New report parts read only kept answers and the user's own records:
    - `organic`: numbers, monthly trend, best keywords, up, down, new, lost;
    - `link_history`: authority and referring domains over time, links won and lost by period, risky sites with their decisions, link work;
    - `gap`: group counts, and the missing and weak keywords worth most;
    - `keyword_lists`: the lists and their most searched keywords.
  - Snapshots keep the toxic-link audit (`link_audit`, `SnapshotData` version 2). A comparison names the risky sites found since and those cleared.
  - MCP adds `metric_history` (free) and `keyword_insights` (paid).

## Consequences
- **Research is deeper without changing the paid-data rules.** Every list can be filtered and paged over all of DataForSEO's rows, in any market. Every new paid report shows its price first, reuses kept and shared answers, and respects budgets.
- **Filtered answers are not shared between copies.** The shared cache sells only unfiltered questions, so a heavily filtered workflow pays more often.
- **History grows with use.** A domain fetched once a month has a point a month. Provider histories fill the gap, at their own price.
- **The gateway must allow the new routes before they work on credits** (P13): backlinks history and bulk referring domains, the Labs history and category endpoints, and the categories GET.
