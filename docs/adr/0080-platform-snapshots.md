# ADR 0080 — GEO-SEO Platform: snapshots and the before/after report

- Status: accepted
- Date: 2026-10-22

## Context
Every module of the platform shows "now" well, but not the way there:
- each module compares only with its own previous run;
- the domain overview and backlinks keep no history at all.

What an agency most needs to show a client is "where we were when the work began, and where we are a month later". The owner asked for a snapshot of the whole project and a report comparing two of them.

Decisions taken with the owner (2026-10-22):
- **The paid parts are taken fresh, with the price shown first.**
- **Snapshots are taken by hand and on a schedule** (weekly or monthly).
- **In scope:**
  - competitors, only those the user chooses;
  - a white-label before/after report (page, PDF, Excel);
  - a written summary in four languages;
  - a monthly e-mail on the web.

## Decision
- **A snapshot is one row with compact JSON** (`snapshots`, migration 9; `snapshot_settings` holds the schedule, the chosen competitors and when the last one started).
  - The JSON (`SnapshotData`, version 1) holds:
    - the organic numbers and position spread;
    - the first 1,000 ranked keywords;
    - the first 100 pages;
    - the backlink summary;
    - the first 1,000 referring domains;
    - each chosen competitor's overview and backlink summary;
    - rank tracking (visibility, positions and pages of every tracked keyword, competitor shares);
    - the latest audit (scores and findings by check);
    - AI visibility (rates, each question × assistant cell, brand shares);
    - Search Console and Analytics totals, with the first 100 queries and pages.
  - Parts that could not be taken are listed with the reason, and the snapshot is then `partial`. It is `failed` when nothing could be kept.
  - Snapshots are deleted with their project. Finished snapshots and the settings travel with sync (ADR 0078); running ones do not.
- **The paid parts ask the same questions as the modules' pages** (`fetch_overview`, `fetch_backlinks`), through the modules' own commands:
  - an answer the page keeps fresh is reused at no cost;
  - what a snapshot buys shows on those pages too;
  - the shared cache's discount (ADR 0079) applies by itself;
  - budgets, sources and payment modes apply as on the pages.
- **When a paid part cannot be bought** (no source, a budget or a quota), the remaining paid parts are skipped and the free parts are still kept. Nothing is paid that cannot complete.
- **The free parts** are read from the modules' reports (`rank_report`, `audit_report`, `ai_report`, `gsc_report`). They cost nothing.
- **The price is shown first.** `snapshot_estimate` sums the modules' estimates: list price, credits with and without the shared-cache discount, parts kept fresh, and why buying is blocked.
  - The first snapshot's dialog offers "take one every month", ticked by default. The person approves the monthly cost once, there; nothing is paid on a schedule they did not choose.
- **Schedules:** monthly every 29 days, weekly every 6.5 days, through `Platform::run_due` (the web runs them while a desktop syncs). The snapshot's row is begun before its job runs, so anything waiting for snapshots sees it at once.
- **Comparing is free and deterministic** (`snapshot/compare.rs`). It gives:
  - every number before and after, with its change, percentage and whether higher is better;
  - keywords won, lost, up and down (by traffic impact), and entering or leaving the top 10;
  - pages that gained or lost most;
  - referring domains won and lost (by authority);
  - tracked keyword moves;
  - audit checks fixed, new, better and worse;
  - AI cells that now name the brand, or no longer do, and brands' shares;
  - the project and its competitors side by side.

  By default the first finished snapshot (the baseline) is compared with the latest; `previous` compares the latest with the one before it.
- **The written summary uses rules, not a model** (`snapshot/summary.rs`):
  - sentences in Turkish, English, German and Spanish;
  - the good news first, at most eight sentences, at most three things that need attention;
  - changes under 1 % are not told;
  - the same two snapshots always read the same.
- **Reports:**
  - Two report parts: `comparison` (baseline → latest) and `comparison_previous` (previous → latest). Each has the summary as lines, the headline numbers as tiles ("1.525, +%23 (önce 1.240)"), and tables of every number and of each kind of move. They are in the page, the PDF and the workbook.
  - A new report schedule, `snapshot`, e-mails the report when a snapshot has finished since it was last sent or saved, and there is one before it to compare with.
  - While a snapshot of the project is being taken, the report looks again every 10 minutes.
  - Combined with monthly snapshots, this is the monthly before/after e-mail.
- **MCP:** `list_snapshots` and `compare_snapshots` are free; `take_snapshot` follows the paid-tool rule (`needs_approval` with the price, `allow_spend_usd` to buy).
- **The page** (Snapshots, in the tracking group) has:
  - the schedule and competitor choice, with the price of one snapshot;
  - a timeline with a chart of traffic, keywords, authority and site health;
  - the comparison: two choosers and tabs for the summary, keywords, pages, backlinks, rankings, audit, AI and competitors;
  - Excel export (`snapshot_xlsx`), and "send as a report" (a new report with the two comparison parts and the `snapshot` schedule).

  Project cards on the dashboard show the change since the baseline (`snapshot_progress`).

## Consequences
- **An agency can prove progress** from the first day: a baseline, monthly snapshots, and a branded before/after e-mail without further work.
- **Each snapshot costs about $0.25 of list price for the project's domain** (about 0.34 with the credit margin), plus about $0.05 per chosen competitor. Kept and shared answers lower it. Monthly snapshots repeat this every month, which the person agrees to when switching them on.
- **History starts with the first snapshot.** Data before it cannot be compared, and keyword and domain lists are compared within their first 1,000 rows.
- **Snapshot JSON is a few hundred kilobytes.** The list reads each finished snapshot's headline numbers from it, which is cheap for the dozens a project gathers in a few years.
- **`SnapshotData` is versioned.** New fields must default when read from older snapshots.
