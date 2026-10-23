---
name: seo-audit
description: Audits a website's technical SEO and GEO readiness with the OctopusOn GEO-SEO Platform MCP tools and turns the findings into a prioritised fix list. Use when the user asks for a site audit, SEO health check, technical SEO review, "what is wrong with my site", crawl issues, or why traffic dropped.
---

# SEO audit

Work from the platform's stored data first; it is free. Answer in the user's language.

## Steps

1. Call `list_projects`. If there is more than one project and the user did not name one, ask which.
2. Call `site_audit_issues` with the `project_id` (pass `lang: "tr"` for a Turkish-speaking user).
   - If there is no audit yet, tell the user to start one on the Site audit page and stop; do not guess.
3. Call `search_console` and `rank_report` (both free) to see which problems cost traffic:
   pages losing clicks or positions matter more than pages nobody visits. Call `core_web_vitals` (free)
   for real-user speed (LCP, INP, CLS) and `opportunities` (free) for the platform's own ranked list.
   - If the audit is old and the user wants a fresh one, `start_site_audit` crawls again (free on the
     desktop; on the web it gives the price first, see below).
4. Rank the findings by impact × effort:
   - **Critical:** pages that cannot be indexed (noindex, blocked by robots.txt, 4xx/5xx, redirect loops,
     canonical to another page), broken internal links on important pages.
   - **High:** duplicate or missing titles and descriptions on pages with impressions, slow or heavy pages,
     missing structured data where a rich result is possible, orphan pages.
   - **GEO readiness:** AI crawlers blocked in robots.txt, no `llms.txt`, little citable content
     (short self-contained answers, lists, tables, dates, sources).
   - **Low:** cosmetic issues on pages without traffic.
5. Only if the user wants to see a key query's competition, use `serp_overview` (paid; see below).

## Answer

- One line with the health score, the GEO readiness score and the trend if a previous audit exists.
- A table: issue, affected pages (count and two or three examples), why it matters, how to fix, effort.
- "Do this week": at most five concrete actions.
- Name the source of each number (audit date, Search Console period).

## Paying for data

Paid tools answer from kept data while it is fresh (`status: "kept"`, nothing spent). Otherwise they return
`status: "needs_approval"` with `price.usd`, `price.credits` and how it would be paid (own key or credits).

1. Tell the user what you want to fetch, why, and the price. Put several paid calls into one question with
   the total; `estimate_cost` prices a call without buying.
2. Call again with `allow_spend_usd` set to that price only after the user says yes. Never raise it on your
   own and never retry with a higher allowance.
3. If a tool says the data cannot be bought (no key, no credits, budget reached), tell the user to open
   Sources in the program. Do not work around it.
