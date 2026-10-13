# ADR 0002 — Monorepo with hexagonal crate layering

- Status: accepted
- Date: 2026-10-12

## Context
Several products (Log Analyser now, Spider later) share parsing, storage, reporting and UI pieces. Each product ships as a separate binary. The Log Analyser exposes both a CLI and a desktop app that must behave identically.

## Decision
- One Cargo workspace (`crates/` libraries, `apps/` binaries) plus a pnpm workspace for frontend packages.
- Ports & adapters: domain crates (`logparse`, `url`, `botverify`, `query`, `analytics`) define traits and pure logic with no I/O framework dependencies. Adapter crates (`store`, `report`) implement them.
- `seonexus-log-app` hosts all use-cases; CLI and Tauri layers only map input to use-case calls.

## Consequences
- Domain logic is unit-testable without databases or a UI.
- Adding a product means adding an `apps/` entry and reusing crates, not copying code.
- Slightly more crates to maintain; dependency direction is enforced by review.
