# ADR 0053 — Read HTML hreflang only from eligible head elements

Status: Accepted

## Context

HTML extraction accepted alternate/hreflang links anywhere in active content and with media qualifiers. Body annotations could therefore create phantom alternate URLs, invalid-language findings and false return links. Repeating the alternate relation token also emitted duplicate annotation/link evidence for one element.

## Decision

Read an HTML hreflang annotation only from an active link element directly inside the parsed head, with alternate in its relation tokens, a hreflang attribute, a resolvable href and no media attribute. An explicitly empty media attribute is also rejected as a conservative eligibility choice. Malformed markup repaired into the body follows its actual parsed location. Existing template exclusion remains in place.

Emit one annotation per eligible element, regardless of repeated relation tokens. Preserve separate elements, target resolution, language spelling and invalid language codes for the existing invalid-code check. This change does not add an absolute-URL rule or extend language-code validation. Other relation/resource roles remain independent: a media-qualified alternate stylesheet still creates its normal stylesheet reference. HTTP Link parsing, PDF annotations and header/HTML storage deduplication retain their existing behavior.

The head/media restrictions follow [Google's localized-page guidance](https://developers.google.com/search/docs/specialty/international/localized-versions#html). Apply the same extraction to source HTML, successful rendered DOM and original-response fallback after rendering fails. Eligible header observations remain available when HTML/source storage is disabled. Do not let an ignored body link satisfy a missing-return check or enqueue an alternate target.

Explain eligibility in the Hreflang explorer. No schema, catalog rule ID/priority, saved historical finding or source-archive changes. Prepared archives keep their recorded annotations; recrawl to collect corrected observations. This does not add rejected-element diagnostics or change missing-return coverage policy.

## Validation

Four parser regressions fail before the fix and cover active head/body/template placement, repaired markup, media including empty values, independent CSS/mixed-content references, per-element relation deduplication, separate declarations and retained invalid-language evidence. Three integrated regressions cover HTML, successful DOM and failed-render source fallback, including real stored annotations, discovery, reciprocity findings, PDF headers, CSS requests and exports with HTML/headers disabled. Full workspace and native desktop/Chrome verification results are recorded in the QA state for this change.

All 618 Rust workspace tests pass (three opt-in browser cases ignored), alongside 135 frontend tests, strict workspace clippy, formatting and frontend/CLI/desktop builds. An older storage fixture is corrected to put its intended valid hreflang in head. Three native loopback cases include real Chrome execution and an intentional render timeout: HTML and fallback save four URLs/eight annotations; successful DOM adds a fifth URL and four annotation rows. Ignored body/media/template targets are never requested. CSS and PDF header mappings remain available with source/header storage disabled, CSV/JSON agree, and the missing body-only return is reported on exactly the source URL. The intentional timeout creates one rendering finding with zero page HTTP failures; the fixture server closes.

Native desktop inspection shows five successful URLs, one missing-return source and 12 hreflang rows, including the JavaScript alternate and PDF mapping. The new hint explains eligibility. The private app returns to its previous Responsive JavaScript overview and closes; both updated app binaries have verified backups and signatures. The main app returns to the unchanged Nexus crawl with 864 URLs and no active job. All 124 pre-existing database/metadata files and both new private fixture files retain their hashes.
