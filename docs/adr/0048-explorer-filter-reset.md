# ADR 0048 — Distinguish clearing search from resetting all explorer filters

Status: Accepted

## Context

The URL search's X button was labeled Clear filters while it only removed search text. A remaining HTTP status or other scope kept the row total restricted, making it appear that the indicators or data were incorrect. The issue chip used the same misleading label. There was no single action to restore an unfiltered view.

## Decision

Add a visible Clear all filters action whenever any explorer filter is active, including a pending debounced search. Reset search and its debounced value, HTTP status, indexability, issue, response group, link scope and duplicate group in one React event. Reset the row offset and selected inspector URL, and focus the search input. Keep the crawl, tab, column preferences, applicable sort and persistent issue exclusions.

Give the search-only and issue-only X buttons explicit Turkish/English accessible names and tooltips. Existing link/duplicate chip actions remain independent. The global action is available for empty results and while requests settle; the normal query/placeholder guards continue controlling exports.

## Validation

TypeScript, all 135 existing frontend tests and the production frontend build pass. No Rust, dependency, schema, crawler or saved-observation changes are required.

Native desktop checks use the existing 16-map JavaScript fixture. Issue plus search returns two rows; clearing search preserves the ten-row issue scope, and clearing all returns all 16 maps. Source 3xx plus target search returns two rows; clearing search preserves 12 source responses, and clearing all restores 16 with status All. A zero-row noindex view resets to both internal pages. Response-group, incoming-link and duplicate-content scopes also reset with their specific chips removed. Existing archive and metadata files are hash-checked before and after inspection, and the private application's previous Responsive JavaScript overview is restored before closing it.

The original Nexus archive verifies pagination and inspector cleanup: the matching 266 internal URLs are opened at rows 251–266, a URL detail is selected, then Clear all filters returns to rows 1–250 with the inspector closed and search focused. The main app returns to its original 864-URL overview with no active job. All 74 baseline archive/metadata files retain their original hashes. The running development desktop picks up the frontend update through its existing local Vite server; no application binary replacement or server restart is needed for this UI-only change.
