# 76. OctopusOn names for crates, packages and folders

Date: 2026-10-18

## Status

Accepted. Supersedes the "packages and folders stay `snx-*`" note of ADR 0074 and ADR 0075.

## Context

ADR 0075 renamed the programs and their installed commands (`octopuson-crawler`, `octopuson-log`, …) but kept the code's internal names. The crates were still `seonexus-*`, the command-line packages `snx-*`, and the desktop apps lived in folders named after their first working titles (`apps/desktop`, `apps/spider`, `apps/finder`). Contributors browsing the open-source repository saw three different names for one product.

## Decision

| Before | After |
|---|---|
| `crates/seonexus-<name>` (`seonexus_<name>`) | `crates/octopuson-<name>` (`octopuson_<name>`) |
| `apps/snx-spider`, `snx-log`, `snx-serp`, `snx-kw`, `snx-finder` | `apps/octopuson-crawler-cli`, `octopuson-log-cli`, `octopuson-serp-cli`, `octopuson-keywords-cli`, `octopuson-index-cli` |
| `apps/desktop`, `spider`, `serp`, `keywords`, `finder` | `apps/log-analyser`, `seo-crawler`, `serp-checker`, `seo-keyword-research`, `index-checker` (the product ids) |
| Tauri crates `seonexus-desktop`, `seonexus-spider-desktop`, … | `octopuson-log-desktop`, `octopuson-crawler-desktop`, `octopuson-serp-desktop`, `octopuson-keywords-desktop`, `octopuson-index-desktop` |
| npm `@seonexus/desktop`, `@octopusseo/spider`, … | `@octopusseo/log-analyser`, `@octopusseo/seo-crawler`, … (the product ids) |

- The command-line packages carry a `-cli` suffix: `octopuson-serp` and `octopuson-keywords` are already library crates. Their `[[bin]]` names, the commands users install, do not change.
- `products.json` `package` and `app` follow the new names. Release tags and the per-program projects (ADR 0074, 0075) are unchanged.

These runtime identifiers stay, so existing installations and files keep working:
- the `.snxsite` extension and its `"format": "seonexus-site"` tag;
- the legacy `SEONexus` data folder lookup;
- the `octopusSEO` data folders and keychain services;
- the bundle ids `com.octopusseo.*` and `mainBinaryName`;
- the `snx-*` archive names that the website still recognises on releases made before ADR 0075.

Earlier ADRs keep the names they were written with.

## Consequences

- One name per product across folders, crates, packages and the website.
- `cargo run -p snx-spider` becomes `cargo run -p octopuson-crawler-cli`; scripts outside the repository that used the old package names must be updated.
- Build caches keyed by the old crate names are rebuilt once.
