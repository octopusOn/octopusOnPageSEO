# Releasing

1. Update `version` in `Cargo.toml` (workspace), `apps/desktop/package.json` and
   `apps/desktop/src-tauri/tauri.conf.json`, and write the release notes in
   `docs/releases/<version>.md` (shown on GitHub and in the website's changelog).
2. Refresh the bundled crawler IP lists: `cargo xtask bot-ranges` (review the diff of
   `crates/seonexus-botverify/data/ip-ranges.json`).
3. Run `cargo xtask ci`.
4. Tag and push: `git tag v0.2.0 && git push origin v0.2.0`.

The `Release` workflow creates a draft release with the notes in `docs/releases/<version>.md`
and builds:

- desktop installers with `tauri-apps/tauri-action` — universal macOS `.app`/`.dmg`, Windows
  `.msi`/NSIS `.exe`, Linux `.deb`/`.rpm`/`.AppImage`;
- `snx-log` archives for Linux x86_64, macOS arm64/x86_64 and Windows x86_64,

and attaches them to the draft. Review it on GitHub and publish it (`gh release edit v0.2.0
--draft=false`); the website lists published releases within an hour.

## Without GitHub

To offer builds from the website alone, build on each platform (`pnpm desktop:build`, on a Mac
with `-- --target universal-apple-darwin`; `cargo build --release -p snx-log`), then run
`pnpm web:downloads` and `pnpm web:build`. Release notes are read from
`docs/releases/<version>.md`.

## Signing

Unsigned builds work but trigger Gatekeeper / SmartScreen warnings. Add these repository
secrets to sign:

- macOS: `APPLE_CERTIFICATE`, `APPLE_CERTIFICATE_PASSWORD`, `APPLE_SIGNING_IDENTITY`,
  `APPLE_ID`, `APPLE_PASSWORD`, `APPLE_TEAM_ID` (notarization),
- Windows: a code-signing certificate configured in `tauri.conf.json` (`bundle.windows`),
- updater (optional): `TAURI_SIGNING_PRIVATE_KEY`, `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`.

See the Tauri distribution guides for details.
