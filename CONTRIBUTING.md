# Contributing to octopusSEO

## Workflow

1. Run `cargo xtask ci` before opening a pull request (Rust checks plus the frontend checks
   when pnpm is installed). CI runs the same checks on Linux, macOS and Windows.
2. Use [Conventional Commits](https://www.conventionalcommits.org/) (`feat(logparse): ...`, `fix(store): ...`). Changelogs and versions are generated from commit history.
3. Significant architectural changes require an ADR in `docs/adr/`.

## Architecture rules

- **Hexagonal layering.** Domain crates (`seonexus-logparse`, `-url`, `-botverify`, `-query`, `-analytics`) must not depend on DuckDB, Tauri, the filesystem or the network. They expose traits (ports); adapters (`seonexus-store`, `seonexus-report`, `seonexus-net`) implement them.
- **Thin front-ends.** `apps/snx-log` and `apps/desktop` only translate user input into calls on `seonexus-log-app` use-cases. A feature is added to the use-case first, then exposed in both front-ends.
- **Small modules.** One responsibility per module. Split files that grow hard to read.

## Code rules

- No `unwrap`, `expect`, `panic!` or `todo!` outside tests (enforced by clippy).
- `unsafe` is forbidden workspace-wide.
- Libraries use `thiserror` error enums; binaries use `miette`/`anyhow`.
- Use `tracing` for logging; never log secrets.
- Every parser change needs snapshot tests (`insta`) and, where applicable, property tests (`proptest`) and a fuzz target (`fuzz/`).
- New dependencies must pass `cargo deny check` and be justified in the PR. Prefer releases that are at least two weeks old.

## Frontend

- `apps/desktop` is TypeScript in strict mode; `pnpm --filter @seonexus/desktop check` runs `tsc` and Vitest.
- Every UI string goes through the typed dictionaries in `src/i18n` (English and Turkish).
- IPC types in `src/api/types.ts` mirror the Rust DTOs; `src-tauri/src/ipc_tests.rs` must cover new commands with the JSON the UI sends.

## Bots

- Add or change bots in `crates/seonexus-botverify/src/catalog.rs` (specific patterns before generic ones) and bump `CATALOG_VERSION`; stored sites are re-classified automatically.
- New IP list sources go to `ranges::SOURCES`; refresh the snapshot with `cargo xtask bot-ranges`.
