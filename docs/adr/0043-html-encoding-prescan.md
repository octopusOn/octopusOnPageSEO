# ADR 0043 — Bounded HTML character encoding detection

Status: Accepted

## Context

The HTML decoder searched a UTF-8 prefix for the first text matching charset=. Comments, unrelated attributes and script configuration text could become false declarations; an earlier legacy byte stopped that search entirely. Spaced charset attributes were missed, unsupported labels prevented later valid declarations from being considered, and an unsupported HTTP charset prevented HTML fallback. This corrupted Turkish SEO text and Unicode URLs and could create false raw/DOM metadata differences.

## Decision

Keep BOM precedence and the UTF-8 default. Resolve only supported HTTP labels before falling back to a byte-oriented, bounded HTML prescan. Parse HTTP parameter values with quote/escape awareness so semicolons within values cannot introduce charset parameters. Retain first-charset parameter selection; an unsupported value falls through to the HTML prescan.

Scan at most the first 1024 bytes without requiring them to be valid UTF-8. Follow HTML prescan tag/comment/attribute boundaries, case-insensitive names, first duplicate attributes and http-equiv pragma gating. Skip unsupported declarations and continue scanning. Direct charset attributes take precedence over content declarations within a meta tag. Remap meta UTF-16 labels to UTF-8 and x-user-defined to Windows-1252; keep BOM/transport decoding separate. Handle the prescan's XML compatibility declarations and UTF-16 XML byte signatures. Incomplete declarations do not become partial candidates.

This is an encoding prescan rather than DOM construction: its behavior follows the byte algorithm, including its handling of tag-like text. No locale guessing, frequency-based autodetection or unbounded reparsing is added. All returned text still replaces invalid byte sequences instead of failing. No dependency, schema or preference changes. Original HTTP content-type evidence and response bytes supplied to Chrome are preserved. Historical archives retain their observations; repeat a crawl to collect corrected text and discovered URLs.

## Validation

Three regressions reproduce false comment detection, unsupported-label fallback failures and a legacy byte hiding a later declaration. Unit tests additionally cover http-equiv attribute order and pragma requirements, quotes/escapes, duplicate attributes, direct charset precedence, tag-name boundaries, exact/partial/late 1024-byte declarations, UTF-16 and x-user-defined meta handling, XML compatibility, BOM precedence and incomplete prefixes.

Application tests use explicit Windows-1254 bytes, UTF-8 and UTF-16 across nine source cases. HTML, injected JavaScript and failed-render fallback preserve Turkish title/description/H1, the UTF-8 URL for /iletişim, original/raw metadata and complete JSON link/rendering exports. Each actual page is fetched once, headers/HTML storage is disabled, and no malformed replacement-character URL is fetched.

Native loopback crawls exercise all nine source cases in HTML and real Chrome with original HTTP headers preserved. HTML stores ten pages; JavaScript stores eleven after discovering an additional Turkish URL created by script. All title/description/H1/canonical values are correct, raw/DOM metadata match, and exported link text/destinations retain Turkish characters. Original Content-Type values remain unchanged, every main response is fetched once, and no replacement-character URL, HTTP error or render failure is invented. The server is closed afterwards. All 547 Rust workspace tests pass (three ignored), together with strict clippy, formatting and desktop/CLI builds. Frontend code is unchanged; its prior 133 passing tests remain the baseline.

References: [HTML encoding sniffing and byte prescan](https://html.spec.whatwg.org/multipage/parsing.html#prescan-a-byte-stream-to-determine-its-encoding), [MIME parameter parsing](https://mimesniff.spec.whatwg.org/#parsing-a-mime-type).
