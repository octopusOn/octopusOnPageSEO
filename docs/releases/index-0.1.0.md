First release of the OctopusOn Index Checker: a desktop app and the `octopuson-index` command line that find the URLs search engines still index but your site no longer serves, and write the redirects that fix them.

## Highlights

- **Discover indexed URLs for free**: `site:` searches on Google, Bing, Yandex and DuckDuckGo in a browser you watch, with automatic subdirectory queries, plus XML sitemaps, robots.txt and URL lists from Search Console or Yandex Webmaster. Serper and SerpApi are optional.
- **Check every URL like a crawler**: 404 and 410, other 4xx and 5xx errors, redirect loops, soft 404s (from page text, redirects to the home page, canonicals or meta refresh) and thin pages under a word-count threshold.
- **Index health**: a score, per-directory clusters with suggested actions, noindex and orphan indexed pages, and broken sitemap URLs.
- **Fixes ready to deploy**: 301 and 410 rules for Nginx (with directory wildcards), Apache, Caddy, IIS, Cloudflare, Vercel and Next.js; WordPress and Shopify CSVs; removal lists, a clean sitemap and IndexNow lists.
- **Scan comparison**: resolved, new, persistent and dropped URLs between scans, and re-checks after a fix.
- **Exports** to Excel, CSV, JSON and Markdown, shared with the CLI.

## Installation notes

- **macOS**: the app is not notarized yet. If macOS says it cannot verify the developer, open **System Settings → Privacy & Security** and choose **Open Anyway**.
- **Windows**: SmartScreen may warn about an unknown publisher; choose **More info → Run anyway**.
