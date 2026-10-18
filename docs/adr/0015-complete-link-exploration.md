# ADR 0015 — Complete link exploration

Status: Accepted

## Context

URL details preview at most 1,000 incoming and outgoing references. The total is shown, but larger lists could not be fully inspected. Searching the general Links table only matched source URLs, and it lacked the destination-status filter needed to investigate broken targets.

## Decision

Keep bounded detail previews and add actions opening the shared Links explorer with an exact URL and incoming/outgoing direction. A removable filter identifies that scope. Entering a new scope resets the previous search, status, issue, sort and page; changing tab or crawl clears the scope. Outgoing scoped rows inspect the destination URL, while incoming rows inspect the referring URL.

Extend the shared PageRequest with an optional typed link_scope. Existing callers retain their behavior. Validate that a scope has a nonempty URL and is used only with the Links tab. Escape the URL as a literal. Scope and other filters share one query for pagination and streaming exports. Search covers the unfixed endpoint, or both endpoints when unscoped. The HTTP class filter uses destination_status on link tables. Unknown statuses do not match a class. The browser demo mirrors these semantics.

Filtered exports include all matching references with the displayed columns; full-tab exports remain explicit and include every link. Neither the crawl database nor its findings are modified. Link rows remain references, so repeated anchors are retained.

## Validation

Test more than 1,500 incoming links, pages beyond the preview limit, unpaginated projected exports, outgoing direction, combined search/status filters, literal URLs containing quotes, invalid scopes and invalid direction deserialization. Check demo parity and inspector actions, then verify the native workflow against the saved nexusmedya crawl.
