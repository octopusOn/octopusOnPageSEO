# ADR 0018 — Similar content with one observed example per URL

Status: Accepted

## Context

The catalog had a near-duplicate rule and the parser already stored visible-text SimHash fingerprints, but no audit evaluated the rule or identified a comparison URL.

## Decision

Evaluate internal, indexable 200 HTML pages with at least 200 visible words, a nonempty exact-text fingerprint and a stored SimHash. Exclude truncated HTTP captures, truncated DOM and JavaScript failures. Use the analyzed representation's fingerprints; HTML bodies are not needed. Qualifying texts have distinct exact hashes, a Hamming distance of at most 3 over 64 bits, and a shorter word count at least 80% of the longer. The distance convention draws on [Manku, Jain and Das Sarma, Detecting Near-Duplicates for Web Crawling](https://research.google.com/pubs/archive/33026.pdf). The minimum length and length-ratio guard are application policy; this is not a Google indexing rule or percentage of matching text.

Group identical normalized fingerprints, index four 16-bit SimHash bands, and verify candidate distances. Any three changed bits leave a band unchanged. Stop after one qualifying example and record a reverse example when the other group has none. Expand findings back to all URLs in each exact-text group. Preserve one example per URL in `content_matches`; never materialize the full pair graph. Examples are deterministic for a saved URL order, but are neither closest-match rankings nor complete/transitive clusters.

Schema 6 creates the table and export view and triggers the transactional audit refresh. The existing failure/pending-audit mechanism preserves old findings on failure. The new explorer, URL inspector and IPC return stored examples only; all table filtering/sorting/export remains in the backend. Search checks both endpoints. Bit distance, eligibility and boilerplate caveats are visible in English and Turkish. A missing example is inconclusive.

## Limits and validation

Memory/index and stored results are linear in pages. Candidate checks depend on bucket collisions; the index does not guarantee subquadratic time for adversarial fingerprints. No million-page benchmark has been run. Visible text includes navigation/footer content and SimHash can collide, so findings require human content review before merging or canonicalizing.

Tests compare indexed witness existence against an exhaustive oracle, exercise 3/4-bit and 80% boundaries, identical fingerprints with different text hashes, 5,000 dense fingerprints, grouped exact duplicates, excluded capture types, literal URL parameters, both-endpoint search, complete filtered exports and schema upgrades. Frontend tests cover actionable examples, safe text rendering and inconclusive empty results. Native QA validates a separate localhost JavaScript crawl before updating the main saved-crawl application.
