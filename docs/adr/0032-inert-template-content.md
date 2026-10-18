# ADR 0032 — Inert HTML template content

Status: Accepted

## Context

The HTML parser exposes template content in its traversal tree. Broad selectors consequently extracted unused template metadata, base URLs, links, resources, headings and structured data. An inactive base could change active link resolution, and an unused robots tag could affect indexability and discovery. Active anchor/heading text also included template descendants.

## Decision

Filter template descendants from document element selection and from active title, heading and anchor text. Apply this consistently to base resolution, metadata, links/resources, images/alt fallback, forms and structured data. Nested templates remain inactive. Keep existing visible-text exclusions and normal body metadata/resource behavior.

The distinction follows the [HTML Standard's template content model](https://html.spec.whatwg.org/multipage/scripting.html#the-template-element): content belongs to an inert fragment until it enters the document. The same extraction handles original HTML, rendered DOM and rendering-failure fallback. JavaScript-cloned content that actually enters the ordinary document participates normally, including metadata and newly discovered links. Shadow-root serialization remains outside the renderer's existing scope.

Existing archive observations are retained. There is no schema migration, reparse or automatic recrawl; repeat a saved plan to collect corrected facts into a separate crawl.

## Validation

Three parser regressions reproduce the previous base, heading/text and structured-data errors before the fix. They cover inactive metadata, nested templates, links/resources, mixed-content/form checks and active body metadata. Integration tests exercise HTML, successful DOM activation and failed-render fallback with HTML storage disabled. They verify raw/final robots flags, link counts, stored facts, finding URLs, structured-data totals and full link exports.

Native CLI QA runs the shared production engine against a temporary local site in HTML and real Chrome modes. HTML stores two URLs with one exported link and no active structured-data block. JavaScript clones one template into the body, stores three URLs, exports two links and records the activated noindex and Product block. Original robots flags remain false. Neither run requests unused resources or URLs resolved through the inert base. The server closes after verification; the updated desktop host reopens the original crawl and restores its selected issue view.
