# ADR 0040 — Responsive image discovery

Status: Accepted

## Context

Image extraction only read img src (with the existing lazy data-src fallback). Responsive alternatives in img srcset and picture/source srcset were absent from resource scheduling, image tables, image audits and exports. Next.js image optimizer URLs and alternatives selected only at other viewport sizes could therefore escape checks.

## Decision

Extract HTTP(S) candidates from img src, img srcset and direct preceding source children of its picture parent. Keep fallback src observations even when responsive alternatives are present. Audit all valid declared alternatives regardless of media, type or viewport selection; this is a resource inventory, not a claim that Chrome loaded every alternative. The Images view explains that scope and that HTTP checks require image crawling to be enabled.

Use the HTML srcset tokenizer rather than splitting on commas: URL commas and data-URL payloads remain intact, trailing delimiter commas are handled, and commas in parenthesized descriptors cannot manufacture new URLs. Validate width, density and future-compatible height descriptors, including conflicts, HTML floating-point syntax and finite nonnegative density. Ignore invalid candidates and non-HTTP schemes while recovering subsequent candidates. Resolve against the active base URL. Unused template content and sources after the owning img or outside picture remain excluded.

Deduplicate resolved URLs within each owning img, retaining separate occurrences for different img elements. Every alternative inherits its owner's alt and lazy attributes. Picture source dimensions override img dimensions when either source dimension is present; a duplicate URL retains the conservative combined dimension observation. Existing dimension checks continue to observe attribute presence, without inferring CSS layout or decoding image pixels.

Reuse the existing Image link kind, resource queue, images table and page-level image findings. Broken alternatives therefore contribute a finding on the referencing page; multiple alternatives do not duplicate that page's finding. Disabling image crawling keeps candidate metadata and unknown response statuses, without inventing broken-image findings. JavaScript uses the rendered DOM; failed rendering falls back to source HTML. No dependency, schema or saved-preference changes. Historical archives keep their original image inventory; repeat a crawl to capture responsive alternatives.

## Validation

Extraction regressions reproduce missing responsive alternatives before implementation. Parser and page tests cover Next.js optimizer queries, URL commas, data URLs, descriptor validation/recovery, base URLs, source order/ownership, mixed content, inert templates, missing src, empty/missing alt, lazy metadata, owner-specific duplicates and source dimensions. Application tests cover HTML, injected JavaScript, failed-render fallback and image crawling disabled, checking actual requests, complete image/link exports and page findings.

Native loopback QA confirms six source candidates in HTML and eight candidates with real Chrome, including two JavaScript-created lazy alternatives and a 404 Next.js-style optimizer variant. The inactive media/type source is checked too, without requesting invalid, inert or following-source URLs. Complete image/link exports preserve URL queries, commas and owner metadata. Each main document is fetched once. With image crawling disabled, all six metadata rows remain and no image HTTP requests or broken-image findings are invented. The native Images view displays all eight rows, the 404 and missing-alt/dimension flags. The fixture server and private app are closed afterwards. All 521 workspace tests pass (three ignored), along with 133 frontend tests, strict clippy, formatting and frontend/desktop/CLI builds. Original Nexus archives remain unchanged.

References: [HTML srcset parsing](https://html.spec.whatwg.org/multipage/images.html#parsing-a-srcset-attribute), [HTML picture](https://html.spec.whatwg.org/multipage/embedded-content.html#the-picture-element), [HTML floating-point syntax](https://html.spec.whatwg.org/multipage/common-microsyntaxes.html#valid-floating-point-number).
