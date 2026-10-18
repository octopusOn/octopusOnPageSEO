# Query language

Every report in the CLI (`octopuson-log query`) and the desktop app is a query in this language. A
query is a filter, optionally followed by directives that group and aggregate the matching hits.

```text
bot:googlebot* verification:verified status:4xx date:last30d group:url metric:hits,last_seen sort:hits:desc limit:500
```

Without `group:` or `metric:` the query lists individual hits (newest first).

## Conditions

A condition is `field` + operator + value, with no spaces around the operator.

| Syntax | Meaning | Example |
|--------|---------|---------|
| `field:value` | equals | `status:404`, `method:POST` |
| `field:a,b,c` | equals any | `status:301,302,308` |
| `field:*text*` | `*` wildcard (case-sensitive, text fields) | `url:/blog/*`, `ua:*Firefox*` |
| `field:~"regex"` | regular expression (RE2), text fields | `path:~"\.(css\|js)$"` |
| `field:>n` `:>=` `:<` `:<=` | comparison, numbers and dates | `bytes:>100000`, `date:>=2026-10-01` |
| `field:a..b` | inclusive range | `response_ms:500..2000`, `date:2026-09-01..2026-09-30` |
| `status:4xx` | status class | `status:5xx` |
| `date:lastNd` | the last N days including today (UTC) | `date:last7d`, `date:last90d` |

Values containing spaces, commas, parentheses or quotes must be quoted: `ua:"Mozilla/5.0 (X11*"`.
Inside quotes `\"` and `\\` are escapes.

### Combining

- Conditions separated by spaces must all hold (AND); `AND` may be written explicitly.
- `OR` binds looser than AND: `status:404 OR status:410 date:last7d` means
  `status:404 OR (status:410 AND date:last7d)`.
- `-` or `NOT` negates: `-status:200`, `NOT (bot:* OR ua:*curl*)`.
- Parentheses group: `(bot:googlebot* OR bot:bingbot) -status:2xx`.

A negated test on a field that can be empty (user agent, referer, bot, …) keeps hits where the
field is empty: `-bot:*` lists browser traffic.

## Fields

| Field | Type | Notes |
|-------|------|-------|
| `date` | date | UTC day of the request |
| `ip` | text | client IP |
| `method` | text | HTTP method |
| `url` | text | request target including the query string |
| `path` | text | target without the query string |
| `directory` | text | first path segment, e.g. `/blog/` (`/` for top-level pages) |
| `params` | boolean | `params:true` — URL has a query string |
| `status` | number | HTTP status |
| `bytes` | number | response size |
| `response_ms` | number | response time in ms (formats that log it, e.g. Traefik) |
| `ua` | text | user agent |
| `referer` | text | referer |
| `host` | text | virtual host / router |
| `protocol` | text | e.g. `HTTP/2.0` |
| `bot` | text | identified bot id, e.g. `googlebot-smartphone` (`octopuson-log bots list`) |
| `bot_category` | text | `search`, `ai-training`, `ai-search`, `ai-assistant`, `ads`, `seo`, `social`, `monitoring`, `other` |
| `verification` | text | `verified`, `spoofed` or `unverified` (bots only) |
| `content_type` | text | guessed from the extension (logs carry no `Content-Type`): `html` (also pages without an extension and `.php`, `.aspx`, …), `image`, `js`, `css`, `font`, `json`, `xml`, `pdf`, `txt`, `media`, `document`, `archive`, `other` |
| `referer_host` | text | host of the referer without `www.`, e.g. `chatgpt.com` |
| `referer_type` | text | `direct` (no referer), `internal` (the request host or the site's own host and its subdomains), `ai` (ChatGPT, Perplexity, Gemini, Copilot, Claude, DeepSeek, Grok, …), `search`, `social`, `email` or `other` |
| `country` | text | country of the client IP (ISO code, e.g. `TR`); needs the IP geolocation data (`octopuson-log geo update`) |
| `asn` | number | autonomous system number of the client IP |
| `as_org` | text | organization of the client IP's network, e.g. `Hetzner Online GmbH` |

## Directives

| Directive | Meaning |
|-----------|---------|
| `group:a,b` | one row per distinct combination: `url`, `path`, `directory`, `status`, `status_class`, `method`, `ip`, `ua`, `referer`, `host`, `protocol`, `day`, `hour`, `month`, `bot`, `bot_category`, `verification`, `content_type`, `referer_host`, `referer_type`, `country`, `asn`, `as_org` |
| `metric:a,b` | aggregates (default `hits`), see below |
| `sort:col[:asc\|desc],…` | order by result columns; metrics default to descending, others ascending |
| `limit:n` | at most n rows |

Each directive may appear once and only at the top level.

### Metrics

| Metric | Meaning |
|--------|---------|
| `hits` | requests |
| `bytes`, `avg_bytes` | bytes sent, mean response size |
| `avg_ms`, `max_ms`, `p95_ms` | response time (formats that log it) |
| `unique_urls`, `unique_ips` | distinct URLs / client IPs |
| `first_seen`, `last_seen` | first / latest request |
| `days_since_last` | days between the latest request of the row and the latest request stored for the site (crawl recency) |
| `last_status` | status code of the latest request |
| `status_codes`, `unique_statuses`, `inconsistent` | distinct status codes, their number, and whether there was more than one |
| `errors` | requests answered with 4xx or 5xx |
| `hits_1xx` … `hits_5xx` | requests per status class, as columns |
| `co2_mg` | estimated emissions of the bytes sent in mg CO2e (Sustainable Web Design model v3: 0.81 kWh/GB, 442 g/kWh global average grid) |
| `bot_hits`, `search_hits`, `ai_hits` | requests from any bot / search engines / AI bots |
| `googlebot_hits`, `googlebot_smartphone_hits`, `googlebot_desktop_hits`, `bingbot_hits`, `yandex_hits` | requests per major crawler, as columns |

Bot columns count by user agent; add `verification:verified` to the filter to count genuine
crawlers only (browsers then drop out of `hits`).

## Performance

The planner answers a query from the smallest data source able to: an hourly rollup (status
and time only), a daily rollup per URL, user agent, status and verdict (most reports), or the
raw Parquet hits. Raw hits are needed for `ip`, `method`, `bytes`, `response_ms`, `referer`,
`referer_host`, `referer_type`, `country`, `asn`, `as_org`, `host`, `protocol` filters or
groups and for `p95_ms`,
`unique_ips`; such queries scan the stored months (date filters prune whole months). The CLI and the desktop app show which
source answered.

## Examples

```text
# Verified Googlebot errors in the last week
bot:googlebot* verification:verified status:>=400 date:last7d group:url,status metric:hits,last_seen

# Mobile vs desktop Googlebot per day
bot:googlebot-smartphone,googlebot-desktop verification:verified group:day,bot

# AI crawlers and the pages they fetch
bot_category:ai-* group:url,bot metric:hits,last_seen sort:hits:desc limit:500

# Crawl waste: parameter URLs crawled by search engines
bot_category:search verification:verified params:true group:path metric:hits,unique_urls

# Slow pages for search engines
bot_category:search group:url metric:hits,p95_ms sort:p95_ms:desc limit:100

# Fake Googlebots by IP
bot:googlebot* verification:spoofed group:ip metric:hits,unique_urls

# Pages answered with more than one status code
group:url metric:unique_statuses,status_codes,hits sort:unique_statuses:desc

# Screaming Frog-style URL table: last status, recency, size, per-bot columns
group:url,content_type metric:hits,last_status,last_seen,days_since_last,avg_bytes,avg_ms,googlebot_hits,bingbot_hits

# Response codes per URL as columns
group:url metric:last_status,inconsistent,hits_2xx,hits_3xx,hits_4xx,hits_5xx

# Visits from AI assistants and their landing pages
referer_type:ai -bot:* group:url,referer_host metric:hits sort:hits:desc

# Bandwidth and estimated CO2 per bot
bot:* group:bot metric:hits,bytes,co2_mg sort:bytes:desc

# Where fake Googlebots come from (needs the IP geolocation data)
bot:googlebot* verification:spoofed group:as_org,country metric:hits,unique_ips sort:hits:desc
```

Country and network data: IP Geolocation by DB-IP (<https://db-ip.com>), CC BY 4.0.
