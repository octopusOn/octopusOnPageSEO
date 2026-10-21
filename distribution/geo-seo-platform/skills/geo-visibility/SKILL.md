---
name: geo-visibility
description: Measures and improves a brand's visibility in AI answers (ChatGPT, Claude, Gemini, Perplexity, Google AI Overviews and AI Mode) with the OctopusOn GEO-SEO Platform MCP tools - mention and citation rates, share of voice, cited sources and the site's GEO readiness. Use when the user asks about GEO, AI search, LLM visibility, being cited by ChatGPT, or AI Overviews.
---

# GEO visibility

Answer in the user's language. Generative engine optimisation (GEO) is about being mentioned and cited
in AI answers, not only ranking in blue links.

## Steps

1. Call `list_projects`; pick the project. Check that its `brands` (brand names) are set; if not, tell
   the user to add them on the project, because mentions are matched by name.
2. Call `ai_visibility` (free). It shows the platform's own prompt runs: for each engine the mention rate,
   citation rate, average position in the answer, sentiment, share of voice against rivals, and the
   domains and pages the answers cite.
   - If there are no runs, tell the user to add questions and run them on the AI visibility page.
3. Optionally, with the user's yes, `ai_mentions` (paid): `summary`, `queries` (the questions where AI
   answers mention the domain) and `compare` (against rivals).
4. Call `site_audit_issues` (free) and read the GEO readiness part: AI crawler access in robots.txt,
   `llms.txt`, structured data coverage and citable content.

## Answer

- Where the brand stands per engine, and against which rival it loses most.
- The sources AI answers trust for these questions: the pages to be mentioned on (reviews, lists,
  directories, forums) and the project's own pages that get cited.
- Actions, most effective first:
  - allow AI crawlers that the user wants (GPTBot, ClaudeBot, PerplexityBot, Google-Extended) and publish
    an `llms.txt`;
  - answer the tracked questions directly on the site: a short self-contained answer, then detail, lists,
    tables, dates and sources; keep facts consistent across pages;
  - add structured data (Organization, Product, FAQ, Article) and keep brand names consistent;
  - earn mentions on the sources the answers cite.
- Remind the user that AI answers vary from run to run: compare rates over several runs, not single answers.

## Paying for data

Paid tools answer from kept data while it is fresh (`status: "kept"`, nothing spent). Otherwise they return
`status: "needs_approval"` with `price.usd`, `price.credits` and how it would be paid (own key or credits).

1. Tell the user what you want to fetch, why, and the price. Put several paid calls into one question with
   the total; `estimate_cost` prices a call without buying.
2. Call again with `allow_spend_usd` set to that price only after the user says yes. Never raise it on your
   own and never retry with a higher allowance.
3. If a tool says the data cannot be bought (no key, no credits, budget reached), tell the user to open
   Sources in the program. Do not work around it.
