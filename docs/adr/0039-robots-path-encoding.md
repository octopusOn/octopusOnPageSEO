# ADR 0039 — Comparable robots.txt path encodings

Status: Accepted

## Context

The shared robots matcher compared rule text directly with request targets. A UTF-8 rule such as `/özel/` did not match the percent-encoded path produced by the crawl URL parser. Differing hex case and escaped unreserved ASCII could likewise change a verdict. Equal raw/encoded rules also had different original string lengths, making an encoded Disallow beat an equivalent raw Allow. An initial UTF-8 BOM hid the first User-agent field, and CR-only files were not split into records.

## Decision

Normalize rules once while parsing and normalize each target once per decision. Reuse the URL library's percent-escape normalization: decode only unreserved ASCII, uppercase other escapes, and keep stray percent characters from creating new escapes. Encode remaining non-ASCII UTF-8 octets before comparison. Reserved separators remain escaped; `%2F` is distinct from `/` and `%3F` from `?`. Decode no escape recursively. Path case, raw wildcard/end-anchor syntax and group selection remain unchanged.

Rank matching rules by their normalized pattern length, preserving Allow on equal specificity. Store a private compiled rule beside its original public rule, so returned verdicts, crawl records, logs and exports keep the source's spelling. Ordinary ASCII targets without escapes use a borrowed fast path; rules are not normalized repeatedly for each URL.

Ignore a leading UTF-8 BOM and accept CR, LF and CRLF records. The same matcher serves Spider HTTP requests, JavaScript resource policy, AI-crawler checks and Log Analyser robots-compliance reports. No schema, dependency or saved-preference changes are required. Historical Spider archives are not rewritten; a new crawl collects corrected blocking observations.

## Validation

Four regressions first reproduce raw UTF-8/encoded mismatches, escaped-unreserved mismatches, wrong specificity and the hidden BOM group. Unit tests then cover both spellings and hex cases, Unicode Allow exceptions, original rule text, separator/double-escape distinctions, literal encoded wildcards and path case. A property test checks ASCII output and stable repeated normalization, including explicit malformed-percent examples from the shared URL normalizer.

HTML and injected-JavaScript use cases verify four blocked and four fetched URLs, zero invented request failures, one shared robots request, original text and rule fields in complete exports, and the same decisions for browser resource policy. Ignore-but-report still fetches blocked pages while retaining their rule evidence. A Log Analyser regression imports encoded Googlebot requests and verifies the disallowed URL count and original UTF-8 rule in its report.

Native loopback QA covers HTML/real Chrome with Respect, HTML ignore-but-report and HTML Ignore. Respect records eight discovered URLs with four blocked and four actual HTTP pages, without requesting the blocked pages. Chrome also blocks a UTF-8 script and XHR while allowing their more-specific UTF-8 exceptions; the allowed script and XHR produce the expected rendered title. Each main response is fetched once, no render/request error is invented, and complete JSON exports preserve original rules and BOM/CR text. Ignore-but-report fetches all eight pages while retaining four blocked flags; Ignore fetches all eight without fetching robots.txt or adding blocked flags. The fixture server is closed afterwards. All 511 workspace tests pass (three ignored), together with strict clippy, formatting and desktop/CLI builds.

References: [RFC 9309 §2.2.2](https://www.rfc-editor.org/rfc/rfc9309.html#section-2.2.2), [Google robots.txt specification](https://developers.google.com/crawling/docs/robots-txt/robots-txt-spec).
