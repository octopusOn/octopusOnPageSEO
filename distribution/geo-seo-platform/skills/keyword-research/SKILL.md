---
name: keyword-research
description: Researches keywords for a project with the OctopusOn GEO-SEO Platform MCP tools - ideas, search volume, difficulty, intent, live SERPs and competitor gaps - and groups them into page-sized topics. Use when the user asks for keyword ideas, what to write about, content planning, search volume or difficulty, or which keywords a page should target.
---

# Keyword research

Answer in the user's language. Keyword data costs money: plan the calls, price them, ask once.

## Steps

1. Call `list_projects`; pick the project (ask if unclear). Its country and language set the market.
2. Agree on seeds with the user: their products or services, the questions customers ask, and
   competitors' strong topics. Two to five seeds are usually enough.
3. Price the plan with `estimate_cost` before buying, for example:
   - `keyword_research` with `report: "ideas"` (or `"related"`, `"suggestions"`) for each seed,
   - `keyword_research` with `report: "overview"` and up to 700 `keywords` to size a list you already have,
   - `report: "wordstat"` for Yandex markets (Turkey, Russia, CIS) when the user wants Yandex demand,
   - `keyword_gap` with `report: "missing"` to see what competitors rank for and the project does not.
   Tell the user the total and wait for a yes (see "Paying for data").
4. Clean the list: drop brand terms of others (unless asked), duplicates, and keywords of the wrong intent
   or country.
5. Group keywords that one page can answer (same intent, same results). For a close call, look at the live
   results with `serp_overview` (paid): if two keywords show mostly the same pages, they belong together.
6. Prioritise each group: volume, difficulty against the site's authority (`domain_overview` summary),
   business value, and whether the site already ranks (`rank_report`, `search_console`, free).

## Answer

- A table per group: main keyword, other keywords, monthly volume (sum), difficulty, intent, CPC,
  existing page or "new page", priority.
- The three groups to start with and why.
- What was bought, what it cost, and which data was already kept.

## Paying for data

Paid tools answer from kept data while it is fresh (`status: "kept"`, nothing spent). Otherwise they return
`status: "needs_approval"` with `price.usd`, `price.credits` and how it would be paid (own key or credits).

1. Tell the user what you want to fetch, why, and the price. Put several paid calls into one question with
   the total; `estimate_cost` prices a call without buying.
2. Call again with `allow_spend_usd` set to that price only after the user says yes. Never raise it on your
   own and never retry with a higher allowance.
3. If a tool says the data cannot be bought (no key, no credits, budget reached), tell the user to open
   Sources in the program. Do not work around it.
