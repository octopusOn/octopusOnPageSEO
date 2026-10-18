# ADR 0023 — Per-tab table column layouts

Status: Accepted

## Context

The explorer received complete result schemas but displayed only a fixed subset of fields. Users could not bring other stored SEO facts into the table or choose their order. Filtered exports already used the displayed columns and ordering, making the same layout contract useful for customization.

## Decision

Add a column dialog to each settled explorer tab. Users can search available fields, select or hide optional columns, reorder them with accessible up/down controls, select all, clear optional fields and restore defaults. Changes stay in a draft until Apply; Cancel, Escape and dismissal leave the table unchanged. Store applied layouts by UI tab in the versioned device-local `spider.columns.v1` preference, independent of crawl records and other tabs.

Validate preference types and sizes before use. Match selected names against the current schema, discard unavailable fields from the display, and fall back to defaults when an entire historical layout no longer matches. An explicitly empty optional selection remains valid. Keep URL identity fields fixed at the start, including both endpoints for link, image and content-match tables. A duplicate group's comparison field is also mandatory while that scope is active. Inject it only for the current view; edits to other fields do not persist the injected field unless it was already explicitly selected. Reset removes the tab preference and resumes current defaults after Apply.

Customized order flows through the existing table projection and filtered streaming export. Hiding the active sort column clears the sort and resets pagination through the existing paginated query; export stays unavailable until the settled view is coherent. The full-tab export retains its existing all-column behavior. Field selection and reordering use the loaded result schema and rows. No backend API, database migration or site request is added. The picker is disabled while data is unavailable, fetching, erroneous or showing a previous query's placeholder schema.

## Validation

Frontend tests cover malformed/oversized storage, tab independence, schema evolution, explicit endpoint-only layouts, fixed-field ordering, temporary comparison fields, custom ordering, escaping and independent export capture. Native QA checks Cancel, field search, selection and ordering, tab switching, persistence after restart, duplicate-scope editing and removal, default restoration and a complete 266-row CSV with the chosen column order. Original crawl database/metadata hashes remain unchanged.
