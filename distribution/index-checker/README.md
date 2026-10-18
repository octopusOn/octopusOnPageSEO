# OctopusOn Index Checker

Search engines keep URLs your site stopped serving. Index Checker finds them on Google, Yandex, Bing, the Wayback Machine and Common Crawl, checks each one and writes the redirects.

**100% free and open source** (no paid version, no trial, no limits) · for SEO and GEO (visibility in AI answers) · Windows, macOS and Linux · updated continuously as search engines and AI assistants change.

![OctopusOn Index Checker](screenshot.jpg)

## Download

Get the installer for your system from the [latest release](../../releases/latest): `.dmg` for macOS (Apple Silicon and Intel), `.msi` or `.exe` for Windows, `.AppImage`, `.deb` or `.rpm` for Linux. More on [octopuson.com](https://octopuson.com/en/index-checker).

## What it does

### Discover what search engines remember

site: searches, archives, sitemaps and your own lists show the URLs crawlers still know about.

- Free result pages of Google, Yandex, Bing and DuckDuckGo
- Wayback Machine and Common Crawl history, XML sitemaps and your Search Console exports
- Optional search APIs: Google Custom Search, Yandex Search API, Serper and SerpApi

### Check every URL like a crawler

Each URL is fetched with its redirects followed and sorted by what needs fixing.

- 404 and 410, other 4xx and 5xx errors
- Soft 404s: 200 pages that say “not found” and deep URLs redirected to the home page
- Thin pages under your word-count threshold, with title, H1, canonical and noindex

### Fix it in minutes

Turn the findings into redirect rules and reports your developers can deploy.

- 301 and 410 rules for Nginx, Apache, Caddy, IIS, Cloudflare, Vercel and Next.js
- Directory-wide wildcard redirects, WordPress and Shopify CSVs, a clean sitemap and IndexNow lists
- CSV, Excel and JSON exports; every scan is saved to reopen and re-verify

## Command line

Every feature also works from the terminal with `octopuson-index`, included in the release archives:

```bash
octopuson-index scan example.com
octopuson-index redirects latest --format nginx -o redirects.conf
```

## Your own APIs or OctopusOn credits

Everything that runs on your computer is free. Search APIs are optional: connect your own keys (DataForSEO, Serper, SerpApi, Yandex), or use all of them with prepaid credits through one connection at [my.octopuson.com](https://my.octopuson.com).

## Source code and issues

This project publishes the releases of the Index Checker. Its source code lives in [octopusOn/octopusOnPageSEO](https://github.com/octopusOn/octopusOnPageSEO), together with the engine every OctopusOn SEO program shares. Bug reports and ideas are welcome in this project's issues.

## Licence

MIT or Apache-2.0, at your option.
