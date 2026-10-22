# ADR 0079 — Credits gateway: providers' answers kept for every account, sold at a discount

- Status: accepted
- Date: 2026-10-22

## Context
Every request on OctopusOn SEO credits goes through api.octopuson.com, and each was paid to the provider anew. Much of that data changes slowly:
- a site's backlink profile within a day;
- DataForSEO Labs databases about monthly;
- Google Ads volumes are monthly figures.

Different people also ask the same things: the same site, the same keyword in the same market.

The owner asked for a cache that answers repeated questions without paying the provider, at a discount for the customer. It is a rule for every program on the account, today's and future ones, and it can be switched off in the admin.

Decisions taken with the owner:
- **Price.** An answer served from the cache costs the customer 30% of its normal price (a 70% discount), adjustable in the admin. Our provider cost for it is zero.
- **Connectors.** Every connector may cache, for as long as its kind of data lasts. The owner takes responsibility for the providers' terms on reselling data.

## Decision
- **One place, every program.** The cache sits in the gateway's passthrough (`apps/gateway/src/proxy.ts`). The desktop programs, the web platform and any later program on credits get it without changes.
- **Rules live with the connector.**
  - `cache: { enabled, normalize, rules: [{ method?, path, hours }] }`, where the first matching rule decides. No rule, or 0 hours, means the answer is not kept.
  - The seeds give every route a rule, 0 included, and a test enforces it for new connectors.
  - Seeded times:
    - backlinks 24 hours, their time series 72;
    - Labs 7 days;
    - Google Ads volumes 30 days;
    - result pages 12 hours;
    - autocomplete 7 days;
    - AI Mode and assistants' answers 24 hours;
    - LLM Mentions 7 days;
    - Serper and SerpApi search 12 hours, Yandex search 12 hours, Wordstat 30 days.
  - `cache-rules` in the CLI gives connectors already in the database the seeds' rules, as a new version, keeping a switched-off cache off.
- **The same question, the same key, whoever asks.**
  - The key is the SHA-256 of connector, method, normalised path, sorted query and canonical body.
  - The canonical body has JSON keys sorted, a client's `tag` and callback fields dropped, and the connector's `normalize` fields (DataForSEO `keyword`, `keywords`, `target`, `targets`) trimmed and in lower case.
  - The key is taken before our credentials are written into the body (Yandex's `folderId`). The query leaves out the apps' own key parameters.
  - Who asked is never recorded.
- **Serving a kept answer.**
  - **Order.** After the rate and spending limits, before the hold.
  - **Price.** The full price (`chargeFor` of the kept provider cost at today's markup) less the discount, rounded up. It is taken through the usual hold and settle, so missing credits answer 402 and idempotency keys work.
  - **Headers.** `X-Octopus-Cache: hit`, `X-Octopus-Cache-Age`, `X-Octopus-Charge-Usd` (discounted) and `X-Octopus-Full-Charge-Usd`.
  - **Usage.** The usage row is marked `cached`, with `saved_micros` (the provider cost avoided) and `full_charge_micros`.
  - **Single flight.** The same question arriving while it is on its way to the provider waits for that answer within the process.
- **Keeping an answer.**
  - Only an HTTP 200 without a provider error inside, up to `cacheMaxEntryKb`, with our credentials masked, compressed with gzip in `answer_cache` (migration `0005_answer_cache`).
  - Expired answers, and then the oldest beyond `cacheMaxTotalMb`, are deleted hourly.
- **Asking anew and asking the price.**
  - `Cache-Control: no-cache` pays the provider and keeps the new answer.
  - `X-Octopus-Cache: peek` answers free whether an answer is kept and what it would cost.
  - The GEO-SEO Platform:
    - asks the price in its estimates on credits;
    - passes the cache by when the person refreshes an answer that is still fresh on their computer;
    - offers "Use discounted answers from the cache" in Sources, on by default.
- **Settings** (Admin → Settings): `cacheEnabled` (on), `cacheDiscountPct` (70), `cacheMaxEntryKb` (4096), `cacheMaxTotalMb` (2048); and per connector, a switch in its quick form.
- **Reports.**
  - Admin → Reports shows, per connector: hits, the hit rate, the provider cost avoided, revenue from the cache, and the answers kept.
  - The usage page labels cached rows "Cache · 70% off" and shows the discount per source.

## Consequences
- **Lower provider costs, higher margin, cheaper repeats for customers.** A popular site or keyword is paid to the provider once per rule period.
- **Freshness is bounded by the rule.** A customer who needs today's data passes the cache by (refresh, or the Sources option).
- **Provider terms.** Selling one provider answer to several customers goes beyond a per-customer resale. The owner chose to cache on every connector; each connector can be switched off on its own if a provider objects.
- **Storage.** The kept answers live in PostgreSQL, compressed, within the space allowed. The gateway logs only the key's first characters, never the question.
- **Desktop programs.** `HttpTransport::take_saved_usd` tells a program what the discount saved. Older programs keep working and simply pay the lower charge.
