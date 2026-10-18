# ADR 0074 — Program versions, release notes and the OctopusOn SEO names

- Status: accepted
- Date: 2026-10-18

## Context
The suite grew from one program to five, and more are coming:
- Log Analyser
- SEO Crawler
- SERP Checker
- SEO Keyword Research
- Index Checker

ADR 0065 already released each program from its own tag. Several things were still missing or
outdated:
- **Release notes:** written by hand, with no rule for what goes in.
- **Tag prefixes:** inconsistent (`v`, `spider-v`, `serp-v`, `kw-v`).
- **Index Checker:** had no tag at all.
- **CLI names:** the command-line programs still carried the old SEONexus prefix (`snx-log`,
  `snx-kw`).

The owner asked for:
- a sound versioning and release-notes system, with notes in English;
- the brand **OctopusOn SEO**;
- `octopus-*` commands;
- the repository moving to `github.com/octopusOn/octopusOnPageSEO`;
- the website at `octopuson.com`.

## Decision
- **One Semantic Version per program.**
  - The tag is `<key>-v<version>`: `log-v`, `spider-v`, `serp-v`, `keywords-v`, `index-v`.
  - The key also names the CLI (`octopus-<key>`), the commit scope and the notes file.
  - The Log Analyser's `v0.1.0` and the unreleased `kw-v` stay readable as legacy prefixes.
- **Conventional Commits scoped by program key.**
  - `scripts/release.mjs` collects a program's commits since its last tag: those with its scope,
    or touching its folders (`paths` in `apps/web/src/lib/products.json`).
  - It derives the next version: breaking → major (minor before 1.0), feat → minor, else patch.
  - It writes `docs/releases/<key>-<version>.md` (English; sections Breaking changes, Added,
    Fixed, Performance, Changed, Security; Install).
  - It prepends `docs/changelog/<key>.md` in Keep a Changelog form.
  - It sets the version in the app's `package.json` and `tauri.conf.json`, and in the CLI's
    `Cargo.toml` and `Cargo.lock`.
  - With `--tag` it commits those files and creates the annotated tag. Pushing stays a manual
    step.
- **CLIs keep their Cargo packages and folders (`apps/snx-*`) but install as `octopuson-log`,
  `octopuson-crawler`, `octopuson-serp`, `octopuson-keywords` and `octopuson-index` (`[[bin]] name`).**
  - Renaming packages would rewrite Cargo.lock and every `-p` in scripts for no user benefit.
  - The workflow builds `-p <package>` and archives `<cli>-<tag>-<target>`.
- **Brand: "OctopusOn SEO" in every visible text.** This covers app and window names, sidebars,
  CLI help, reports, the website and docs. Identifiers stay as they are, so existing data and
  keys keep working:
  - application data folders (`ORGANIZATION`/`APPLICATION`, `ProjectDirs`);
  - keychain service names;
  - bundle identifiers (`com.octopusseo.*`);
  - the `.snxsite` extension;
  - crawler user-agent product tokens (`octopusSEO-Spider/0.1`, whose URL now points to
    octopuson.com).
- **Each program has its own ground color and icon.**
  - The ground matches its window (`data-product`): violet for the Log Analyser, teal for the
    Spider, blue for SERP, emerald for SEO Keyword Research, crimson for Index Checker.
  - The app icons were recolored from the shared mark.
  - The website gives each program page its ground and shows every program on the home page and
    in the header menu.
- **Downloads link to GitHub.** Serving files from the website is opt-in
  (`OCTOPUSSEO_LOCAL_DOWNLOADS=1`).

## Consequences
- **Writing notes:** a release starts from generated notes; editors only write the highlights
  and polish the wording. Commit messages become user-facing text, so they need care.
- **Shared crates:** changes appear in the notes of every program that touches them only when the
  commit also touches the program's folders. A shared fix worth announcing needs the program's
  scope, e.g. `fix(spider,serp): …`.
- **Old commands:** users of the 0.1 CLIs type new command names. Old archives (`snx-*`) are
  still recognised by the website.
- **Data folders:** they keep the octopusSEO names on disk. Renaming them would need a migration.
