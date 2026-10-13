# ADR 0004 — Desktop UI with Tauri 2 and React

- Status: accepted
- Date: 2026-10-12

## Context
The desktop app needs virtualized tables with millions of rows, rich charts, a visual filter builder and a small binary on Windows, macOS and Linux.

## Decision
Tauri 2 shell with a React + TypeScript frontend (TanStack Table/Virtual/Query, ECharts). All heavy work stays in Rust; the frontend requests paginated results over Tauri IPC and receives progress via channels. Capabilities and CSP are kept minimal.

## Alternatives considered
Pure-Rust GUIs (Slint, egui, Iced): fully native but large data-grid and charting components are immature, slowing development.

## Implementation notes (2026-10-12)
- `apps/desktop/src-tauri` registers one thin async command per use-case; work runs on the
  blocking pool, errors are mapped to `{kind, message, span?}` so the UI can underline query
  syntax errors. `ipc_tests.rs` drives every command through Tauri's mock runtime with the exact
  JSON the frontend sends, keeping both sides in sync.
- Import progress streams over a `tauri::ipc::Channel`; cancellation uses the use-case's
  `CancelToken`. Query results are paged (500 rows) into a virtualized grid.
- The UI has no component framework: CSS variables (light/dark), lucide icons, ECharts
  (tree-shaken). Strings live in typed English/Turkish dictionaries; a test enforces that every
  key is translated with the same placeholders.
- `pnpm dev` in a plain browser installs a demo backend (`src/dev/mockBackend.ts`) via Tauri's
  IPC mocks, so the UI can be built and reviewed without Rust. It is excluded from production
  builds.
- Capabilities grant only `core:default` and the open/save dialogs; the CSP allows no remote
  content.

## Consequences
- Requires Node/pnpm in the development toolchain.
- UI components can be shared with future products via `packages/ui`.
