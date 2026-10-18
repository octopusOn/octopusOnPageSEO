# ADR 0049 — Preserve discoveries through rejected bases and cross-document fragment links

Status: Accepted

## Context

The parser used data/javascript base declarations to resolve all relative references, even though browsers reject those declarations as document bases. This erased relative canonical, hreflang, resource and hyperlink observations. The hyperlink resolver also skipped every empty or fragment-only reference before considering its base, losing links that actually address another document.

## Decision

Keep the first active base element with href, using the existing template exclusion. Resolve its declaration against the document address and keep that resolved declaration in base_href for inspection. If parsing fails or its scheme is data/javascript, use the document address for extraction; do not select a later base. Other schemes retain their existing resolution behavior and the HTTP-only discovery boundary.

For a/area hyperlinks, empty and fragment-only values may resolve to the effective base's HTTP(S) document. Compare the base and current document after removing literal fragments, retaining full query strings and encoded hashes. Emit a link only when these document addresses differ. Keep every observed anchor's text/rel attributes while the frontier deduplicates the actual request. Same-document jumps continue to be omitted, avoiding artificial self-inlinks. Normal nonempty hyperlinks and non-hyperlink fragment handling are unchanged.

The same parser serves original responses, rendered DOM and original-response fallback after rendering failure. Storage schema, historical audits and the catalog are unchanged; a new crawl captures corrected observations. Existing saved archives are preserved.

The fallback behavior follows the [HTML base element algorithm](https://html.spec.whatwg.org/multipage/semantics.html#the-base-element), and document identity follows [document base URLs](https://html.spec.whatwg.org/multipage/urls-and-fetching.html#document-base-urls). This change does not introduce evaluation of CSP base-uri policies into the static extractor.

## Validation

Regressions first reproduce missing relative observations and dropped cross-document fragments. Parser cases cover rejected schemes, malformed/empty first declarations, ignored later bases and templates, other schemes, cross-origin targets, query-only document changes, empty/fragment a/area references and rel/text preservation. Integrated HTML, JavaScript and failed-render tests check saved facts, canonical findings, link exports, three separate references to one fetched destination, and the absence of artificial same-document inlinks.

All 582 Rust workspace tests pass (three opt-in browser tests ignored), alongside 135 frontend tests, strict clippy, formatting and frontend/CLI/desktop builds. Native HTML and real Chrome runs save eight and nine URL observations respectively, with no page HTTP or rendering errors. Chrome executes a relative script through a rejected data base, preserves the source canonical and adds the ninth URL through JavaScript. Source/rendered hyperlink counts are four/five. The target query and encoded hash survive, its three references remain separate and its document is fetched once. CSV and JSON exports agree. Browser-only requests, including any speculative requests against the later base, do not become saved URL or link evidence.

The native desktop verifies the preserved base declaration, corrected canonical, source/DOM comparison and three outgoing references with observed HTTP 200. The fixture server is stopped, the private app returns to its previous Responsive JavaScript overview and closes, and both replaced app binaries have verified backups and signatures. The main app reopens the original Nexus overview with 864 URLs and zero active jobs. All 74 baseline archive/metadata files and both new UI fixture files retain their hashes.
