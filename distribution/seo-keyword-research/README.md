# OctopusOn SEO Keyword Research

Discover keywords, group them into clusters one page can target, and map the clusters to your site.

**100% free and open source** (no paid version, no trial, no limits) · for SEO and GEO (visibility in AI answers) · Windows, macOS and Linux · updated continuously as search engines and AI assistants change.

![OctopusOn SEO Keyword Research](screenshot.jpg)

## Download

Get the installer for your system from the [latest release](../../releases/latest): `.dmg` for macOS (Apple Silicon and Intel), `.msi` or `.exe` for Windows, `.AppImage`, `.deb` or `.rpm` for Linux. More on [octopuson.com](https://octopuson.com/en/seo-keyword-research).

## What it does

### Discover from every source

Seeds, competitors, files and your own Google data, with the source and seed of every keyword kept.

- Yandex Wordstat, DataForSEO (Google Ads, Labs, autocomplete) and SerpApi
- Search Console queries and Google Ads Keyword Planner with your own OAuth client
- Ahrefs, Semrush, Keyword Planner and Search Console imports; budgets and a cost estimate before every run

### Clusters you can explain

Group by wording for free, by shared top-ten results for certainty, or both.

- Turkish-aware wording clusters for 50,000 keywords in seconds
- Soft or hard result-page clustering with a shared-results threshold
- Manual moves and splits that later clusterings keep

### Clusters against your site

See which page serves each cluster, where your pages compete, and where competitors rank without you.

- Target pages from Search Console and result pages
- Cannibalisation and content gaps
- Content briefs: questions people ask, related searches and the top pages

## Command line

Every feature also works from the terminal with `octopuson-keywords`, included in the release archives:

```bash
octopuson-keywords import 1 ahrefs-keywords.csv
octopuson-keywords cluster 1 --method hybrid
octopuson-keywords export clusters 1 clusters.xlsx
```

## Your own APIs or OctopusOn credits

Everything that runs on your computer is free. Search APIs are optional: connect your own keys (DataForSEO, Serper, SerpApi, Yandex), or use all of them with prepaid credits through one connection at [my.octopuson.com](https://my.octopuson.com).

## Source code and issues

This project publishes the releases of the SEO Keyword Research. Its source code lives in [octopusOn/octopusOnPageSEO](https://github.com/octopusOn/octopusOnPageSEO), together with the engine every OctopusOn SEO program shares. Bug reports and ideas are welcome in this project's issues.

## Licence

MIT or Apache-2.0, at your option.
