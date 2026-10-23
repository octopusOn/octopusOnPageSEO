# OctopusOn GEO-SEO Platform 0.2.0

The platform grows to the depth of Semrush and Ahrefs. Every list can be filtered over all of DataForSEO's rows, in any market, and every keyword, domain and address opens in any module. Site Explorer, keyword research, backlinks and gap analysis go much deeper. New modules cover local SEO on Google Maps and content briefs. Above them sit a project dashboard, opportunities ranked by what they bring, and alerts in the program, by e-mail and on your desktop. Scheduled rank checks now cost about 70% less.

## Added

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

## Changed

- Scheduled rank checks go through DataForSEO's standard queue: about 70% cheaper, with results a few minutes later. "Check now" stays instant.
- A new report leaves out the parts that show data bought on other pages; add them in the editor.

## Fixed

- The price of the domain's history was shown about ten times too low.
- A question in several parts showed the price of every part even when some were kept fresh; it now shows what would be paid.
- The program now shows its own release version.

## Known limits

- **Queued checks on credits.** On OctopusOn credits, the cheaper queued checks wait for an update of the OctopusOn service; until then, scheduled checks on credits run live. With your own DataForSEO key they use the queue now.
- **Reviews on credits.** Google reviews are read only through the queue, so on credits they need your own DataForSEO key for now.
- **AI writing** (briefs, page improvements, review replies) needs your own Claude, ChatGPT or Gemini key.
- **Core Web Vitals** need a Google API key (free) under Sources.

## Install

Download the installer for your system from the assets below: `.dmg` (macOS, universal), `.msi` or `.exe` (Windows), `.AppImage`, `.deb` or `.rpm` (Linux).
The command-line tool `octopuson-platform` is in the `octopuson-platform-platform-v0.2.0-<target>` archives.

**macOS:** until the app is notarized, macOS says Apple could not verify it is free of malware. Drag it to Applications and run `xattr -dr com.apple.quarantine "/Applications/OctopusOn GEO-SEO Platform.app"` once in Terminal, or choose **Open Anyway** in System Settings → Privacy & Security.
