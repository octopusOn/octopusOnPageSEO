# Releasing

Every OctopusOn SEO program is versioned and released on its own from this repository
([ADR 0074](adr/0074-program-versioning.md)). A release is a GitHub release with English notes,
the program's installers and its command-line archives. The website lists it on the program's
page and links every download to GitHub.

| Program | Key | Tag | GitHub project | Desktop app | CLI (Cargo package) |
|---|---|---|---|---|---|
| SEO Crawler | `crawler` | `crawler-v0.2.0` | `octopusOn/seo-crawler` | `apps/spider` | `octopuson-crawler` (`snx-spider`) |
| Log Analyser | `log` | `log-v0.2.0` (earlier: `v0.1.0`) | `octopusOn/log-analyser` | `apps/desktop` | `octopuson-log` (`snx-log`) |
| SERP Checker | `serp` | `serp-v0.2.0` | `octopusOn/serp-checker` | `apps/serp` | `octopuson-serp` (`snx-serp`) |
| SEO Keyword Research | `keywords` | `keywords-v0.2.0` | `octopusOn/seo-keyword-research` | `apps/keywords` | `octopuson-keywords` (`snx-kw`) |
| Index Checker | `index` | `index-v0.2.0` | `octopusOn/index-checker` | `apps/finder` | `octopuson-index` (`snx-finder`) |

The table comes from `apps/web/src/lib/products.json`, which the release workflow, the website
and `scripts/release.mjs` all read. A new program is one more entry there and in
`apps/cms/config/cms.php`.

## Program projects

Each program has a GitHub project of its own for its releases, README, stars and issues
([ADR 0075](adr/0075-program-names-and-projects.md)). Development stays in this repository.

Set them up once:

1. Create the five public repositories under the `octopusOn` organisation, each with an initial
   README:

   ```bash
   for repo in seo-crawler log-analyser serp-checker seo-keyword-research index-checker; do
     gh repo create "octopusOn/$repo" --public --add-readme --homepage "https://octopuson.com/en/$repo"
   done
   ```

2. Create a fine-grained personal access token with **Contents: read and write** on those five
   repositories.
3. Add it to this repository as the `PROGRAMS_TOKEN` secret.

With the secret in place:
- releases go to the program's project as tag `v<version>`;
- the `Program projects` workflow copies `distribution/<id>/README.md` and `screenshot.jpg` there
  whenever they change. Regenerate them from the CMS seed with `node scripts/program-readmes.mjs`.

Without the secret, releases stay in this repository under the monorepo tag.

## Versions

- [Semantic Versioning](https://semver.org/) per program: `MAJOR.MINOR.PATCH`.
  - A breaking change bumps MAJOR. Before 1.0 it bumps MINOR.
  - A new feature bumps MINOR.
  - A fix bumps PATCH.
  - Pre-releases use a suffix, `0.3.0-beta.1`, and are published as GitHub pre-releases.
- The version lives in the app's `package.json` and `src-tauri/tauri.conf.json`, and in the
  CLI's `Cargo.toml` and `Cargo.lock`. The release script sets all of them. The workflow refuses
  a tag that does not match `tauri.conf.json`.
- Commits follow [Conventional Commits](https://www.conventionalcommits.org/) with the
  program's key as the scope: `feat(crawler): crawl hreflang clusters`,
  `fix(log): keep IPv6 hosts`.
  - Shared crates use their own scope (`fix(net): …`). They appear in the notes of every program
    whose folders the commit touches (`paths` in products.json).
  - `chore`, `ci`, `test`, `docs`, `style` and `build` commits stay out of the notes.
  - A `!` after the type, or a `BREAKING CHANGE:` footer, marks a breaking change.

## Release notes

Notes are written in English for the GitHub release, the website's changelog and anyone who
downloads the program.

- `docs/releases/<key>-<version>.md` holds one release.
  - The first line is `# <Program name> <version>`.
  - Then two or three sentences of highlights.
  - Then the sections that apply, in this order: **Breaking changes**, **Added**, **Fixed**,
    **Performance**, **Changed**, **Security**.
  - Then the **Install** paragraph.
- `docs/changelog/<key>.md` holds every release of the program, newest first, in
  [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) form.
- Write for users:
  - Say what changed for them, not how the code changed.
  - One line per change, imperative or descriptive ("Crawl hreflang clusters", "Imports no longer
    stop at a corrupt gzip member").
  - Name settings and screens as the app shows them.

## Steps

1. Preview the notes and the next version:

   ```bash
   pnpm release crawler --dry-run
   ```

   The script reads the commits since the program's last tag. It picks the next version from
   them. To choose one yourself, add `patch`, `minor`, `major` or an exact version.
2. Write them:

   ```bash
   pnpm release crawler
   ```

   This creates `docs/releases/crawler-<version>.md`, prepends the changelog and sets the version
   numbers.
3. Edit the notes: write the highlights and polish the bullets.
4. For the Log Analyser, refresh the bundled crawler IP lists with `cargo xtask bot-ranges` and
   review the diff of `crates/seonexus-botverify/data/ip-ranges.json`.
5. Run `cargo xtask ci`.
6. Commit and tag:

   ```bash
   pnpm release crawler --tag
   ```

   This commits exactly the release files and creates the annotated tag `crawler-v<version>`. Then
   push with `git push origin HEAD crawler-v<version>`.
7. The `Release` workflow:
   - resolves the program from the tag and creates a draft release titled `<Program name>
     <version>` with the notes;
   - builds the desktop installers with `tauri-apps/tauri-action`: universal macOS
     `.app`/`.dmg`, Windows `.msi`/NSIS `.exe`, Linux `.deb`/`.rpm`/`.AppImage`;
   - builds the CLI archives `<cli>-<tag>-<target>` for Linux x86_64, macOS arm64/x86_64 and
     Windows x86_64.

   Review the draft on GitHub and publish it (`gh release edit v<version> --repo octopusOn/seo-crawler --draft=false`).
   The website shows it within an hour.

## Without GitHub

The website links downloads to GitHub. To serve builds from the website itself instead, for
example on an intranet:

1. Build on each platform with `pnpm desktop:build`, `pnpm spider:build`, `pnpm serp:build`,
   `pnpm keywords:build` or `pnpm finder:build`. On a Mac, add `-- --target
   universal-apple-darwin`.
2. Build the CLI with `cargo build --release -p <package> --target <triple>`.
3. Run `pnpm web:downloads -- --product <seo-crawler|log-analyser|serp-checker|seo-keyword-research|index-checker>`
   for each program. The files land in `apps/web/public/downloads/<tag>/`.
4. Build the site with `OCTOPUSSEO_LOCAL_DOWNLOADS=1 pnpm web:build`.

## Signing

Unsigned builds work but trigger Gatekeeper / SmartScreen warnings ("Open Anyway" /
"Run anyway"); the download page explains both.

### macOS (Developer ID + notarization)

1. Join the Apple Developer Program (individual or organization; the enrolled name is shown as
   the developer).
2. Create a **Developer ID Application** certificate (Xcode → Settings → Accounts → Manage
   Certificates, or a CSR on developer.apple.com) and export it with its key as `.p12`.
3. Create an app-specific password for the Apple ID (appleid.apple.com).
4. Add the repository secrets `APPLE_CERTIFICATE` (`base64 -i cert.p12`),
   `APPLE_CERTIFICATE_PASSWORD`, `APPLE_SIGNING_IDENTITY`
   (`Developer ID Application: Name (TEAMID)`), `APPLE_ID`, `APPLE_PASSWORD` (the
   app-specific password) and `APPLE_TEAM_ID`.

The release workflow then signs, notarizes and staples the app; without the secrets it signs ad
hoc. Locally, export the same variables before building.

### Windows

Configure a code-signing certificate in `tauri.conf.json` (`bundle.windows`), for example
Azure Trusted Signing. Optional updater signing uses `TAURI_SIGNING_PRIVATE_KEY` and
`TAURI_SIGNING_PRIVATE_KEY_PASSWORD`.
