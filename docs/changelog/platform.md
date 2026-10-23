# OctopusOn GEO-SEO Platform changelog

All notable changes, newest first. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the program uses [Semantic Versioning](https://semver.org/).

## [0.2.0] - 2026-10-23

### Added

- **Everything opens everything:** every keyword, domain and address has a menu to look at it in another module, track it, add it to a list or make a content brief; a global search in the title bar; a project switcher in the sidebar.
- **Tables:** choose columns, 50, 100 or 500 rows a page, totals, and a short explanation behind every metric's `?`.
- **Filters over every row:** filters and sorting run on DataForSEO's side over all its rows, with the price shown first; when every row is already loaded they stay free in the program.
- **Other markets:** Site Explorer, Keyword Explorer and Gap can look at another country and language.
- **Metric history:** each domain or backlink summary you fetch leaves the day's numbers, so trends grow from your own data.
- **Site Explorer:**
  - the full organic history since 2020, the OA history, the keyword mix (intent, SERP features, brand), traffic by country and a competitor map;
  - rank changes (new, up, down, lost), subfolders, and paid search with the ads seen.
- **Keyword research:**
  - Keyword Magic (broad, phrase, exact, questions, word groups);
  - search history since 2021, the parent topic and traffic potential, and searches in other countries;
  - each result's strength and a year of top-10 history;
  - grouping a list by shared result pages.
- **Backlinks:**
  - OA and referring-domain history since 2019;
  - a toxic-link audit with keep, watch or disavow decisions and a Google-ready disavow file;
  - link prospects with their outreach status;
  - outbound links from your audit.
- **Gap and market:** Semrush-style groups (shared, missing, weak, strong, untapped, unique), a page gap, traffic share over time, categories and their leading sites.
- **Local SEO:** your business on Google Maps — a grid of up to 9×9 points showing where you rank, average rank and share of local voice, the businesses that beat you, your profile and rivals, reviews with the rating over time and reply drafts.
- **Content:**
  - briefs from the top 10 results (terms, outline, questions, length, internal links);
  - a live content score for drafts and pages, and a page checker;
  - optional AI writing on your own Claude, ChatGPT or Gemini key.
- **Site audit:** internal links (orphans, depth, internal PageRank), hreflang, a page explorer, the trend across audits and every finding as CSV.
- **Core Web Vitals:** real-user LCP, INP and CLS on phones and desktops, with history and PageSpeed scores, on your Google API key.
- **Search Console:** 14 and 240-day periods, year-on-year comparison, a brand/non-brand split, 5,000-row lists and full exports.
- **Rank tracking:** every place in one table, SERP features you hold, landing pages, competitors' positions per keyword, comparison with any day, and estimated visits.
- **Opportunities:** what to do next, ranked by estimated extra clicks and effort, with done, snooze and ignore.
- **Project dashboard:** every module's numbers and trends, the first opportunities, the latest alerts, what runs next and this month's spending.
- **Alerts:**
  - drops in rankings, visibility, clicks, site health or map share; new high-priority problems; AI mention changes; bad reviews; fast-growing competitors; and an optional weekly link watch;
  - shown in the bell and the alert centre, as desktop notifications, and on the web by e-mail at once or as a daily digest.
- **Reports:** new parts for organic search, backlinks over time, the keyword gap, keyword lists and what to do next; the AI part shows each run.
- **Snapshots:** also keep the map grid and the toxic-link audit, and compare them.
- **AI visibility:** brands' share of voice and citations run by run, and the pages AI answers cite most.
- **AI assistants (MCP):** the project dashboard, opportunities, alerts, keyword lists, briefs, link work, Core Web Vitals, metric history and keyword insights. Assistants can also track keywords, check rankings, audit the site, ask the AI questions and make briefs, always with the price first.

### Changed

- Scheduled rank checks go through DataForSEO's standard queue: about 70% cheaper, with results a few minutes later. "Check now" stays instant.
- A new report leaves out the parts that show data bought on other pages; add them in the editor.

### Fixed

- The price of the domain's history was shown about ten times too low.
- A question in several parts showed the price of every part even when some were kept fresh; it now shows what would be paid.
- The program now shows its own release version.

## [0.1.1] - 2026-10-22

### Added

- Your projects sync with app.octopuson.com through your account: projects, settings, keywords, AI questions, keyword lists, reports and branding, finished rank checks, AI answers, audits and Search Console numbers. Your own API keys stay on each computer.
- While syncing, scheduled rank checks, AI visibility runs, audits and Search Console syncs run on the web server; switch sync off in Settings to run them on this computer.
- The first sync of a computer with projects, with an account that has projects on the web too, asks whether to merge, keep this computer's data or take the web's.
- The details of each project's latest audit come along to the other side.
- Settings shows the sync, the changes waiting and "Sync now".
- On credits, answers kept by OctopusOn cost 70% less: the estimate shows the discounted price, and "Use discounted answers from the cache" in Sources turns it off.
- Refreshing an answer that is still fresh on your computer always asks the provider anew.

## [0.1.0] - 2026-10-22

### Added

- Domain overview: organic keywords, traffic and traffic value, 24 months of history, top pages, competitors and subdomains.
- Keyword Explorer: volumes, difficulty, CPC and intent, ideas, related keywords and suggestions, the live result page, Yandex Wordstat, and keyword lists grouped into topics.
- Backlinks: the profile with OA authority, live, new, lost and broken links, referring domains, anchors and linked pages, domains won and lost.
- Gap analysis against up to five competitors: missing and shared keywords, link gaps and many sites side by side.
- Rank tracking on Google, Bing and Yandex in up to 10 places, with visibility, competitors' top-10 and top-3 counts, AI Overview citations and cannibalisation.
- Site audit with 118 checks and GEO readiness: AI crawler access, llms.txt, structured data and citable content.
- AI visibility in ChatGPT, Gemini, Google AI Mode, Claude and Perplexity: mention and citation rates, the brand's place in each answer's list, rivals named instead of you, share of voice, cited sources and LLM Mentions.
- Search Console and Google Analytics 4, joined with rankings and the audit.
- White-label reports as PDF, Excel and HTML; on the web, read-only links and scheduled e-mails.
- AI assistants through MCP (the desktop program with `mcp`, or app.octopuson.com/mcp with an access token), five agent skills, and the `octopuson-platform` command line.
- Errors support can act on offer "Report to support".

