# ADR 0083 — GEO-SEO Platform: opportunities, the project dashboard and alerts

- Status: accepted
- Date: 2026-10-23

## Context
The modules answered "how is the site doing" one page at a time. Agencies asked for three things:
- **What to do next**, as one list ordered by value.
- **One page per project** that shows everything at a glance.
- **Being told** when something important changes, without opening the program.

The owner chose rule-based opportunities (no model), a project dashboard, and alerts in the app, by e-mail and as desktop notifications.

## Decision
- **Inputs added for the rules (P9).**
  - **Core Web Vitals** (`octopuson-google/src/vitals.rs`):
    - CrUX field data with 25 weeks of history, and PageSpeed lab scores for up to 10 pages;
    - on the user's Google API key (source `google-api`), sent in the `X-goog-api-key` header;
    - one row per target, device and day (`vital_checks`, migration 19).
  - **The audit.** An internal link report (orphans from the sitemap, link depth, internal PageRank), hreflang, a page explorer, a trend across audits, and every finding as CSV.
  - **Search Console.**
    - A brand split (the project's brands and its domain name, folded);
    - year-on-year comparison;
    - 5,000 rows per list and full exports.
- **Opportunities** (`modules/opportunities.rs`) are computed on read from the modules' own reports.
  - **The rules:**
    - keywords at positions 4–20 with volume;
    - keywords that dropped or were lost;
    - low CTR for the position (not brand queries);
    - pages losing 20 % of their clicks;
    - cannibalised queries;
    - high-priority audit checks;
    - orphan, weakly linked and deep pages;
    - Core Web Vitals failing on phones, slow pages;
    - AI answers naming a rival and not the site;
    - content scores under 50;
    - weak map grids;
    - competitors growing fastest between snapshots;
    - lost links worth winning back.
  - **The measures.**
    - Gain is clicks a month from CTR curves.
    - Impact is a log scale (5,000 clicks = 100).
    - Priority is impact × effort (low 1, medium 0.75, high 0.5).
  - **Decisions.** Each opportunity can be marked done (hidden 30 days, then shown again if still true), ignored, or snoozed until a day. Decisions sync (`opportunity_states`, migration 20); the opportunities themselves are never stored.
- **The project dashboard** (`project_dashboard`, page `project`):
  - every module's headline numbers with sparklines and change;
  - the first ten opportunities and the latest alerts;
  - what the schedule runs next;
  - this month's spending by module.

  The dashboard's project cards open it.
- **Alerts** (`modules/alerts.rs`, store `alerts.rs`, migration 21).
  - **Rules per project, with thresholds:**
    - a keyword falling N places or leaving the top 10;
    - visibility falling X %;
    - Search Console clicks falling X %;
    - site health falling N points, or new high-priority problems;
    - the AI mention rate changing N points;
    - the map SoLV falling N points;
    - a new review of 1–2 stars;
    - a competitor growing much faster between snapshots;
    - a weekly link watch (new and lost linking sites; paid, about $0.05 a week; off by default).
  - **When they are looked for.**
    - `JobManager::on_finish` hands each finished job to the alerts, which read only the rule groups that job feeds;
    - after a sync or queue collection;
    - a six-hour safety sweep.
  - **One alert, once.**
    - Only copies that run the schedule look for alerts; a desktop that syncs leaves it to the web.
    - An alert's key comes from its source (a check's time, a period's end, an audit's start, a review's id), and its sync uid from the project's uid and that key. Two copies finding the same alert keep one, and reading it travels.
    - Mailing and notifying are recorded per copy.
  - **Delivery.**
    - **In the app:** a bell with the unread count and an alert centre.
    - **By e-mail (web):** at once or as a daily digest after 06:00 UTC, to up to five recipients, in the chosen language.
    - **On the desktop:** a system notification through the operating system's own notifier (osascript, notify-send, a Windows toast). Up to two for a project are shown one by one; more are grouped.
  - **Fairness to other work.** Alerts run in the background. A foreground call takes the store first (`in_background` and a waiting counter), so pages never wait for alerts.
- **Reports and assistants (P12).**
  - An `opportunities` report part lists the open opportunities with their gain and effort.
  - MCP adds `project_dashboard`, `opportunities` and `alerts` (free).
  - MCP actions: `track_keywords` (free), and `check_rankings`, `start_site_audit` (web), `run_ai_visibility` and `create_content_brief`. Each paid action answers `needs_approval` with its price until `allow_spend_usd` covers it.

## Consequences
- **What to do next is explainable.** Each opportunity names its rule, its numbers and the page that acts on it. The same data always gives the same list, and nothing is paid to compute it.
- **Alerts are only as fresh as the data under them.** Without scheduled checks there is nothing new to alert on. The link watch is the only alert that buys data.
- **E-mail needs the web.** A desktop that does not sync shows alerts in the app and as notifications only.
- **Windows notifications use PowerShell's toast API with the program's app id.** They depend on the installer registering that id.
