# ADR 0016 — Redirect-chain inspection and complete error propagation

Status: Accepted

## Context

The redirect-to-error rule checked only the immediate target. For A → B → 404, B was flagged but A was missed. The inspector showed individual Location values without reconstructing the chain. Loop evaluation repeated traversal for every starting URL, including already-resolved suffixes.

## Decision

Resolve the graph of observed HTTP 3xx Location targets with an iterative, memoized traversal. Propagate terminal HTTP/request errors and loops to every preceding redirect. Unrecorded destinations are inconclusive and do not create an error finding. Memory is proportional to stored redirects and failed URL endpoints; traversal does not recurse or materialize all pairs of paths.

Expose a bounded read-only `CrawlReader::redirect_trace` use case through `url_redirects` IPC and the URL inspector's Redirect chain tab. Query saved URL records by parameter, stop at a terminal response/failure, and include the closing repeated URL for cycles. At most 100 URL rows are returned. An unknown target, missing Location or row limit has its own outcome. The view makes no new HTTP requests and excludes JavaScript navigation and meta refresh. The browser demo follows the same response contract.

Schema marker 4 refreshes saved findings through the existing transactional audit pass. Each migration marks audit_required in the same transaction as its schema marker. Successful audit evaluation clears that flag in its findings transaction. A failed evaluation retains prior findings and its pending flag so the next open retries, even after the schema migration was committed. Raw URL/link records, configuration and crawl counters are preserved.

## Validation

Tests cover multi-hop errors, cycles and their incoming tails, self-loops, a 5,000-node shared chain, unknown targets, request errors, robots blocks, missing Location, quoted URLs, and the 100-row boundary. A saved-crawl upgrade test simulates audit failure and confirms retry, preserved old findings and unchanged records/metadata. Frontend tests cover HTTP steps, inconclusive outcomes and escaped stored content. Native QA uses an isolated localhost crawl for error, success, cycle, unchecked, missing-Location and blocked-target cases; request counts establish that opening trace views makes no network requests.
