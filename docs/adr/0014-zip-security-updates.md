# ADR 0014 — ZIP security updates

Status: Accepted

## Context

The dependency audit reported RUSTSEC-2026-0367, RUSTSEC-2026-0368 and RUSTSEC-2026-0369 for transitive ZIP 6 and 8 dependencies. The workspace's direct ZIP 9.0.4 dependency does not replace incompatible major versions required by DuckDB's build script and the Excel writer.

## Decision

Upgrade DuckDB and libduckdb-sys to 1.10506.1, whose build dependency accepts ZIP 9. Keep the existing bundled database features.

The latest published rust_xlsxwriter 0.99.1 still requires ZIP 8. Pin the official repository to [`ade0a017439d169b2a9c4848fe1cb66919c0ca3d`](https://github.com/jmcnamara/rust_xlsxwriter/commit/ade0a017439d169b2a9c4848fe1cb66919c0ca3d), the upstream commit upgrading ZIP to 9, and retain constant-memory writing. This also includes upstream changes preceding that commit; exercise the existing Excel export regression tests. Its worksheet initialization is now fallible: perform it in `TableSink::begin` and propagate temporary-file failures. Cargo.lock records the immutable revision. Allow only that official repository in cargo-deny; unknown Git sources remain denied. No advisory is suppressed.

Replace the Git dependency with a crates.io release once a published version includes this fix. Keep the lockfile update limited to these parent packages and dependencies removed by ZIP unification.

## Validation

Run cargo-deny across the workspace, the workspace Rust tests and Clippy, and rebuild the native app. Export tests include CSV, JSON and XLSX, filtered selections, empty results and failure-safe replacement of existing files.
