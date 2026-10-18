# OctopusOn SEO Crawler

A free SEO spider and website audit tool that crawls like a search engine: JavaScript rendering, broken links, redirect chains, duplicate content, hreflang and an explainable SEO score, with no URL limit.

**100% free and open source** (no paid version, no trial, no limits) · for SEO and GEO (visibility in AI answers) · Windows, macOS and Linux · updated continuously as search engines and AI assistants change.

![OctopusOn SEO Crawler](screenshot.jpg)

## Download

Get the installer for your system from the [latest release](../../releases/latest): `.dmg` for macOS (Apple Silicon and Intel), `.msi` or `.exe` for Windows, `.AppImage`, `.deb` or `.rpm` for Linux. More on [octopuson.com](https://octopuson.com/en/seo-crawler).

**macOS:** the app is not notarized by Apple yet, so on first launch macOS says Apple could not verify it is free of malware. Drag the app to Applications, run this command once in Terminal, then open it again:

```bash
xattr -dr com.apple.quarantine "/Applications/OctopusOn SEO Crawler.app"
```

Or close the message, open **System Settings → Privacy & Security** and click **Open Anyway**. On Windows, if SmartScreen warns about an unknown publisher, choose **More info → Run anyway**.

## What it does

### A free SEO spider that crawls like Googlebot

Crawl as Googlebot Smartphone, Googlebot Desktop, Bingbot, Chrome or your own user agent, at a polite speed, with robots.txt read the way Google reads it.

- Spider mode or list mode, with URL, depth and time limits and include and exclude patterns
- 1–64 parallel requests, speed profiles and a requests-per-second cap
- robots.txt: respect it, ignore it but report, or ignore it; X-Robots-Tag per crawler
- XML sitemaps, sitemap indexes and gzip files, with their redirect chains
- Live progress, pause and resume, several crawls at once and saved crawls to reopen

### JavaScript rendering for JavaScript SEO

Single-page apps and client-side frameworks are crawled the way search engines render them, in a fresh, isolated browser context for every URL.

- Uses the Chrome, Chromium or Edge already on your computer
- Desktop, mobile or custom viewport
- Raw vs rendered: title, description, canonical, robots directives, word count and links
- Flags a noindex removed by JavaScript and re-crawls failed renders

### Site audit with an explainable SEO score

A 0–100 SEO score built from 100+ checks. Every deduction leads to the URLs behind it, with an explanation and a fix.

- Response codes, redirects and security: HTTPS, HSTS and mixed content
- Titles, meta descriptions, headings, canonicals, directives and pagination
- Images, structured data, Open Graph and Twitter cards, mobile viewport
- Prioritised fixes, grouped into nine systems

### Ready for AI search and GEO

Check that AI crawlers can reach your content, next to the classic SEO checks.

- AI readiness: AI bot access in robots.txt and llms.txt
- Structured data that rich results and AI answers rely on
- Pair it with Log Analyser to see which AI bots really visit

### Crawl comparison, exports and Search Console data

Follow a site from one crawl to the next, hand results to clients and find quick wins in Google data.

- Compare two crawls on 30 SEO fields and see new, fixed and remaining findings
- Filtered exports to Excel, CSV and JSON from every tab
- Choose the columns of every tab
- SEO Opportunities: low-CTR pages, near wins and traffic losses from Search Console and GA4 data

## Command line

Every feature also works from the terminal with `octopuson-crawler`, included in the release archives:

```bash
octopuson-crawler crawl https://example.com --render javascript
octopuson-crawler export latest --tab internal --out internal.xlsx
```

## Your own APIs or OctopusOn credits

Everything that runs on your computer is free. Search APIs are optional: connect your own keys (DataForSEO, Serper, SerpApi, Yandex), or use all of them with prepaid credits through one connection at [my.octopuson.com](https://my.octopuson.com).

## Source code and issues

This project publishes the releases of the SEO Crawler. Its source code lives in [octopusOn/octopusOnPageSEO](https://github.com/octopusOn/octopusOnPageSEO), together with the engine every OctopusOn SEO program shares. Bug reports and ideas are welcome in this project's issues.

## Licence

MIT or Apache-2.0, at your option.
