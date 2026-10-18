# ADR 0075 — Search-friendly program names, one GitHub project per program, APIs and credits on the site

- Status: accepted
- Date: 2026-10-18

## Context
After ADR 0074 the owner asked for:
- **Searchable names** that say what each program is:
  - SEO Spider → **SEO Crawler**
  - SERP Finder → **SERP Checker**
  - Keyword Miner → **SEO Keyword Research**
  - Index Check → **Index Checker**
- **The SEO Crawler first** in every list.
- **Commands named `octopuson-*`.**
- **A GitHub project of its own for every program.**
- **A website that says:**
  - the programs are open source;
  - they serve SEO and GEO (visibility in AI answers) and are updated continuously;
  - they can be used free, with your own third-party APIs, or with one OctopusOn connection for every API (my.octopuson.com);
  - with a credit calculator.
- **Animated program heroes** like the Log Analyser's.

## Decision
- **Names.**
  - Full names are "OctopusOn" plus the program name: OctopusOn SEO Crawler, OctopusOn Log Analyser, OctopusOn SERP Checker, OctopusOn SEO Keyword Research, OctopusOn Index Checker.
  - The suite brand stays "OctopusOn SEO".
  - Product ids, URL slugs and GitHub projects use the same words: `seo-crawler`, `log-analyser`, `serp-checker`, `seo-keyword-research`, `index-checker`. Old slugs redirect.
- **Commands and tags.**
  - The commands are `octopuson-crawler`, `octopuson-log`, `octopuson-serp`, `octopuson-keywords` and `octopuson-index` (`[[bin]] name`; packages and folders stay `snx-*`).
  - The Crawler's key is `crawler` (tags `crawler-v…`, commit scope `crawler`). `spider-v` stays a legacy prefix.
- **Identifiers are kept.** These still carry the old names so existing data and keys keep working:
  - data folders and keychain service names, e.g. `octopusSEO SERP Finder`;
  - bundle identifiers;
  - user-agent tokens.
- **One GitHub project per program: `octopusOn/seo-crawler`, `octopusOn/log-analyser`, `octopusOn/serp-checker`, `octopusOn/seo-keyword-research` and `octopusOn/index-checker`** (products.json `repo`).
  - **Development:** stays in `octopusOn/octopusOnPageSEO`, where the shared engine lives.
  - **Releases:** the release workflow publishes the program's draft release, installers and CLI archives in its project, as tag `v<version>`, when the `PROGRAMS_TOKEN` secret can write there. Without it, releases stay in the source repository.
  - **README:** the `Program projects` workflow keeps each project's README and screenshot in step with `distribution/<id>/`. `scripts/program-readmes.mjs` writes those from the CMS seed.
  - **Website:** reads each program's releases from its project and still finds older releases in the source repository.
- **Website content (CMS seed, four languages).**
  - **Home page:** says open source, SEO and GEO, and continuously updated.
  - **New blocks:**
    - `api_options`: free / your own API keys / one OctopusOn connection, the last leading to `CMS_ACCOUNT_URL` (my.octopuson.com);
    - `credit_calculator`: per-request credits edited in the admin, a new `number` field type. The seed values are the providers' list prices × 1.35 (the default service fee) ÷ $0.001 per credit, marked as estimates.
  - **Program pages:** each opens with a hero whose `screenshot` visual floats the program's window and types its commands in a terminal.
  - **API options:** the SERP Checker, SEO Keyword Research and Index Checker pages show the API options.
  - **Order:** the SEO Crawler comes first in every list.

## Consequences
- The release workflow needs a `PROGRAMS_TOKEN` secret (a fine-grained token with Contents read/write on the five projects). The projects must exist, with at least one commit, before the first release.
- Users find downloads, stars and issues on the program's own project. Code changes still go to one repository.
- Credit prices on the site are estimates the admin keeps current. The program shows the real price before every run.
- Renaming the data folders to the new names would need a migration. Until then they keep the old names on disk.
