# ADR 0005 — Bot identification and verification

- Status: accepted
- Date: 2026-10-12

## Context
SEO log analysis is mostly about crawlers: which bots request which URLs, how often, and with
which responses. A user-agent string proves nothing, so "Googlebot" hits must be verified
against the operator's infrastructure. Operators publish verification data in two ways:

- **IP range lists** (same JSON shape everywhere: `{"prefixes": [{"ipv4Prefix": …}]}`): Google,
  Bing, OpenAI, Anthropic, Perplexity, Apple, DuckDuckGo, Common Crawl, Mistral, Ahrefs,
  Amazon.
- **Forward-confirmed reverse DNS** (FCrDNS): Google, Bing, Apple, Yandex, Baidu, Amazon,
  Seznam, Naver, Yahoo, Petal, Ahrefs.

Sites hold up to billions of hits, rollups must stay valid, and verification data changes
over time (new ranges, DNS checks run later).

## Decision

### Identification (pure)
- `seonexus-botverify` holds a typed catalog of ~100 bots (`id`, name, operator,
  category, user-agent patterns, verification sources). Categories: `search`, `ai-training`,
  `ai-search`, `ai-assistant`, `ads`, `seo`, `social`, `monitoring`, `other`.
- Patterns are lists of lowercase substrings that must all occur; the first matching bot
  wins, so specific bots (`googlebot-image`) precede generic ones (`googlebot-desktop`).
  Unknown crawlers, HTTP libraries and headless browsers fall back to generic ids.
- Classification runs on parser worker threads (cached per chunk) and is stored as a
  dimension attribute: `user_agents.bot`, `user_agents.bot_category`. Rollups are keyed by
  `ua_id`, so bot filters and groupings work on rollups without extra keys.
- `CATALOG_VERSION` is stored per site; when it changes, existing user agents are
  re-classified on the next write access.

### Verification
- Verdicts are per `(ip, bot)` pair in the site table `bot_ips(ip, bot, verdict, method,
  checked_at)`: `verified`, `spoofed` or `unverified` (no means to verify, data missing, DNS
  skipped or failed). Raw hits are never rewritten; Parquet stays immutable.
- IP lists: a bundled snapshot (refreshed with `cargo xtask bot-ranges`) works offline; `snx-log
  bots update` downloads current lists into `<data>/bot-ranges.json`, which overrides the
  snapshot per source when newer. An IP outside every list of the bot's operator is `spoofed`.
- DNS: reverse lookup, suffix match, forward lookup must contain the IP. Imports only use DNS
  when asked (`--verify-dns`) because it is slow and needs the network; `snx-log bots verify
  --dns` checks pending pairs later. DNS runs on a dedicated bounded thread pool.
- The import flush asks a `BotIpVerifier` port for verdicts of new pairs, stores them and
  adds `verification` as a key of `r_day_url`. When verdicts change, the bot rows of
  `r_day_url` are rebuilt from Parquet (`import_id` is recovered from file names).

### Query language
- Fields and dimensions `bot`, `bot_category`, `verification`; hit listings show `bot` and
  `verification`. On raw hits `verification` joins `bot_ips` on `(ip, bot)`.

### Adapters
- `seonexus-net` implements HTTP downloads (ureq, rustls) and DNS (`dns-lookup`, the system
  resolver). Domain crates stay free of network code.

## Consequences
- Bot dashboards are answered from rollups; verification is consistent with the latest data.
- Re-verification touches only bot rows; human traffic rollups are never rebuilt.
- A new catalog version re-classifies user agents (fast: one pass over a small dimension).
- The IP snapshot ages; the CLI warns when it is older than 30 days and not updated.
