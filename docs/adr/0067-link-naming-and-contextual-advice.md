# ADR 0067 — Observed link names and contextual recommendations

Status: Accepted; scoring model v4.

## Context

The floating WhatsApp link on Nexus contains a decorative SVG and `aria-label="WhatsApp"`. The old extractor stored an empty text and remediation reconstructed an empty anchor, hiding both the icon and its explicit name. The same path missed referenced labels, title fallbacks and later image alts. Optional x-default and an arbitrary internal-link count also reduced scores, while normal external WhatsApp redirects appeared as defects.

## Decision

Record hyperlink text content, descriptive name, name source, SVG/image presence and observation provenance separately. Explicit ordered aria-labelledby references take precedence over aria-label, followed by content/image alt, nonhidden SVG names and the link title fallback. Do not follow cyclic reference chains. Exclude template/script/style and known hidden/decorative descendants; explicitly referenced hidden labels can still name a link. A graphic alone is not a descriptive name. This is static naming evidence, not a full browser accessibility computation or a claim about ranking signals. Follow [W3C naming guidance](https://www.w3.org/WAI/ARIA/apg/practices/names-and-descriptions/) and [Google's text/image/title guidance](https://developers.google.com/search/docs/crawling-indexing/links-crawlable).

Carry names through HTML and rendered DOM crawling and persist compact evidence in schema v10's `link_names`, even with HTML/header saving disabled. Keep `links.text` as the effective descriptive label for new records, with separate content/name provenance available in built-in tables and remediation.

Prepared v7–v9 archives remain readable without migration. Where complete saved HTML/DOM exists, recover naming evidence for legacy empty/generic links only after matching the entire ordered hyperlink sequence and normalized targets, including repeated destinations. On mismatch, truncation or absent HTML, keep uncertainty and raw evidence; never infer a name from the destination hostname. A legacy empty/generic finding with no confirmed naming-failure witness is unverified rather than penalized. Mark the overall assessment partial and the Links branch health unavailable until a new crawl records labels. A mixed source with a confirmed bad label still counts. Use temporary tables and an archive-fingerprint RAM cache bounded to three entries and 100,000 recovered rows per entry. Original databases and historical finding comparisons remain unchanged.

Current reports, scoring and remediation share these observations. A mixed page keeps its genuinely unlabeled link with the exact recorded position. Unknown older evidence does not generate an invented empty `<a></a>`; an observed SVG is retained in illustrative correction guidance.

Missing x-default and link counts above the application's 300-link review heuristic remain recommendations without automatic score penalties. [Google does not prescribe an ideal link count](https://developers.google.com/search/docs/crawling-indexing/links-crawlable#internal-links-cross-reference-your-own-content). Do not turn a number or an optional annotation into a universal SEO failure.

Successful external `https://wa.me/<6–15 digits>` contact-route redirects are expected routing, not a defect to fix by replacing the contact URL. Keep their observed HTTP responses visible, but omit a redirect finding/penalty when no other genuine redirect witness remains. Other hosts/routes, internal redirects and broken/error responses retain their checks. This exception does not validate the phone number's ownership or WhatsApp availability. Retain all raw records and expose separate name, canonical, contact-routing and optional-advice exclusion counts.

## Validation

Parser regressions cover SVG-only links with and without labels, hidden SVG titles, ordered/hidden/missing/self-referencing IDs, title fallback, multiple image alts, mixed text/images and inert content. Storage regressions cover repeated destinations with only one unnamed link, raw-record preservation, absent/mismatched/truncated HTML, source-disabled recorded names, generic-only legacy links, optional score policy, expected WhatsApp versus real internal redirects and spoofed/unrecognized routes. Read-only legacy recovery is tested twice against byte-identical database contents. HTML and injected-JavaScript application tests persist naming evidence with HTML/header storage disabled. Frontend tests cover accurate SVG guidance, unknown old markup and v4 exclusion accounting.
