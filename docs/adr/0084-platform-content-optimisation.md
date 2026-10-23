# ADR 0084 — GEO-SEO Platform: content briefs and AI-assisted writing

- Status: accepted
- Date: 2026-10-23

## Context
Ranking data tells an agency which keywords matter, but not what a page needs to compete for them. Surfer, Clearscope and Semrush's SEO Writing Assistant read the top results and turn them into a brief and a score. The owner wanted the same, with AI writing as an option on the user's own assistant key or credits. The rules themselves stay deterministic.

## Decision
- **A brief is made from the keyword's result page and the pages ranking on it** (`src/content.rs`, `modules/content.rs`, migration 18).
  - **The result page.**
    - It is the Keyword Explorer's own question, so a kept answer is reused.
    - It is paid only when not fresh; `content_brief_estimate` gives the price first.
    - `serp::snapshot_of` reads its PAA questions and related searches.
  - **The pages.** The top ten are read with the program's own fetcher, four at a time, with the Octopus user agent. Redirects are followed up to five, and only the main text is kept (60,000 characters).
  - **What the brief holds** (`build_brief`, with the SERP Checker's scoring):
    - recommended terms (TF-IDF) and a heading outline;
    - questions to answer;
    - a target length and the intent;
    - internal link suggestions from the latest audit crawl's titles and H1s.
  - **Sync.** A brief and its scores sync (`content_brief`, `content_score`).
- **Content score** (`score_content`, free).
  - **What is scored:** a URL or a pasted Markdown draft, against a brief.
  - **What counts:**
    - term coverage, length and headings;
    - title and description;
    - readability.
  - **What it gives back:** a 0–100 score and the missing pieces.
  - **The page checker** scores tracked pages, and each Search Console page against its top query (`content_pages`).
- **AI writing is optional and explicit** (`octopuson_llm::write`).
  - **Assistants:** Claude (adaptive thinking), ChatGPT (Responses) or Gemini, with no web search.
  - **What it writes:** a full brief, improvements for a page, or a review reply.
  - **Price first.** `content_writing_estimate` and `reply_estimate` give it before anything is asked.
  - **Own key only for now.** The gateway has no assistant connector for long prompts yet (DataForSEO's LLM Responses caps prompts at 500 characters), so credits are refused with a message until P13 adds one.
- **The page** (Content):
  - briefs, and a brief's detail with a live-scoring editor;
  - Markdown drawn without HTML (no injection);
  - writer choice and score chips.

  "Create a content brief" sits in the entity menu on keywords, on the Keyword Explorer and on SERP clusters. Opportunities for near-top, dropped and low-content keywords link to it.
- **Assistants (P12).**
  - `content_briefs` reads briefs (free).
  - `create_content_brief` makes one: `needs_approval` with the result page's price unless it is kept fresh.

## Consequences
- **Briefs are reproducible and cheap.** One result page (often already kept) plus the program's own fetching; scoring is free and runs as the user types.
- **AI text is never produced unasked and never hidden.** The assistant, model and cost are kept with the brief.
- **Competitors' pages are read openly, one page per result.** The fetcher names itself with the Octopus crawler agent and does not crawl further. It does not read robots.txt, as a browser opening the page would not. On the web it reaches public addresses only.
- **A page that does not answer gives no text.** It is still listed with its position and counts as empty in the brief.
