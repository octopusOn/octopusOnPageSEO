# ADR 0065 — One repository, several products: releases and website

- Status: accepted
- Date: 2026-10-18

## Context
The repository builds three programs: the Log Analyser (`apps/desktop`, `snx-log`), the Spider (`apps/spider`, `snx-spider`) and the SERP Finder (`apps/serp`, `snx-serp`). The release workflow, the website's downloads and its changelog knew only the Log Analyser: tags `v*`, `apps/desktop` as the Tauri project, `snx-log` archives, and a footer that called the Spider "coming soon". Each program has its own version and release rhythm, and the published `v0.1.0` release and its download links must keep working.

## Decision
- **Products are listed once**, in `apps/web/src/lib/products.json`: id, name, short name, tag prefix, CLI, desktop app folder and page path. The website (`site.ts`), the local download collector (`scripts/collect-downloads.mjs`) and the release workflow read this file, so adding a program means adding an entry.
- **Tags name the product** by prefix: `v0.2.0` is the Log Analyser (unchanged), `spider-v0.1.0` the Spider, `serp-v0.1.0` the SERP Finder. The longest prefix followed by a digit wins, so `serp-v…` never matches the Log Analyser's `v`.
- **Release workflow**: a `resolve` job reads the tag, looks the product up with `jq`, checks that the app's `tauri.conf.json` has the tag's version and passes on the app folder, CLI and title. The draft, desktop and CLI jobs use these outputs. Release notes are `docs/releases/<version>.md` for the Log Analyser and `docs/releases/<prefix><version>.md` (`spider-0.1.0.md`, `serp-0.1.0.md`) for the others. CLI archives are named `<cli>-<tag>-<target>`.
- **Website**: every release carries its product and tag. Downloads and the latest release are chosen per product; the download page has a section per product, the Spider and the SERP Finder have their own pages (`/spider`, `/serp-finder`), and the home page and footer link all three. Locally collected files live in `public/downloads/<tag>/`, which keeps the Log Analyser's existing `/downloads/v<version>/` URLs; manifest entries without a product are the Log Analyser's.
- The GitHub releases list is read 100 at a time, so each product finds its newest release in one request.

## Consequences
- Products release independently from one repository and one CI; the website needs no change for a new version.
- A tag without a known prefix fails the workflow instead of producing a misnamed release.
- The Spider and the SERP Finder pages say "not released yet" until their first tags are published; publishing a release makes them offer downloads within an hour.
- The Index 404 Finder and later programs join by adding an entry to `products.json`, dictionary texts and, if wanted, a page.
