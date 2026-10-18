# ADR 0061 — Cloudflare defaults in existing URL exclusions

Status: Accepted

## Context

Cloudflare email placeholders may return HTTP errors when fetched as pages, causing a shared address to appear as broken on hundreds of source pages. The user requested that these addresses be handled through the existing Exclude URL patterns field, without separate controls.

## Decision

Populate the existing `urls.exclude` defaults with four editable regex lines: exact email-protection path; versioned hexadecimal email-decode script; challenge-platform root/subtree; exact trace, rum and script_monitor/report paths. Anchor the HTTP(S) URL and path boundaries. Keep Cloudflare image transformations and similarly named normal pages included. Removing the lines restores normal checks, using the existing mechanism.

Do not introduce an independent policy, settings group, database schema or result category. Preserve existing saved preferences and replayed crawl patterns. Add the same four lines to this user's current exclusion field through the UI, then save. Existing archive records stay unchanged; repeat a crawl with these patterns to apply them. Browser resource requests continue to use their existing policy, so email decoding resources can run during JavaScript rendering.

## Evidence and validation

Official sources: [Cloudflare endpoints](https://developers.cloudflare.com/fundamentals/reference/cdn-cgi-endpoint/), [email obfuscation](https://developers.cloudflare.com/waf/tools/scrape-shield/email-address-obfuscation/), [internal requests skipped by Bot Management](https://developers.cloudflare.com/bots/troubleshooting/bot-management-skips/).

Test URL/path boundaries and negatives, normal image discovery, retained source links, restoration after clearing exclusions, and browser resource policy. Match frontend and Rust defaults. Native QA verifies the existing text area and a disposable local crawl; user archives are retained and hash-checked.
