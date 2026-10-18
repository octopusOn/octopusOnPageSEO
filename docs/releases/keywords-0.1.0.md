First release of the OctopusOn SEO Keyword Research: a desktop app and the `octopuson-keywords` command line to discover keywords, group them into clusters one page can target, and map the clusters to your site.

## Highlights

- **Discovery from every source**:
  - Yandex Wordstat (top requests and associations, within its hourly quota).
  - DataForSEO: Google Ads ideas and volumes; Labs ideas, related searches, suggestions and competitor keywords; autocomplete.
  - SerpApi autocomplete.
  - Search Console queries and Google Ads Keyword Planner with your own Google OAuth client.
  - Every keyword keeps the source and seed that found it.
- **Imports** from Ahrefs, Semrush, Keyword Planner, Search Console and Wordstat exports (CSV, TSV, TXT, Excel or a Search Console ZIP), with their volumes, ranges, cost, competition, difficulty and monthly searches.
- **Turkish-aware keywords**: variants that differ in case, diacritics or spacing are one keyword. Wording clusters remove Turkish suffixes but keep intent words apart.
- **Three clusterings**:
  - by wording, free and instant for 50,000 keywords;
  - by shared top-ten results, soft or hard, by page or by site;
  - hybrid, with unverified members marked.
  - Clusters show searches, intent (with a mixed-intent warning), difficulty and cost.
- **Manual moves and splits** that later clusterings keep.
- **Site mapping**: the target page of each cluster, cannibalisation, gaps and the competitors that rank, from Search Console and stored result pages.
- **Content briefs** from the pillar's result page: questions people ask, related searches and the top pages.
- **Exports** of keywords and clusters to Excel, CSV and JSON.
- Monthly budgets and a cost estimate before every paid run.
- Keys and the Google sign-in stay in the system keychain, bound to the provider's address.

## The free suggestion endpoints

Google's and Yandex's suggestion endpoints are not official APIs, and Yandex's terms forbid automated queries without permission. They are off until you switch them on with the terms accepted.

When on:
- The SEO Keyword Research asks them with its own user agent, one request every two seconds at least, and at most 2,000 per engine and day.
- The first refusal stops that engine until the next day.

## Installation notes

- **macOS**: the app is not notarized yet. If macOS says it cannot verify the developer, open **System Settings → Privacy & Security** and choose **Open Anyway**.
- **Windows**: SmartScreen may warn about an unknown publisher; choose **More info → Run anyway**.
- Google Ads keyword planning needs at least Basic access for your Google Cloud project.
