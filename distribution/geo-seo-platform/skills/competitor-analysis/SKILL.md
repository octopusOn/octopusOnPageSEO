---
name: competitor-analysis
description: Compares a project's website with its competitors using the OctopusOn GEO-SEO Platform MCP tools - organic traffic, keywords, top pages, backlinks, keyword and link gaps, and presence in AI answers. Use when the user asks who their competitors are, how they compare, why a rival ranks higher, or where the opportunities are.
---

# Competitor analysis

Answer in the user's language. Most of this data is paid: price the whole plan first and ask once.

## Steps

1. Call `list_projects`; pick the project. Its `competitors` list is the starting set.
   - If it is empty, offer `domain_overview` with `report: "competitors"` (paid) to find the sites that
     share the most keywords, and let the user choose up to four.
2. Price the plan with `estimate_cost`, tell the user the total, and wait for a yes. A full comparison is:
   - `domain_overview` `summary` for the project and each competitor (pass `domain`),
   - `keyword_gap` `bulk` (traffic, keywords, authority side by side), `missing` and `shared`,
   - `keyword_gap` `links` (sites that link to competitors but not to the project),
   - `backlinks` `summary` for the project and each competitor (pass `target`),
   - optionally `domain_overview` `pages` for a competitor's top pages and `ai_mentions` `compare`.
   Offer a lighter plan (bulk + missing only) when the user wants to spend less.
3. Read what is free as well: `rank_report` competitor shares and `ai_visibility` share of voice.

## Answer

- A side-by-side table: organic traffic, traffic value, keywords, authority (OA), referring domains,
  AI share of voice.
- Where each competitor wins: topics (from missing keywords), page types (from top pages), link sources.
- Opportunities, ordered: keyword groups to target, pages to improve, link prospects from `links`.
- Say what the numbers are: third-party estimates for the project's market, not the competitors' own
  analytics.

## Paying for data

Paid tools answer from kept data while it is fresh (`status: "kept"`, nothing spent). Otherwise they return
`status: "needs_approval"` with `price.usd`, `price.credits` and how it would be paid (own key or credits).

1. Tell the user what you want to fetch, why, and the price. Put several paid calls into one question with
   the total; `estimate_cost` prices a call without buying.
2. Call again with `allow_spend_usd` set to that price only after the user says yes. Never raise it on your
   own and never retry with a higher allowance.
3. If a tool says the data cannot be bought (no key, no credits, budget reached), tell the user to open
   Sources in the program. Do not work around it.
