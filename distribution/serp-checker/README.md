# OctopusOn SERP Checker

Rankings, and why pages rank: the SERP Checker crawls the top results, scores content and sites, and explains keyword difficulty.

**100% free and open source** (no paid version, no trial, no limits) · for SEO and GEO (visibility in AI answers) · Windows, macOS and Linux · updated continuously as search engines and AI assistants change.

![OctopusOn SERP Checker](screenshot.jpg)

## Download

Get the installer for your system from the [latest release](../../releases/latest): `.dmg` for macOS (Apple Silicon and Intel), `.msi` or `.exe` for Windows, `.AppImage`, `.deb` or `.rpm` for Linux. More on [octopuson.com](https://octopuson.com/en/serp-checker).

## What it does

### Your sources, your choice

Search Google, Bing and Yandex through the free browser mode or the API you already pay for — or add your own.

- Yandex Search API, DataForSEO, Serper and SerpApi built in
- Your own provider with a request template and JSONPath, CSS or regex mapping
- Fallback order per engine, monthly budgets and a cost estimate before every run

### Scores for pages and sites

The pages that rank are crawled with the SEO Crawler engine and scored on what can be measured.

- Content: keyword placement, topic coverage, depth, structure, questions, schema, freshness
- Site: domain age (RDAP, WHOIS, Wayback), Open PageRank, referring domains
- Technical health from the SEO Crawler's audit

### Keyword difficulty you can check

Difficulty is the click-weighted strength of the ten results, with every component and its contribution shown.

- Search intent, SERP features and your opportunity
- What it takes to rank: length, subheadings, schema, title, questions to answer
- Content gaps against the pages that rank

### Rank tracking

Projects check their keywords daily or weekly while the app is open, or from cron with the command line.

- Positions, best positions and history per keyword
- Visibility, share of voice and cannibalisation
- Stored result pages, read again when selectors change

## Command line

Every feature also works from the terminal with `octopuson-serp`, included in the release archives:

```bash
octopuson-serp analyze "türk kahvesi nasıl yapılır" --engine google --country tr --site example.com
octopuson-serp track --due
```

## Your own APIs or OctopusOn credits

Everything that runs on your computer is free. Search APIs are optional: connect your own keys (DataForSEO, Serper, SerpApi, Yandex), or use all of them with prepaid credits through one connection at [my.octopuson.com](https://my.octopuson.com).

## Source code and issues

This project publishes the releases of the SERP Checker. Its source code lives in [octopusOn/octopusOnPageSEO](https://github.com/octopusOn/octopusOnPageSEO), together with the engine every OctopusOn SEO program shares. Bug reports and ideas are welcome in this project's issues.

## Licence

MIT or Apache-2.0, at your option.
