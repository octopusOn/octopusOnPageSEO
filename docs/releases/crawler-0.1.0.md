First release of the OctopusOn SEO Crawler: a desktop app and the `octopuson-crawler` command line that crawl websites the way search engines do and audit them.

## Highlights

- **Spider and URL-list crawls** with parallel requests, rate limits, user-agent presets, robots.txt, sitemaps, scope patterns and query-parameter removal. No built-in URL limit.
- **JavaScript rendering** through an installed Chrome, Chromium or Edge, with raw versus rendered comparisons and explicit per-page rendering errors.
- **More than 100 technical checks**: response codes, redirect chains and loops, titles and descriptions, headings, canonical and hreflang relationships, images, links, orphan pages, structured data, security, response times, and AI crawler access including `llms.txt`.
- **An explainable SEO score**, broken down system by system, with every deduction leading back to its findings and URLs.
- **Live crawl activity** with pause, resume and stop; saved crawls on disk to reopen, repeat and compare.
- **Comparisons** of crawls and of findings, duplicate and near-duplicate content groups, and an XML sitemap builder.
- **Exports** to Excel, CSV and JSON, shared with the CLI.

## Installation notes

- **macOS**: the app is not notarized yet. If macOS says it cannot verify the developer, open **System Settings → Privacy & Security** and choose **Open Anyway**.
- **Windows**: SmartScreen may warn about an unknown publisher; choose **More info → Run anyway**.
- JavaScript rendering needs an installed Chrome, Chromium or Edge.
