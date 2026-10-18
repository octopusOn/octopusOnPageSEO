# ADR 0042 — Ignored robots records preserve shared crawler groups

Status: Accepted

## Context

The robots.txt parser reset its agent-header state for unknown key/value records. A Crawl-delay, Host or custom field between two User-agent lines prematurely flushed the earlier bot into an empty group. Subsequent Allow/Disallow rules then applied only to the later bot, potentially fetching a page the earlier bot should have blocked. The shared parser also serves JavaScript resource policy, AI crawler access checks and Log Analyser robots-compliance reports.

## Decision

Ignored records leave group state unchanged. A real Allow/Disallow record still ends the user-agent header, including an empty rule value; the following User-agent starts a separate group. Sitemap extraction continues independently without terminating groups. Preserve existing rule text, rule precedence, wildcard/default selection, repeated-group merging, path normalization and original robots.txt exports.

This does not implement Crawl-delay, Host, Request-rate, Noindex or custom extension semantics. Request timing continues to use the configured crawl rate. The correction only prevents ignored fields from changing the grouping of supported records, consistent with RFC 9309 §2.2.4.

No dependencies, schema or saved preferences change. Historical Spider archives retain stored observations; repeat a crawl to collect corrected blocking decisions and discovered pages. Log Analyser reports use the corrected parser when generated again from existing request logs.

## Validation

Two regressions reproduce a shared Googlebot/Bingbot rule being lost before the fix. Tests then verify the same result when nine ignored records are inserted at every boundary of a multi-group robots file. A separate test preserves actual group boundaries after nonempty and empty Allow/Disallow fields.

Spider application tests cover both Googlebot and Bingbot in HTML and injected JavaScript, plus ignore-but-report. They check actual HTTP requests, shared robots caching, Allow exceptions, the selected specific group taking precedence over the wildcard group, browser resource policy, original rule/file/sitemap evidence and exact blocked-page findings in complete exports. AI audit tests verify GPTBot and CCBot share a block across an ignored extension. Log Analyser imports disallowed/allowed Googlebot requests and verifies one blocked URL with its original rule.

Six native loopback runs cover Googlebot and Bingbot in HTML/real Chrome with Respect, plus Googlebot ignore-but-report and Ignore. Respect records five URLs, blocks the private page without requesting it, and keeps the three allowed destinations fetchable. Chrome blocks private script/XHR requests while loading the Allow exception and resources outside the wildcard-only block; the expected rendered title is observed. Each main document and shared robots file is requested once. Ignore-but-report fetches the private page with its blocked flag; Ignore fetches all pages without reading robots.txt. Exports retain original text/rules/sitemap evidence. No request/render error is invented, and the server is closed afterwards. All 534 Rust workspace tests pass (three ignored), together with strict clippy, formatting and desktop/CLI builds. Frontend code is unchanged; its prior 133 passing tests remain the baseline.

References: [RFC 9309 §2.2.4](https://www.rfc-editor.org/rfc/rfc9309.html#section-2.2.4), [Google robots.txt specification](https://developers.google.com/crawling/docs/robots-txt/robots-txt-spec).
