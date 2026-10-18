// The first connectors, all drafts: each is activated in the admin panel only after the
// provider's written permission to resell is recorded and our credentials are entered.

import type { ConnectorInput } from "./connector.ts";
import type { Db } from "./db/client.ts";
import { getConnector, saveConnector } from "./plugins.ts";

export const SEED_CONNECTORS: ConnectorInput[] = [
  {
    id: "dataforseo",
    name: "DataForSEO",
    description: "Google, Bing and Yandex result pages, keyword volumes and ideas, Labs data, autocomplete and backlink summaries.",
    upstream: "https://api.dataforseo.com",
    auth: { type: "basic", secret: "credentials" },
    routes: [
      { method: "POST", path: "/v3/serp/*/organic/live/*" },
      { method: "POST", path: "/v3/serp/google/autocomplete/live/advanced" },
      { method: "POST", path: "/v3/keywords_data/google_ads/*/live" },
      { method: "POST", path: "/v3/dataforseo_labs/google/*/live" },
      { method: "POST", path: "/v3/backlinks/summary/live" },
    ],
    // Account data and asynchronous tasks (task ids are readable by anyone on our account).
    deny: ["/v3/appendix/**", "/v3/**/task_get/**", "/v3/**/task_post", "/v3/**/tasks_ready", "/v3/**/tasks_fixed"],
    pricing: {
      cost: { from: "json", path: "$.cost" },
      hold_usd: 0.5,
      error: { path: "$.tasks[0].status_code", ok: [20000] },
    },
    limits: { rpm: 120, concurrency: 6, timeout_s: 120, max_body_kb: 512 },
    terms: { confirmed: false, note: "Ask DataForSEO to confirm in writing that reselling through octopusSEO credits is allowed." },
    status: "draft",
    test: {
      method: "POST",
      path: "/v3/serp/google/organic/live/advanced",
      body: [{ keyword: "kahve", location_code: 2792, language_code: "tr", depth: 10 }],
    },
    balance: { method: "GET", path: "/v3/appendix/user_data", value: "$.tasks[0].result[0].money.balance", unit: "USD", warn_below: 20 },
  },
  {
    id: "serpapi",
    name: "SerpApi",
    description: "Google, Bing and Yandex result pages and Google autocomplete.",
    upstream: "https://serpapi.com",
    auth: { type: "query", param: "api_key", secret: "api_key" },
    strip_query: ["api_key"],
    routes: [{ method: "GET", path: "/search.json" }],
    // The archive returns earlier searches made on our account.
    deny: ["/searches/**", "/account.json", "/account"],
    pricing: {
      cost: { from: "fixed", usd: 0.015 },
      hold_usd: 0.05,
      error: { path: "$.search_metadata.status", ok: ["Success"] },
    },
    limits: { rpm: 60, concurrency: 4, timeout_s: 90, max_body_kb: 16 },
    terms: { confirmed: false, note: "SerpApi terms (section 2) require express written permission to resell." },
    status: "draft",
    test: { method: "GET", path: "/search.json", query: { engine: "google", q: "kahve", gl: "tr", hl: "tr" } },
    balance: { method: "GET", path: "/account.json", value: "$.total_searches_left", unit: "searches", warn_below: 500 },
  },
  {
    id: "serper",
    name: "Serper",
    description: "Google result pages and autocomplete.",
    upstream: "https://google.serper.dev",
    auth: { type: "header", name: "X-API-KEY", template: "{secret}", secret: "api_key" },
    routes: [
      { method: "POST", path: "/search" },
      { method: "POST", path: "/autocomplete" },
    ],
    pricing: { cost: { from: "credits", path: "$.credits", usd_per_credit: 0.001 }, hold_usd: 0.02 },
    limits: { rpm: 120, concurrency: 6, timeout_s: 60, max_body_kb: 16 },
    terms: { confirmed: false, note: "Serper's data licence restricts use by end users; ask for written permission." },
    status: "draft",
    test: { method: "POST", path: "/search", body: { q: "kahve", gl: "tr", hl: "tr" } },
  },
  {
    id: "yandex-search",
    name: "Yandex Search API",
    description: "Yandex result pages (yandex.com.tr and other Yandex sites).",
    upstream: "https://searchapi.api.cloud.yandex.net",
    auth: { type: "header", name: "Authorization", template: "Api-Key {secret}", secret: "api_key" },
    inject: [{ type: "json_field", field: "folderId", secret: "folder_id" }],
    routes: [{ method: "POST", path: "/v2/web/search" }],
    pricing: { cost: { from: "fixed", usd: 0.004 }, hold_usd: 0.05 },
    limits: { rpm: 60, concurrency: 4, timeout_s: 60, max_body_kb: 16 },
    terms: { confirmed: false, note: "Yandex ties results to use in the customer's product; ask Yandex about reselling." },
    status: "draft",
  },
  {
    id: "wordstat",
    name: "Yandex Wordstat",
    description: "Yandex search counts: top requests, dynamics and regions.",
    upstream: "https://searchapi.api.cloud.yandex.net",
    auth: { type: "header", name: "Authorization", template: "Api-Key {secret}", secret: "api_key" },
    inject: [{ type: "json_field", field: "folderId", secret: "folder_id" }],
    routes: [
      { method: "POST", path: "/v2/wordstat/topRequests" },
      { method: "POST", path: "/v2/wordstat/dynamics" },
      { method: "POST", path: "/v2/wordstat/regions" },
      { method: "POST", path: "/v2/wordstat/getRegionsTree" },
    ],
    pricing: { cost: { from: "fixed", usd: 0.00016 }, hold_usd: 0.01 },
    limits: { rpm: 30, concurrency: 2, timeout_s: 60, max_body_kb: 16 },
    terms: { confirmed: false, note: "Ask Yandex whether Wordstat data may be resold." },
    status: "draft",
  },
];

/** Adds the seed connectors that are not in the database yet; edited ones are left alone. */
export async function seedConnectors(db: Db, actorId = "system"): Promise<string[]> {
  const added: string[] = [];
  for (const seed of SEED_CONNECTORS) {
    if (await getConnector(db, seed.id)) continue;
    await saveConnector(db, seed, actorId);
    added.push(seed.id);
  }
  return added;
}
