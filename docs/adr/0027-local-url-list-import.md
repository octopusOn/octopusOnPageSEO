# ADR 0027 — Local URL list import

Status: Accepted

## Context

Desktop list crawling accepted pasted URLs but could not import local crawler exports or text lists. Importing must not silently damage query strings, discard overflow records, overwrite a draft or start network work before review.

## Decision

Add a local TXT/CSV/TSV picker to the existing New crawl list mode. Use the browser File API for explicit user-selected files; no filesystem plugin, backend command, schema change or dependency is needed. Decode UTF-8 strictly and bound each file to 10 MiB, 100,000 data records and 512 columns.

Parse quoted delimited records with escaped quotes and embedded newlines, preserving physical record-start line numbers. Detect comma/semicolon/tab headers and Excel separator declarations; recognize exactly one URL header or require single-column headerless input. Reject malformed quoting and inconsistent widths before returning a preview, preventing an unquoted delimiter in a URL from silently truncating its query. TXT ignores blank/comment lines. Accept absolute HTTP(S), reject credentials, controls/whitespace and malformed escapes, remove fragments and deduplicate serialized URLs without stripping query parameters.

For CSV autodetection, inspect the first complete logical record, including quoted header newlines. Parse that header with each supported separator and prefer a unique separator exposing recognized URL headers over punctuation frequency. If more than one separator exposes a URL header, reject the file with a dedicated separator error; an explicit `sep=` declaration or TSV resolves the choice. Keep the existing single-column/headerless fallback. This prevents competing delimiters from silently selecting a different set of addresses. Full-file structural validation and physical line accounting still run before any preview is returned.

A bounded preview displays exact unique/duplicate/invalid counts, five error contexts and ten sample URLs. Never echo rejected URL credentials. Applying explicitly replaces or appends to the URL draft; append validates existing entries and preserves order while deduplicating overlaps. Malformed input, cancelled preview and failed append leave the draft intact. Pending review disables Start. Reading uses a revision token so cancellation, mode switching and unmount cannot apply an obsolete file result. Exclude hidden file inputs from modal focus and focus trapping.

Only Start runs the existing HTML/JavaScript list crawler. File reading and draft actions make no site requests. Keep rendering and other crawl settings unchanged. No remote lists, compressed files, XML/sitemap-index expansion, arbitrary URL-column guessing or Excel workbooks are included.

Keep Spider and list URL drafts independently for the lifetime of the dialog, so switching from an imported list cannot concatenate its URLs into a single-address input. Initialize a single-address Spider draft from the first nonempty seed, retaining every seed for saved multi-seed replays. Switching modes clears pending import state and validation errors without losing either draft. Manual URL normalization splits all CR/LF conventions in list mode and rejects embedded ASCII controls before `URL` can silently remove them; encoded characters are unchanged.

## Validation

Parser tests cover BOM/CRLF/comments, physical multiline positions, selected header columns, quoted delimiters/quotes, semicolon/TSV/Excel declarations, header-only/single-column files, invalid URLs/credentials, zero/duplicate records, exact record/column bounds, file limits, strict UTF-8, read failures and replace/append semantics. SSR checks cover escaped filenames, accurate counts, non-echoed rejected credentials and bounded previews. Native QA imports six CSV records into three valid URLs, one duplicate and two invalid rows; appending TXT produces four unique draft URLs. A malformed CSV cannot apply or start. Cancel preserves all four URLs. No requests occur until Start; the local completed list crawl contains exactly those four URLs and does not fetch its unlisted hyperlink.

Regression checks reject newline/tab/control concatenation, preserve encoded characters and split mixed CR/LF pasted lists without changing paths or query strings. SSR retains saved multiple Spider seeds and renders only the first initial URL for the single-address flow. Native QA covers independent draft edits across mode switches and abandoning a pending preview.

CSV detection regressions cover multiline headers after blank lines, accurate physical error lines, metadata-header punctuation exceeding the true delimiter count, competing URL columns under different delimiters and explicit separator recovery. Native QA imports a multiline semicolon CSV as two unique URLs, one duplicate and one invalid record on physical line 7; a competing-delimiter file disables both apply actions and Start without changing the draft. Cancelling preserves both URLs. Adding `sep=;` selects only the intended Address URL, and applying it changes the draft without starting a crawl.
